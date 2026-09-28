//! Hybrid recall: BM25 + token overlap fused with RRF(k=60).
//!
//! Hindsight TEMPR (semantic/keyword/graph/temporal) x agentmemory
//! triple-stream (BM25/vector/graph). See PLAN.md Phase 2.
//!
//! Two streams ship: FTS5 BM25, and a token-overlap ranker that is the
//! stand-in for the second stream. There is no vector stream and no graph
//! stream, and none is planned behind a flag — the kernel fuses whatever it
//! is given, and today that is two.

use std::collections::HashMap;

/// RRF constant (agentmemory k=60).
pub const RRF_K: f64 = 60.0;

/// Approximate characters per token (the usual English 4 chars ≈ 1 token).
const CHARS_PER_TOKEN: usize = 4;

/// Joins lowercased tokens inside the scratch buffer that [`lowercase_tokens`]
/// borrows them out of. Every character in that buffer is alphanumeric except
/// this one, so splitting on it recovers the tokens unambiguously.
const TOKEN_SEP: char = '\n';

/// Appended to content that was cut down to fit the token budget.
pub const TRUNCATION_MARKER: &str = "…[truncated]";

/// A ranked candidate from one retrieval stream.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedHit {
    /// Memory id.
    pub id: String,
    /// 1-based rank within its stream.
    pub rank: usize,
}

/// Per-stream weights for the fusion, plus an optional cross-stream agreement
/// bonus.
///
/// The default is [`FusionWeights::SHIPPED`]. Before the E1 sweep the shipped
/// value was both streams at 1.0; the sweep moved `overlap` to 0.25, and that
/// measured move is the whole reason the struct exists. Weights are here to be
/// *swept* (`examples/sweep_fusion.rs`), and nothing in the shipped binary
/// changes one per request — see `docs/NEXT_ITERATION.md` for the grid.
///
/// `bm25` and `overlap` scale stream 0 and stream 1. A weight of 0.0 drops its
/// stream's contribution entirely, which is what makes a one-stream arm a row
/// in the sweep rather than a different code path. Any third stream keeps 1.0:
/// there is no third stream, and guessing a weight for one would be a silent
/// no-op dressed up as configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FusionWeights {
    /// Weight on the FTS5 BM25 stream (stream 0).
    pub bm25: f64,
    /// Weight on the token-overlap stream (stream 1).
    pub overlap: f64,
    /// Additive bonus for an id **both** streams returned, in units of
    /// `1/(k + 1)`.
    ///
    /// Expressed as a fraction of one rank-1 unit rather than as a flat number
    /// because the fused score it is added to is a sum of `1/(k + rank)`: at
    /// k=60 a rank-1 hit in both streams is worth ~0.0328, so a flat `0.05`
    /// would be worth three such hits and "a small boost" would be neither.
    /// `0.05` here is a ~2.5% lift on a doubly-found rank-1 hit.
    pub agreement: f64,
    /// The RRF `k` constant: score = Σ wᵢ/(k + rank).
    ///
    /// Carried here rather than passed beside the weights so the fusion is one
    /// value a caller can hold, compare and sweep, and so `k` and the agreement
    /// bonus — which is denominated in `1/(k+1)` — can never be set from two
    /// places that disagree.
    pub k: f64,
}

