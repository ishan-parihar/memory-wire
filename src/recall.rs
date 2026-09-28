//! Hybrid recall: BM25 + token overlap fused with RRF(k=60).
//!
//! Hindsight TEMPR (semantic/keyword/graph/temporal) x agentmemory
//! triple-stream (BM25/vector/graph). See PLAN.md Phase 2.
//!
//! Two streams ship: FTS5 BM25, and a token-overlap ranker that is the stand-in
//! for the second. A third stream (distinct-term coverage, Phase E4) and an IDF
//! weighting of the second (Phase E3) were both measured, rejected, and removed;
//! `docs/NEXT_ITERATION.md` is what they left behind. There is no vector stream
//! and no graph stream, and none is planned behind a flag — the kernel fuses
//! whatever it is given, and today that is two.
//!
//! # What is *available* and inert
//!
//! Four retrieval mechanisms are implemented here and all four ship **off**, at
//! a default of `0.0` or at the value the code already had. With the shipped
//! [`FusionWeights::SHIPPED`], `rrf_fuse` reduces to the plain two-stream
//! `Σ 1/(k + rank)` it has always been, bit for bit — the inertness is pinned by
//! `a_zero_bm25_magnitude_should_leave_the_fusion_bit_identical` in this file
//! and by `the_shipped_recall_should_be_the_pre_change_fusion` in `api.rs`.
//!
//! They are off because `AGENTS.md` §1 forbids choosing a value by looking at
//! `eval/RESULTS.md`. Each mechanism below states the argument that justifies
//! its *existence* — which is a mechanistic argument and may be made now — while
//! every number that would make it *do* something is left at an inert default
//! for a later selection pass against a dev set (`eval/data/locomo/`, per
//! `docs/EVALUATION_HYGIENE.md` §3.1). Turning one on is a one-line edit to
//! [`FusionWeights::SHIPPED`].
//!
//! | Mechanism | Knob | Inert default | What it restores |
//! |---|---|---|---|
//! | BM25 magnitude | [`FusionWeights::bm25_magnitude`] | `0.0` | the score FTS5 already computed and the rank-only RRF form discards |
//! | Recency stream | [`FusionWeights::recency`] | `0.0` | a third fusion stream over `memories.created_at`, which no stream reads |
//! | Sentence overlap | [`FusionWeights::overlap_scope`] | [`OverlapScope::Document`] | per-segment instead of whole-document overlap scoring |
//! | Temporal classifier | [`is_temporal_query`] | not on the recall path | lets the recency weight apply only to time-denoting queries |
//!
//! These are `docs/RERANKING_PLAN.md` §8 Step 2 ("restore the BM25 magnitude,
//! mechanistic, no free parameters") and §4 items 3 and 7, implemented
//! *before* their values are chosen.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use chrono::{DateTime, Utc};

use crate::memory::Memory;

/// RRF constant (agentmemory k=60).
pub const RRF_K: f64 = 60.0;

/// Seconds in a day, the unit [`FusionWeights::recency_half_life_days`] is
/// counted in. Fixed rather than derived from a clock so the decay a test
/// asserts is the decay that runs.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// Approximate characters per token (the usual English 4 chars ≈ 1 token).
const CHARS_PER_TOKEN: usize = 4;

/// Joins lowercased tokens inside the scratch buffer that [`lowercase_tokens`]
/// borrows them out of. Every character in that buffer is alphanumeric except
/// this one, so splitting on it recovers the tokens unambiguously.
const TOKEN_SEP: char = '\n';

/// Appended to content that was cut down to fit the token budget.
pub const TRUNCATION_MARKER: &str = "…[truncated]";

/// A ranked candidate from one retrieval stream.
///
/// No magnitude field, deliberately: adding one would break every
/// `RankedHit { id, rank }` literal in the tree, `examples/oracle_rerank.rs`
/// among them, and this struct's shape is the public contract other harnesses
/// construct against. The BM25 magnitude travels beside the stream instead —
/// see [`rrf_fuse_with_magnitudes`].
#[derive(Debug, Clone, PartialEq)]
pub struct RankedHit {
    /// Memory id.
    pub id: String,
    /// 1-based rank within its stream.
    pub rank: usize,
}

/// How much of a document the overlap stream scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlapScope {
    /// The whole document, which is what [`overlap_score`] and
    /// [`rank_candidates`] have always done. The default, and therefore the
    /// shipped behaviour.
    #[default]
    Document,
    /// The single densest segment, via [`best_sentence_overlap`].
    ///
    /// Whole-document overlap is a *presence* test: a query token counts if it
    /// appears anywhere at all, so a long session that mentions each of four
    /// query words once, in four different places, outranks a short one that is
    /// densely about all four. Scoring the best segment instead asks the
    /// narrower question — is there a *span* of this document that is about the
    /// query — which is the same thing BM25's term-proximity intuition already
    /// encodes and which a whole-document count cannot.
    BestSentence,
}

/// When [`FusionWeights::recency`] applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecencyPolicy {
    /// Every query, at the configured weight. The default.
    #[default]
    Always,
    /// Only queries [`is_temporal_query`] reads as denoting a point or a span in
    /// time; every other query gets `0.0`, which drops the stream outright.
    ///
    /// The asymmetry is the reason to have the policy at all: "what did I ask
    /// for last week" and "how does the rate limiter work" are both recall
    /// queries, but only the first is about *when*, and a recency prior that
    /// fires on the second one is answering a question nobody asked.
    TemporalQueriesOnly,
}

/// A recency half-life that is inert rather than chosen.
///
/// `30.0` days is a **placeholder, not a measurement**. It is safe to leave
/// here precisely because [`FusionWeights::recency`] ships at `0.0`, which
/// drops the stream before the half-life is ever read; and a value that *were*
/// needed cannot be picked by looking at `eval/RESULTS.md` (`AGENTS.md` §1), so
/// there is nothing honest to put here but a stated placeholder. Select it on a
/// dev set, or replace the whole decay with a half-life read from the caller's
/// own retention window.
pub const RECENCY_HALF_LIFE_PLACEHOLDER_DAYS: f64 = 30.0;

