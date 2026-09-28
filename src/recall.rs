//! Hybrid recall: BM25 + token overlap fused with RRF(k=60).
//!
//! Hindsight TEMPR (semantic/keyword/graph/temporal) x agentmemory
//! triple-stream (BM25/vector/graph). See PLAN.md Phase 2.
//!
//! Three streams ship: FTS5 BM25, a token-overlap ranker that is the stand-in
//! for the second stream, and — at weight 0.0, i.e. switched off — a
//! distinct-term-coverage ranker over the same candidates as the second. There
//! is no vector stream and no graph stream, and none is planned behind a flag —
//! the kernel fuses whatever it is given, and today that is three.

use std::collections::HashMap;
use std::cmp::Ordering;

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
/// `bm25`, `overlap` and `coverage` scale streams 0, 1 and 2. A weight of 0.0
/// drops its stream's contribution entirely, which is what makes a one-stream
/// arm a row in the sweep rather than a different code path. There is no fourth
/// stream, and a stream past the third would silently fall through to 1.0 — a
/// no-op dressed up as configuration, which is why the weight lookup names every
/// index it serves instead of counting them.
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
    /// Weight on the third stream — **distinct-term coverage** (Phase E4).
    ///
    /// Ranks the *same* candidate pool the overlap stream ranks, but by how
    /// much of the query's distinct-token set each document contains, ignoring
    /// how often the query repeats a token. The two orderings are identical
    /// whenever the query has no repeated token, so this is a no-op on most
    /// queries by construction rather than by tuning; it earns its place only
    /// on a query that repeats a word. `0.0` — what ships — drops the stream
    /// entirely, so an unmeasured E4 costs the recall path one `f64` per
    /// candidate and nothing else.
    pub coverage: f64,
    /// Whether the overlap stream weights matched query terms by IDF (Phase E3).
    ///
    /// `false` — what ships — is the raw distinct-token count the stream has
    /// always scored on, computed on the exact code path it has always used, so
    /// the E1 grid rows stay bit-identical. `true` replaces each matched term's
    /// unit contribution with its inverse document frequency in the candidate
    /// pool, `ln(1 + (N − df + 0.5)/(df + 0.5))`.
    ///
    /// Carried in the struct rather than beside it because it is a property of
    /// the stream, and the only way to price it is to run both orderings over
    /// the same index in one sweep.
    pub overlap_idf: bool,
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
    ///
    /// `coverage` and `overlap_idf` are Phase E3/E4 and are documented at their
    /// fields; both ship off unless a gate said otherwise.
    pub const SHIPPED: FusionWeights = FusionWeights {
        bm25: 1.0,
        overlap: 0.25,
        agreement: 0.0,
        coverage: 0.0,
        overlap_idf: false,
        k: RRF_K,
    };

    /// The weight of stream `i`, which is positional because the fusion is.
    ///
    /// Every index it can be handed is named. The fall-through is 1.0 rather
    /// than 0.0 so that a caller adding a stream without a weight gets a
    /// *visible* number in the swept grid rather than a stream that vanished.
    fn of(&self, i: usize) -> f64 {
        match i {
            0 => self.bm25,
            1 => self.overlap,
            2 => self.coverage,
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
///
/// Returns `(score, distinct tokens matched)`. The count is what E4's coverage
/// stream ranks on, so it is read out of the same pass rather than recomputed.
fn score_doc(
    doc: &str,
    index: &HashMap<&str, (usize, usize)>,
    seen: &mut [bool],
    buf: &mut String,
) -> (usize, usize) {
    seen.fill(false);
    let mut score = 0;
    let mut matched = 0;
    for token in lowercase_tokens(doc, buf) {
        if let Some(&(slot, repeats)) = index.get(token) {
            if !seen[slot] {
                seen[slot] = true;
                score += repeats;
                matched += 1;
            }
        }
    }
    (score, matched)
}

/// Score a document against query tokens by overlap count (BM25 stand-in).
///
/// Returns the number of distinct query tokens present in the document.
pub fn overlap_score(query_tokens: &[String], doc: &str) -> usize {
    let (index, mut seen) = build_query_index(query_tokens);
    score_doc(doc, &index, &mut seen, &mut String::new()).0
}

/// One document's result from the overlap stream: the score it ranks on, and
/// how much of the query's distinct-token set it covers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlapHit {
    /// The stream's ranking score — the raw distinct-token count, or its
    /// IDF-weighted sum when [`FusionWeights::overlap_idf`] is set.
    pub score: f64,
    /// Distinct query tokens present, over distinct query tokens in the query.
    ///
    /// The denominator is a constant within one query, so this ordering is the
    /// distinct-match count ordering — it differs from `score` only where the
    /// query repeats a token, which is exactly the difference E4 is after.
    pub coverage: f64,
}

/// Inverse document frequency of one query token within the candidate pool.
///
/// `ln(1 + (N - df + 0.5)/(df + 0.5))` — the BM25 weighting, in the same shape
/// FTS5's `bm25()` uses it, so the second stream is weighted the way the first
/// one already is rather than by a second, different idea of rarity.
///
/// **Why the pool, and not the FTS5 index.** FTS5 exposes no per-term
/// statistic to SQL: `bm25()` is a whole-document score, there is no `idf()`
/// function, and the one virtual table that does expose per-term document
/// counts — `fts5vocab` — has to be *created* and populated, which is a new
/// schema object and a migration. The pool is also the right denominator,
/// independently: the overlap stream ranks the pool, so "how rare is this term"
/// is only ever a question relative to the set being ranked. `N` is the pool
/// length (`store::RECALL_POOL_LIMIT` newest rows ∪ the BM25 top-50, so at most
/// ~250) and `df` is the number of *those* documents containing the term,
/// counted in the single tokenizing pass the stream already makes over the pool.
///
/// **Unseen terms.** A term with `df == 0` occurs in no pool document, so its
/// bit is never set and it contributes nothing — the term cannot inflate a
/// score, it is simply absent, exactly as a non-matching term is today. A term
/// in *every* document (`df == N`) gets `ln(1 + 0.5/(N+0.5))`, a small but
/// strictly positive weight: universal terms still separate a matching document
/// from a non-matching one, so the stream can never silently empty itself of
/// the documents that match a query at all.
fn idf(n_docs: usize, df: usize) -> f64 {
    let n = n_docs as f64;
    let d = df as f64;
    (1.0 + (n - d + 0.5) / (d + 0.5)).ln()
}

/// IDF-weighted scores and coverage for every document, in one tokenizing pass.
///
/// One pass is not an optimisation detail: re-tokenizing the pool to read `df`
/// back off the second pass would double the cost of the dominant loop in
/// [`rank_candidates`]. Instead each document's matched query tokens go into a
/// flat bit matrix (`n_docs * ceil(slots/64)` words) as they are seen, and the
/// weighted sum is pure arithmetic over that matrix afterwards. At this store's
/// pool ceiling that is 2 KB for a query of up to 64 distinct tokens and 4 KB for
/// 128, and it is the only per-recall allocation the IDF path makes.
fn score_docs_idf(
    docs: &[&str],
    index: &HashMap<&str, (usize, usize)>,
    seen: &mut [bool],
    buf: &mut String,
) -> Vec<OverlapHit> {
    let slots = index.len();
    let words = slots.div_ceil(64);
    let n = docs.len();
    let mut masks = vec![0u64; n * words];
    let mut df = vec![0usize; slots];
    for (d, doc) in docs.iter().enumerate() {
        seen.fill(false);
        let base = d * words;
        for token in lowercase_tokens(doc, buf) {
            if let Some(&(slot, _)) = index.get(token) {
                if !seen[slot] {
                    seen[slot] = true;
                    df[slot] += 1;
                    masks[base + slot / 64] |= 1u64 << (slot % 64);
                }
            }
        }
    }
    // Repeats per slot, hoisted out of the per-document loop: the query-side
    // count is a property of the query, and reading it through the `HashMap`
    // inside the inner loop would be a lookup per matched term per document.
    let mut repeats = vec![0.0f64; slots];
    for &(slot, rep) in index.values() {
        repeats[slot] = rep as f64;
    }
    let weight: Vec<f64> = (0..slots).map(|s| idf(n, df[s])).collect();
    let mut out = Vec::with_capacity(n);
    for (d, _) in docs.iter().enumerate() {
        let base = d * words;
        let mut score = 0.0;
        let mut matched = 0usize;
        for (w, chunk) in masks[base..base + words].iter().enumerate() {
            let mut bits = *chunk;
            while bits != 0 {
                let slot = w * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                score += repeats[slot] * weight[slot];
                matched += 1;
            }
        }
        out.push(OverlapHit {
            score,
            coverage: matched as f64 / slots as f64,
        });
    }
    out
}

/// Rank documents against a query (higher score first, stable by position).
///
/// `docs` is walked in iteration order; the returned indices are positions in it.
/// `idf` selects the E3 weighting — see [`FusionWeights::overlap_idf`]. The
/// unweighted path is the one that shipped, byte for byte: it is the same
/// single pass over the same scratch buffer, so a sweep row that turns E3 off
/// reproduces the E1 numbers rather than merely resembling them.
pub fn rank_candidates<'a, I>(query: &str, docs: I, idf: bool) -> Vec<(usize, OverlapHit)>
where
    I: IntoIterator<Item = &'a str>,
{
    let qt = tokenize(query);
    // Hoisted once: the query index, its dedup flags, and the scratch buffer are
    // reused for every document, so scoring the pool allocates nothing per doc.
    let (index, mut seen) = build_query_index(&qt);
    let slots = index.len();
    if slots == 0 {
        return Vec::new();
    }
    let mut buf = String::new();
    let scale = 1.0 / slots as f64;
    let mut out: Vec<(usize, OverlapHit)> = if idf {
        let docs: Vec<&str> = docs.into_iter().collect();
        score_docs_idf(&docs, &index, &mut seen, &mut buf)
            .into_iter()
            .enumerate()
            .collect()
    } else {
        docs.into_iter()
            .enumerate()
            .map(|(i, d)| {
                let (score, matched) = score_doc(d, &index, &mut seen, &mut buf);
                (
                    i,
                    OverlapHit { score: score as f64, coverage: matched as f64 * scale },
                )
            })
            .collect()
    };
    // A document matching no query token scores exactly 0.0 in both modes —
    // every weight is strictly positive — so this test keeps its meaning: it is
    // still "matched at least one query token", not "scored above some floor".
    out.retain(|(_, h)| h.score > 0.0);
    out.sort_by(|a, b| {
        b.1.score
            .partial_cmp(&a.1.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
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
        let ranked = rank_candidates("jose auth", docs.iter().map(String::as_str), false);
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
        assert_eq!(rank_candidates("auth", docs, false).len(), 1);
        assert_eq!(rank_candidates("auth", docs, false)[0].0, 0);
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

    // ---- E3: IDF weighting in the overlap stream -----------------------------

    /// A small, deterministic corpus: one term everywhere, one term on one row.
    ///
    /// `FILLER` is padded to `FILLER_ROWS` copies of itself, and `NEEDLE` occurs
    /// exactly once, in the row named by `NEEDLE_ROW`. Every term the filler rows
    /// and the needle row share is therefore *maximally* common and the needle
    /// term is *maximally* rare — the two extremes of what IDF separates, with
    /// no generator and no randomness in the way.
    const FILLER: &str = "the release notes cover auth and deploys";
    const FILLER_ROWS: usize = 40;
    const NEEDLE: &str = "quixotic";
    /// Late in the pool on purpose: every tie in these tests is broken by
    /// position, so the needle has to be somewhere a position tiebreak loses.
    const NEEDLE_ROW: usize = 20;

    /// `NEEDLE_ROWS` (query) × the filler corpus. Position 0 is the needle row;
    /// every other row is a copy of [`FILLER`].
    fn needle_corpus() -> Vec<String> {
        (0..FILLER_ROWS)
            .map(|i| {
                if i == NEEDLE_ROW {
                    format!("{FILLER} {NEEDLE}")
                } else {
                    FILLER.to_string()
                }
            })
            .collect()
    }

    /// **The behaviour E3 exists to create.** A document matching one *rare*
    /// query term must outrank a document matching an equal number of *common*
    /// ones.
    ///
    /// The query is `auth deploys quixotic release`. Every filler row contains
    /// `auth` and `deploys` and neither rare term; the needle row contains
    /// `quixotic` and `release` and neither common one. **Both rows match exactly
    /// two query terms**, so a raw token count ties them and the tiebreak is pool
    /// position — and the needle row is placed late, so the raw stream puts a
    /// filler row first. Only term rarity can reverse that.
    #[test]
    fn idf_weighting_should_rank_a_rare_term_above_an_equal_count_of_common_ones() {
        let corpus: Vec<String> = (0..FILLER_ROWS)
            .map(|i| {
                if i == NEEDLE_ROW {
                    format!("{NEEDLE} release rollout note")
                } else {
                    format!("auth deploys rollout note {i}")
                }
            })
            .collect();
        let docs: Vec<&str> = corpus.iter().map(String::as_str).collect();
        let query = format!("auth deploys {NEEDLE} release");

        let raw = rank_candidates(&query, docs.iter().copied(), false);
        let raw_needle = raw.iter().find(|(i, _)| *i == NEEDLE_ROW).unwrap().1.score;
        let raw_filler = raw.iter().find(|(i, _)| *i == 1).unwrap().1.score;
        assert_eq!(raw_needle, raw_filler, "the counts must tie, or this proves nothing");
        assert_ne!(raw[0].0, NEEDLE_ROW, "raw counting must lose the tie on position");

        let weighted = rank_candidates(&query, docs.iter().copied(), true);
        assert_eq!(
            weighted[0].0, NEEDLE_ROW,
            "two rare terms must outrank two common ones once rarity is counted"
        );
    }

    /// The same claim in its stronger form: **one** rare term beats **two**
    /// common ones, where a raw count is not a tie but points the other way.
    ///
    /// A filler row matches `auth` and `deploys`; the needle row matches only
    /// `quixotic`. Raw scores are 2 and 1 and the filler wins outright. This is
    /// the mechanism the recall-curve harness is built to expose — a haystack
    /// repeating a small set of topic sentences against one row carrying a
    /// unique token — so it is the case that has to invert, not the tie.
    #[test]
    fn idf_weighting_should_let_one_rare_term_beat_two_common_ones() {
        let corpus: Vec<String> = (0..FILLER_ROWS)
            .map(|i| {
                if i == NEEDLE_ROW {
                    format!("{NEEDLE} rollout note")
                } else {
                    format!("auth deploys rollout note {i}")
                }
            })
            .collect();
        let docs: Vec<&str> = corpus.iter().map(String::as_str).collect();
        let query = format!("auth deploys {NEEDLE}");

        let raw = rank_candidates(&query, docs.iter().copied(), false);
        assert_ne!(raw[0].0, NEEDLE_ROW, "raw counting ranks the two-common-term row first");
        let weighted = rank_candidates(&query, docs.iter().copied(), true);
        assert_eq!(weighted[0].0, NEEDLE_ROW, "rarity must invert that");
    }

    /// E3 weights by **term rarity**; it is E4 that stops a query-side repeat
    /// being a ranking advantage. Pinning the boundary so the two levers cannot
    /// be confused later: with every query term equally rare, IDF leaves the
    /// ordering exactly where the repeat-weighting put it.
    #[test]
    fn idf_should_change_order_only_by_rarity_not_by_repeats() {
        let docs = ["auth", "jose retry"];
        let query = "auth auth auth jose retry";
        // Both documents contain every term they match, so `df == 1` for all
        // three query terms across a two-document pool: one shared weight, and
        // therefore nothing for IDF to reweight. The query-side repeat is still
        // worth what it always was.
        let order = |v: Vec<(usize, OverlapHit)>| v.into_iter().map(|(i, _)| i).collect::<Vec<_>>();
        let weighted = order(rank_candidates(query, docs, true));
        let raw = order(rank_candidates(query, docs, false));
        assert_eq!(weighted, raw, "equal rarity must leave the order alone");
        assert_eq!(idf(2, 1), idf(2, 1), "one shared df must give one shared weight");
    }

    /// Turning E3 off must reproduce the raw count exactly, in both score and
    /// order, for every corpus — including the one the E3 tests use.
    ///
    /// This is what keeps the E1 rows comparable: `FusionWeights::overlap_idf:
    /// false` is meant to be the code that shipped, not a similar-looking
    /// reimplementation, and the sweep diffs its rows against E1's committed
    /// numbers.
    #[test]
    fn the_unweighted_path_should_reproduce_the_raw_distinct_token_count() {
        for query in [
            "auth deploys quixotic",
            "auth auth auth jose retry",
            "the the the",
            "jose",
            "nomatch at all",
            "über ΑΘΗΝΑ auth",
        ] {
            for corpus in [
                needle_corpus(),
                vec![
                    "auth".to_string(),
                    "jose retry".to_string(),
                    "nothing here".to_string(),
                ],
                vec![FILLER.to_string()],
            ] {
                let docs: Vec<&str> = corpus.iter().map(String::as_str).collect();
                let ranked = rank_candidates(query, docs.iter().copied(), false);
                let qt = tokenize(query);
                for (i, hit) in &ranked {
                    assert_eq!(
                        hit.score,
                        overlap_score(&qt, &corpus[*i]) as f64,
                        "raw score drift for query {query:?} at row {i}"
                    );
                }
                assert!(
                    ranked.windows(2).all(|w| w[0].1.score >= w[1].1.score),
                    "raw mode must stay sorted by descending score"
                );
            }
        }
    }

    /// A term every document carries still has to separate the documents that
    /// contain it from the ones that do not.
    ///
    /// `ln(1 + (N-df+0.5)/(df+0.5))` goes to a *small positive* number at
    /// `df == N` rather than to zero or negative, so a query made only of
    /// universal terms does not empty the stream: the documents that match it
    /// are still returned, and still in a stable order. A weight that could reach
    /// zero would silently drop them, and a weight that could go negative would
    /// rank the worst match first.
    #[test]
    fn a_universal_term_should_still_score_strictly_positive() {
        let docs = ["the auth", "the deploys", "unrelated text"];
        let ranked = rank_candidates("the", docs, true);
        assert_eq!(ranked.len(), 2, "a universal term must still match its documents");
        for (_, hit) in &ranked {
            assert!(hit.score > 0.0, "a universal term must score above zero: {}", hit.score);
        }
        // And it must be a *small* weight: the whole point is that a term in
        // every document carries almost no ranking information.
        let univ = idf(3, 3);
        let rare = idf(3, 1);
        assert!(univ > 0.0 && univ < rare, "universal {univ} must be positive and below rare {rare}");
    }

    /// An unseen term must contribute nothing rather than something large.
    ///
    /// A query token that occurs in no pool document gets the *largest* IDF the
    /// formula can produce, so if the weighting ever consulted `df` without the
    /// document's own match set it would hand free score to matches of other
    /// terms. It cannot: the term's bit is never set in any row, so it is absent
    /// from every score.
    #[test]
    fn an_unseen_term_should_contribute_nothing() {
        let docs = ["auth deploys", "auth deploys"];
        let seen = rank_candidates("auth", docs, true);
        let unseen = rank_candidates("auth quixotic", docs, true);
        assert_eq!(seen[0].1.score, unseen[0].1.score, "an unmatched term must not move a score");
        assert!(idf(2, 0) > idf(2, 1), "df=0 is the highest weight, and must still be unreachable");
    }

    /// Coverage is the distinct-match count over the query's distinct-token
    /// count, so it is the quantity E4's third stream ranks on, and it is read
    /// out of the same pass as the score rather than recomputed.
    #[test]
    fn coverage_should_count_distinct_query_terms_not_query_occurrences() {
        // `auth` three times in a two-distinct-term query; the document matches
        // `auth` and not `deploys`, so it covers one of two — the repetition
        // must not raise it to 3/2 or lower it to 1/3.
        let hit = rank_candidates("auth auth auth deploys", ["auth only"], false)[0].1;
        assert_eq!(hit.coverage, 0.5, "one of two distinct query terms");
        // Every distinct term matched is 1.0 whatever the query's repetition.
        let all = rank_candidates("auth auth auth deploys", ["deploys auth"], false)[0].1;
        assert_eq!(all.coverage, 1.0);
        // And it is identical in both modes, so E4 is not itself an E3 effect.
        let weighted = rank_candidates("auth auth auth deploys", ["auth only"], true)[0].1;
        assert_eq!(weighted.coverage, hit.coverage);
    }

    // ---- E4: the distinct-term-coverage stream ------------------------------

    /// The order the coverage stream hands to [`rrf_fuse`], built the way
    /// `api.rs` builds it: re-sorted from the overlap stream's own output rather
    /// than re-scored. Kept here so a test can state a property of that stream
    /// instead of re-deriving the sort in three places.
    fn coverage_order(query: &str, docs: &[&str]) -> Vec<usize> {
        let mut ranked = rank_candidates(query, docs.iter().copied(), false);
        ranked.sort_by(|a, b| {
            b.1.coverage
                .partial_cmp(&a.1.coverage)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        ranked.iter().map(|(i, _)| *i).collect()
    }

    /// **The behaviour E4 exists to create.** Ranking by *distinct* query terms
    /// covered must invert a raw count that is decided by how often the *query*
    /// repeats a word.
    ///
    /// `auth` three times in the query, `jose` and `retry` once each. Row 0
    /// contains only `auth` — the raw stream charges it 3 and ranks it first.
    /// Row 1 contains `jose` and `retry` — the raw stream charges it 2, but it
    /// covers two of the three distinct terms the caller actually asked about,
    /// so the coverage stream has to put it first.
    #[test]
    fn coverage_should_rank_broader_distinct_match_above_a_repeated_query_term() {
        let docs = ["auth", "jose retry"];
        let query = "auth auth auth jose retry";
        let by_score: Vec<usize> = rank_candidates(query, docs, false)
            .iter()
            .map(|(i, _)| *i)
            .collect();
        assert_eq!(by_score, vec![0, 1], "the raw count charges the repeat three times");
        assert_eq!(coverage_order(query, &docs), vec![1, 0], "coverage must reverse it");
    }

    /// The coverage stream is a no-op for a query with no repeated token, which
    /// is most queries — and the reason E4 is a narrow lever rather than a
    /// second re-ranking of the overlap stream. Stated as a property so a future
    /// change to the scorer cannot quietly turn it into a duplicate stream that
    /// double-counts every matched term.
    #[test]
    fn coverage_should_be_the_same_ordering_as_the_score_when_no_token_repeats() {
        let docs = [
            "auth deploys",
            "auth",
            "auth deploys retry quixotic",
            "retry",
            "nothing relevant",
        ];
        for query in ["auth deploys retry", "jose retry", "auth quixotic deploys"] {
            let by_score: Vec<usize> = rank_candidates(query, docs, false)
                .iter()
                .map(|(i, _)| *i)
                .collect();
            assert_eq!(by_score, coverage_order(query, &docs), "coverage must be a no-op for {query:?}");
        }
    }
}