impl FusionWeights {
    /// What ships. **Not equal weights** — see `docs/NEXT_ITERATION.md`,
    /// Phase E1, for the 500-question sweep behind `overlap: 0.25`.
    ///
    /// At equal weights the token-overlap stream is a *net negative*: it was
    /// costing 4.0pp of R@5 and 4.8pp of NDCG@10 on LongMemEval-S, and the loss
    /// was concentrated exactly where the deficit was — `single-session-preference`
    /// 56.7% → 86.7% and `single-session-assistant` 87.5% → 100.0% on R@5, with
    /// no category regressing. The cause is structural rather than incidental:
    /// `fts_match_query` ORs every query token, so BM25's own ordering already
    /// accounts for term matching, and a raw token-count voter at equal weight
    /// promotes documents that merely repeat the query's words over documents
    /// BM25 ranked on term rarity.
    ///
    /// `0.25` and not `0.0`, though `0.0` scores 1.7pp higher on NDCG@10 and
    /// 2.2pp on MRR (and loses on R@5 by exactly one question of 500). A zero
    /// weight drops the stream's candidates entirely, and BM25 is `LIMIT 50` in
    /// SQL — so on any query matching more than 50 rows the candidate set is
    /// truncated to 50. LongMemEval-S cannot see that: 38–62 sessions per bank,
    /// with BM25 truncating at 50, means the suite contains no such query.
    /// `a_query_matching_more_rows_than_bm25_returns_must_still_surface_the_overflow`
    /// pins the difference (120 matching rows → 120 hits here, exactly 50 at
    /// weight 0.0), and `bench_recall_curve` is the harness that prices it.
    ///
    /// The agreement bonus stays 0.0: swept at 0.05/0.10/0.25/0.50 it moved
    /// **zero** of 500 questions at every magnitude, so it is not a knob this
    /// build turns — it is measured-and-rejected, kept in the struct only
    /// because the field is what made that measurement expressible.
    pub const SHIPPED: FusionWeights = FusionWeights {
        bm25: 1.0,
        overlap: 0.25,
        agreement: 0.0,
        k: RRF_K,
    };

    /// The weight of stream `i`, which is positional because the fusion is.
    fn of(&self, i: usize) -> f64 {
        match i {
            0 => self.bm25,
            1 => self.overlap,
            _ => 1.0,
        }
    }
}

impl Default for FusionWeights {
    fn default() -> Self {
        Self::SHIPPED
    }
}

/// Fuse ranked streams with Reciprocal Rank Fusion: score = Σ wᵢ/(k + rank).
///
/// Returns `(id, score)` sorted by descending score, then id for stability.
/// With [`FusionWeights::SHIPPED`] this is the plain `Σ 1/(k + rank)` it always
/// was, at the same k.
pub fn rrf_fuse(streams: &[Vec<RankedHit>], weights: &FusionWeights) -> Vec<(String, f64)> {
    // k <= 0 (or NaN) makes 1/(k + rank) explode or flip sign, which silently
    // reorders the fused list. Clamp to a safe floor; the default k is RRF_K, so
    // this only bites a hand-tuned or corrupt caller.
    let k = weights.k.max(1.0);
    let mut scores: HashMap<&str, f64> = HashMap::new();
    // Ids each stream contributed to, so the agreement bonus can be added once
    // per doubly-found id after the weighted pass rather than inside it.
    let mut found_in: HashMap<&str, u8> = HashMap::new();
    for (i, stream) in streams.iter().enumerate() {
        let w = weights.of(i);
        // A zero weight means the stream is not a stream: it must contribute
        // neither a score nor an entry. Inserting its hits anyway would leave
        // them in the output at score 0.0 — sorted in by id, past everything a
        // real stream ranked — so a one-stream sweep arm would silently return
        // the dropped stream's tail as unranked filler, and the agreement bonus
        // would credit ids the fused list no longer has.
        if w == 0.0 {
            continue;
        }
        for hit in stream {
            let rank = hit.rank.max(1) as f64;
            let entry = scores.entry(hit.id.as_str()).or_default();
            *entry += w / (k + rank);
            let seen = found_in.entry(hit.id.as_str()).or_default();
            *seen |= 1 << (i.min(7) as u32);
        }
    }
    if weights.agreement != 0.0 {
        let unit = weights.agreement / (k + 1.0);
        for (id, streams) in found_in.iter() {
            if streams.count_ones() > 1 {
                if let Some(s) = scores.get_mut(id) {
                    *s += unit;
                }
            }
        }
    }
    let mut out: Vec<(String, f64)> = scores
        .into_iter()
        .map(|(id, s)| (id.to_string(), s))
        .collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    out
}