/// Per-stream weights for the fusion, plus an optional cross-stream agreement
/// bonus.
///
/// The default is [`FusionWeights::SHIPPED`]. Before the E1 sweep the shipped
/// value was both streams at 1.0; the sweep moved `overlap` to 0.25, and that
/// measured move is the whole reason the struct exists. Weights are here to be
/// *swept* (`examples/sweep_fusion.rs`), and nothing in the shipped binary
/// changes one per request — see `docs/NEXT_ITERATION.md` for the grid.
///
/// `bm25` and `overlap` scale streams 0 and 1. A weight of 0.0 drops its
/// stream's contribution entirely, which is what makes a one-stream arm a row in
/// the sweep rather than a different code path. A stream past the second would
/// silently fall through to 1.0 — a no-op dressed up as configuration, which is
/// why the weight lookup names every index it serves instead of counting them.
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
    /// Lift, in units of one rank-1 RRF hit, that the *best-BM25* row of the
    /// query's stream may claim over the plain rank-only RRF score.
    /// **Inert at `0.0`, which is what ships.**
    ///
    /// # Why this exists
    ///
    /// FTS5's `bm25()` returns a real number — more negative is better — and
    /// `store.rs` selects on it and then discards it, so the fusion sees a
    /// *rank* and nothing else. That loses exactly the case
    /// `docs/RERANKING_PLAN.md` §4 item 3 names: a row at rank 7 with a far
    /// better BM25 score than a marginal row at rank 1 is invisible, because
    /// RRF is a function of rank alone and 1/(60+7) is only 9% below
    /// 1/(60+1) no matter how large the score gap was. The value is already
    /// computed; restoring it is a signal restoration, not a new signal.
    ///
    /// # The algebra, and why this one
    ///
    /// The blend is `rrf + w · u/(k+1)`, where `u` is the row's BM25 magnitude
    /// **min-max normalised over the query's own BM25 stream** to `[0, 1]` with
    /// `1` at the best row. Three properties fall out, and each is the reason a
    /// different obvious alternative was rejected:
    ///
    /// - *Bounded.* `u ∈ [0, 1]`, so the whole term is at most `w/(k+1)` and the
    ///   magnitude can never outvote a full rank-1 RRF hit on its own. The raw
    ///   alternative — `w · magnitude` — has no such bound and no portable
    ///   scale: `bm25()` returns roughly `-0.001` for a one-token query that
    ///   matches a thousand rows and roughly `-14` for a three-token query that
    ///   matches three, so one `w` cannot mean one thing across queries.
    /// - *Per-query relative, which is the tie-breaker's job.* Min-max over the
    ///   stream says "better BM25 than its competitors for this query", not
    ///   "confidently relevant". A hard confidence floor would be the second
    ///   thing — a threshold — and a threshold is exactly what `AGENTS.md` §1
    ///   forbids acquiring without a dev set.
    /// - *Denominated in `1/(k+1)`*, the same unit as the `agreement` bonus, so
    ///   `w` is legible: `0.25` means "the best-BM25 row of this query gets a
    ///   quarter of a rank-1 hit" and one number governs both additive terms.
    ///
    /// # What `0.0` buys
    ///
    /// Exact: `+ 0.0` is bit-identity on a finite `f64`, so the whole term is
    /// skipped, not merely multiplied by zero. A single-stream test suite
    /// upstream of this cannot tell the difference — which is the point, and is
    /// pinned rather than asserted in prose.
    pub bm25_magnitude: f64,
    /// Weight on the recency stream (stream 2), the RRF term a
    /// [`recency_rank`] ordering is fused under. **Inert at `0.0`, which is
    /// what ships**, and which drops the stream outright rather than shrinking
    /// it — the behaviour `a_zero_weight_should_remove_its_stream` already
    /// pins for streams 0 and 1.
    ///
    /// `created_at` is `TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z'`, indexed
    /// as `idx_memories_bank_time(bank_id, created_at)`, and until this field
    /// existed it was read by the forgetting sweep and by nothing on the recall
    /// path at all: neither stream is time-aware, so two memories identical in
    /// text order but three months apart were indistinguishable to the ranking.
    /// A third stream is the only way to make the index behind `created_at` pay
    /// for itself on recall.
    ///
    /// # A third stream at weight 1.0 is a wash, and that is algebra
    ///
    /// RRF's discount is the *same* `1/(k + rank)` shape in every stream, so
    /// when the new stream's ordering is the exact reverse of the existing one,
    /// a weight of `1.0` hands the tail `+1/(k+1)` and takes `1/(k+1) - 1/(k+n)`
    /// off the leader — and for `n ≥ 2` the leader keeps the higher sum. Three
    /// streams at unit weight cannot reorder a two-stream result that the third
    /// one merely inverts. The useful weights are therefore the ones far from
    /// 1.0, and the sign matters as much as the size: a recency prior that agrees
    /// with the lexical ranking is nearly free, and one that fights it has to be
    /// paid for.
    ///
    /// This is a property of RRF, not of this implementation, and it is stated
    /// here because the value has to be selected against a dev set and the
    /// selection would otherwise rediscover it.
    pub recency: f64,
    /// Half-life, in days, of [`FusionWeights::recency`]'s exponential decay.
    /// See [`RECENCY_HALF_LIFE_PLACEHOLDER_DAYS`] for why the default is a
    /// stated placeholder rather than a chosen number.
    pub recency_half_life_days: f64,
    /// Which questions [`FusionWeights::recency`] is allowed to fire on.
    pub recency_policy: RecencyPolicy,
    /// How much of a document the overlap stream scores. The default,
    /// [`OverlapScope::Document`], is the whole-document count that has always
    /// shipped.
    pub overlap_scope: OverlapScope,
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
        // Inert, and inert *by construction* rather than by luck: 0.0 skips the
        // magnitude term outright, 0.0 drops the third stream before its
        // half-life is read, and `Document` is the whole-document count this
        // crate has always run. See the module docs for why they are off.
        bm25_magnitude: 0.0,
        recency: 0.0,
        recency_half_life_days: RECENCY_HALF_LIFE_PLACEHOLDER_DAYS,
        recency_policy: RecencyPolicy::Always,
        overlap_scope: OverlapScope::Document,
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
            2 => self.recency,
            _ => 1.0,
        }
    }

    /// The recency weight that actually applies to `query`, which is
    /// [`Self::recency`] narrowed by [`Self::recency_policy`].
    ///
    /// The single place that policy is resolved, so a caller building the third
    /// stream and the fusion weighting it cannot disagree about whether the
    /// stream is live: both read this, and a `0.0` here means the stream is
    /// never built at all — which is what keeps [`is_temporal_query`] and the
    /// clock behind it off the shipped recall path entirely.
    pub fn recency_weight_for(&self, query: &str) -> f64 {
        match self.recency_policy {
            RecencyPolicy::Always => self.recency,
            RecencyPolicy::TemporalQueriesOnly if is_temporal_query(query) => self.recency,
            RecencyPolicy::TemporalQueriesOnly => 0.0,
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
    // No magnitudes: the shipped `bm25_magnitude` is 0.0, so this is the
    // magnitude-free kernel and the hot recall path never builds a lookup for a
    // term that cannot be added. `HashMap::new` allocates nothing.
    rrf_fuse_with_magnitudes(streams, &HashMap::new(), weights)
}

/// [`rrf_fuse`] with the BM25 magnitudes FTS5 returned, keyed by memory id.
///
/// `magnitudes` is the **BM25 stream's** `bm25()` values, more negative better —
/// the second element of every pair `store::KeywordHits` already carries and
/// `api.rs` currently discards. Only ids present in `magnitudes` receive a term;
/// an id that is not there (an overlap-only row, or a backend whose
/// `Store::keyword_search` is the empty default) scores `0.0`, so the magnitude
/// can never penalise a row for not having come from the stream that produced
/// them.
///
/// The blend itself is documented on [`FusionWeights::bm25_magnitude`]. The
/// short form: `u` is min-max normalised over `magnitudes` to `[0, 1]` (`1` at
/// the best, `0` at the worst) and the term added is `w · u/(k + 1)`, so the
/// best-BM25 row of the query may claim at most a `w` fraction of one rank-1
/// RRF hit and a single-row or all-tied stream contributes nothing at all.
///
/// The normalisation is taken over the map's values rather than over
/// `streams[0]`, which is the same set under the documented contract; taking it
/// over the map means a caller that hands over a magnitude for a row the stream
/// dropped cannot stretch the range and flatten everyone else's term.
pub fn rrf_fuse_with_magnitudes(
    streams: &[Vec<RankedHit>],
    magnitudes: &HashMap<String, f64>,
    weights: &FusionWeights,
) -> Vec<(String, f64)> {
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
    // Added after the weighted pass, and skipped entirely at the shipped 0.0:
    // an exact `+ 0.0` is bit-identity on a finite f64, so the whole term is
    // left out rather than added as a no-op, and the fused scores are the same
    // `f64` values the two-stream kernel produced. Nothing downstream —
    // ordering, the id tiebreak, the reported score — can move.
    if weights.bm25_magnitude != 0.0 && !magnitudes.is_empty() {
        add_bm25_magnitude(&mut scores, magnitudes, weights, k);
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

/// Add `w · u/(k+1)` to every fused id that carries a BM25 magnitude.
///
/// `u` is the magnitude min-max normalised over `magnitudes`: `(m - worst) /
/// (best - worst)` with `best` the most negative value present. Degenerate
/// inputs — an empty map, a single entry, every magnitude equal, or any
/// non-finite one — contribute nothing to anything rather than dividing by zero
/// or re-ranking on the survivors, because "no spread to speak of", "no
/// magnitude" and "corrupt magnitude" must all be silence, and a `NaN` score
/// would poison `partial_cmp` for every id that shares it.
fn add_bm25_magnitude(
    scores: &mut HashMap<&str, f64>,
    magnitudes: &HashMap<String, f64>,
    weights: &FusionWeights,
    k: f64,
) {
    let mut best = f64::INFINITY;
    let mut worst = f64::NEG_INFINITY;
    for m in magnitudes.values() {
        // One non-finite magnitude is corrupt input, not a value to normalise
        // around. Normalising over the survivors would re-rank the whole stream
        // because of one bad row, which is a silent reordering; silence is not.
        if !m.is_finite() {
            return;
        }
        best = best.min(*m);
        worst = worst.max(*m);
    }
    // `best` is +inf when the map was empty; either way there is no range to
    // normalise over, so no row gets a term.
    let spread = best - worst;
    if !spread.is_finite() || spread == 0.0 {
        return;
    }
    let unit = weights.bm25_magnitude / (k + 1.0);
    for (id, m) in magnitudes {
        // Not in `scores` means the stream never ranked it, so there is nothing
        // to add the term to — a magnitude must never conjure a row.
        if let Some(score) = scores.get_mut(id.as_str()) {
            *score += unit * ((m - worst) / spread);
        }
    }
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
/// Returns the score.
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

/// The largest per-segment overlap anywhere in `doc`.
///
/// The alternative to [`overlap_score`] that [`OverlapScope::BestSentence`]
/// selects. Whole-document overlap is a presence test — a query token counts if
/// it appears *anywhere* — so a 2,000-word session that names each of five query
/// words exactly once, in five unrelated places, scores 5 and outranks a
/// forty-word memory that is densely about all five. Scoring segments and
/// keeping the maximum asks the narrower question BM25's intuition already
/// asks: is there a *span* here that is about the query.
///
/// # Segments
///
/// A newline is a boundary first and sentence-terminal punctuation second.
/// Newline because that is the unit this project's own documents arrive in:
/// the capture hook writes one line per compacted turn, and
/// `examples/longmemeval.rs` joins a session's turns with `"\n"`, so a newline
/// is where a new utterance starts. The three sentence terminators because a
/// memory stored as prose has no newlines at all. The whole document is always
/// scored as one segment too, so a single-sentence memory scores exactly what
/// [`overlap_score`] scores it — this can only ever *lower* a score, never lose
/// one.
///
    /// # Invariants
    ///
    /// - `best_sentence_overlap <= overlap_score` for every document. Each segment's
    ///   token set is a subset of the document's, so its distinct-query-token count
    ///   cannot exceed the document's, and the max over segments cannot either. A
    ///   boundary-free document *is* a single segment, so the two scores are equal
    ///   for it and the scope choice cannot perturb a one-sentence memory.
/// - Same tokenization as everything else: the same internal scorer and the same
///   scratch-buffer tokenizer, so this is a change of *granularity* and not of
///   definition. `overlap_score` is not touched.
    ///
    /// Cost is one pass over the document's tokens either way — the segments
    /// partition it — so this is not a slower scorer, only a differently-partitioned
    /// one.
    ///
    /// # How much it can move, before anyone goes looking
    ///
    /// Against a `1.0`-weighted BM25 and a `0.25`-weighted overlap stream, this
    /// scope can only reorder rows the overlap stream separates by enough ranks.
    /// Moving a document up one overlap rank is worth `0.25·(1/61 − 1/62) ≈
    /// 2.6e-4`, while BM25's own adjacent-rank gap is `≈ 2.6e-4` unwidened — so a
    /// one-rank swap is a coin flip and a small pool never flips at all. A real
    /// flip needs the scattered document to fall several ranks down the overlap
    /// stream, which needs a pool with several denser documents in it. It is not a
    /// dead mechanism, but it is a narrow one, and the narrowness is a property
    /// of the shipped weights rather than of the scorer.
pub fn best_sentence_overlap(query_tokens: &[String], doc: &str) -> usize {
    let (index, mut seen) = build_query_index(query_tokens);
    best_sentence_overlap_indexed(doc, &index, &mut seen, &mut String::new())
}

/// [`best_sentence_overlap`] against a hoisted query index, so a pool of
/// documents pays the tokenization setup once.
fn best_sentence_overlap_indexed(
    doc: &str,
    index: &HashMap<&str, (usize, usize)>,
    seen: &mut [bool],
    buf: &mut String,
) -> usize {
    let mut best = 0;
    for segment in segments(doc) {
        best = best.max(score_doc(segment, index, seen, buf));
    }
    best
}

/// Split `doc` into sentence-ish segments: newlines first, then `.` `!` `?`.
///
/// Returns borrowed slices, so the split costs no allocation. Empty segments
/// (a run of punctuation, a blank line) are dropped because they score 0 by
/// construction and would only make the caller do work.
fn segments(doc: &str) -> impl Iterator<Item = &str> {
    doc.split(['\n', '.', '!', '?'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Rank documents against a query (higher score first, stable by position).
///
/// `docs` is walked in iteration order; the returned indices are positions in it,
/// each paired with the score it ranked on. This is the raw single-pass scorer
/// the overlap stream has always used, on the code path it has always used it
/// on: an E1 grid row that reproduces the committed numbers is reproducing this
/// function, not something adjacent to it.
pub fn rank_candidates<'a, I>(query: &str, docs: I) -> Vec<(usize, f64)>
where
    I: IntoIterator<Item = &'a str>,
{
    let qt = tokenize(query);
    // Hoisted once: the query index, its dedup flags, and the scratch buffer are
    // reused for every document, so scoring the pool allocates nothing per doc.
    let (index, mut seen) = build_query_index(&qt);
    if index.is_empty() {
        return Vec::new();
    }
    let mut buf = String::new();
    let mut out: Vec<(usize, f64)> = docs
        .into_iter()
        .enumerate()
        .map(|(i, d)| (i, score_doc(d, &index, &mut seen, &mut buf) as f64))
        .collect();
    // A document matching no query token scores exactly 0, so this test keeps its
    // meaning: it is still "matched at least one query token", not "scored above
    // some floor".
    out.retain(|&(_, score)| score > 0.0);
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    out
}

/// [`rank_candidates`] with the scoring granularity the caller asks for.
///
/// [`OverlapScope::Document`] *is* [`rank_candidates`] — the same call, the same
/// function, not a reimplementation of it — so a whole-document recall runs the
/// identical scorer it always has, and
/// `a_whole_document_recall_should_use_the_whole_document_scorer` pins that.
///
/// [`OverlapScope::BestSentence`] keeps the rest of the ranker unchanged, so the
/// two scopes differ in exactly one place: the per-document number. The `> 0.0`
/// retain still means "matched at least one query token" under either scope, and
/// the position tiebreak is still the one the E1 sweep rows were measured with.
pub fn rank_candidates_scoped<'a, I>(
    query: &str,
    docs: I,
    scope: OverlapScope,
) -> Vec<(usize, f64)>
where
    I: IntoIterator<Item = &'a str>,
{
    match scope {
        OverlapScope::Document => rank_candidates(query, docs),
        OverlapScope::BestSentence => {
            let qt = tokenize(query);
            let (index, mut seen) = build_query_index(&qt);
            if index.is_empty() {
                return Vec::new();
            }
            let mut buf = String::new();
            let mut out: Vec<(usize, f64)> = docs
                .into_iter()
                .enumerate()
                .map(|(i, d)| {
                    (i, best_sentence_overlap_indexed(d, &index, &mut seen, &mut buf) as f64)
                })
                .collect();
            out.retain(|&(_, score)| score > 0.0);
            out.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            });
            out
        }
    }
}

/// Whether `query` is about *when* something happened.
///
/// A pure function of the query text, with no store read, no clock read, and no
/// numeric threshold anywhere — which is what makes it legitimate to write
/// before a selection pass. Every term in the pattern is a lexical-semantic
/// judgement about English, and the whole list is justified term by term in the
/// comments on the `TEMPORAL_RE` static below. A threshold set by comparing
/// benchmark numbers would not be legitimate; a word list is a definition, not a
/// fit.
///
/// # The one genuine ambiguity
///
/// `last`, `first`, `next`, `previous`, `final` and `initial` are homographs.
/// "What did I do last" is about time; "what is the last value of x" is about
/// the *final element of a set the question itself is asking for*, and no
/// recency prior should fire on it. The rule used is grammatical rather than
/// lexical — a word of this class denotes time when it modifies a **time-denoting
/// noun** (`last week`, `the previous session`) or stands as an **adverbial**,
/// i.e. at the end of the question or before its punctuation:
/// `TEMPORAL_RE` encodes exactly those two shapes, and nothing else for this
/// class. `latest`, `newest`, `oldest` and `earliest` are *not* homographs —
/// their English meaning already is recency, so they count wherever they appear.
pub fn is_temporal_query(query: &str) -> bool {
    TEMPORAL_RE.is_match(query)
}

/// The term list [`is_temporal_query`] matches, each group justified by what the
/// term denotes in English.
///
/// Compiled once behind a `LazyLock` for the reason `capture.rs` gives for its
/// redaction patterns: a `Regex::new` per call dominates the function it
/// serves. Every alternative is linear — bounded character classes and simple
/// `\s+` gaps, no nesting, no backreferences — so the crate's non-backtracking
/// NFA keeps the scan O(len × states).
static TEMPORAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)",
        // (1) Days and clock times named outright — words that are *only* ever a
        //     point in time, in any use of them.
        r"\b(?:today|yesterday|tomorrow|tonight|midnight|noon|overnight)\b",
        // (2) Calendar units. These denote time both as the point asked for
        //     ("last week") and as the unit an interval is measured in ("how
        //     many days"). "quarter" is deliberately absent: it denotes a fiscal
        //     quarter as often as it denotes 25 cents, and a recall query in a
        //     coding bank is not about money.
        r"|\b(?:day|days|week|weeks|weekend|weekends|month|months|year|years|",
        r"decade|anniversary)\b",
        // (3) Times of day — points in time as plainly as a date is.
        r"|\b(?:morning|afternoon|evening|night|nights|midday|daytime|nighttime)\b",
        // (4) The named days of the week: a closed enumeration whose every
        //     member denotes a time.
        r"|\b(?:monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b",
        // (5) The conversational units an agent's memory is actually written
        //     in. "message"/"chat" are here and not elsewhere because bare
        //     "message" is as often a *content* noun — "what did the error
        //     message say" asks about a message, not about when — while the
        //     ordinate form belongs to group (12), where "the last message" is
        //     unambiguously a time question.
        r"|\b(?:session|sessions|conversation|conversations|chat|chats|turn|turns)\b",
        // (6) Nouns denoting a *recorded* point or an ordering of points: "the
        //     date of the deploy", "the timestamp on the row", "its timeline".
        //     "time" is here only in the interrogative of group (11) and in the
        //     ordinate of group (12), because bare "time" is a content noun in
        //     this domain — "the time complexity of quicksort" is not a question
        //     about when. "history" is absent for the same reason: "the git
        //     history" is a thing being asked *about*.
        r"|\b(?:date|dates|timestamp|timestamps|timeline|chronology|era|epoch)\b",
        // (7) Adverbials of position in time. Each denotes time in every use:
        //     "before"/"after" are the ordering pair, "since"/"until" the
        //     interval pair, "then"/"when" the deixis pair, and the rest name a
        //     relation to a reference moment. "already" and "current" are absent
        //     — both describe a *state*, and both are common content words in a
        //     coding bank.
        r"|\b(?:now|then|when|whenever|since|until|before|after|afterwards|",
        r"afterward|previously|formerly|earlier|later|recently|lately|ago|",
        r"meanwhile|subsequently|just\s+now)\b",
        // (8) Superlatives whose English meaning *is* recency, so they denote
        //     time wherever they fall. None is a homograph: there is no
        //     non-temporal reading of "latest" to screen out.
        r"|\b(?:latest|newest|oldest|earliest|recent|up-to-date)\b",
        // (9) Position in a *sequence of events* rather than in the calendar:
        //     "what did we originally configure", "what eventually happened",
        //     "to begin with". The conversational corollaries of when.
        r"|\b(?:initially|firstly|originally|eventually|eventual|",
        r"finally|ultimately)\b",
        // (10) Change of state across a boundary in time. A question that names
        //     one is asking what held before the change, which is a question
        //     about when: "used to" / "use to" (the past-tense modal and the
        //     infinitive, which make the same claim after "did we"), "no longer",
        //     "since I switched".
        r"|\b(?:used|use)\s+to\b|\bno\s+longer\b|\bsince\s+(?:i|we|you|they|he|she|it)\b",
        // (11) Time asked for as a quantity, and the fixed questions that frame
        //     it. "how long" is a duration question even when the thing measured
        //     is a budget, which is the honest reading of the words.
        r"|\bhow\s+(?:long|often|far\s+back)\b|\bhow\s+many\s+times\b",
        r"|\bwhat\s+time\b|\bwhat\s+day\b|\bwhich\s+day\b|\bwhat\s+date\b",
        r"|\bwhich\s+date\b|\bwhat\s+year\b|\bsince\s+when\b|\bas\s+of\b",
        r"|\b(?:duration|elapsed|deadline|ttl)\b",
        r"|\b(?:daily|weekly|monthly|yearly|annually|hourly)\b",
        // (12) The homograph class, in the two shapes that denote time: modifying
        //      a time-denoting noun, or standing as a trailing adverbial. The noun
        //      list is groups (2), (3), (4) and (5) plus "time" and "message",
        //      which are only safe behind an ordinate. "the last value of x"
        //      matches neither shape — "value" is not a time-denoting noun and
        //      the word is not trailing — and that is the entire reason this
        //      class is written as two shapes instead of six bare words.
        r"|\b(?:last|first|next|previous|prior|final|initial)\s+",
        r"(?:time|times|week|weeks|weekend|weekends|month|months|year|years|",
        r"day|days|decade|night|nights|morning|afternoon|evening|midday|",
        r"today|yesterday|tomorrow|monday|tuesday|wednesday|thursday|friday|",
        r"saturday|sunday|session|sessions|conversation|conversations|chat|",
        r"chats|turn|turns|message|messages|attempt|attempts)\b",
        //     The trailing-adverbial shape: "what did I do last", "which one
        //     came first".
        r"|\b(?:last|first|next|previous|prior|final|initial)\s*[?.!]?\s*$",
    ))
    .expect("temporal query regex")
});

