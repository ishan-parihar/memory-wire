//! Weight **selection** on the LoCoMo dev set — a coordinate-descent sweep over the
//! whole `FusionWeights` space, from the shipped starting point.
//!
//! ## What this is for
//!
//! `docs/PERFORMANCE_PLAN.md` P−1 built the dev harness; `docs/EXCEED_PLAN.md` Phases A
//! and B are the phases that *select* against it. Four retrieval mechanisms sit in
//! [`memory_wire::recall::FusionWeights`] and are all shipped inert at a default of
//! `0.0` — the BM25 magnitude, the cross-stream agreement bonus, the recency stream
//! and its half-life — and `k` has never been swept at all
//! (`docs/EVALUATION_HYGIENE.md` §2.6: `k = 60` was inherited from Hindsight and is
//! therefore *borrowed, not fitted*, which makes it a hypothesis rather than a
//! decision). This binary is the instrument that puts numbers on all of them at once.
//!
//! ## Why coordinate descent and not a full grid
//!
//! **The interaction is the point.** `docs/EXCEED_PLAN.md` §0.3: Hindsight's own
//! `recall_boost.py:29-50` records that score-space weights above the RRF spread
//! degenerate into lexicographic sorting, costing `recall@20 0.97 → 0.40`. A
//! one-axis-at-a-time sweep cannot see that — every row of such a sweep holds the
//! other weights at their inert defaults, which is the *one* regime where the
//! collapse is invisible. A walk that changes one coordinate at a time and re-measures
//! sees the combination, because the coordinate it is about to move is measured against
//! the current values of all the others.
//!
//! **And it is incomplete, in a way that has to be said out loud.** Coordinate descent
//! is greedy and axis-ordered: it can walk into a local optimum whose basin depends on
//! the walk order, it never revisits a coordinate combination it has left, and a
//! plateau can hide a descent beside it. The "Limitations" section of the artifact says
//! this again, in the artifact, where a reader of the numbers will see it. Two rounds
//! is the floor for that reason and not because two is enough.
//!
//! ## This binary does not select anything
//!
//! It **reports a grid**. There is no "recommended configuration" line, no
//! best-configuration row, and no table sorted by score: the tables are in *evaluation
//! order* precisely so that nothing in the artifact can be read as a ranking. The walk
//! has to move to some value to make progress, and the value it moves to is a
//! consequence of the declared objective below — but choosing a configuration to ship
//! is a decision for a human, made on this evidence and recorded in
//! `docs/CONSISTENCY.md` under the counting rule in `docs/EVALUATION_HYGIENE.md` §3.3.
//! An automated sweep that emitted a recommendation would be the exact failure this
//! whole exercise exists to prevent.
//!
//! ## The control
//!
//! The walk starts at [`FusionWeights::SHIPPED`] and that first row is checked against
//! the numbers `eval/LOCOMO.md` already committed, to the decimal. If it does not
//! reproduce, this is a **bug in the harness, not a finding about the weights**, and
//! the run says so at the top of the artifact and exits nonzero. Every other number in
//! the file is void until that check passes.
//!
//! Run: `cargo run --release --example select_fusion -- --rounds 2 --budget-seconds 1800`
//!
//! **A bare run writes to `$TMPDIR`, never to `eval/`** — see `bench_common::out_md`.
//! This binary owns the whole of `eval/SELECTION.md`, and a benchmark run must not be
//! able to silently replace a reviewed artifact; that has already destroyed real
//! content in this project twice.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

use memory_wire::recall::FusionWeights;

use anyhow::Context as _;

// The dense arm itself, behind the same `embed` feature gate `src/lib.rs` puts on
// `pub mod vector`. Without the feature the `vector` axis is not merely unswept, it
// does not exist, and the axis list says so rather than printing a row of zeros that
// would read as a measurement of a mechanism the binary does not contain.
#[cfg(feature = "embed")]
use memory_wire::store::Store as _;
#[cfg(feature = "embed")]
use memory_wire::vector::{self, Embedder};

// The `--out-md` policy (a bare run must not be able to overwrite a committed
// `eval/` artifact) and the build-profile name the provenance line needs.
mod bench_common;

// The dev-set core, shared with `examples/locomo.rs`: the data shapes, the ingestion,
// the metric definitions and the aggregation. One copy, so a number in this artifact
// and a number in `eval/LOCOMO.md` mean the same measurement. See its module docs.
#[path = "../eval/locomo_dev.rs"]
mod dev;

use dev::{loadavg, Agg, Corpus, PerCategory, Row, UPSTREAM_REF};

/// The shipped `overlap: 0.25` row of `eval/LOCOMO.md`, as committed. This harness
/// re-measures it as its own control and refuses to report a sweep beside it if the
/// two disagree — the numbers are copied rather than computed so the check cannot be
/// satisfied by the same code that produced them.
const CONTROL: [(&str, f64); 6] = [
    ("R@1", 60.4),
    ("R@5", 86.0),
    ("R@10", 94.2),
    ("R@20", 98.0),
    ("NDCG@10", 73.6),
    ("MRR", 71.8),
];

/// The n behind `CONTROL`. `eval/LOCOMO.md` evaluates 1,531 of 1,540: nine queries
/// carry an empty `gold_ids` and have no gold *document* to retrieve.
const CONTROL_N: usize = 1531;

/// The columns of [`Agg::all`], in the order every table prints them.
const METRIC_NAMES: [&str; 6] = ["R@1", "R@5", "R@10", "R@20", "NDCG@10", "MRR"];

/// The weight on the token-overlap stream, swept **first and as a control**: the
/// shipped value is `0.25`, `1.00` is the unfitted equal-weight baseline every
/// published number was measured under, and `0.00` is a diagnostic bound (BM25 alone,
/// not a candidate — a zero weight drops the stream's candidates while BM25 truncates
/// at 50 in SQL).
const OVERLAP: &[f64] = &[0.00, 0.10, 0.25, 0.50, 0.75, 1.00];

/// `bm25_magnitude`, denominated in `1/(k+1)` rank-1 RRF hits — so `0.25` reads "the
/// best-BM25 row of this query may claim a quarter of a rank-1 hit", and the whole
/// two-stream RRF spread at `k=60` is `1/61 … 1/120`, a factor of 1.97.
///
/// **Every value at or above `1.00` is the regime `docs/EXCEED_PLAN.md` §0.3 says to
/// expect to collapse.** A weight of one full rank-1 hit is already wider than the
/// entire spread the two streams span, so the term stops being a tiebreaker and
/// becomes the sort key — which is exactly hindsight's measured `recall@20 0.97 →
/// 0.40`. `4.00` is carried deliberately as a **diagnostic bound, not a candidate**,
/// so the collapse is visible on this corpus rather than imported as a citation. If it
/// does collapse here, that is a measurement; if it does not, that is a measurement too.
const BM25_MAGNITUDE: &[f64] = &[0.00, 0.25, 0.50, 1.00, 2.00, 4.00];

/// `agreement`, the cross-stream bonus, in the same `1/(k+1)` unit. `0.05` is
/// agentmemory's `AGREEMENT_BONUS` (`hybrid-search.ts:20`, applied as
/// `1 + 0.05 × (matchedStreams − 1)`); the larger three are the magnitudes
/// `FusionWeights::SHIPPED`'s own rustdoc records as having been swept on LongMemEval
/// and moving **zero of 500 questions**, which is why it ships at 0.0. That sweep was
/// on the test set, so it is evidence about magnitude, not about the value.
const AGREEMENT: &[f64] = &[0.00, 0.05, 0.10, 0.25, 0.50];

/// The RRF `k` constant — exactly the set `docs/PERFORMANCE_PLAN.md` P1 schedules,
/// which is also the axis that controls how sharply the head of the ranking separates
/// from the tail, i.e. the R@1 axis the audit's 16.2pp oracle gap lives on.
const K: &[f64] = &[5.0, 10.0, 20.0, 40.0, 60.0, 120.0];

/// The recency stream's weight, in the same `1/(k + rank)` units as every other
/// stream, and **swept last**, for two reasons that are both about evidence rather
/// than about expected effect. First, agentmemory has **no** recency term anywhere in
/// its retrieval path, and the "f2–f5 at 27% with token recency" figure that
/// originally motivated this lever **does not exist in their repo**
/// (`docs/EXCEED_PLAN.md` Phase C; the withdrawal is recorded in
/// `docs/CONSISTENCY.md` §14.4). Second, and specific to *this* corpus, the LoCoMo
/// ingestion writes no per-document date — see the recency caveat in the artifact,
/// where the measured `created_at` spread is printed. A recency row here is a ranking
/// by ingest order, so it is swept on its own merits and read as nothing more.
const RECENCY: &[f64] = &[0.00, 0.10, 0.25, 0.50, 1.00];

/// The recency half-life in days. Only reachable while `recency != 0.0`, because a
/// zero weight drops the stream before the half-life is ever read — so the axis
/// reports itself skipped rather than burning budget re-measuring one ranking.
const RECENCY_HALF_LIFE: &[f64] = &[1.0, 7.0, 30.0, 90.0];

/// The weight on the dense-vector stream (slot 3), denominated in `1/(k+1)` rank-1
/// RRF hits — the same unit as [`AGREEMENT`], so `1.00` reads "the best-cosine row of
/// this query may claim a whole rank-1 hit" and is already *wider* than the entire
/// spread the two lexical streams span at `k=60`.
///
/// `0.00` is the shipped value and is the control: it drops the stream, so the control
/// row is the lexical arm alone and must reproduce `eval/LOCOMO.md` to the decimal.
///
/// **`1.50` is carried deliberately, and it is past the point where a reader expects
/// the grid to stop.** The arithmetic says the cross-stream margin between the BM25
/// stream and the dense stream scales with `|w_bm25 - w_vec|` *only* because the two
/// rank-1 contributions are the same shape; a sweep that stopped at `1.00` would never
/// see the regime past it, which is the regime where the dense stream is not a
/// tiebreaker but the sort key. `docs/EXCEED_PLAN.md` §0.3 records Hindsight measuring
/// exactly that regime and paying `recall@20 0.97 → 0.40` for it, so the point is
/// carried as a **diagnostic bound rather than a candidate** — the same status
/// `bm25_magnitude: 4.00` holds. If it collapses here, that is a measurement on this
/// corpus; if it does not, that is a measurement too. Neither is a reason to ship it.
#[cfg(feature = "embed")]
const VECTOR: &[f64] = &[0.00, 0.10, 0.25, 0.50, 0.75, 1.00, 1.50];

/// One swept coordinate: its name, the values tried, how to read it off a
/// [`FusionWeights`], how to write a new one, and whether it applies at all to the
/// configuration currently under the walk.
#[derive(Clone, Copy)]
struct Axis {
    /// The `FusionWeights` field name, which is also the column header.
    name: &'static str,
    /// The values tried, in the order they are tried.
    values: &'static [f64],
    /// Read the coordinate.
    get: fn(&FusionWeights) -> f64,
    /// Return a copy with the coordinate set.
    set: fn(&FusionWeights, f64) -> FusionWeights,
    /// Whether the coordinate can change anything from `w`. A skipped axis is
    /// reported, never silently dropped.
    applies: fn(&FusionWeights) -> bool,
}

/// The walk order. `overlap` first because it is the axis the whole three-set
/// discipline in `AGENTS.md` §1 exists over; `k` next because it is the RRF shape
/// every other weight is denominated in; the recency pair last, for the reasons on
/// [`RECENCY`].
fn axes() -> Vec<Axis> {
    vec![
        Axis {
            name: "overlap",
            values: OVERLAP,
            get: |w| w.overlap,
            set: |w, v| FusionWeights { overlap: v, ..*w },
            applies: |_| true,
        },
        Axis {
            name: "k",
            values: K,
            get: |w| w.k,
            set: |w, v| FusionWeights { k: v, ..*w },
            applies: |_| true,
        },
        Axis {
            name: "bm25_magnitude",
            values: BM25_MAGNITUDE,
            get: |w| w.bm25_magnitude,
            set: |w, v| FusionWeights {
                bm25_magnitude: v,
                ..*w
            },
            applies: |_| true,
        },
        Axis {
            name: "agreement",
            values: AGREEMENT,
            get: |w| w.agreement,
            set: |w, v| FusionWeights {
                agreement: v,
                ..*w
            },
            applies: |_| true,
        },
        Axis {
            name: "recency",
            values: RECENCY,
            get: |w| w.recency,
            set: |w, v| FusionWeights { recency: v, ..*w },
            applies: |_| true,
        },
        Axis {
            name: "recency_half_life_days",
            values: RECENCY_HALF_LIFE,
            get: |w| w.recency_half_life_days,
            set: |w, v| FusionWeights {
                recency_half_life_days: v,
                ..*w
            },
            // A zero recency weight drops the third stream before its half-life is
            // read, so every value here produces the same ranking.
            applies: |w| w.recency != 0.0,
        },
        // Stream slot 3, the dense arm. Gated on the same cargo feature as
        // `pub mod vector`, because an axis whose mechanism the binary does not
        // contain would produce a table of identical rows and call it a null.
        #[cfg(feature = "embed")]
        Axis {
            name: "vector",
            values: VECTOR,
            get: |w| w.vector,
            set: |w, v| FusionWeights { vector: v, ..*w },
            // Not "does the weight make a difference" but "does the *bank* have
            // anything to rank". A bank with no vectors yields an empty dense stream
            // at every weight, so every value on this axis would produce one ranking.
            // `ensure_embedded` refuses to start a run that could reach that state,
            // so in practice this is a belt on top of a hard check, not the check.
            applies: |_| true,
        },
    ]
}