/// Tokenize to lowercase alphanumeric tokens.
///
/// Splits on non-alphanumeric boundaries for BM25-style matching.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The tokens of [`tokenize`], borrowed out of one caller-owned scratch buffer.
///
/// Tokenization is identical — Unicode `to_lowercase` per char, split on
/// `!c.is_alphanumeric`, empty segments dropped — but every token lands in
/// `buf`, which the caller clears and refills per document. Scoring the recall
/// candidate pool therefore costs one owned buffer for the whole pool instead
/// of one `String` per token, and the returned slices are `&str` into it.
fn lowercase_tokens<'a>(text: &str, buf: &'a mut String) -> impl Iterator<Item = &'a str> + 'a {
    buf.clear();
    for raw in text.split(|c: char| !c.is_alphanumeric()) {
        if raw.is_empty() {
            continue;
        }
        for c in raw.chars() {
            buf.extend(c.to_lowercase());
        }
        buf.push(TOKEN_SEP);
    }
    // The split is a shared borrow of a buffer that is no longer written to.
    buf.split(TOKEN_SEP).filter(|t| !t.is_empty())
}

/// Per-query state hoisted out of the per-document loop: each distinct query
/// token mapped to its dedup slot and how many times the query repeats it.
fn build_query_index(query: &[String]) -> (HashMap<&str, (usize, usize)>, Vec<bool>) {
    let mut index: HashMap<&str, (usize, usize)> = HashMap::with_capacity(query.len());
    for token in query {
        // A first occurrence takes the next free slot; a repeat reuses its own.
        let slot = index.len();
        let entry = index.entry(token.as_str()).or_insert((slot, 0));
        entry.1 += 1;
    }
    let slots = index.len();
    (index, vec![false; slots])
}

/// Score one document against the hoisted state from [`build_query_index`].
///
/// `seen` is the dedup, replacing the per-document `HashSet` of document tokens
/// this used to build: a flag per distinct query token says whether the current
/// document has already contributed it, so a document that repeats a token still
/// counts it once. The score is the same number either way — the count runs over
/// query tokens, weighted by how often the query repeats them — and nothing here
/// allocates: document tokens are slices of `buf` and `seen` is just reset.
fn score_doc(
    doc: &str,
    index: &HashMap<&str, (usize, usize)>,
    seen: &mut [bool],
    buf: &mut String,
) -> usize {
    seen.fill(false);
    let mut score = 0;
    for token in lowercase_tokens(doc, buf) {
        if let Some(&(slot, repeats)) = index.get(token) {
            if !seen[slot] {
                seen[slot] = true;
                score += repeats;
            }
        }
    }
    score
}

/// Score a document against query tokens by overlap count (BM25 stand-in).
///
/// Returns the number of distinct query tokens present in the document.
pub fn overlap_score(query_tokens: &[String], doc: &str) -> usize {
    let (index, mut seen) = build_query_index(query_tokens);
    score_doc(doc, &index, &mut seen, &mut String::new())
}