/// Rank memories by how recently they were created — the third fusion stream.
///
/// Each memory is decayed against `now` by [`FusionWeights::recency_half_life_days`]
/// and the survivors are returned as a [`RankedHit`] list, newest first, which
/// [`rrf_fuse`] then fuses under [`FusionWeights::recency`] like any other stream.
/// The decay decides *the stream's internal order* and nothing else: it never
/// reaches the fused score, so there is one scale to select (`recency`) and one
/// shape to reason about (plain `1/(k+rank)`), rather than a second additive
/// term in units nobody has agreed on.
///
/// # Why the stream exists at all
///
/// `memories.created_at` is `TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z'`,
/// indexed as `idx_memories_bank_time(bank_id, created_at)`, and until this
/// function existed it was read by the forgetting sweep and by nothing on the
/// recall path: neither stream was time-aware, so two memories identical in text
/// order but months apart ranked identically. A third stream is the only way the
/// index behind that column pays for itself on recall.
///
/// # What it is worth, honestly
///
/// **The signal is real; the evidence for it is not, yet.** `eval/RESULTS.md`
/// records `single-session-preference` as the weakest of the six LongMemEval
/// categories at 86.7% R@5 (n=30) — still the weakest, but *above*
/// agentmemory's published BM25-only 86.2%, not below it, and up from the 56.7%
/// the same category scored at equal fusion weights before the E1 reweight. The
/// brief for this mechanism cited 56.7% and "below the competitor's BM25-only
/// arm"; `AGENTS.md` §2 says the artifact wins, so both of those are out of date
/// and neither is repeated here as support. The cited competitor ablation — an
/// f2–f5 slice at 27% with token recency against 14% without — **could not be
/// found in the audited agentmemory checkout** (`../_audit/agentmemory`, searched
/// across `benchmark/`, `docs/` and every markdown file) and is therefore not
/// quoted as evidence at all.
///
/// And the mechanism is currently inert for a second, more basic reason, which
/// is a property of the harness rather than of this function:
/// **`examples/longmemeval.rs` does not read the dataset's timestamps.** The
/// dataset carries a real per-session time in `haystack_dates`
/// (`"2023/05/20 (Sat) 02:21"`, and a `question_date` beside it), but the
/// harness's `Turn` struct deserializes only `role` and `content`, and every
/// `Memory` it writes carries `created_at: None` — so `store.rs` stamps each row
/// with SQLite's insert clock, microseconds apart, in `haystack_session_ids`
/// order. On that suite `created_at` is therefore *ingest order*, not
/// conversation time, and a recency stream there ranks by "which row we
/// inserted last". Fixing the harness is another agent's file; until it is fixed
/// this stream has no valid measurement on the project's own test set, and the
/// 0.0 default is what keeps it from pretending otherwise.
///
/// # Determinism
///
/// With `now` supplied by the caller this function is pure. On the shipped path
/// nothing calls it, so the shipped recall reads no clock at all and stays
/// bit-reproducible — which `eval/RESULTS.md` states about the committed
/// artifact. A caller that supplies `Utc::now()` opts into a clock-dependent
/// ordering, which is inherent: "how recent is this" has no clock-free answer.
///
/// # Degenerate inputs
///
/// A `half_life_days` of zero, negative or `NaN`, and a `created_at` that is
/// absent or unparseable, all decay to `0.0` and rank at the bottom rather than
/// dividing by zero or producing a `NaN` a sort would silently misplace. An
/// unknown age is the epoch sentinel the store itself writes for rows that
/// predate timestamping, and its honest reading is "least recent", the same
/// reading `expire_before` gives it.
pub fn recency_rank(
    mems: &[Memory],
    now: DateTime<Utc>,
    half_life_days: f64,
) -> Vec<RankedHit> {
    if mems.is_empty() {
        return Vec::new();
    }
    let half_life_secs = half_life_days * SECONDS_PER_DAY;
    // Not a knob, a guard: the formula divides by it, and a non-positive or
    // non-finite value would put a NaN in every score.
    let usable = half_life_secs.is_finite() && half_life_secs > 0.0;
    let mut scored: Vec<(f64, &str)> = mems
        .iter()
        .map(|m| {
            let decay = match (usable, &m.created_at) {
                (true, Some(stamp)) => match DateTime::parse_from_rfc3339(stamp) {
                    // A future stamp is a clock skew, not a negative age: clamping
                    // at 0.0 makes it rank as brand new, which is what a stamp
                    // later than the reading clock has to mean.
                    Ok(t) => {
                        let age = (now - t.with_timezone(&Utc)).num_milliseconds() as f64 / 1000.0;
                        half_life_decay(age, half_life_secs)
                    }
                    Err(_) => 0.0,
                },
                _ => 0.0,
            };
            (decay, m.id.as_str())
        })
        .collect();
    // Descending decay, then id ascending, so two memories written in the same
    // millisecond — the common case on a bulk ingest — order deterministically
    // rather than by whatever the iterator happened to yield.
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.1.cmp(b.1))
    });
    scored
        .into_iter()
        .enumerate()
        .map(|(i, (_, id))| RankedHit { id: id.to_string(), rank: i + 1 })
        .collect()
}