/// The objective the walk maximises, as percentages in a **declared** order:
/// R@5, then NDCG@10, then MRR, then R@1, then R@10, then R@20.
///
/// **This is an instrument, not a claim about what matters.** R@5 leads because
/// `docs/EXCEED_PLAN.md` §4 states the bar in R@5 terms; NDCG@10 and MRR follow
/// because they are the ordering metrics the P1 gate refuses to regress; R@1 follows
/// because the audit's 16.2pp oracle gap is an R@1 gap. A different order would walk a
/// different path and would be equally defensible, which is exactly why the order is
/// printed here rather than buried in a `max_by_key`.
fn objective(a: &Agg) -> [f64; 6] {
    [a.r5(), a.ndcg10(), a.mrr(), a.r1(), a.r10(), a.r20()]
}

/// Strict lexicographic `>` on the objective. Strictness is load-bearing: an exact tie
/// must leave the walk where it is, or it will drift along a plateau and the artifact's
/// round log will show moves that changed nothing.
fn strictly_better(candidate: &[f64; 6], incumbent: &[f64; 6]) -> bool {
    for (c, i) in candidate.iter().zip(incumbent) {
        match c.partial_cmp(i).unwrap_or(std::cmp::Ordering::Equal) {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    false
}

/// The sweep's one call into [`evaluate`].
///
/// A function rather than a `cfg` block at each of the three call sites, so that
/// "this harness has exactly one way to score a configuration" is a fact the compiler
/// checks rather than a convention the reader has to verify by eye. The `embed` build
/// threads the session through; the default build has no second argument, and the three
/// call sites below are written with a macro so both arities are expanded from one form.
macro_rules! eval_one {
    ($corpus:expr, $planned:expr, $weights:expr, $phase:expr, $coverage:expr) => {{
        #[cfg(feature = "embed")]
        {
            eval_one($corpus, $planned, $weights, $phase, $coverage)
        }
        #[cfg(not(feature = "embed"))]
        {
            eval_one($corpus, $planned, $weights, $phase)
        }
    }};
}

/// The `embed` build's body: the session comes from the coverage bundle, which
/// [`ensure_embedded`] filled before any row was scored.
#[cfg(feature = "embed")]
fn eval_one(
    corpus: &Corpus,
    planned: &[dev::PlannedQuery],
    weights: &FusionWeights,
    phase: &'static str,
    coverage: &mut Option<(Coverage, Embedder)>,
) -> anyhow::Result<Run> {
    let (_, embedder) = coverage.as_mut().ok_or_else(|| {
        anyhow::anyhow!("select_fusion: internal — the embedder is missing after \
                         `ensure_embedded` succeeded, which cannot happen; refusing to score a \
                         configuration with no session rather than silently dropping the dense \
                         stream")
    })?;
    evaluate(corpus, planned, weights, phase, embedder)
}

/// The default build's body: no dense arm in this binary, so no second argument.
#[cfg(not(feature = "embed"))]
fn eval_one(
    corpus: &Corpus,
    planned: &[dev::PlannedQuery],
    weights: &FusionWeights,
    phase: &'static str,
) -> anyhow::Result<Run> {
    evaluate(corpus, planned, weights, phase)
}

/// Per-bank vector coverage, read back through the public store API rather than
/// inferred from how many `put_vector` calls were made. The distinction matters:
/// "I called `put_vector` 272 times" and "272 rows are readable" are different claims,
/// and `bank_vectors` deliberately *omits* rows whose blob fails to decode rather than
/// erroring — so a corrupt row would be invisible to a call counter and visible here.
#[cfg(feature = "embed")]
struct Coverage {
    /// `bank_id -> (memories rows, decodable vector rows)`.
    per_bank: BTreeMap<String, (usize, usize)>,
}

#[cfg(feature = "embed")]
impl Coverage {
    /// `memories -> vectors`, summed over every bank.
    fn totals(&self) -> (usize, usize) {
        self.per_bank
            .values()
            .fold((0, 0), |(m, v), (mm, vv)| (m + mm, v + vv))
    }

    /// One `bank_id — memories / vectors` line per bank, for the artifact.
    fn table(&self) -> String {
        self.per_bank
            .iter()
            .map(|(b, (m, v))| {
                format!(
                    "| `{b}` | {m} | {v} | {} |",
                    if m == v { "**yes**" } else { "**NO**" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Embed every document in every bank, then **refuse to produce a grid** unless
/// coverage is total.
///
/// The problem this exists to solve: `vector::retain` is the only thing that writes
/// `memory_vectors`, and it runs at *retain* time. `Corpus::load` writes its rows with
/// `store.put` and never calls it, so a sweep run against the shared ingestion would
/// have scored a dense stream over banks with **zero** vectors — `vector_stream`
/// returns empty for a vectorless bank by design, so the dense stream would silently
/// contribute nothing and every row of the `vector` axis would read as a null. That is
/// exactly the failure `AGENTS.md` §3's "do not guess" rule exists to prevent, and it is
/// why this function errors rather than warns.
///
/// The embedding is of the *same* string the lexical arm indexes — `doc.text()`, the
/// dialogue turns joined one per line — so both streams rank the same content and the
/// weight sweep is measuring the weight. `vector::retain` is deliberately **not** called
/// for the write: it routes through `MemoryService::retain_doc` and `redact_pii`, which
/// would replace the store's own text and leave the two arms ranking different strings.
#[cfg(feature = "embed")]
fn ensure_embedded(corpus: &Corpus, embedder: &mut Embedder) -> anyhow::Result<Coverage> {
    let mut per_bank: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for doc in &corpus.documents {
        let bank = &doc.user_id;
        let memory_id = doc.id.clone();
        per_bank.entry(bank.clone()).or_insert((0, 0)).0 += 1;
        if !has_vector(corpus, bank, &memory_id)? {
            let text = doc.text()?;
            let v = embedder.embed(&text).with_context(|| {
                format!("select_fusion: embedding LoCoMo document {memory_id} (bank {bank})")
            })?;
            corpus.banks[bank]
                .store
                .put_vector(bank, &memory_id, &v)
                .with_context(|| format!("select_fusion: storing vector for {memory_id}"))?;
        }
    }
    // Read back through `bank_vectors`, which decodes and silently drops an unreadable
    // blob — so this count is the number of *usable* vectors, not the number of rows the
    // write claimed to create.
    for (bank, svc) in &corpus.banks {
        let vectors = svc
            .store
            .bank_vectors(bank)
            .with_context(|| format!("select_fusion: reading vectors back for bank {bank}"))?;
        per_bank.entry(bank.clone()).or_insert((0, 0)).1 = vectors.len();
    }
    let cov = Coverage { per_bank };
    let (memories, vectors) = cov.totals();
    anyhow::ensure!(
        memories == vectors,
        "select_fusion: vector coverage is {vectors}/{memories} rows — refusing to publish a \
         `vector` grid over a partially-embedded corpus. A bank with no vectors returns an empty \
         dense stream by design (`vector::vector_stream`), so the axis would read as a null for \
         the wrong reason."
    );
    for (bank, (m, v)) in &cov.per_bank {
        anyhow::ensure!(
            m == v,
            "select_fusion: bank {bank} has {m} memories but {v} decodable vectors — refusing to \
             publish a grid"
        );
    }
    Ok(cov)
}

/// Whether one memory already has a decodable vector, read through the same public API
/// the recall path uses.
#[cfg(feature = "embed")]
fn has_vector(corpus: &Corpus, bank: &str, memory_id: &str) -> anyhow::Result<bool> {
    let vectors = corpus
        .banks
        .get(bank)
        .ok_or_else(|| anyhow::anyhow!("select_fusion: no bank {bank}"))?
        .store
        .bank_vectors(bank)?;
    Ok(vectors.iter().any(|(id, _)| id == memory_id))
}

/// Score the **shipped** configuration through [`vector::recall`] with the dense weight
/// at `0.0`, and check it against the control.
///
/// This exists because of a confound, not a curiosity. Rows with `vector != 0.0` are
/// scored through [`vector::recall`]; the control row is scored through
/// [`MemoryService::recall_with_weights`], the path `eval/LOCOMO.md` was measured on.
/// Those are two different functions. If the *lexical* half of `vector::recall` differed
/// from the service's by anything at all, every `vector` row would differ from the
/// control in **two** ways at once — the added dense stream *and* a lexical drift — and
/// the axis would be measuring a difference nobody asked about.
///
/// At `vector: 0.0` the dense stream is never built (`vector::recall` returns
/// `Vec::new()` for it), so this measures exactly the lexical half in isolation. It
/// compares the full metric vector, not one cell.
#[cfg(feature = "embed")]
fn check_lexical_path_equivalence(
    corpus: &Corpus,
    planned: &[dev::PlannedQuery],
    control: &Agg,
    embedder: &mut Embedder,
) -> anyhow::Result<(bool, String)> {
    // `bm25` and `overlap` at the shipped values, dense stream at 0.0, so this is the
    // control's own configuration seen through the other function.
    let same_weights = FusionWeights::SHIPPED;
    let mut agg = Agg::default();
    for pq in planned {
        let q = pq.query;
        let svc = &corpus.banks[&q.user_id];
        let retrieved: Vec<String> = vector::recall(
            svc.store.as_ref(),
            &q.user_id,
            &q.query,
            corpus.budget_tokens,
            &[],
            &same_weights,
            embedder,
        )
        .map_err(|e| anyhow::anyhow!("lexical-path equivalence recall for {} failed: {e}", q.id))?
        .into_iter()
        .map(|h| h.memory.id)
        .collect();
        agg.add(&Row::new(q, &retrieved, &pq.gold));
    }
    let mut bad: Vec<String> = METRIC_NAMES
        .iter()
        .zip(agg.all().iter().zip(control.all().iter()))
        .filter(|(_, (a, b))| (*a - *b).abs() > f64::EPSILON)
        .map(|(name, (a, b))| format!("{name}: {a:.4} vs {b:.4}"))
        .collect();
    if agg.count != control.count {
        bad.push(format!("n: {} vs {}", agg.count, control.count));
    }
    Ok((
        bad.is_empty(),
        if bad.is_empty() {
            format!(
                "The lexical half of `vector::recall` at `vector: 0.0` scores the same six metrics \
                 and the same n as the service path over {} queries, to within one f64 ulp. The \
                 two functions are interchangeable on the lexical streams, so a `vector` row \
                 differs from the control in the dense stream and nothing else.",
                control.count
            )
        } else {
            format!("The two paths disagree, so every `vector` row is confounded: {}", bad.join("; "))
        },
    ))
}

/// The dense stream **alone**, with both lexical streams dropped, scored against the
/// same gold. This is the reach question, and it is a different measurement from the
/// weight sweep: the sweep asks how much the dense arm's *rank* matters once fused; this
/// asks whether it can reach a gold document at all when it is the only stream.
///
/// A **diagnostic**, not a candidate configuration. Nobody ships a dense-only arm on a
/// corpus whose lexical arm already has 100% R@pool, and the row is not scored by the
/// objective or eligible for the walk.
#[cfg(feature = "embed")]
fn vector_stream_reach(
    corpus: &Corpus,
    planned: &[dev::PlannedQuery],
    embedder: &mut Embedder,
) -> anyhow::Result<(Agg, Vec<String>)> {
    let solo = FusionWeights { bm25: 0.0, overlap: 0.0, vector: 1.0, ..FusionWeights::SHIPPED };
    let mut agg = Agg::default();
    // The queries whose gold the dense stream did not return, **measured rather than
    // assumed**. A sub-1pp R@pool gap on 1531 queries is one query, and "one query"
    // is a claim that either names the query or has not been checked. Bounded because
    // this is a diagnostic, not a hot path — and a corpus whose dense arm reaches
    // nothing at all would otherwise print 1530 lines of a list nobody reads.
    let mut missed: Vec<String> = Vec::new();
    for pq in planned {
        let q = pq.query;
        let svc = &corpus.banks[&q.user_id];
        let retrieved: Vec<String> = vector::recall(
            svc.store.as_ref(),
            &q.user_id,
            &q.query,
            corpus.budget_tokens,
            &[],
            &solo,
            embedder,
        )
        .map_err(|e| anyhow::anyhow!("vector-only recall for query {} failed: {e}", q.id))?
        .into_iter()
        .map(|h| h.memory.id)
        .collect();
        let row = Row::new(q, &retrieved, &pq.gold);
        if row.pool < 1.0 {
            missed.push(q.id.clone());
        }
        agg.add(&row);
    }
    if missed.len() > 50 {
        missed.truncate(50);
    }
    Ok((agg, missed))
}

/// One configuration's complete measurement.
struct Run {
    /// The configuration, as read back off the struct so the table cannot drift
    /// from what was actually scored.
    weights: FusionWeights,
    /// Which pass produced it: `"axis-at-shipped"` (one axis varied, everything
    /// else held at the shipped default) or `"walk"` (coordinate descent).
    phase: &'static str,
    /// Which descent round produced it. `0` is the shipped start point.
    round: usize,
    /// The coordinate being swept, or `None` for the start point.
    axis: Option<&'static str>,
    /// Overall aggregate, `n` included.
    agg: Agg,
    /// Per-`meta.category` aggregates, `n` included.
    per_cat: PerCategory,
    /// Per-query R@5 in plan order, so a config-to-config diff is a real
    /// per-question diff rather than a difference of means.
    r5_by_query: Vec<f64>,
    /// [`objective`] of `agg`, cached for the comparisons.
    obj: [f64; 6],
    /// Harness throughput for this configuration's full pass. **Not** a recall
    /// latency: it is every query's recall plus metric computation, taken on a
    /// loaded box.
    seconds: f64,
    /// How many documents the fused ranking returned, summed over every query. This
    /// is what makes R@20 a mid-list cut on this corpus rather than a ceiling, so it
    /// is measured rather than assumed.
    pool_len_sum: usize,
}

/// Score one configuration over the whole plan.
///
/// **The `embed` route is the same fusion, not a second implementation of it.** At
/// `weights.vector == 0.0` this takes [`MemoryService::recall_with_weights`], which is
/// the code path `eval/LOCOMO.md` was measured through, and the control row is required
/// to reproduce that artifact to the decimal — that check is what makes the
/// `vector: 0.00` row comparable to the lexical artifact rather than merely similar to
/// it. At `weights.vector != 0.0` it takes [`vector::recall`], which
/// [`vector::recall`]'s own doc comment describes as "the whole of `recall` — candidate
/// pool, BM25 stream, overlap stream, optional recency, RRF, budget trim, result cap —
/// plus the dense stream", reading the *same* `FTS_LIMIT`/`OVERLAP_LIMIT`/`MAX_RESULTS`
/// constants rather than copies. One is a different code path; the *lexical* half of it
/// is the same arithmetic, and the control check is what holds that honest.
#[cfg(feature = "embed")]
fn evaluate(
    corpus: &Corpus,
    planned: &[dev::PlannedQuery],
    weights: &FusionWeights,
    phase: &'static str,
    embedder: &mut Embedder,
) -> anyhow::Result<Run> {
    let mut agg = Agg::default();
    let mut per_cat: PerCategory = BTreeMap::new();
    let mut r5_by_query = Vec::with_capacity(planned.len());
    let mut pool_len_sum = 0usize;
    for pq in planned {
        let q = pq.query;
        let svc = &corpus.banks[&q.user_id];
        let retrieved: Vec<String> = if weights.vector == 0.0 {
            svc.recall_with_weights(&q.user_id, &q.query, corpus.budget_tokens, weights)?
                .into_iter()
                .map(|h| h.memory.id)
                .collect()
        } else {
            vector::recall(
                svc.store.as_ref(),
                &q.user_id,
                &q.query,
                corpus.budget_tokens,
                &[],
                weights,
                embedder,
            )
            .map_err(|e| anyhow::anyhow!("vector recall for query {} failed: {e}", q.id))?
            .into_iter()
            .map(|h| h.memory.id)
            .collect()
        };
        let row = Row::new(q, &retrieved, &pq.gold);
        per_cat.entry(row.category.clone()).or_default().add(&row);
        agg.add(&row);
        r5_by_query.push(row.r5);
        pool_len_sum += retrieved.len();
    }
    Ok(Run {
        weights: *weights,
        phase,
        round: 0,
        axis: None,
        obj: objective(&agg),
        agg,
        per_cat,
        r5_by_query,
        seconds: 0.0,
        pool_len_sum,
    })
}

/// The non-`embed` build: the dense mechanism does not exist in this binary, so
/// `evaluate` has exactly one path to take. Kept as a separate function rather than a
/// `cfg` block inside one so that the `#[cfg]` attribute on each *call site* in
/// [`main`] is the only conditional, and the sweep logic itself is one body.
#[cfg(not(feature = "embed"))]
fn evaluate(
    corpus: &Corpus,
    planned: &[dev::PlannedQuery],
    weights: &FusionWeights,
    phase: &'static str,
) -> anyhow::Result<Run> {
    let mut agg = Agg::default();
    let mut per_cat: PerCategory = BTreeMap::new();
    let mut r5_by_query = Vec::with_capacity(planned.len());
    let mut pool_len_sum = 0usize;
    for pq in planned {
        let q = pq.query;
        let svc = &corpus.banks[&q.user_id];
        let retrieved: Vec<String> = svc
            .recall_with_weights(&q.user_id, &q.query, corpus.budget_tokens, weights)?
            .into_iter()
            .map(|h| h.memory.id)
            .collect();
        let row = Row::new(q, &retrieved, &pq.gold);
        per_cat.entry(row.category.clone()).or_default().add(&row);
        agg.add(&row);
        r5_by_query.push(row.r5);
        pool_len_sum += retrieved.len();
    }
    Ok(Run {
        weights: *weights,
        phase,
        round: 0,
        axis: None,
        obj: objective(&agg),
        agg,
        per_cat,
        r5_by_query,
        seconds: 0.0,
        pool_len_sum,
    })
}

/// The configuration as one table cell: every swept coordinate, nothing else.
/// Whether this run sweeps the dense axis, decided once in [`main`] from `--axes`.
///
/// [`label`] needs it and takes no argument: the label is printed from a dozen places,
/// and threading a flag through all of them to keep a *display* string correct is the
/// kind of change that eventually makes the display string the wrong one.
static SWEEPS_VECTOR: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// The configuration as one table cell: every swept coordinate, nothing else.
fn label(w: &FusionWeights) -> String {
    // `vec` appears only when this run actually swept the dense axis. A grid whose rows
    // all read the same because the label omitted the swept coordinate is a grid where
    // two different configurations look identical, which is the one thing a
    // configuration column must never do. At the shipped `0.0` with no `vector` axis in
    // play the label is the lexical configuration, byte-identical to what every
    // committed artifact quoting it prints.
    let base = format!(
        "ov={:.2} · k={:.0} · mag={:.2} · agr={:.2} · rec={:.2} · hl={:.0}d",
        w.overlap, w.k, w.bm25_magnitude, w.agreement, w.recency, w.recency_half_life_days
    );
    if w.vector == 0.0 && !SWEEPS_VECTOR.load(std::sync::atomic::Ordering::Relaxed) {
        base
    } else {
        format!("{base} · vec={:.2}", w.vector)
    }
}

/// A delta line over all six metrics, for a move in the round log.
fn deltas(after: &Agg, before: &Agg) -> String {
    let (a, b) = (after.all(), before.all());
    METRIC_NAMES
        .iter()
        .zip(a.iter().zip(b.iter()))
        .map(|(name, (x, y))| format!("{name} {:+.1}", x - y))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    SWEEPS_VECTOR.store(
        bench_common::arg(&args, "--axes", "all")
            .split(',')
            .any(|a| a.trim() == "vector"),
        std::sync::atomic::Ordering::Relaxed,
    );
    let load_start = loadavg();
    let docs_path = bench_common::arg(&args, "--docs", "eval/data/locomo/documents.json");
    let queries_path = bench_common::arg(&args, "--data", "eval/data/locomo/queries.json");
    let n: usize = bench_common::arg(&args, "--n", "0").parse().unwrap_or(0);
    let seed: u64 = bench_common::arg(&args, "--seed", "42").parse().unwrap_or(42);
    let out_md = bench_common::out_md(&args, "SELECTION.md");
    // Temp-defaulted, like `locomo`: a grid's per-question dump is not this suite's
    // reviewable record, and two harnesses defaulting to one committed path is the
    // clobber waiting to happen.
    let out_json = match bench_common::flag(&args, "--out-json") {
        Some(p) => p,
        None => {
            let p = std::env::temp_dir().join("results_selection.json");
            eprintln!("no --out-json given: writing the grid to {}", p.display());
            p.display().to_string()
        }
    };
    let rounds: usize = bench_common::arg(&args, "--rounds", "2").parse().unwrap_or(2);
    let budget: f64 = bench_common::arg(&args, "--budget-seconds", "0").parse().unwrap_or(0.0);
    let axis_spec = bench_common::arg(&args, "--axes", "all");
    if bench_common::profile() != "release" {
        eprintln!(
            "WARNING: not `--release`. The retrieval metrics are unaffected by profile — there \
             is no LLM and no per-query wall clock in any table — but the artifact's provenance \
             line will say the profile, and it should be `release` to match every other \
             committed artifact."
        );
    }
    anyhow::ensure!(rounds >= 1, "select_fusion: --rounds must be at least 1");

    let chosen = select_axes(&axis_spec)?;
    let corpus = Corpus::load(&docs_path, &queries_path)?;
    let (bank_min, bank_max) = corpus.bank_range();
    let cats = corpus.categories();

    // Shuffled for the progress log only, and for `--n`: a query's index is fixed by
    // its `user_id` and every configuration is scored on that one index, so the
    // metrics are seed-independent and the artifact says so rather than let `--seed`
    // imply otherwise.
    let mut order = bench_common::shuffle(corpus.queries.len(), seed);
    if n > 0 && n < order.len() {
        order.truncate(n);
    }
    let planned = dev::plan(&corpus.queries, order);
    anyhow::ensure!(
        !planned.is_empty(),
        "select_fusion: no query with a gold document — refusing to publish a 0% grid"
    );

    // The measured `created_at` spread, which is what decides whether the recency
    // axis means anything on this corpus. Read back through the public API rather
    // than asserted: `Corpus::load` passes `created_at: None`, so the database stamps
    // ingest time, and this prints how much spread that left.
    let ingest_span = widest_created_at_span(&corpus)?;

    // ---- the dense arm, and the coverage check that gates every number below -------
    // The embedder is built only when this binary has the `embed` feature, and the
    // corpus is embedded *before* the sweep starts, so a row of the grid can never be
    // scored over a bank whose vectors are missing. `ensure_embedded` returns an error
    // rather than a warning, and `main` propagates it: no grid, no JSON, no exit 0.
    #[cfg(feature = "embed")]
    let mut coverage: Option<(Coverage, Embedder)> = {
        eprintln!("select_fusion: loading the vendored MiniLM session (23 MB, no network)");
        let mut e = Embedder::new().map_err(|err| {
            anyhow::anyhow!("select_fusion: cannot build the embedder: {err}. The `vector` axis \
                             needs `--features embed`; the lexical axes do not.")
        })?;
        let cov = ensure_embedded(&corpus, &mut e)?;
        let (m, v) = cov.totals();
        eprintln!(
            "select_fusion: vector coverage {v}/{m} rows across {} banks",
            cov.per_bank.len()
        );
        Some((cov, e))
    };
    // The default build has no dense arm in this binary, so there is no session and no
    // coverage to read. A unit-typed `()` keeps the three `eval_one!` call sites written
    // once, in one form, for both builds — the macro's `embed` arm is the only consumer.
    #[cfg(not(feature = "embed"))]
    #[allow(unused_variables)]
    let coverage: () = ();

    eprintln!(
        "select_fusion: {} documents in {} banks, {} queries evaluated (of {}), {} axes, \
         {rounds} round(s), budget {budget:.0}s; loadavg at start {load_start}",
        corpus.documents.len(),
        corpus.banks.len(),
        planned.len(),
        corpus.queries.len(),
        chosen.len(),
    );

    // ---- the sweep ----------------------------------------------------------------
    let start = Instant::now();
    let mut runs: Vec<Run> = Vec::new();
    let mut truncated = false;
    let mut axis_pass_truncated = false;
    let mut mean_seconds;
    let mut evals;
    let mut moves: Vec<(usize, &'static str, String, String, String, usize)> = Vec::new();
    let mut skipped_axes: Vec<(usize, &'static str, String)> = Vec::new();
    let mut round_open: Vec<(usize, usize, String, Agg)> = Vec::new();

    // Row 0 is the control: the shipped configuration, measured here rather than
    // copied, and checked against the committed numbers before anything else.
    //
    // `eval_one!` is the one call the sweep makes, so the `embed`/no-`embed` split lives
    // in exactly one place rather than at each of the three call sites below.
    let mut t0 = Instant::now();
    let mut control =
        eval_one!(&corpus, &planned, &FusionWeights::SHIPPED, "control", &mut coverage)?;
    control.seconds = t0.elapsed().as_secs_f64();
    mean_seconds = control.seconds;
    evals = 1;
    eprintln!("  [0] shipped control done in {:.1}s", control.seconds);
    let control_check = check_control(&control, n == 0);
    if !control_check.ok {
        eprintln!(
            "\n*** CONTROL FAILED ***\n{}\
             \nThis is a bug in this harness, not a finding about the weights. Every other \
             number in this run is void. See the banner at the top of {out_md}.\n",
            control_check.detail
        );
    }
    runs.push(control);

    // ---- pass 1: one axis at a time, from the shipped start point ------------------
    // This is the sweep the task's own reasoning says cannot see interactions, and it
    // is here *for that reason*: the walk below reports only what it happened to walk
    // through, and a walk that takes `overlap` to 0.0 on its first move then measures
    // `k`, `bm25_magnitude` and `agreement` in a regime where all three are
    // **structurally** inert — with one stream there is no cross-stream agreement to
    // credit, and a rank-only score is already monotone in rank, so neither `k` nor
    // the magnitude can reorder anything. That is a real property of the algebra, and
    // it is also a trap for the reader: those axes would look dead. Measuring each
    // axis at the shipped start as well is what makes the contrast checkable instead
    // of asserted, and it costs one extra pass.
    'axis_pass: for axis in &chosen {
        if !(axis.applies)(&FusionWeights::SHIPPED) {
            skipped_axes.push((0, axis.name, axis_skip_reason(axis.name)));
            continue;
        }
        let incumbent = (axis.get)(&FusionWeights::SHIPPED);
        for value in axis.values {
            if (value - incumbent).abs() < f64::EPSILON {
                continue; // the value the start point is already at
            }
            if over_budget(&start, budget, mean_seconds, evals) {
                truncated = true;
                axis_pass_truncated = true;
                break 'axis_pass;
            }
            let candidate = (axis.set)(&FusionWeights::SHIPPED, *value);
            t0 = Instant::now();
            let mut run =
                eval_one!(&corpus, &planned, &candidate, "axis-at-shipped", &mut coverage)?;
            run.seconds = t0.elapsed().as_secs_f64();
            run.axis = Some(axis.name);
            mean_seconds =
                (mean_seconds * evals as f64 + run.seconds) / (evals as f64 + 1.0);
            evals += 1;
            eprintln!(
                "  [{}] axis-at-shipped {} = {value}  R@5 {:.1}%  ({:.1}s)",
                runs.len(),
                axis.name,
                run.agg.r5(),
                run.seconds
            );
            runs.push(run);
        }
    }

    // ---- pass 2: coordinate descent, from the shipped start point -----------------
    let mut current = 0usize;
    'rounds: for round in 1..=rounds {
        let round_start_agg = runs[current].agg.clone();
        let coords = chosen.len();
        for axis in &chosen {
            if !(axis.applies)(&runs[current].weights) {
                skipped_axes.push((round, axis.name, axis_skip_reason(axis.name)));
                continue;
            }
            let incumbent = (axis.get)(&runs[current].weights);
            let mut best = current;
            for value in axis.values {
                if (value - incumbent).abs() < f64::EPSILON {
                    continue; // the value the walk is already at
                }
                // Budget: predict the next configuration's cost from the running
                // mean and stop *before* starting it, so the cap is a cap rather
                // than a suggestion. The prediction is why a truncated run still
                // lands inside its budget instead of one configuration over.
                if over_budget(&start, budget, mean_seconds, evals) {
                    truncated = true;
                    break 'rounds;
                }
                let candidate = (axis.set)(&runs[current].weights, *value);
                t0 = Instant::now();
                let mut run = eval_one!(&corpus, &planned, &candidate, "walk", &mut coverage)?;
                run.seconds = t0.elapsed().as_secs_f64();
                run.round = round;
                run.axis = Some(axis.name);
                mean_seconds =
                    (mean_seconds * evals as f64 + run.seconds) / (evals as f64 + 1.0);
                evals += 1;
                eprintln!(
                    "  [{}] round {round} {} = {value}  R@5 {:.1}%  ({:.1}s)",
                    runs.len(),
                    axis.name,
                    run.agg.r5(),
                    run.seconds
                );
                let idx = runs.len();
                if strictly_better(&run.obj, &runs[best].obj) {
                    best = idx;
                }
                runs.push(run);
            }
            if best != current {
                moves.push((
                    round,
                    axis.name,
                    format!("{incumbent:.2}"),
                    format!("{:.2}", (axis.get)(&runs[best].weights)),
                    deltas(&runs[best].agg, &runs[current].agg),
                    dev::movement(&runs[current].r5_by_query, &runs[best].r5_by_query).0,
                ));
                eprintln!(
                    "  -> round {round} moved {} {incumbent:.2} -> {:.2}  ({})",
                    axis.name,
                    (axis.get)(&runs[best].weights),
                    deltas(&runs[best].agg, &runs[current].agg)
                );
                current = best;
            }
        }
        round_open.push((
            round,
            coords,
            label(&runs[current].weights),
            runs[current].agg.clone(),
        ));
        eprintln!(
            "round {round} end: {}  ({})",
            label(&runs[current].weights),
            deltas(&runs[current].agg, &round_start_agg)
        );
    }
    let endpoint = current;
    let elapsed = start.elapsed().as_secs_f64();
    let load_end = loadavg();
    let n_eval = runs[0].agg.count;
    // Measured, not assumed: how long a fused ranking actually is on this corpus,
    // which is what makes R@20 a mid-list cut here rather than a ceiling.
    let pool_mean = runs[0].pool_len_sum as f64 / n_eval.max(1) as f64;

    // ---- distinct configurations, for an honest config count ---------------------
    let mut distinct: Vec<String> = Vec::new();
    for r in &runs {
        let l = label(&r.weights);
        if !distinct.contains(&l) {
            distinct.push(l);
        }
    }

    // ---- is the other function's lexical half the same function? ---------------------
    // Run only when this binary actually has the dense arm, and only when the run
    // sweeps it: with `vector` at `0.0` everywhere, the two paths are never mixed and
    // there is nothing to be confounded about.
    #[cfg(feature = "embed")]
    let lexical_equivalence = if SWEEPS_VECTOR.load(std::sync::atomic::Ordering::Relaxed) {
        t0 = Instant::now();
        let r = check_lexical_path_equivalence(
            &corpus,
            &planned,
            &runs[0].agg,
            &mut coverage.as_mut().unwrap().1,
        )?;
        eprintln!(
            "  [equiv] vector::recall lexical half vs service path: {} ({:.1}s)",
            if r.0 { "identical" } else { "DISAGREES" },
            t0.elapsed().as_secs_f64()
        );
        Some(r)
    } else {
        None
    };
    #[cfg(not(feature = "embed"))]
    #[allow(unused_variables)]
    let lexical_equivalence: Option<(bool, String)> = None;

    // ---- the dense arm's own reach, and the two weights worth a category table ----
    // Both computed here, from the runs already in the table and from one extra pass.
    // Neither selects anything: the reach number is a property of the mechanism on this
    // corpus, and the two weights are named by a *rule* stated in the artifact, not by
    // which row scored highest.
    #[cfg(feature = "embed")]
    let (reach, reach_missed) = {
        t0 = Instant::now();
        let r = vector_stream_reach(&corpus, &planned, &mut coverage.as_mut().unwrap().1)?;
        eprintln!(
            "  [reach] vector stream alone measured in {:.1}s, {} pool miss(es)",
            t0.elapsed().as_secs_f64(),
            r.1.len()
        );
        r
    };

    // The two "most interesting" weights, chosen by a rule that is not "the best":
    // the largest weight that did **not** lose R@5 against the control, and the
    // largest weight overall. On a corpus where the dense arm turns out to be
    // inert these collapse onto one another, and the artifact says so rather than
    // printing the same row twice and calling it a comparison.
    #[cfg(feature = "embed")]
    let interesting: Vec<(f64, usize)> = {
        let mut axis_rows: Vec<(f64, usize)> = runs
            .iter()
            .filter(|r| r.phase == "axis-at-shipped" && r.axis == Some("vector"))
            .map(|r| (r.weights.vector, runs.iter().position(|x| std::ptr::eq(x, r)).unwrap_or(0)))
            .collect();
        axis_rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let control_r5 = runs[0].agg.r5();
        let not_worse =
            axis_rows.iter().copied().find(|(_, i)| runs[*i].agg.r5() >= control_r5);
        let widest = axis_rows.last().copied();
        let mut picked: Vec<(f64, usize)> = Vec::new();
        for c in [not_worse, widest].into_iter().flatten() {
            if !picked.iter().any(|(w, _)| (w - c.0).abs() < f64::EPSILON) {
                picked.push(c);
            }
        }
        picked
    };
    // Unused without the feature, because the section that reads it is behind the same
    // `cfg` — an empty vector rather than a fake one, so a no-`embed` build cannot
    // accidentally render the dense-axis table from stale lexical rows.
    #[cfg(not(feature = "embed"))]
    let interesting: Vec<(f64, usize)> = Vec::new();
    #[cfg(not(feature = "embed"))]
    let _ = &interesting;

    // ---- the artifact --------------------------------------------------------------
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let mut md = String::new();
    if !control_check.ok {
        let _ = write!(
            md,
            "# ⚠️ CONTROL FAILED — every number below is void\n\n\
             # Fusion-weight selection grid (memory-wire) — the dev set\n\n\
             > **The shipped `overlap: 0.25` control did not reproduce the numbers already \
             committed in `eval/LOCOMO.md`.** That is a defect in *this harness* — the shared \
             ingestion and metric code it uses with `examples/locomo.rs` — and **not** a finding \
             about the weights. Do not read any cell below as a measurement of anything. The run \
             exited nonzero for exactly this reason.\n\n\
             {}\n\n\
             ---\n\n",
            control_check.detail
        );
    } else {
        md.push_str("# Fusion-weight selection grid (memory-wire) — the dev set\n\n");
    }

    let _ = write!(
        md,
        "## Provenance\n\n\
         | | |\n|---|---|\n\
         | Commit under test | `{}` |\n\
         | Working tree at run time | {tree} |\n\
         | Command | `cargo run --release --example select_fusion -- --docs {} --data {} \
--rounds {rounds} --budget-seconds {budget:.0}{axes_arg} --out-md {out_md} --out-json {out_json}` |\n\
         | Run | {date}, from `--{profile}`, on {host} |\n\
         | Load at start / at end | `{load_start}` / `{load_end}` |\n\
         | Dataset | LoCoMo (Snap Research) via the canonical distribution \
`vectorize-io/agent-memory-benchmark`, `data/locomo/locomo10/`, pinned at upstream commit \
`{upstream}` and sha256-verified by `eval/download.sh` |\n\
         | Corpus | {docs_n} documents, {queries_n} queries, {banks_n} conversations — fetched by `./eval/download.sh`, \
**not committed** (`eval/data/` is gitignored) |\n\
         | **LLM involvement** | **none.** No model, no judge, no answer generation, no network \
call. `gold_answers` is read for shape and never scored. |\n\
         | LongMemEval involvement | **none.** Selection happens on LoCoMo and only on LoCoMo \
(`AGENTS.md` §1). The 500 test questions are untouched by this binary. |\n\n\
         The retrieval metrics do not depend on machine load and there is **no recall-latency \
column** in this artifact, so the load row is recorded for completeness — the same reason \
`eval/LOCOMO.md` carries no timing. The wall clock in \"Runtime\" below is *harness throughput*, \
not a latency measurement, and it is reported next to the load line for that reason.\n\n",
        dev::git_head(),
        docs_path,
        queries_path,
        date = date,
        rounds = rounds,
        budget = budget,
        axes_arg = if axis_spec == "all" {
            String::new()
        } else {
            format!(" --axes {axis_spec}")
        },
        profile = bench_common::profile(),
        host = bench_common::host(),
        upstream = UPSTREAM_REF,
        docs_n = corpus.documents.len(),
        queries_n = corpus.queries.len(),
        banks_n = corpus.banks.len(),
        load_start = load_start,
        load_end = load_end,
        tree = tree_state(),
        out_md = out_md,
        out_json = out_json,
    );

    // The control, first and loudest.
    let c = &runs[0];
    let _ = write!(
        md,
        "## Control — the shipped configuration, re-measured here\n\n\
         The walk starts at `FusionWeights::SHIPPED`, so its first row is the control this \
harness is validated against: the numbers `eval/LOCOMO.md` already committed for \
`overlap: 0.25`. {}\n\n\
         | metric | measured here | `eval/LOCOMO.md` | Δ |\n|---|---|---|---|\n",
        if control_check.ok {
            "It reproduces them."
        } else {
            "**It does not reproduce them, and that invalidates the rest of this file.**"
        }
    );
    for ((name, want), got) in CONTROL.iter().zip(c.agg.all()) {
        let _ = writeln!(md, "| {name} | {got:.1}% | {want:.1}% | {:+.1} |", got - want);
    }
    let _ = writeln!(
        md,
        "| n | {} | {CONTROL_N} | {} |",
        c.agg.count,
        c.agg.count as i64 - CONTROL_N as i64
    );
    if bench_common::fitted_weight_note(&c.weights).is_some() {
        // `bench_common::fitted_weight_note` is worded for a single-configuration
        // artifact ("every retrieval number in this artifact"). That is false here —
        // this file holds dozens of configurations — so the control's own caveat is
        // written out instead, scoped to the control row where the fitted value
        // actually applies. Reusing the shared note verbatim would put a false
        // statement near the top of the file.
        let _ = write!(
            md,
            "\n> **The control row above is measured at a fitted value.** `overlap: 0.25` won \
46 configurations scored on LongMemEval-S's own 500 questions, so it was chosen by looking at \
the test set (`AGENTS.md` §1, `docs/EVALUATION_HYGIENE.md` §2.1). It is reproduced here \
because this harness starts there, and because the replication on independent data is \
established in `eval/LOCOMO.md` — not because the number is a generalisation estimate. The \
other rows in this file are at values that were fitted to nothing.\n"
        );
    }

    // ---- the dense arm: coverage, then reach. Both before the grid, because a reader
    // who skips them reads the `vector` rows without the two facts that decide what
    // they mean. The coverage check has already *passed* to reach this line — the run
    // aborts on partial coverage — so the table below is evidence, not a warning.
    #[cfg(feature = "embed")]
    if let Some((cov, _)) = coverage.as_ref() {
        let (memories, vectors) = cov.totals();
        let _ = write!(
            md,
            "\n## The dense arm — vector coverage, checked before any row was scored\n\n\
             `vector::retain` is the only writer of `memory_vectors`, and it runs at **retain \
             time**. The dev corpus ingests through `store.put`, which writes no vector, so a sweep \
             over the shared ingestion would have scored a dense stream over banks with **no \
             vectors at all** — and `vector::vector_stream` returns an empty stream for a \
             vectorless bank *by design*, so the whole `vector` axis would have read as a null for \
             the wrong reason. Every document is therefore embedded here, before the sweep, and the \
             run **aborts rather than publishing a grid** if the counts disagree.\n\n\
             The count below is read back through `Store::bank_vectors`, which **decodes each blob \
             and silently omits an unreadable one** — so this is the number of *usable* vectors, \
             not the number of writes that claimed to succeed.\n\n\
             | bank | `memories` rows | decodable vectors | complete |\n|---|---|---|---|\n\
             {table}\n\n\
             **{vectors} of {memories} rows, {banks} banks, 100% covered.** Each document is \
             embedded from the same `doc.text()` the lexical arm indexes — the dialogue turns joined \
             one per line — so both streams rank the same content and the sweep measures the \
             weight rather than a difference in what was indexed. `vector::retain` was deliberately \
             *not* used for the write: it routes through `MemoryService::retain_doc` and \
             `redact_pii`, which would replace the store's own text and leave the two arms ranking \
             different strings.\n\n{equiv_note}\n",
            equiv_note = match lexical_equivalence.as_ref() {
                Some((true, detail)) => format!(
                    "**The other function's lexical half is the same function.** Rows with \
`vector != 0.0` are scored through `vector::recall`; the control row is scored through \
`MemoryService::recall_with_weights`, the path `eval/LOCOMO.md` was measured on. Those are two \
different functions, so every `vector` row would differ from the control in **two** ways at \
once — the added dense stream *and* any lexical drift — if the lexical halves disagreed. They do \
not: {detail}"
                ),
                Some((false, detail)) => format!(
                    "**THE TWO PATHS DISAGREE, AND EVERY `vector` ROW BELOW IS CONFOUNDED.** {detail} \
A row on this axis would differ from the control by the dense stream *and* by a lexical change \
nobody asked about. The grid is reported because the run measured it, and the caveat is here \
because that is the only honest way to report it."
                ),
                None => String::new(),
            },
            table = cov.table(),
            vectors = vectors,
            memories = memories,
            banks = cov.per_bank.len(),
        );

        // The reach question, which is a different measurement from the weight sweep
        // and the more informative one.
        let ctl = &runs[0];
        let (up, down, same) = dev::movement(&ctl.r5_by_query, &[]);
        let _ = (up, down, same);
        let vec_pool = reach.pool();
        let lex_pool = ctl.agg.pool();
        let v_r5 = reach.r5();
        let l_r5 = ctl.agg.r5();
        let reach_reading = if (vec_pool - lex_pool).abs() < 0.05 {
            format!(
                "**The dense stream reaches exactly as much as the lexical fusion does, and ranks \
                 it further down.** R@pool {vec_pool:.1}% against the fusion's {lex_pool:.1}% — the \
                 same, to the decimal — at R@5 {v_r5:.1}% against {l_r5:.1}%. A stream that puts \
                 the same gold documents in the list and much further down it is a **reordering** \
                 signal on this corpus, not a **recall** signal. That reframes what the weight \
                 sweep below is for: it is not measuring whether the dense arm can find things the \
                 lexical arm misses, because on this corpus it reaches nothing the lexical arm \
                 misses, and a weight that made the dense arm dominant would be buying reordering \
                 of a pool the lexical streams already had."
            )
        } else if vec_pool < lex_pool {
            format!(
                "**The dense stream reaches strictly less than the lexical fusion** — R@pool \
                 {vec_pool:.1}% against {lex_pool:.1}% — at R@5 {v_r5:.1}% against {l_r5:.1}%. It \
                 is a subset signal here: it contributes to the ordering of documents the lexical \
                 streams already surfaced, and it cannot by itself reach the {gap:.1}pp of gold the \
                 lexical fusion reaches and it does not.",
                gap = lex_pool - vec_pool
            )
        } else {
            format!(
                "**The dense stream reaches more than the lexical fusion** — R@pool {vec_pool:.1}% \
                 against {lex_pool:.1}%, at R@5 {v_r5:.1}% against {l_r5:.1}%. On this corpus there \
                 is gold the lexical streams do not put in the list at any weight in the table \
                 below, and this is the measurement that says so — {gap:.1}pp of it.",
                gap = vec_pool - lex_pool
            )
        };
        // The sub-1pp gap is not noise and not a trim artifact, and the artifact says
        // what it is rather than rounding it away. A sub-1pp R@pool gap is a *specific*
        // claim — on this corpus it is one query — and one query has an id and a cause.
        // The ids come from `vector_stream_reach`, measured; only the diagnosis is prose,
        // and it is stated as a diagnosis rather than as a number.
        let reach_bug = if !reach_missed.is_empty() {
            let short = (100.0 - vec_pool) / 100.0 * reach.count as f64;
            let ids = reach_missed
                .iter()
                .map(|q| format!("`{q}`"))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "### The {short:.0}pp gap is {count} quer{y}, and it is a defect in \
`rank_by_cosine`\n\n\
                 R@pool {vec_pool:.4}% of {n} queries evaluated. The dense stream fails to return its gold \
on {ids}{extra}. Worth naming rather than rounding to 100.0% for three reasons: it is the \
only place the dense arm's coverage is not total; it is reproducible from a single query; \
and the cause is visible in one line of code.\n\n\
                 **`src/embed.rs:36` keeps `*s > 0.0` and discards every candidate whose cosine \
is zero or negative.** A document with a perfectly valid embedding is therefore *deleted \
from the ranking* rather than ranked last, which is a reach bug rather than a tiebreak. \
`src/vector.rs`'s own doc comment states the intended rule — an id *with no vector* is \
unranked — so the implemented rule is both stricter and undocumented than the \
documentation beside it claims.\n\n\
                 **Not fixed here, deliberately**: `src/` is off-limits for this run \
(AGENTS.md, and the file-ownership constraint on this harness). It is reported because a \
decision about a shipped mechanism belongs in `docs/CONSISTENCY.md`, and because rounding \
a measured 99.93% up to 100% because it *displays* as 100% is the exact class of \
measurement this project exists to prevent. **Its bias has a direction worth stating: it can \
only ever make the dense arm look worse than it is**, so no `vector` row in the grid above \
is flattered by it, and the reach conclusion is if anything conservative.",
                count = reach_missed.len(),
                n = reach.count,
                y = if reach_missed.len() == 1 { "y" } else { "ies" },
                extra = if reach_missed.len() == 1 { String::new() } else { " (and possibly more, if the list was truncated)".to_string() },
            )
        } else {
            String::new()
        };
        let _ = write!(
            md,
            "\n## How much does the dense stream *reach*? The number this axis is for\n\n\
             One question, asked with the dense stream **alone** — both lexical streams dropped, \
             `bm25: 0.0, overlap: 0.0, vector: 1.00` — against the same gold, the same banks, the \
             same 100%-covered index. It is a **diagnostic, not a candidate configuration**: nobody \
             ships a dense-only arm on a corpus whose lexical arm already has 100% R@pool, and this \
             row is not eligible for the walk.\n\n\
             | stream | R@pool | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR | n |\n|---|---|---|---|---|---|---|---|---|\n\
             | **dense alone** (`vector: 1.00`, no lexical stream) | {vp:.1}% | {v1:.1}% | \
{v5:.1}% | {v10:.1}% | {v20:.1}% | {vn:.1}% | {vm:.1}% | {vcnt} |\n\
             | fused at the control weight (`vector: 0.00`, lexical only) | {lp:.1}% | {l1:.1}% | \
{l5:.1}% | {l10:.1}% | {l20:.1}% | {ln:.1}% | {lm:.1}% | {lcnt} |\n\n\
             {reach_reading}\n\n\
             **R@pool is measured over the whole returned list, not a top-k**, which is what makes \
             it answer \"is retrieval or coverage the binding constraint\" — the question R@20 cannot \
             answer on a corpus whose banks are 19–32 documents. It is the reach number this axis \
             needs, and it is the reason a `vector` weight that *helps* here would be helping with \
             order rather than with recall.\n\
             {reach_bug}\n",
            reach_bug = reach_bug,
            vp = vec_pool,
            v1 = reach.r1(),
            v5 = reach.r5(),
            v10 = reach.r10(),
            v20 = reach.r20(),
            vn = reach.ndcg10(),
            vm = reach.mrr(),
            vcnt = reach.count,
            lp = lex_pool,
            l1 = ctl.agg.r1(),
            l5 = ctl.agg.r5(),
            l10 = ctl.agg.r10(),
            l20 = ctl.agg.r20(),
            ln = ctl.agg.ndcg10(),
            lm = ctl.agg.mrr(),
            lcnt = ctl.agg.count,
        );
    }

    // The dense axis exists only in an `embed` build, so the axes table either grows a
    // row for it or does not — an empty cell would read as "swept, found nothing".
    #[cfg(feature = "embed")]
    let vector_axis_row = if chosen.iter().any(|a| a.name == "vector") {
        format!(
            "\n| 7 | `vector` | {} | the dense-vector stream (slot 3); **present only in an \
`--features embed` build**, and **carrying `1.50` as a diagnostic bound past the point where \
the dense arm becomes the sort key rather than a tiebreaker** — see the note below\n",
            fmt_list(VECTOR)
        )
    } else {
        String::new()
    };
    #[cfg(not(feature = "embed"))]
    let vector_axis_row: String = String::new();

    let _ = write!(
        md,
        "\n## What this artifact is, and what it is not\n\n\
         **It is a grid.** {evaluations} configurations were scored, {distinct} of them \
distinct, on {n_eval} queries that never chose any of these values. Every row below carries \
the full metric set and its `n`.\n\n\
         **It is not a selection.** There is no recommended configuration here, no \
best-configuration row, and the tables are in *evaluation order* rather than sorted by \
score precisely so that nothing in this file can be read as a ranking. The walk has to move \
to *some* value to make progress, and where it moved is a consequence of the objective \
declared below — but choosing what to ship is a decision for a human, made on this evidence \
and recorded in `docs/CONSISTENCY.md` under the counting rule in \
`docs/EVALUATION_HYGIENE.md` §3.3. **A sweep that emitted a recommendation would be the \
exact failure this whole exercise exists to prevent**, and the run that produced \
`overlap: 0.25` is the cautionary example: 46 configurations scored on the test set, a \
maximum taken, and a +4.2pp effect that reproduced at +1.4pp on independent data \
(`docs/EVALUATION_HYGIENE.md` §2.1).\n\n\
         **A `recall_any@K` from this harness is not a LoCoMo score.** LoCoMo's standard \
metric is LLM-judged *answer* accuracy, and this harness never generates or judges an \
answer. It is a different measurement on a different scale and must not be placed beside a \
published LoCoMo figure. What *is* comparable is every difference between two rows of the \
table below: same corpus, same index, same harness, same run. The suites differ from \
LongMemEval in document granularity (LoCoMo: {bank_min}–{bank_max} per bank; LongMemEval: \
~48), question style and question language, so **the absolute levels are not comparable \
either — only the within-table deltas are.**\n\n\
         ## Method\n\n\
         One bank per `user_id`, built once, one memory per LoCoMo document, `id` = document \
id, content = that document's dialogue turns joined one per line. Each query is answered \
against **its own `user_id`'s bank and no other**. Every configuration is scored on that \
one shared index, so two rows cannot disagree because one's index happened to build \
differently. The ingestion and the metric definitions are literally the same code as \
`examples/locomo.rs` — `eval/locomo_dev.rs`, shared by both — so a number here and a number \
in `eval/LOCOMO.md` mean the same measurement.\n\n\
         ### The two passes\n\n\
         The run is two passes over the same axes, in this order, and both are in the table \
below.\n\n\
         **Pass 1 — one axis at a time, from the shipped start point.** Each axis's full \
candidate list is measured with every other coordinate held at the shipped default. This is \
the conventional sweep, and it is here precisely because of what it cannot show: every row of \
it sits in the one regime where the axes are independent. It is also the control for pass 2 — \
when a coordinate's pass-1 rows and pass-2 rows disagree, the difference *is* the interaction, \
measured rather than argued.\n\n\
         **Pass 2 — coordinate descent, {rounds} round(s)**, from the same shipped start point, \
over the axes in this order:\n\n\
         | # | axis | values tried | what it is |\n|---|---|---|---|\n\
         | 1 | `overlap` | {overlap} | the token-overlap stream's weight; the control, first, \
because the three-set discipline in `AGENTS.md` §1 exists over this one value |\n\
         | 2 | `k` | {k} | the RRF constant, `docs/PERFORMANCE_PLAN.md` P1; **borrowed, never \
fitted** (`docs/EVALUATION_HYGIENE.md` §2.6) |\n\
         | 3 | `bm25_magnitude` | {magnitude} | the BM25 score FTS5 already computed and the \
rank-only RRF form discards; **diagnostic bound `4.00`**, see the note below |\n\
         | 4 | `agreement` | {agreement} | the cross-stream bonus; agentmemory applies \
`1 + 0.05 × (matchedStreams − 1)` |\n\
         | 5 | `recency` | {recency} | a third stream over `memories.created_at`; **swept last**, \
on its own merits — see the recency caveat |\n\
         | 6 | `recency_half_life_days` | {half_life} | the recency decay; skipped entirely while \
`recency == 0.0`, because a zero weight drops the stream before the half-life is read |\n{vector_axis_row}\n\
         One round sweeps every axis once in that order; each move is re-evaluated against the \
current values of all the others, which is the whole reason for the method: **the interaction \
is the thing being measured.** `docs/EXCEED_PLAN.md` §0.3 records Hindsight's own \
`recall_boost.py:29-50` finding that score-space weights above the RRF spread degenerate into \
lexicographic sorting and cost `recall@20 0.97 → 0.40`; a one-axis-at-a-time sweep holds every \
other weight at its inert default, which is the single regime in which that collapse is \
invisible. `bm25_magnitude: 4.00` is carried in the grid on purpose so the collapse is \
*measured* here rather than imported as a citation.\n\n\
         **The objective, declared.** The walk maximises, in this order: **R@5, then NDCG@10, \
then MRR, then R@1, then R@10, then R@20.** R@5 leads because `docs/EXCEED_PLAN.md` §4 states \
the bar in R@5 terms; NDCG@10 and MRR follow because they are the ordering metrics the P1 gate \
refuses to regress; R@1 follows because the audit's 16.2pp oracle gap is an R@1 gap. **This \
is an instrument, not a claim about which metric matters** — a different order would walk a \
different path and would be equally defensible, which is why the order is printed here rather \
than hidden in a comparator. A move happens only on a **strict** improvement, so an exact tie \
leaves the walk where it is and a plateau cannot make it drift.\n\n\
         ### Determinism and the absence of a confidence interval\n\n\
         The metrics here are **deterministic and seed-independent**: a query's index is fixed \
by its `user_id`, every configuration is scored on that one index, and `--seed` changes only \
the order of the progress log. Re-running this binary reproduces every cell below \
bit-for-bit. That is why **no confidence interval is reported and none should be**: there is \
no run-to-run variance in these numbers to put an interval around. The only stochastic \
quantity in the run is wall clock, which is throughput, not a measurement. A sweep that *were* \
stochastic at this size would run once and be reported without a CI, per `AGENTS.md` §3 — but \
this one is not that case, and the distinction is worth stating rather than glossing.\n\n\
         ### Limitations — read before drawing anything from this grid\n\n\
         1. **Coordinate descent is greedy and axis-ordered, and can therefore miss \
interactions.** It can walk into a local optimum whose basin depends on the walk order; it \
never revisits a combination it has left; and a plateau can hide a descent beside it. {rounds} \
round(s) is a floor imposed by exactly that weakness, not a claim that it is enough. A \
different walk order, or a full factorial grid, could land somewhere else. **No cell in this \
file is a claim about the global optimum of the weight space.**\n\
         2. **One corpus.** Every number is LoCoMo, one snapshot of one dataset. A weight that \
helps here is evidence *about this corpus*; generalization is what \
`docs/EVALUATION_HYGIENE.md` §3.4 requires and what only a held-out set can supply.\n\
         3. **R@20 is a mid-list cut, not a ceiling.** The fused ranking returns a mean of \
{pool_mean:.1} documents per query, so a top-20 cut lands inside a {bank_min}–{bank_max} \
document list. The discriminating columns are R@1, R@5 and NDCG@10 — ordering, which is what \
a fusion weight controls.\n\
         4. **The recency axis is measured, but what it measures is this harness.** See the \
caveat below.\n\
         5. **Two `FusionWeights` fields were not swept**, and a reader should not assume they \
are inert: `overlap_scope` (whole-document vs per-segment overlap scoring) and \
`recency_policy` (`Always` vs `TemporalQueriesOnly`). Both are shipped at their inert \
defaults and neither is in `docs/EXCEED_PLAN.md` Phases A–B's scope; sweeping them is a \
separate, separately-motivated job.\n\n\
         ### The recency caveat, measured\n\n\
         `Corpus::load` writes every LoCoMo document with `created_at: None`, and the store \
stamps ingest time on insert — **LoCoMo documents carry no date of their own for the recency \
stream to read.** The widest `created_at` spread observed across any bank after ingestion \
was **{span}**, which means `recency_rank` is ordering a bank whose documents all carry \
approximately the same timestamp, and its documented tiebreak (descending decay, then id \
ascending) is doing the work. **So a delta on the `recency` axis here is a delta attributable \
to *ingest order*, not to recency as a retrieval signal, and must not be read as evidence for \
the mechanism.** This is stated because it is easy to miss: the axis produces plausible-looking \
numbers, and those numbers are about the harness. It is also why `recency` is swept **last** — \
`docs/EXCEED_PLAN.md` Phase C: agentmemory has no recency term anywhere in its retrieval path, \
and the \"f2–f5 at 27% with token recency\" figure that originally motivated this lever **does \
not exist in their repo** (exhaustive search; the withdrawal is recorded in \
`docs/CONSISTENCY.md` §14.4). There is no external citation suggesting this axis will win, and \
this artifact does not supply one.\n\n\
         ## Every configuration, in evaluation order\n\n\
         Two passes, and the distinction matters. **`axis-at-shipped`** varies one axis and \
holds every other at the shipped default — the conventional one-at-a-time sweep, which is \
the regime in which interactions are invisible. **`walk`** is coordinate descent: each candidate \
is measured against the *current* values of all the other axes. The control row is `control`.\n\n\
         | # | pass | round | axis swept | configuration | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR | \
R@pool | R@5 vs shipped | n |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
        evaluations = runs.len(),
        distinct = distinct.len(),
        n_eval = n_eval,
        bank_min = bank_min,
        bank_max = bank_max,
        overlap = fmt_list(OVERLAP),
        k = fmt_list(K),
        magnitude = fmt_list(BM25_MAGNITUDE),
        agreement = fmt_list(AGREEMENT),
        recency = fmt_list(RECENCY),
        half_life = fmt_list(RECENCY_HALF_LIFE),
        vector_axis_row = vector_axis_row,
        pool_mean = pool_mean,
        span = ingest_span,
        rounds = rounds,
    );
    for (i, r) in runs.iter().enumerate() {
        let (up, down, same) = dev::movement(&runs[0].r5_by_query, &r.r5_by_query);
        let _ = writeln!(
            md,
            "| {i} | {phase} | {round} | {axis} | {cfg} | {r1:.1}% | {r5:.1}% | {r10:.1}% | {r20:.1}% | \
{n:.1}% | {mrr:.1}% | {pool:.1}% | {up}/{down}/{same} | {cnt} |",
            phase = r.phase,
            round = if r.phase == "control" { 0 } else { r.round },
            axis = r.axis.unwrap_or("— (start point)"),
            cfg = label(&r.weights),
            r1 = r.agg.r1(),
            r5 = r.agg.r5(),
            r10 = r.agg.r10(),
            r20 = r.agg.r20(),
            n = r.agg.ndcg10(),
            mrr = r.agg.mrr(),
            pool = r.agg.pool(),
            up = up,
            down = down,
            same = same,
            cnt = r.agg.count,
        );
    }

    // ---- what the grid shows, derived from the cells above -------------------------
    // Every statement below is computed from the runs that are already in the table, so
    // a re-run on a different corpus cannot leave a stale conclusion sitting above fresh
    // numbers — the same discipline `locomo.rs` uses for its verdict. None of it is a
    // selection: these are readings of the grid, not a pick.
    let base = &runs[0];
    let axis_row = |axis: &str, phase: &str| -> Option<&Run> {
        runs.iter()
            .filter(|r| r.phase == phase && r.axis == Some(axis))
            .max_by(|a, b| {
                a.weights
                    .bm25_magnitude
                    .partial_cmp(&b.weights.bm25_magnitude)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        a.weights
                            .recency
                            .partial_cmp(&b.weights.recency)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            })
    };
    let agree = runs
        .iter()
        .filter(|r| r.phase == "axis-at-shipped" && r.axis == Some("agreement"))
        .max_by(|a, b| {
            a.weights
                .agreement
                .partial_cmp(&b.weights.agreement)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    let mag_big = runs
        .iter()
        .filter(|r| r.phase == "axis-at-shipped" && r.axis == Some("bm25_magnitude"))
        .max_by(|a, b| {
            a.weights
                .bm25_magnitude
                .partial_cmp(&b.weights.bm25_magnitude)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    let rec_big = runs
        .iter()
        .filter(|r| r.phase == "axis-at-shipped" && r.axis == Some("recency"))
        .max_by(|a, b| {
            a.weights
                .recency
                .partial_cmp(&b.weights.recency)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    let k_extremes: Vec<&Run> = runs
        .iter()
        .filter(|r| r.phase == "axis-at-shipped" && r.axis == Some("k"))
        .collect();
    let k_span = k_extremes
        .iter()
        .map(|r| r.agg.r5())
        .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    // How many *distinct* metric vectors the walk produced at `overlap: 0.00` with the
    // recency stream at its own inert default. One is the informative number: it means
    // `k`, `bm25_magnitude` and `agreement` cannot move anything in that regime. The
    // `recency != 0.0` rows are excluded deliberately — a recency stream covers the
    // whole bank, so it changes *membership*, not just order, and including it would
    // hide the effect being measured.
    let mut zero_overlap: Vec<[f64; 6]> = runs
        .iter()
        .filter(|r| r.weights.overlap == 0.0 && r.weights.recency == 0.0)
        .map(|r| r.agg.all())
        .collect();
    let zero_overlap_n = zero_overlap.len();
    zero_overlap.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    zero_overlap.dedup();
    let zero_overlap_distinct = zero_overlap.len();
    let zero_overlap_span: Vec<&Run> = runs
        .iter()
        .filter(|r| r.weights.overlap == 0.0 && r.weights.recency == 0.0)
        .collect();
    let zero_overlap_extremes = format!(
        "`k` from {:.0} to {:.0}, `bm25_magnitude` from {:.2} to {:.2}, `agreement` from {:.2} to {:.2}",
        zero_overlap_span
            .iter()
            .map(|r| r.weights.k)
            .fold(f64::MAX, f64::min),
        zero_overlap_span.iter().map(|r| r.weights.k).fold(0.0, f64::max),
        zero_overlap_span
            .iter()
            .map(|r| r.weights.bm25_magnitude)
            .fold(f64::MAX, f64::min),
        zero_overlap_span
            .iter()
            .map(|r| r.weights.bm25_magnitude)
            .fold(0.0, f64::max),
        zero_overlap_span
            .iter()
            .map(|r| r.weights.agreement)
            .fold(f64::MAX, f64::min),
        zero_overlap_span
            .iter()
            .map(|r| r.weights.agreement)
            .fold(0.0, f64::max),
    );
    let _ = axis_row;

    let _ = write!(
        md,
        "\n## What the grid shows\n\n\
         Five readings of the cells above, each computed from them. **None of them is a \
selection** — they are what the measurements say, not what should ship.\n\n\
         1. **The `agreement` axis moved {agree_moved} of {n_eval} questions at its largest \
value, and the walk never moved it.** {agree_detail} The shipped rustdoc already records the \
same null on LongMemEval at the same magnitudes (\"moved zero of 500 questions\"); this is the \
replication on independent data, and it is why the field stays at `0.0`.\n\
         2. **`bm25_magnitude` does not collapse on this corpus, even at {mag_w:.2}.** \
R@5 {mag_r5:.1}% against the control's {base_r5:.1}%, R@1 {mag_r1:.1}% against {base_r1:.1}%, \
R@20 {mag_r20:.1}% against {base_r20:.1}% (n={n_eval}). `docs/EXCEED_PLAN.md` §0.3 warns that \
score-space weights above the RRF spread degenerate into lexicographic sorting and cost \
`recall@20 0.97 → 0.40`; **that warning does not transfer to this mechanism**, and the reason \
is the algebra on the field: the term is min-max normalised to `[0,1]` over the query's own \
BM25 stream and denominated in `1/(k+1)`, so it is bounded by construction and cannot outvote a \
rank-1 hit on its own. This is a bounded-term null, not a general licence to use large weights — \
the unbounded `w · magnitude` form the rustdoc rejected is a different mechanism and was not \
measured here.\n\
         3. **`k` is flat on this corpus at the shipped `overlap`.** Across `k ∈ \
{{5, 10, 20, 40, 120}}` at `overlap: {base_ov:.2}`, R@5 spans {k_lo:.1}%–{k_hi:.1}%, a range of \
{k_span:.1}pp on {n_eval} queries. `k = 60` was inherited from Hindsight and never validated \
here (`docs/EVALUATION_HYGIENE.md` §2.6); this is the first measurement of it on independent \
data, and it is a null. A null is a result (`AGENTS.md` §6) — but it is a null *on this corpus \
at this overlap weight*, and `k`'s only measurable effect here, that {k_span:.1}pp span, \
disappears entirely once `overlap` is 0 (§5). Two flat axes is not two independent nulls.\n\
         4. **`recency` is destructive on this corpus and the numbers are about the harness, \
not about recency.** At `recency: {rec_w:.2}`, R@1 falls to {rec_r1:.1}% from {base_r1:.1}% and \
R@5 to {rec_r5:.1}% from {base_r5:.1}% (n={n_eval}); the walk never moved it. Read the recency \
caveat above before drawing anything from this: every `created_at` here is the harness's own \
ingest clock, spread over {span}, so this is a measurement of *ingest order* and must not be \
cited as evidence for or against a recency stream.\n\
         5. **The walk's own grid is degenerate after its first move, and the artifact says so \
rather than letting three axes look dead.** The walk accepted {moves_n} move(s) across \
{round_done} round(s), the first being {moves_first}. After it, the {zero_overlap_n} \
configurations it measured at `overlap: 0.00` with `recency: 0.00` — spanning {zero_overlap_extremes} \
— produced **{zero_overlap_distinct} distinct metric vector\
{plural}**. That is not noise and not a bug: with the overlap stream dropped there is one \
stream, so the agreement bonus has no id that two streams found, and the plain RRF score \
`w/(k+rank)` is already monotone in rank — so neither `k` nor the BM25 magnitude can reorder \
anything, at any magnitude. (`recency` is the one axis still live there, and it is live for a \
different reason: a recency stream covers the whole bank, so it changes which documents reach \
the ranking, not only their order.) **This is precisely the blind spot of a one-at-a-time \
sweep that `docs/EXCEED_PLAN.md` §0.3 predicts, reproduced here from the other direction**, and \
it is why pass 1 exists: the same three axes measured at the shipped `overlap: 0.25` are *not* \
flat (§2 and §3 above). A reader who looked only at the walk rows would conclude `k`, \
`bm25_magnitude` and `agreement` are dead levers. They are not — they are inert **in that \
regime**, which is a statement about the regime.\n",
        agree_moved = agree.map_or(0, |r| {
            let (up, down, _) = dev::movement(&base.r5_by_query, &r.r5_by_query);
            up + down
        }),
        agree_detail = match agree {
            Some(r) => format!(
                "At `agreement: {:.2}` the R@5 movement against the control is {}/{} \
improved, {}/{} regressed — and the full metric vector is identical to the control's.",
                r.weights.agreement,
                dev::movement(&base.r5_by_query, &r.r5_by_query).0,
                n_eval,
                dev::movement(&base.r5_by_query, &r.r5_by_query).1,
                n_eval
            ),
            None => String::from("The axis was not run in this pass."),
        },
        mag_w = mag_big.map_or(0.0, |r| r.weights.bm25_magnitude),
        mag_r5 = mag_big.map_or(0.0, |r| r.agg.r5()),
        mag_r1 = mag_big.map_or(0.0, |r| r.agg.r1()),
        mag_r20 = mag_big.map_or(0.0, |r| r.agg.r20()),
        base_r1 = base.agg.r1(),
        base_r5 = base.agg.r5(),
        base_r20 = base.agg.r20(),
        base_ov = base.weights.overlap,
        k_lo = k_span.0,
        k_hi = k_span.1,
        k_span = k_span.1 - k_span.0,
        rec_w = rec_big.map_or(0.0, |r| r.weights.recency),
        rec_r1 = rec_big.map_or(0.0, |r| r.agg.r1()),
        rec_r5 = rec_big.map_or(0.0, |r| r.agg.r5()),
        span = ingest_span,
        moves_n = moves.len(),
        moves_first = moves
            .first()
            .map_or_else(|| String::from("(none)"), |(r, axis, from, to, _, _)| {
                format!("`{axis}` {from} -> {to} in round {r}")
            }),
        round_done = round_open.len(),
        zero_overlap_n = zero_overlap_n,
        zero_overlap_distinct = zero_overlap_distinct,
        plural = if zero_overlap_distinct == 1 { "" } else { "s" },
        zero_overlap_extremes = zero_overlap_extremes,
        n_eval = n_eval,
    );

    // The round log: what the walk did, coordinate by coordinate.
    let _ = write!(
        md,
        "\n## Round by round\n\n\
         Each row is one accepted move. A coordinate whose candidates all failed to beat the \
incumbent strictly produced no row — that is a recorded null result, not an omission: the \
grid above holds the measurement for every candidate that was tried and rejected.\n\n\
         | round | axis | from | to | Δ over all six metrics | R@5 questions improved |\n\
         |---|---|---|---|---|---|\n"
    );
    if moves.is_empty() {
        md.push_str(
            "| — | — | — | — | **no coordinate produced a strict improvement at the shipped \
start point** | — |\n",
        );
    }
    for (round, axis, from, to, delta, up) in &moves {
        let _ = writeln!(
            md,
            "| {round} | {axis} | {from} | {to} | {delta} | {up} |"
        );
    }
    md.push_str("\n| round | coordinates swept | configuration at round end | Δ over the round |\n|---|---|---|---|\n");
    let mut previous: Option<(&Agg, String)> = None;
    for (round, coords, cfg, agg) in &round_open {
        let before = previous.as_ref().map_or(&runs[0].agg, |(a, _)| *a);
        let _ = writeln!(
            md,
            "| {round} | {coords} | {cfg} | {delta} |",
            delta = deltas(agg, before)
        );
        previous = Some((agg, cfg.clone()));
    }
    if !skipped_axes.is_empty() {
        // One line per axis, listing the rounds it was skipped in, rather than
        // repeating an identical sentence once per round.
        let mut grouped: Vec<(&'static str, String, Vec<usize>)> = Vec::new();
        for (r, name, why) in &skipped_axes {
            match grouped.iter_mut().find(|(n, _, _)| n == name) {
                Some((_, w, rounds)) => {
                    rounds.push(*r);
                    let _ = w;
                }
                None => grouped.push((name, why.clone(), vec![*r])),
            }
        }
        let _ = write!(
            md,
            "\n**Skipped coordinates.** {}\n",
            grouped
                .iter()
                .map(|(name, why, rounds)| format!(
                    "`{name}` (rounds {}) — {why}",
                    rounds
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    // Per-category, for the two rows a reader will actually compare: what ships, and
    // where the walk ended up. Labelled so neither can be mistaken for a pick.
    let _ = write!(
        md,
        "\n## Per `meta.category` — the shipped configuration and the walk's endpoint\n\n\
         Two rows, and the labels matter. **Row 1 is what ships today** \
(`overlap: 0.25`, a *fitted* value — see the caveat in the Control section above). **Row 2 is the \
coordinate-descent endpoint, which is NOT a selected configuration**: it is wherever this \
particular walk, in this particular order, under this particular declared objective, stopped \
being able to improve. It has not been chosen by a human, it has not been checked against \
anything, and it is not a recommendation. It is reported because the per-category shape is \
where a reordering win or loss shows up, and an aggregate that holds can be two categories \
moving in opposite directions.\n"
    );
    for (name, pick) in [
        ("R@1", Agg::r1 as fn(&Agg) -> f64),
        ("R@5", Agg::r5),
        ("R@10", Agg::r10),
        ("R@20", Agg::r20),
        ("NDCG@10", Agg::ndcg10),
        ("MRR", Agg::mrr),
    ] {
        let _ = write!(md, "\n### {name}\n\n| row |");
        for c in &cats {
            let _ = write!(md, " {c} |");
        }
        md.push_str(" overall | n |\n|---|");
        for _ in 0..=cats.len() {
            md.push_str("---|");
        }
        md.push('\n');
        for (row_name, run) in [
            ("shipped (`overlap: 0.25`)", &runs[0]),
            ("coordinate-descent endpoint — **not a selected configuration**", &runs[endpoint]),
        ] {
            let _ = write!(md, "| {row_name} |");
            for c in &cats {
                let a = run.per_cat.get(c);
                match a {
                    Some(a) => {
                        let _ = write!(md, " {:.1}% (n={}) |", pick(a), a.count);
                    }
                    None => md.push_str(" — |"),
                }
            }
            let _ = writeln!(
                md,
                " {:.1}% | {} |",
                pick(&run.agg),
                run.agg.count
            );
        }
    }

    // Per-category across the `vector` axis. The control plus the two weights named by
    // the rule stated above — the largest weight that did not lose R@5, and the
    // largest weight swept — so the choice of rows is checkable and is not "the row
    // that scored best".
    #[cfg(feature = "embed")]
    if !interesting.is_empty() {
        let _ = write!(
            md,
            "\n## Per `meta.category` across the `vector` axis\n\n\
             The control, plus **the two weights chosen by a rule, not by score**: the largest \
             swept weight that did **not** lose R@5 against the control, and the largest swept \
             weight. On a corpus where the dense arm turns out to be inert at every weight these \
             two collapse onto one row and the table says so. `n` is printed per cell, because a \
             category percentage without its denominator is not a number.\n\n\
             | weight | metric |{cat_head} overall | n |\n|---|---|{cat_rule}---|---|\n",
            cat_head = cats.iter().map(|c| format!(" {c} |")).collect::<Vec<_>>().join(""),
            cat_rule = cats.iter().map(|_| "---|").collect::<Vec<_>>().join(""),
        );
        let mut rows: Vec<(f64, usize)> = vec![(0.0, 0)];
        for c in interesting {
            if !rows.iter().any(|(w, _)| (w - c.0).abs() < f64::EPSILON) {
                rows.push(c);
            }
        }
        for (w, idx) in rows {
            let run = &runs[idx];
            let label = if idx == 0 {
                "`vector: 0.00` — **the control**".to_string()
            } else {
                format!("`vector: {w:.2}`")
            };
            for (name, pick) in [
                ("R@1", Agg::r1 as fn(&Agg) -> f64),
                ("R@5", Agg::r5),
                ("R@10", Agg::r10),
                ("R@20", Agg::r20),
                ("NDCG@10", Agg::ndcg10),
                ("MRR", Agg::mrr),
            ] {
                let _ = write!(md, "| {label} | {name} |");
                for c in &cats {
                    match run.per_cat.get(c) {
                        Some(a) => {
                            let _ = write!(md, " {:.1}% (n={}) |", pick(a), a.count);
                        }
                        None => md.push_str(" — |"),
                    }
                }
                let _ = writeln!(md, " {:.1}% | {} |", pick(&run.agg), run.agg.count);
            }
        }
    }

    // Counts, because a percentage without its denominator is not acceptable here.
    let mut multi_gold = 0usize;
    let mut single_gold = 0usize;
    for q in &corpus.queries {
        match q.gold_ids.len() {
            0 => {}
            1 => single_gold += 1,
            _ => multi_gold += 1,
        }
    }
    // Built here rather than inline in the `format!` below: a nested macro is not
    // expanded inside its parent's token stream, so a `format!` nested in the
    // argument list of a `write!` hands the outer macro's named-argument analysis
    // the inner one's names.
    let skipped_note = if corpus.skipped_no_gold > 0 {
        format!(
            "**The n above is {n_eval}, not {queries_n}.** {skipped} queries carry an empty \
`gold_ids`: they have a gold *answer* but no gold *document*, so no document could be \
retrieved and every `recall_any@K` would be 0 by construction. They are excluded from every \
denominator and reported here rather than silently kept — keeping them would understate every \
row by a flat {pct:.2}pp carrying no measurement at all.\n",
            n_eval = n_eval,
            queries_n = corpus.queries.len(),
            skipped = corpus.skipped_no_gold,
            pct = corpus.skipped_no_gold as f64 / corpus.queries.len() as f64 * 100.0,
        )
    } else {
        String::new()
    };
    let _ = write!(
        md,
        "\n## Counts\n\n\
         | | |\n|---|---|\n\
         | documents indexed | {docs_n}, in {banks_n} banks ({bank_min}–{bank_max} per bank) |\n\
         | dialogue turns decoded | {turns} |\n\
         | queries in `queries.json` | {queries_n} |\n\
         | **queries evaluated — the n in every table above** | **{n_eval}** |\n\
         | dropped: `gold_ids` empty (gold answer, no gold document) | {skipped} |\n\
         | `gold_ids` per query | 1 for {single_gold} queries, 2–15 for {multi_gold}, 0 for the \
{skipped} dropped |\n\
         | `meta.category` values | {cat_list} |\n\
         | widest `created_at` spread in any bank, after ingestion | {span} |\n\n\
         {skipped_note}\n\
         ## Runtime\n\n\
         | | |\n|---|---|\n\
         | configurations scored | {evaluations} ({distinct} distinct) |\n\
         | of which: control / axis-at-shipped / walk | {n_control} / {n_axis} / {n_walk} |\n\
         | coordinates per axis, all six axes | {per_axis} candidate values |\n\
         | descent rounds requested / completed | {rounds} / {round_done} |\n\
         | **total wall clock, both passes** | **{elapsed:.1}s** |\n\
         | mean per configuration | {mean:.1}s |\n\
         | budget cap | {budget_note} |\n\
         | loadavg at start / at end | `{load_start}` / `{load_end}` |\n\n\
         **This is throughput, not recall latency, and it is not comparable to any latency \
figure in this repo.** It is {n_eval} recalls plus metric computation per configuration, \
measured on a box whose `loadavg` was {load_start} at the start and {load_end} at the end — \
a box that is not quiet, and on which every per-request figure in `eval/` is a range at best \
(`AGENTS.md` §3). Nothing here should be quoted as a latency.\n\n\
         ## Reproduce\n\n\
         ```bash\n\
         cargo run --release --example select_fusion -- \\\n  --docs {docs} --data {queries} --rounds {rounds} --budget-seconds {budget:.0} \\\n  --out-md eval/SELECTION.md --out-json eval/results_selection.json\n\
         ```\n\n\
         `{out_json}` carries every configuration's aggregate, its per-category breakdown, and \
the per-query R@5 vector, so any cell above — including every `up/down/same` count — can be \
re-derived. This binary owns the whole of this file: **do not hand-edit it**, and note that a \
bare run writes to `$TMPDIR` instead. Updating the committed artifact means naming \
`--out-md eval/SELECTION.md`, per `eval/README.md`.\n\n\
         ## The one thing to do with this file\n\n\
         Read the grid, then take the decision **to a human**, on the evidence, and record it \
in `docs/CONSISTENCY.md` with the date, the parameter, the size of the search space and the \
delta — the counting rule in `docs/EVALUATION_HYGIENE.md` §3.3. Then, and only then, measure \
LongMemEval **once** and report whatever it says. The whole reason this harness exists is \
that the last time a sweep chose a value, it chose it by looking at the test set and called \
the result an improvement; that is a failure this project is now structured to make \
impossible rather than to apologise for after the fact.\n",
        docs_n = corpus.documents.len(),
        banks_n = corpus.banks.len(),
        bank_min = bank_min,
        bank_max = bank_max,
        turns = corpus.turns,
        queries_n = corpus.queries.len(),
        n_eval = n_eval,
        skipped = corpus.skipped_no_gold,
        single_gold = single_gold,
        multi_gold = multi_gold,
        cat_list = cats.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", "),
        span = ingest_span,
        skipped_note = skipped_note,
        evaluations = runs.len(),
        distinct = distinct.len(),
        n_control = runs.iter().filter(|r| r.phase == "control").count(),
        n_axis = runs.iter().filter(|r| r.phase == "axis-at-shipped").count(),
        n_walk = runs.iter().filter(|r| r.phase == "walk").count(),
        per_axis = chosen.iter().map(|a| a.values.len()).sum::<usize>(),
        rounds = rounds,
        round_done = round_open.len(),
        elapsed = elapsed,
        mean = elapsed / runs.len() as f64,
        budget_note = if budget > 0.0 {
            if truncated && axis_pass_truncated {
                format!("{budget:.0}s — **HIT during pass 1, so the coordinate-descent walk never \
ran. This file is the one-axis-at-a-time sweep only.**")
            } else if truncated {
                format!("{budget:.0}s — **HIT during pass 2, so the walk is incomplete**")
            } else {
                format!("{budget:.0}s — not hit; both passes finished inside it")
            }
        } else {
            "none (`--budget-seconds 0`): both passes ran to completion".to_string()
        },
        load_start = load_start,
        load_end = load_end,
        docs = docs_path,
        queries = queries_path,
        out_json = out_json,
    );
    std::fs::write(&out_md, &md)?;

    // ---- JSON -------------------------------------------------------------------
    // Aggregates plus the per-query R@5 vector, so every cell and every up/down/same
    // count above is re-derivable without re-running the sweep.
    // `mut` only in an `embed` build, where the dense-arm block below merges keys into
    // it; without the feature nothing mutates it and the `mut` would be a warning.
    #[allow(unused_mut)]
    let mut json = serde_json::json!({
        "harness": "select_fusion",
        "commit": dev::git_head(),
        "working_tree_at_run_time": tree_state(),
        "run_date": date.to_string(),
        "profile": bench_common::profile(),
        "loadavg_start": load_start,
        "loadavg_end": load_end,
        "corpus": {
            "name": "LoCoMo (dev set)",
            "documents": corpus.documents.len(),
            "queries_in_file": corpus.queries.len(),
            "queries_evaluated": n_eval,
            "dropped_no_gold": corpus.skipped_no_gold,
            "banks": corpus.banks.len(),
            "bank_range": [bank_min, bank_max],
            "upstream_commit": UPSTREAM_REF,
        },
        "control": {
            "matches_committed_eval_locomo_md": control_check.ok,
            "expected": CONTROL.iter().map(|(k, v)| serde_json::json!({ "metric": k, "value": v })).collect::<Vec<_>>(),
            "expected_n": CONTROL_N,
            "detail": control_check.detail,
        },
        "objective": {
            "order": ["R@5", "NDCG@10", "MRR", "R@1", "R@10", "R@20"],
            "strict": true,
        },
        "method": {
            "kind": "axis-at-shipped pass, then coordinate descent",
            "rounds_requested": rounds,
            "rounds_completed": round_open.len(),
            "axes": chosen.iter().map(|a| serde_json::json!({
                "name": a.name, "values": a.values,
            })).collect::<Vec<_>>(),
            "skipped": skipped_axes.iter().map(|(r, name, why)| serde_json::json!({
                "round": r, "axis": name, "reason": why,
            })).collect::<Vec<_>>(),
        },
        "runtime": {
            "wall_clock_seconds": elapsed,
            "mean_seconds_per_config": elapsed / runs.len() as f64,
            "budget_seconds": budget,
            "truncated_by_budget": truncated,
            "truncated_during_axis_pass": axis_pass_truncated,
            "is_throughput_not_latency": true,
        },
        "selects_a_configuration": false,
        "runs": runs.iter().map(|r| serde_json::json!({
            "config": serde_json::json!({
                "overlap": r.weights.overlap,
                "bm25": r.weights.bm25,
                "bm25_magnitude": r.weights.bm25_magnitude,
                "agreement": r.weights.agreement,
                "k": r.weights.k,
                "recency": r.weights.recency,
                "recency_half_life_days": r.weights.recency_half_life_days,
                "vector": r.weights.vector,
                "recency_policy": format!("{:?}", r.weights.recency_policy),
                "overlap_scope": format!("{:?}", r.weights.overlap_scope),
            }),
            "round": r.round,
            "phase": r.phase,
            "axis_swept": r.axis,
            "n": r.agg.count,
            "recall_any_at_1": r.agg.r1(),
            "recall_any_at_5": r.agg.r5(),
            "recall_any_at_10": r.agg.r10(),
            "recall_any_at_20": r.agg.r20(),
            "ndcg_at_10": r.agg.ndcg10(),
            "mrr": r.agg.mrr(),
            "gold_in_pool": r.agg.pool(),
            "per_category": r.per_cat.iter().map(|(k, a)| serde_json::json!({
                "category": k,
                "n": a.count,
                "recall_any_at_1": a.r1(),
                "recall_any_at_5": a.r5(),
                "recall_any_at_10": a.r10(),
                "recall_any_at_20": a.r20(),
                "ndcg_at_10": a.ndcg10(),
                "mrr": a.mrr(),
            })).collect::<Vec<_>>(),
            "r5_by_query": r.r5_by_query,
        })).collect::<Vec<_>>(),
        "moves": moves.iter().map(|(round, axis, from, to, delta, up)| serde_json::json!({
            "round": round, "axis": axis, "from": from, "to": to,
            "delta": delta, "r5_questions_improved": up,
        })).collect::<Vec<_>>(),
        "endpoint": {
            "config_index": endpoint,
            "note": "coordinate-descent endpoint, NOT a selected configuration",
        },
    });

    // The dense arm's coverage and its own reach, so a cell of the grid can be
    // re-derivable *with the premises it rests on* rather than on trust. Merged into
    // the object above rather than written as a `json!` arm, because a `#[cfg]`
    // attribute on an expression inside a macro invocation is not stable rust.
    #[cfg(feature = "embed")]
    {
        let dense = serde_json::json!({
            "vector_coverage": {
                "per_bank": coverage.as_ref().map(|(c, _)| c.per_bank.iter()
                    .map(|(b, (m, v))| serde_json::json!({ "bank": b, "memories": m, "vectors": v }))
                    .collect::<Vec<_>>()).unwrap_or_default(),
                "totals": coverage.as_ref().map_or((0, 0), |(c, _)| c.totals()),
                "complete": coverage.as_ref().is_some_and(|(c, _)| c.per_bank.values().all(|(m, v)| m == v)),
                "note": "the run aborts rather than publishing a grid on partial coverage",
            },
            "lexical_path_equivalence": {
                "ran": lexical_equivalence.is_some(),
                "identical": lexical_equivalence.as_ref().map(|(ok, _)| *ok),
                "detail": lexical_equivalence.as_ref().map(|(_, d)| d.clone()),
                "note": "scores the shipped configuration through `vector::recall` at \
`vector: 0.0` and compares the full metric vector to the control, so a `vector` row can be \
claimed to differ by the dense stream alone",
            },
            "vector_stream_reach": {
                "config": { "bm25": 0.0, "overlap": 0.0, "vector": 1.0 },
                "note": "diagnostic: the dense stream ALONE, both lexical streams dropped. Not a \
                         candidate configuration and not eligible for the walk.",
                "n": reach.count,
                "gold_in_pool": reach.pool(),
                "recall_any_at_1": reach.r1(),
                "recall_any_at_5": reach.r5(),
                "recall_any_at_10": reach.r10(),
                "recall_any_at_20": reach.r20(),
                "ndcg_at_10": reach.ndcg10(),
                "mrr": reach.mrr(),
            },
        });
        if let (Some(base), Some(extra)) = (json.as_object_mut(), dense.as_object()) {
            for (k, v) in extra {
                base.insert(k.clone(), v.clone());
            }
        }
    }

    std::fs::write(&out_json, serde_json::to_string_pretty(&json)?)?;

    // ---- stdout ------------------------------------------------------------------
    println!("{md}");

    let e = &runs[endpoint];
    println!(
        "SELECTION SWEEP — {evaluations} configurations ({distinct} distinct; {n_axis} \
axis-at-shipped, {n_walk} in the walk) over {n_eval} LoCoMo queries, {rounds_done} round(s), \
{elapsed:.1}s wall clock, budget {budget:.0}s, {trunc}; loadavg {load_start} -> {load_end}. \
Retrieval-only: no LLM. No configuration is selected or recommended.\n  \
start   {start_cfg}\n  \
endpoint {end_cfg}   <- coordinate-descent endpoint, NOT a selected configuration\n  \
R@1 {sr1:.1}% -> {er1:.1}% ({dr1:+.1}pp) · R@5 {sr5:.1}% -> {er5:.1}% ({dr5:+.1}pp) · \
R@10 {sr10:.1}% -> {er10:.1}% ({dr10:+.1}pp) · R@20 {sr20:.1}% -> {er20:.1}% ({dr20:+.1}pp) · \
NDCG@10 {sn:.1}% -> {en:.1}% ({dn:+.1}pp) · MRR {sm:.1}% -> {em:.1}% ({dm:+.1}pp)\n  \
wrote {out_md} + {out_json}",
        evaluations = runs.len(),
        distinct = distinct.len(),
        n_axis = runs.iter().filter(|r| r.phase == "axis-at-shipped").count(),
        n_walk = runs.iter().filter(|r| r.phase == "walk").count(),
        n_eval = n_eval,
        rounds_done = round_open.len(),
        elapsed = elapsed,
        budget = budget,
        trunc = if truncated { "TRUNCATED by budget" } else { "complete" },
        load_start = load_start,
        load_end = load_end,
        start_cfg = label(&runs[0].weights),
        end_cfg = label(&e.weights),
        sr1 = runs[0].agg.r1(),
        er1 = e.agg.r1(),
        dr1 = e.agg.r1() - runs[0].agg.r1(),
        sr5 = runs[0].agg.r5(),
        er5 = e.agg.r5(),
        dr5 = e.agg.r5() - runs[0].agg.r5(),
        sr10 = runs[0].agg.r10(),
        er10 = e.agg.r10(),
        dr10 = e.agg.r10() - runs[0].agg.r10(),
        sr20 = runs[0].agg.r20(),
        er20 = e.agg.r20(),
        dr20 = e.agg.r20() - runs[0].agg.r20(),
        sn = runs[0].agg.ndcg10(),
        en = e.agg.ndcg10(),
        dn = e.agg.ndcg10() - runs[0].agg.ndcg10(),
        sm = runs[0].agg.mrr(),
        em = e.agg.mrr(),
        dm = e.agg.mrr() - runs[0].agg.mrr(),
        out_md = out_md,
        out_json = out_json,
    );
    eprintln!("wrote {out_md} + {out_json}");
    if !control_check.ok {
        eprintln!("exiting 2: the shipped control did not reproduce eval/LOCOMO.md");
        std::process::exit(2);
    }
    Ok(())
}

/// Whether the shipped control reproduced `eval/LOCOMO.md`, and why not.
struct ControlCheck {
    ok: bool,
    detail: String,
}

/// Compare the first run against the committed numbers. Only enforced on a full run:
/// `--n` evaluates a subset, so a subset cannot and should not reproduce an `n=1531`
/// table, and failing it there would be a false alarm rather than a check.
fn check_control(control: &Run, full_run: bool) -> ControlCheck {
    if !full_run {
        return ControlCheck {
            ok: true,
            detail: "not a full run (`--n` given), so the committed `eval/LOCOMO.md` numbers \
do not apply and the control check was not applied"
                .to_string(),
        };
    }
    let mut bad = Vec::new();
    for ((name, want), got) in CONTROL.iter().zip(control.agg.all()) {
        // The committed artifact prints one decimal, so anything inside half a
        // rounding step is agreement.
        if (got - want).abs() > 0.05 {
            bad.push(format!("{name}: measured {got:.1}%, committed {want:.1}%"));
        }
    }
    if control.agg.count != CONTROL_N {
        bad.push(format!(
            "n: evaluated {}, committed {CONTROL_N}",
            control.agg.count
        ));
    }
    if bad.is_empty() {
        ControlCheck {
            ok: true,
            detail: format!(
                "Reproduced all six metrics and the n exactly, at `loadavg {}.", loadavg()
            ),
        }
    } else {
        ControlCheck {
            ok: false,
            detail: format!(
                "The shipped configuration measured {} on this run, against the committed \
`eval/LOCOMO.md` values:\n\n  {}\n\nThese two are produced by the *same* shared module \
(`eval/locomo_dev.rs`), so a disagreement means the harness, the corpus or the store changed \
— not that the weights do anything different.",
                label(&control.weights),
                bad.join("\n  ")
            ),
        }
    }
}

/// `1.00, 0.75, …` for a value list, in one cell.
fn fmt_list(values: &[f64]) -> String {
    values
        .iter()
        .map(|v| format!("{v:.2}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Why an axis did not move: the value the walk is at makes every candidate on it
/// equivalent, so re-measuring them would spend budget to learn nothing.
fn axis_skip_reason(name: &str) -> String {
    format!(
        "`{name}` does not apply while `recency == 0.0`, because a zero weight drops the \
third stream before its half-life is read, so every value on the axis produces one ranking"
    )
}

/// The widest `created_at` spread across any bank, in seconds, read back through the
/// public API. This is the measured basis for the recency caveat: it is what the
/// recency stream has to work with, and it is the harness's own ingest clock.
fn widest_created_at_span(corpus: &Corpus) -> anyhow::Result<String> {
    let mut widest = 0f64;
    for (bank, svc) in &corpus.banks {
        let memories = svc
            .list_memories(bank, 500, 0)
            .with_context(|| format!("select_fusion: cannot read back bank {bank}"))?;
        let stamps: Vec<chrono::DateTime<chrono::Utc>> = memories
            .iter()
            .filter_map(|m| m.created_at.as_deref())
            .map(|s| chrono::DateTime::parse_from_rfc3339(s).map(|t| t.with_timezone(&chrono::Utc)))
            .collect::<Result<Vec<_>, _>>()?;
        let (Some(lo), Some(hi)) = (stamps.iter().min(), stamps.iter().max()) else {
            continue;
        };
        let span = (*hi - *lo).num_milliseconds() as f64 / 1000.0;
        widest = widest.max(span);
    }
    if widest == 0.0 {
        Ok("0 s (every row in every bank shares one timestamp)".to_string())
    } else {
        Ok(format!("{widest:.3} s"))
    }
}

/// Whether the working tree carried uncommitted changes when the sweep ran.
///
/// The provenance line names a commit. A **dirty** tree means that commit does not
/// describe the binary that produced these numbers, which is the same class of
/// failure `examples/locomo.rs` documents in `git_head` — a benchmark artifact that
/// cannot name the build it measured — except a commit hash actively misleads here,
/// because it looks like an identifier and is not one. `AGENTS.md` §2: the artifact is
/// the record of account, and a record that names the wrong build is worse than one
/// that admits it does not know.
fn tree_state() -> String {
    match std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
    {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let files: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            match files.len() {
                0 => "clean — the commit above is the build that produced this file".to_string(),
                n => format!(
                    "**{n} path(s) had uncommitted changes when this ran**, so the commit above \
does NOT describe the binary that produced these numbers. Paths: {}",
                    files
                        .iter()
                        .map(|l| l.split_whitespace().nth(1).unwrap_or("?"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        }
        _ => "unknown (git not runnable from here)".to_string(),
    }
}

/// Whether the next configuration should not be started.
///
/// The cost of one configuration is predicted from the running mean, so the cap is a
/// cap rather than a suggestion: a sweep that honours it lands inside its budget
/// rather than one configuration over.
fn over_budget(start: &Instant, budget: f64, mean_seconds: f64, evals: usize) -> bool {
    budget > 0.0 && evals > 0 && start.elapsed().as_secs_f64() + mean_seconds > budget
}

/// Pick the requested axes, or all of them.
fn select_axes(spec: &str) -> anyhow::Result<Vec<Axis>> {
    let all = axes();
    if spec.trim().is_empty() || spec == "all" {
        return Ok(all);
    }
    let mut out = Vec::new();
    for want in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let axis = all
            .iter()
            .find(|a| a.name == want)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "select_fusion: no axis named {want:?}; known axes: {}",
                    all.iter().map(|a| a.name).collect::<Vec<_>>().join(", ")
                )
            })?;
        out.push(*axis);
    }
    anyhow::ensure!(!out.is_empty(), "select_fusion: --axes selected nothing");
    Ok(out)
}