/// Rank documents against a query (higher overlap first, stable by position).
///
/// `docs` is walked in iteration order; the returned indices are positions in it.
pub fn rank_candidates<'a, I>(query: &str, docs: I) -> Vec<(usize, usize)>
where
    I: IntoIterator<Item = &'a str>,
{
    let qt = tokenize(query);
    // Hoisted once: the query index, its dedup flags, and the scratch buffer are
    // reused for every document, so scoring the pool allocates nothing per doc.
    let (index, mut seen) = build_query_index(&qt);
    let mut buf = String::new();
    let mut out: Vec<(usize, usize)> = docs
        .into_iter()
        .enumerate()
        .map(|(i, d)| (i, score_doc(d, &index, &mut seen, &mut buf)))
        .filter(|(_, s)| *s > 0)
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// Trim ranked ids to a hard token budget (approx. 4 characters per token).
///
/// The budget is a ceiling, not a suggestion: the returned content never
/// measures more than `budget_tokens` (measured in characters, so multi-byte
/// text is not over-counted). A top hit that alone overflows the budget is cut
/// down to exactly the budget rather than bypassing the cap or being dropped —
/// R@1 survives even a one-token budget. The [`TRUNCATION_MARKER`] is appended
/// only when it fits inside the budget; below that the cut is left unlabelled,
/// because a marker that overruns the cap the caller set is the worse of the
/// two. Lower-ranked items that do not fit are skipped so smaller ones further
/// down can still fill what is left. Returns `(id, content)` in rank order.
pub fn trim_to_budget(
    ids: &[String],
    contents: &HashMap<&str, &str>,
    budget_tokens: usize,
) -> Vec<(String, String)> {
    if budget_tokens == 0 {
        return Vec::new();
    }
    // saturating: budget arrives from the HTTP body, so it is untrusted input.
    let cap = budget_tokens.saturating_mul(CHARS_PER_TOKEN);
    let marker_len = TRUNCATION_MARKER.chars().count();
    let mut used = 0usize;
    let mut out = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let Some(content) = contents.get(id.as_str()).copied() else {
            continue;
        };
        let len = content.chars().count();
        let room = cap.saturating_sub(used);
        if len <= room {
            used += len;
            out.push((id.clone(), content.to_string()));
            continue;
        }
        // Over budget. The top hit is cut to the room that is left, which
        // consumes the whole budget; everything below it is skipped from here.
        if i == 0 {
            let cut: String = if room > marker_len {
                let head: String = content.chars().take(room - marker_len).collect();
                format!("{head}{TRUNCATION_MARKER}")
            } else {
                content.chars().take(room).collect()
            };
            out.push((id.clone(), cut));
            used = cap;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Mixed-case, multi-script, punctuation-heavy and empty strings. Every
    /// tokenization equivalence check below runs over this one corpus.
    const CORPUS: [&str; 11] = [
        "",
        "   ",
        "a",
        "auth uses jose",
        "Auth, Uses; JOSE!",
        "auth auth auth",
        "2024-01-01T00:00:00Z",
        "ÜBER straße ΑΘΗΝΑ",
        "tabs\tand\nnewlines\r\nhere",
        "___---___",
        "snake_case kebab-case dot.case İstanbul",
    ];

    /// The scorer as it was before the per-token allocation was removed: query
    /// tokens filtered through a per-document `HashSet`. Kept as the reference
    /// for the differential check, so the rewrite is pinned to score-identical
    /// behaviour rather than to whatever it happens to do now.
    fn overlap_score_reference(query_tokens: &[String], doc: &str) -> usize {
        let lowered = tokenize(doc);
        let doc_tokens: HashSet<&str> = lowered.iter().map(String::as_str).collect();
        query_tokens
            .iter()
            .filter(|t| doc_tokens.contains(t.as_str()))
            .count()
    }

    fn contents<'a>(pairs: &[(&'a str, &'a str)]) -> HashMap<&'a str, &'a str> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn rrf_should_rank_shared_hits_first() {
        let a = vec![
            RankedHit { id: "m1".to_string(), rank: 1 },
            RankedHit { id: "m2".to_string(), rank: 2 },
        ];
        let b = vec![RankedHit { id: "m2".to_string(), rank: 1 }];
        let fused = rrf_fuse(&[a, b], &FusionWeights::SHIPPED);
        assert_eq!(fused[0].0, "m2");
    }

    #[test]
    fn rrf_should_clamp_non_positive_k() {
        let stream = vec![vec![RankedHit { id: "m1".to_string(), rank: 1 }]];
        let at = |k: f64| FusionWeights { k, ..FusionWeights::SHIPPED };
        let safe = rrf_fuse(&stream, &at(1.0));
        for bad_k in [0.0, -5.0, f64::NAN] {
            let got = rrf_fuse(&stream, &at(bad_k));
            assert_eq!(got[0].1, safe[0].1, "k={bad_k} must clamp to the floor");
            assert!(got[0].1.is_finite() && got[0].1 > 0.0);
        }
    }

    /// The identity the kernel must preserve: with both streams at 1.0 and no
    /// bonus, the fused score is a plain `Σ 1/(k + rank)`. Pinned against a
    /// literal computation rather than against whatever the code does, so a
    /// weight that leaked into the arithmetic is caught here rather than turning
    /// up later as a moved benchmark.
    ///
    /// This is deliberately *not* the shipped default — E1 moved `overlap` off
    /// 1.0 — which is exactly why it needs a test of its own: it is the claim
    /// that a weight is a multiplier and not a rewrite of the formula.
    #[test]
    fn unit_weights_should_reproduce_plain_rrf() {
        let a = vec![
            RankedHit { id: "m1".to_string(), rank: 1 },
            RankedHit { id: "m2".to_string(), rank: 2 },
            RankedHit { id: "m3".to_string(), rank: 7 },
        ];
        let b = vec![
            RankedHit { id: "m2".to_string(), rank: 1 },
            RankedHit { id: "m4".to_string(), rank: 4 },
        ];
        let unit = FusionWeights { overlap: 1.0, ..FusionWeights::SHIPPED };
        let fused = rrf_fuse(&[a, b], &unit);
        let got: HashMap<&str, f64> =
            fused.iter().map(|(id, s)| (id.as_str(), *s)).collect();
        let rrf = |ranks: &[f64]| ranks.iter().map(|r| 1.0 / (60.0 + r)).sum::<f64>();
        assert!((got["m1"] - rrf(&[1.0])).abs() < 1e-12);
        assert!((got["m2"] - rrf(&[2.0, 1.0])).abs() < 1e-12);
        assert!((got["m3"] - rrf(&[7.0])).abs() < 1e-12);
        assert!((got["m4"] - rrf(&[4.0])).abs() < 1e-12);
        assert!(FusionWeights::default() == FusionWeights::SHIPPED);
    }

    /// A zero weight must *drop* its stream, not merely shrink it — that is what
    /// makes a one-stream arm a row in the sweep rather than a separate path.
    #[test]
    fn a_zero_weight_should_remove_its_stream() {
        let bm25_only = vec![
            RankedHit { id: "m1".to_string(), rank: 1 },
            RankedHit { id: "m2".to_string(), rank: 2 },
        ];
        let overlap_only = vec![RankedHit { id: "m3".to_string(), rank: 1 }];
        let w = FusionWeights { overlap: 0.0, ..FusionWeights::SHIPPED };
        let fused = rrf_fuse(&[bm25_only, overlap_only], &w);
        let ids: Vec<&str> = fused.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["m1", "m2"], "the dropped stream must contribute nothing");
    }

    /// The agreement bonus lifts a doubly-found hit above a hit one stream rates
    /// higher, and leaves a singly-found hit exactly where it was.
    #[test]
    fn the_agreement_bonus_should_lift_only_doubly_found_hits() {
        let bm25 = vec![
            RankedHit { id: "shared".to_string(), rank: 6 },
            RankedHit { id: "solo".to_string(), rank: 1 },
        ];
        let overlap = vec![RankedHit { id: "shared".to_string(), rank: 1 }];
        let base = rrf_fuse(&[bm25.clone(), overlap.clone()], &FusionWeights::SHIPPED);
        let w = FusionWeights { agreement: 0.05, ..FusionWeights::SHIPPED };
        let boosted = rrf_fuse(&[bm25, overlap], &w);
        let score = |rows: &Vec<(String, f64)>, id: &str| {
            rows.iter().find(|(i, _)| i == id).map(|(_, s)| *s).expect("id present")
        };
        assert!(score(&boosted, "shared") > score(&base, "shared"));
        assert!((score(&boosted, "shared") - score(&base, "shared") - 0.05 / 61.0).abs() < 1e-12);
        assert_eq!(
            score(&boosted, "solo"),
            score(&base, "solo"),
            "a one-stream hit must not be touched by the bonus"
        );
        assert_eq!(boosted[0].0, "shared", "the bonus must actually reorder");
    }

    #[test]
    fn rank_should_match_keyword_overlap() {
        let docs = ["auth uses jose".to_string(), "rate limiting notes".to_string()];
        let ranked = rank_candidates("jose auth", docs.iter().map(String::as_str));
        assert_eq!(ranked[0].0, 0);
    }

    #[test]
    fn overlap_score_should_count_a_repeated_document_token_once() {
        // The dedup the old per-document `HashSet` provided: a document that
        // repeats a query token still contributes it once, so counting document
        // tokens directly would over-count here by 3x.
        let once = tokenize("auth");
        assert_eq!(overlap_score(&once, "auth auth auth"), 1);
        assert_eq!(overlap_score(&once, "auth"), 1);

        // The count runs over the *query* side, so a query that repeats a token
        // still weights it once per occurrence. Preserved, not normalised away.
        let twice = tokenize("auth auth");
        assert_eq!(overlap_score(&twice, "auth auth"), 2);
        assert_eq!(overlap_score(&twice, "auth"), 2);
    }

    #[test]
    fn overlap_score_should_be_zero_without_overlap() {
        let query = tokenize("auth jose");
        assert_eq!(overlap_score(&query, ""), 0);
        assert_eq!(overlap_score(&query, "a"), 0, "one char is not the whole token");
        assert_eq!(overlap_score(&query, "--- ;;; ..."), 0, "punctuation carries no tokens");
        assert_eq!(overlap_score(&tokenize(""), "auth"), 0, "an empty query matches nothing");
        assert_eq!(
            overlap_score(&[String::new()], "auth"),
            0,
            "an empty query token must not match the buffer's token separator"
        );
    }

    #[test]
    fn overlap_score_should_fold_case_for_non_ascii() {
        // Both sides go through the same Unicode `to_lowercase`; an ASCII-only
        // case comparison would score all of these 0.
        assert_eq!(overlap_score(&tokenize("ÜBER"), "über"), 1);
        assert_eq!(overlap_score(&tokenize("über"), "ÜBER"), 1);
        assert_eq!(overlap_score(&tokenize("ΑΘΗΝΑ"), "αθηνα"), 1);
        assert_eq!(overlap_score(&tokenize("MiXeD"), "mIxEd"), 1);
    }

    #[test]
    fn overlap_score_should_be_unchanged_by_the_buffered_scorer() {
        for query in [
            "auth",
            "auth auth",
            "jose uses",
            "über",
            "ΑΘΗΝΑ αθηνα",
            "2024-01-01",
            "İstanbul",
            "nomatch",
            "",
        ] {
            let qt = tokenize(query);
            for doc in CORPUS {
                assert_eq!(
                    overlap_score(&qt, doc),
                    overlap_score_reference(&qt, doc),
                    "score drift for query {query:?} against {doc:?}"
                );
            }
        }
    }

    #[test]
    fn lowercase_tokens_should_match_tokenize() {
        for text in CORPUS {
            let mut buf = String::new();
            let borrowed: Vec<&str> = lowercase_tokens(text, &mut buf).collect();
            assert_eq!(borrowed, tokenize(text), "token drift on {text:?}");
        }
    }

    #[test]
    fn lowercase_tokens_should_borrow_every_token_from_one_buffer() {
        // The property the allocation fix rests on: the tokens are slices of one
        // owned buffer, not one `String` per token. They tile it exactly, so
        // separate allocations could not produce these two addresses.
        let mut buf = String::new();
        let (first, last_end) = {
            let tokens: Vec<&str> =
                lowercase_tokens("auth uses jose, again AUTH and ÜBER 123", &mut buf).collect();
            assert!(tokens.len() >= 6, "corpus should exercise several tokens");
            let first = tokens[0].as_ptr() as usize;
            let last_end = tokens
                .iter()
                .map(|t| t.as_ptr() as usize + t.len())
                .max()
                .expect("token set is not empty");
            for t in &tokens {
                let start = t.as_ptr() as usize;
                assert!(
                    start >= first && start + t.len() <= last_end,
                    "{t:?} escaped the scratch buffer into its own allocation"
                );
            }
            (first, last_end)
        };
        assert_eq!(first, buf.as_ptr() as usize, "first token starts at byte 0");
        assert_eq!(
            last_end,
            buf.as_ptr() as usize + buf.len() - TOKEN_SEP.len_utf8(),
            "last token ends one separator short of the buffer's end"
        );
    }

    #[test]
    fn rank_candidates_should_not_carry_tokens_between_documents() {
        // One scratch buffer serves the whole pool, so it is cleared per
        // document; if it were not, `auth` would still be in it when the second
        // document is scored and that document would rank on a stale token.
        let docs = ["auth uses jose", "jose only"];
        assert_eq!(rank_candidates("auth", docs), vec![(0, 1)]);
    }

    #[test]
    fn trim_should_return_nothing_for_zero_budget() {
        let ids = vec!["a".to_string()];
        let c = contents(&[("a", "some content that would otherwise fit")]);
        assert!(trim_to_budget(&ids, &c, 0).is_empty());
    }

    #[test]
    fn trim_should_never_exceed_the_budget() {
        let ids = vec!["a".to_string()];
        let big = "x".repeat(400);
        let c = contents(&[("a", big.as_str())]);
        let kept = trim_to_budget(&ids, &c, 10);
        assert_eq!(kept.len(), 1, "top hit is truncated, not dropped");
        assert!(kept[0].1.ends_with(TRUNCATION_MARKER));
        assert!(
            kept[0].1.chars().count() <= 10 * CHARS_PER_TOKEN,
            "truncated content must stay inside the cap"
        );
    }

    #[test]
    fn trim_should_measure_chars_not_bytes() {
        // 40 two-byte chars: byte counting sees 80 (over budget), char counting
        // sees exactly 10 tokens, so this must pass through untouched.
        let ids = vec!["a".to_string()];
        let wide = "é".repeat(40);
        let c = contents(&[("a", wide.as_str())]);
        let kept = trim_to_budget(&ids, &c, 10);
        assert_eq!(kept, vec![("a".to_string(), wide)]);
    }

    #[test]
    fn trim_should_skip_oversized_and_fill_later_smaller_items() {
        let small_a = "x".repeat(30);
        let huge = "y".repeat(500);
        let small_c = "z".repeat(20);
        let ids = vec!["a".to_string(), "huge".to_string(), "c".to_string()];
        let c = contents(&[
            ("a", small_a.as_str()),
            ("huge", huge.as_str()),
            ("c", small_c.as_str()),
        ]);
        let kept = trim_to_budget(&ids, &c, 20);
        assert_eq!(
            kept.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            ["a", "c"],
            "a skipped item must not abandon the rest of the budget"
        );
    }

    #[test]
    fn trim_should_respect_token_budget() {
        let ids = vec!["a".to_string(), "b".to_string()];
        let big = "x".repeat(400);
        let c = contents(&[("a", big.as_str()), ("b", "y")]);
        assert_eq!(trim_to_budget(&ids, &c, 10).len(), 1);
    }

    #[test]
    fn trim_should_cut_the_top_hit_to_exactly_the_cap_for_a_tiny_budget() {
        let ids = vec!["a".to_string()];
        let big = "x".repeat(400);
        let c = contents(&[("a", big.as_str())]);
        let marker_len = TRUNCATION_MARKER.chars().count();

        // Below the marker's own length the label cannot be paid for, so the hit
        // is cut to the cap and left unlabelled — never dropped for being too
        // small a budget, and never given a tail that overruns the cap.
        for tokens in 1..=marker_len / CHARS_PER_TOKEN {
            let cap = tokens * CHARS_PER_TOKEN;
            let kept = trim_to_budget(&ids, &c, tokens);
            assert_eq!(kept.len(), 1, "budget {tokens} must still return the top hit");
            assert_eq!(kept[0].1.chars().count(), cap, "budget {tokens} must fill exactly the cap");
            assert!(!kept[0].1.contains(TRUNCATION_MARKER), "the marker does not fit in {cap} chars");
        }

        // One step past it, the marker fits and is appended, still inside the cap.
        let tokens = marker_len / CHARS_PER_TOKEN + 1;
        let kept = trim_to_budget(&ids, &c, tokens);
        assert!(kept[0].1.ends_with(TRUNCATION_MARKER), "the marker must appear once it fits");
        assert_eq!(kept[0].1.chars().count(), tokens * CHARS_PER_TOKEN);
    }
}