/// `0.5^(age / half_life)`: 1.0 at age zero, 0.5 at one half-life, and never
/// negative, so a decay is always a well-formed RRF ranking input.
///
/// A negative age (a stamp in the future) returns 1.0 rather than a value above
/// it, so clock skew cannot make one row outrank every other row by an unbounded
/// amount.
fn half_life_decay(age_seconds: f64, half_life_seconds: f64) -> f64 {
    if age_seconds <= 0.0 {
        return 1.0;
    }
    0.5f64.powf(age_seconds / half_life_seconds)
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
        assert_eq!(rank_candidates("auth", docs).len(), 1);
        assert_eq!(rank_candidates("auth", docs)[0].0, 0);
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

    // ---- the raw ranker, against its own reference ---------------------------

    /// A small, deterministic pool: one term everywhere, one term on one row.
    ///
    /// `FILLER` is padded to `FILLER_ROWS` copies of itself, and `NEEDLE` occurs
    /// exactly once, in the row named by `NEEDLE_ROW` — a pool where one document
    /// carries a term none of the others do, with no generator and no randomness
    /// in the way.
    const FILLER: &str = "the release notes cover auth and deploys";
    const FILLER_ROWS: usize = 40;
    const NEEDLE: &str = "quixotic";
    /// Late in the pool on purpose: every tie is broken by position, so the
    /// needle has to be somewhere a position tiebreak loses.
    const NEEDLE_ROW: usize = 20;

    /// The filler corpus: row [`NEEDLE_ROW`] is the needle row, every other row
    /// is a copy of [`FILLER`].
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
    /// The shipped ranker scores every document exactly as the standalone
    /// [`overlap_score`] does — same number, same order — for every corpus here.
    ///
    /// This is what keeps the E1 rows comparable: the raw single-pass scorer *is*
    /// the code that shipped, not a similar-looking reimplementation, and the
    /// sweep diffs its rows against E1's committed numbers.
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
                let ranked = rank_candidates(query, docs.iter().copied());
                let qt = tokenize(query);
                for (i, score) in &ranked {
                    assert_eq!(
                        *score,
                        overlap_score(&qt, &corpus[*i]) as f64,
                        "raw score drift for query {query:?} at row {i}"
                    );
                }
                assert!(
                    ranked.windows(2).all(|w| w[0].1 >= w[1].1),
                    "the ranker must stay sorted by descending score"
                );
            }
        }
    }

    // ---- (a) BM25 magnitude ------------------------------------------------

    /// A stream whose BM25 magnitudes are chosen so the *rank* and the
    /// *magnitude* orderings disagree: `weak` is rank 1 with a marginal score,
    /// `strong` is rank 7 with a far better one. Rank-only RRF cannot see the
    /// difference; that is the whole gap this mechanism closes.
    fn split_stream() -> Vec<RankedHit> {
        vec![
            RankedHit { id: "weak".to_string(), rank: 1 },
            RankedHit { id: "mid-a".to_string(), rank: 2 },
            RankedHit { id: "mid-b".to_string(), rank: 3 },
            RankedHit { id: "mid-c".to_string(), rank: 4 },
            RankedHit { id: "mid-d".to_string(), rank: 5 },
            RankedHit { id: "mid-e".to_string(), rank: 6 },
            RankedHit { id: "strong".to_string(), rank: 7 },
        ]
    }

    fn split_magnitudes() -> HashMap<String, f64> {
        let rows = [
            ("weak", -0.5),
            ("mid-a", -2.0),
            ("mid-b", -3.0),
            ("mid-c", -4.0),
            ("mid-d", -5.0),
            ("mid-e", -6.0),
            ("strong", -12.0),
        ];
        rows.iter().map(|(id, m)| ((*id).to_string(), *m)).collect()
    }

    fn score_of(rows: &[(String, f64)], id: &str) -> f64 {
        rows.iter()
            .find(|(i, _)| i == id)
            .map(|(_, s)| *s)
            .expect("id present in the fused list")
    }

    /// The inertness claim, as a bit-identity claim rather than an approximate
    /// one: at the shipped `0.0` the magnitude term is not added at all, so the
    /// scores are the *same `f64` values* the two-stream kernel produces — not
    /// merely numbers that agree to some tolerance. An approximate comparison
    /// would pass even if a stray `1e-18` leaked in and a downstream tiebreak
    /// turned it into a reordered list.
    #[test]
    fn a_zero_bm25_magnitude_should_leave_the_fusion_bit_identical() {
        let streams = [split_stream()];
        let magnitudes = split_magnitudes();
        let plain = rrf_fuse(&streams, &FusionWeights::SHIPPED);
        let explicit_zero = rrf_fuse_with_magnitudes(
            &streams,
            &magnitudes,
            &FusionWeights { bm25_magnitude: 0.0, ..FusionWeights::SHIPPED },
        );
        assert_eq!(
            plain.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            explicit_zero.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>()
        );
        for ((id, a), (_, b)) in plain.iter().zip(&explicit_zero) {
            assert_eq!(a, b, "score drift at {id}: {a:?} vs {b:?}");
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "score at {id} is not the same f64, only close to it"
            );
        }
        // And the default is 0.0, so the shipped path is the plain kernel.
        assert_eq!(FusionWeights::SHIPPED.bm25_magnitude, 0.0);
    }

    /// The mechanism itself: a row at rank 7 with a far better BM25 score
    /// overtakes a marginal rank-1 row once the knob is turned up, and does not
    /// at the default. Both directions are asserted — a knob that did nothing
    /// would pass the first half of this test alone.
    #[test]
    fn the_bm25_magnitude_knob_should_let_a_better_score_at_a_worse_rank_win() {
        let streams = [split_stream()];
        let magnitudes = split_magnitudes();
        let at = |w: f64| {
            rrf_fuse_with_magnitudes(
                &streams,
                &magnitudes,
                &FusionWeights { bm25_magnitude: w, ..FusionWeights::SHIPPED },
            )
        };
        let off = at(0.0);
        let on = at(0.5);
        assert_eq!(off[0].0, "weak", "at 0.0 the plain rank-1 RRF order stands");
        assert_eq!(on[0].0, "strong", "a far better score at rank 7 must win");
        // The algebra, pinned: 1/(60+7) + 0.5·1/61 for the strong row against
        // 1/(60+1) + 0 for the weak one, because u is 1.0 and 0.0 respectively.
        let k = FusionWeights::SHIPPED.k;
        let expected_strong = 1.0 / (k + 7.0) + 0.5 / (k + 1.0);
        let expected_weak = 1.0 / (k + 1.0);
        assert!((score_of(&on, "strong") - expected_strong).abs() < 1e-12);
        assert!((score_of(&on, "weak") - expected_weak).abs() < 1e-12);
    }

    /// The term is bounded, and the bound is the property that makes the knob a
    /// tie-breaker rather than a replacement for the ranking: the best-magnitude
    /// row claims at most `w` of one rank-1 RRF unit, and the worst claims none.
    /// An unbounded term (`w · raw_bm25`) would let a single magnitude outvote
    /// the whole fusion at any `w` above about 0.1.
    #[test]
    fn the_magnitude_term_should_be_bounded_by_one_rank_one_unit() {
        let streams = [split_stream()];
        let magnitudes = split_magnitudes();
        let k = FusionWeights::SHIPPED.k;
        let w = 0.4;
        let fused = rrf_fuse_with_magnitudes(
            &streams,
            &magnitudes,
            &FusionWeights { bm25_magnitude: w, ..FusionWeights::SHIPPED },
        );
        let base = rrf_fuse(&streams, &FusionWeights::SHIPPED);
        for id in ["strong", "mid-c", "weak"] {
            let lift = score_of(&fused, id) - score_of(&base, id);
            assert!(lift >= -1e-15 && lift <= w / (k + 1.0) + 1e-15, "{id} lifted {lift}");
        }
        assert!((score_of(&fused, "strong") - score_of(&base, "strong") - w / (k + 1.0)).abs() < 1e-12);
        assert!((score_of(&fused, "weak") - score_of(&base, "weak")).abs() < 1e-12);
    }

    /// Degenerate magnitude inputs must be silence, never a NaN. A NaN in a
    /// score poisons `partial_cmp` for every id that shares it, and the sort
    /// falls back to the id tiebreak — a silent reordering rather than a
    /// failure. One row, all rows equal, and a non-finite value are the three
    /// ways the normalisation can divide by zero or propagate infinity.
    #[test]
    fn degenerate_magnitudes_should_add_no_term_at_all() {
        let one: HashMap<String, f64> = [("weak".to_string(), -1.0)].into();
        let flat: HashMap<String, f64> = ["a", "b"].iter().map(|i| ((*i).to_string(), -3.0)).collect();
        let poisoned: HashMap<String, f64> = [
            ("weak".to_string(), f64::NAN),
            ("mid-a".to_string(), -1.0),
            ("strong".to_string(), -9.0),
        ]
        .into();
        for magnitudes in [one, flat, poisoned] {
            let fused = rrf_fuse_with_magnitudes(
                &[split_stream()],
                &magnitudes,
                &FusionWeights { bm25_magnitude: 1.0, ..FusionWeights::SHIPPED },
            );
            let base = rrf_fuse(&[split_stream()], &FusionWeights::SHIPPED);
            assert_eq!(
                fused.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
                base.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
                "magnitudes {magnitudes:?} must not reorder anything"
            );
            for (_, s) in &fused {
                assert!(s.is_finite(), "a degenerate input produced {s}");
            }
        }
    }

    /// An id the magnitude map names but the fusion never ranked gets no term —
    /// it is not in `scores`, so there is nothing to add to. A magnitude must
    /// never conjure a row into the output.
    #[test]
    fn a_magnitude_for_an_unranked_id_should_not_add_a_row() {
        let mut magnitudes = split_magnitudes();
        magnitudes.insert("not-a-candidate".to_string(), -99.0);
        let fused = rrf_fuse_with_magnitudes(
            &[split_stream()],
            &magnitudes,
            &FusionWeights { bm25_magnitude: 1.0, ..FusionWeights::SHIPPED },
        );
        assert!(!fused.iter().any(|(id, _)| id == "not-a-candidate"));
        assert_eq!(fused.len(), 7);
    }

    // ---- (d) sentence-level overlap ----------------------------------------

    /// The property the mechanism rests on: a long document that names each
    /// query token once, in unrelated places, must stop outranking a short one
    /// that is densely about all of them.
    #[test]
    fn best_sentence_overlap_should_score_the_densest_segment_only() {
        let qt = tokenize("auth jose retry deploy token");
        let scattered = "auth is a thing. jose is a thing. retry is a thing. \
                         deploy is a thing. token is a thing.";
        let dense = "auth jose retry deploy token";
        assert_eq!(overlap_score(&qt, scattered), 5, "the whole document has all five");
        assert_eq!(
            best_sentence_overlap(&qt, scattered),
            1,
            "no single segment is about more than one of them"
        );
        assert_eq!(overlap_score(&qt, dense), 5);
        assert_eq!(best_sentence_overlap(&qt, dense), 5, "one segment holds all five");
    }

    /// The two scores are two granularities of one definition, so the finer can
    /// only ever be at or below the coarser — checked over the shared corpus
    /// rather than on a hand-picked case, and against every query shape the
    /// existing tests use.
    #[test]
    fn best_sentence_overlap_should_never_exceed_the_whole_document_score() {
        for query in [
            "auth",
            "auth auth",
            "jose uses",
            "über",
            "ΑΘΗΝΑ αθηνα",
            "2024-01-01",
            "nomatch",
            "",
        ] {
            let qt = tokenize(query);
            for doc in CORPUS {
                assert!(
                    best_sentence_overlap(&qt, doc) <= overlap_score(&qt, doc),
                    "sentence score exceeded document score for {query:?} / {doc:?}"
                );
            }
        }
    }

    /// A single-sentence memory is unaffected by the choice of scope, and a
    /// newline is a boundary — which matters because that is the unit this
    /// project's own documents arrive in.
    #[test]
    fn best_sentence_overlap_should_split_on_newlines_and_sentence_ends() {
        let qt = tokenize("auth jose");
        assert_eq!(best_sentence_overlap(&qt, "user: hi\nuser: auth and jose"), 2);
        assert_eq!(best_sentence_overlap(&qt, "user: auth and jose."), 2);
        assert_eq!(best_sentence_overlap(&qt, "auth! jose?"), 1);
        assert_eq!(best_sentence_overlap(&qt, "..."), 0);
        assert_eq!(best_sentence_overlap(&qt, ""), 0);
    }

    /// The scope knob selects the granularity and changes nothing else: the
    /// default scope is literally [`rank_candidates`], and under the other scope
    /// each returned score is the sentence score, in the same order the
    /// whole-document ranker would have produced for equal scores.
    #[test]
    fn a_whole_document_recall_should_use_the_whole_document_scorer() {
        let docs = ["auth jose retry deploy", "auth. jose. retry. deploy.", "unrelated filler"];
        let default = rank_candidates_scoped("auth jose", docs, OverlapScope::default());
        assert_eq!(default, rank_candidates("auth jose", docs));
        assert_eq!(OverlapScope::default(), OverlapScope::Document);

        let scoped = rank_candidates_scoped("auth jose", docs, OverlapScope::BestSentence);
        let qt = tokenize("auth jose");
        for (idx, score) in &scoped {
            assert_eq!(*score, best_sentence_overlap(&qt, docs[*idx]) as f64);
        }
        // Same retain rule under both scopes: a document matching no query token
        // is dropped rather than ranked at zero, and the finer scope is the one
        // that drops it here.
        assert_eq!(
            scoped.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            vec![0, 1],
            "row 0 is one unbroken segment; row 1 splits into four; row 2 is gone"
        );
    }

    // ---- (b) recency stream ------------------------------------------------

    fn aged(id: &str, created: &str) -> Memory {
        Memory {
            id: id.to_string(),
            bank_id: "b".to_string(),
            content: "content".to_string(),
            context: None,
            created_at: Some(created.to_string()),
        }
    }

    /// The decay is the ranking, and the ranking is the only thing the half-life
    /// touches: one half-life old loses to brand new, two half-lives lose to one,
    /// and rows written in the same instant order by id rather than by whatever
    /// the iterator yielded.
    #[test]
    fn recency_rank_should_order_by_half_life_decay() {
        let now = DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z").expect("stamp").with_timezone(&Utc);
        let day = |n: i64| (now - chrono::Duration::days(n)).to_rfc3339();
        let mems = vec![
            aged("two-halves", &day(2)),
            aged("fresh", &day(0)),
            aged("one-half", &day(1)),
        ];
        let ranked = recency_rank(&mems, now, 1.0);
        let ids: Vec<&str> = ranked.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["fresh", "one-half", "two-halves"]);
        assert_eq!(
            ranked.iter().map(|h| h.rank).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "the stream is a plain 1..N ranking, which is what RRF consumes"
        );
        // Same instant, two rows: id ascending, so the order does not depend on
        // the input order or on the hash seed.
        let same = vec![aged("zulu", &day(3)), aged("alpha", &day(3))];
        let tied = recency_rank(&same, now, 1.0);
        assert_eq!(
            tied.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
            ["alpha", "zulu"]
        );
    }

    /// A half-life of zero, a negative one, a `NaN`, an absent stamp and an
    /// unparseable one all rank at the bottom instead of producing `NaN` or an
    /// infinite decay. The store writes the epoch sentinel for rows that predate
    /// timestamping, and "unknown age" reads as least recent — the same reading
    /// `Store::expire_before` gives it.
    ///
    /// A degenerate half-life is a caller error, and the answer it gets is total
    /// silence rather than a plausible-looking ranking: every row decays to
    /// `0.0`, so the stream is the id order. That is deterministic and it cannot
    /// be mistaken for a recency signal.
    #[test]
    fn recency_rank_should_never_produce_a_nan_or_a_leading_nonsense_row() {
        let now = DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z").expect("stamp").with_timezone(&Utc);
        let mems = vec![
            aged("known", "2026-05-31T00:00:00Z"),
            Memory { created_at: None, ..aged("absent", "2026-05-31T00:00:00Z") },
            aged("junk", "not a timestamp at all"),
            aged("epoch", "1970-01-01T00:00:00Z"),
        ];
        let ids = |rows: Vec<RankedHit>| -> Vec<String> { rows.into_iter().map(|h| h.id).collect() };

        // A usable half-life: the one row with a parseable stamp leads, and the
        // three ways of saying "age unknown" tie below it in id order.
        let good = ids(recency_rank(&mems, now, 1.0));
        assert_eq!(good, ["known", "absent", "epoch", "junk"]);

        for half_life in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let ranked = recency_rank(&mems, now, half_life);
            assert_eq!(ids(ranked), ["absent", "epoch", "junk", "known"], "half-life {half_life}");
        }
        // A stamp in the future is clock skew, not a negative age: it ranks as
        // brand new rather than above brand new.
        let skewed = recency_rank(&[aged("future", "2030-01-01T00:00:00Z")], now, 1.0);
        assert_eq!(skewed[0].id, "future");
    }

    // ---- (e) temporal query classifier -------------------------------------

    /// Positives: every shape the term list claims, one per justification group.
    #[test]
    fn is_temporal_query_should_accept_questions_that_ask_when() {
        for q in [
            // group 1 — days named outright
            "what did I do today",
            "what happened yesterday",
            "which deploy is going out tomorrow",
            // groups 2-4 — calendar units, times of day, weekdays
            "what did we decide last week",
            "how many days did the migration take",
            "what did I set up this morning",
            "what happened on tuesday",
            // group 5 — conversational units
            "what was the last session about",
            "summarise our previous conversation",
            // group 6 — recorded points
            "what is the date of the release",
            "which row has the newest timestamp",
            // group 7 — adverbials
            "what did I configure before the refactor",
            "what changed since I switched branches",
            "how recently did I touch the rate limiter",
            // group 8 — recency superlatives
            "what is the latest approach",
            "show me the newest observation",
            // group 9 — sequence position
            "what did we originally pick",
            "what eventually broke",
            // group 10 — state change across a boundary
            "what did we use to run",
            "what config is no longer valid",
            "since I moved to postgres what changed",
            // group 11 — duration and periodicity
            "how long did the soak take",
            "how often does the sweeper run",
            "what time does the cron fire",
            "what is the ttl on this bank",
            "is the report generated daily",
            // group 12 — the homograph class in its two temporal shapes
            "what did I do last time",
            "what did i do last",
            "what did we pick first",
            "the previous session's decision",
            "the last message i sent",
        ] {
            assert!(is_temporal_query(q), "expected temporal: {q:?}");
        }
    }

    /// Negatives, led by the two traps the mechanism is specified against: a
    /// trailing "last" is temporal, and "the last value of x" is not — the same
    /// word, and the difference is grammatical, not lexical. The rest are the
    /// content nouns this project's own bank is full of, which is why
    /// `recall.rs` could not simply list every time-ish word: a coding-memory
    /// bank makes "time complexity", "git history" and "current working
    /// directory" ordinary content.
    #[test]
    fn is_temporal_query_should_reject_questions_that_ask_what_or_how() {
        for q in [
            // The traps.
            "what is the last value of x in the config file",
            "what is the time complexity of quicksort",
            "what does the git history show",
            "what is the current working directory",
            // Plain content questions.
            "how does the auth middleware work",
            "what is the retry budget",
            "which crate handles pii redaction",
            "list all memories tagged ops",
            "who is on call",
            "what did the error message say",
            "show me the last error line",
            "what is the first step of the release runbook",
            "why did the timeout fire",
            "what port does the server bind",
            "",
            "!!!???",
        ] {
            assert!(!is_temporal_query(q), "expected NOT temporal: {q:?}");
        }
    }

    /// The classifier decides the recency *weight*, so the two must be wired to
    /// each other and the wiring must be a single value: the same call the
    /// stream builder gates on.
    #[test]
    fn the_recency_policy_should_gate_the_weight_on_the_classifier() {
        let always = FusionWeights { recency: 1.0, ..FusionWeights::SHIPPED };
        let gated = FusionWeights {
            recency: 1.0,
            recency_policy: RecencyPolicy::TemporalQueriesOnly,
            ..FusionWeights::SHIPPED
        };
        assert_eq!(always.recency_weight_for("how does auth work"), 1.0);
        assert_eq!(gated.recency_weight_for("how does auth work"), 0.0);
        assert_eq!(gated.recency_weight_for("what did I do last time"), 1.0);
        // And the shipped default is off regardless of the query.
        assert_eq!(FusionWeights::SHIPPED.recency_weight_for("what did I do last time"), 0.0);
        assert_eq!(FusionWeights::default().recency_weight_for("today"), 0.0);
    }

    // ---- the shipped configuration, all together ---------------------------

    /// Every mechanism this file added, in one assertion: the shipped default is
    /// off, off *and* inert, and turning any one of them on is a value the
    /// caller has to write down.
    #[test]
    fn the_shipped_weights_should_leave_every_new_mechanism_inert() {
        let s = FusionWeights::SHIPPED;
        assert_eq!(s.bm25_magnitude, 0.0, "the BM25 magnitude term is skipped");
        assert_eq!(s.recency, 0.0, "the third stream is dropped");
        assert_eq!(s.recency_weight_for("what did I do today"), 0.0);
        assert_eq!(s.overlap_scope, OverlapScope::Document);
        assert_eq!(s.recency_policy, RecencyPolicy::Always);
        assert_eq!(s, FusionWeights::default());
    }

    /// A zero weight on stream 2 drops it exactly the way it drops streams 0 and
    /// 1 — the same `continue`, the same reason, one more index in `of()`.
    #[test]
    fn a_zero_recency_weight_should_remove_its_stream() {
        let streams = || {
            vec![
                vec![
                    RankedHit { id: "m1".to_string(), rank: 1 },
                    RankedHit { id: "m2".to_string(), rank: 2 },
                ],
                vec![RankedHit { id: "m2".to_string(), rank: 1 }],
                vec![RankedHit { id: "m3".to_string(), rank: 1 }],
            ]
        };
        let dropped = rrf_fuse(&streams(), &FusionWeights { recency: 0.0, ..FusionWeights::SHIPPED });
        let ids: Vec<&str> = dropped.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["m2", "m1"], "the dropped stream must contribute nothing");
        let live = rrf_fuse(&streams(), &FusionWeights { recency: 1.0, ..FusionWeights::SHIPPED });
        assert!(live.iter().any(|(id, _)| id == "m3"), "a live stream must vote");
    }
}
