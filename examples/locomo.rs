//! LoCoMo retrieval harness — the **dev set**, and the answer to whether the
//! shipped `overlap: 0.25` is a discovery or an artifact of 500 test questions.
//!
//! ## Why this exists
//!
//! `docs/EVALUATION_HYGIENE.md` §2.1: `overlap: 0.25` was chosen by sweeping 46
//! configurations against LongMemEval-S's 500 questions. That makes it
//! **fitted**, which makes every published number measured under it a
//! non-generalisation estimate, and which makes `AGENTS.md` §1 ("the evaluation
//! set is not a training set") a rule this project had already broken. LoCoMo is
//! an independent labelled corpus from an entirely different source
//! (Snap Research, via the canonical `vectorize-io/agent-memory-benchmark`
//! distribution), so it is the only set available that can settle the question.
//!
//! **The first job of this harness is measurement, not selection.** It answers
//! one question — does `0.25` beat the unfitted `1.00` on data that never chose
//! it — and the grid exists to put both on one table, not to find a winner. A
//! weight that wins here is a *replication*, not a licence to pick a new value:
//! picking one off this table is selection on the dev set, which is what the dev
//! set is for, but it has to be a deliberate act recorded as one and never
//! checked against LongMemEval.
//!
//! ## What is measured, and what is not
//!
//! **Retrieval only. No LLM, no judge, no network call, no answer generation.**
//! `gold_answers` is read for shape and then never used: LoCoMo's standard metric
//! is LLM-judged *answer* accuracy, which this harness cannot compute and does
//! not approximate. `recall_any@K` is a **different measurement** on a
//! **different scale** and is not comparable to any published LoCoMo number — the
//! artifact says so at length, in its own "Comparability" section, because a
//! reader who misses that will assume it is.
//!
//! ## Method
//!
//! One bank per `user_id` (the conversation, and the isolation unit), built once
//! and reused: one memory per LoCoMo document, `id` = document id, content = the
//! document's dialogue turns joined. Each query is answered against its own
//! `user_id`'s bank and against no other. This mirrors the isolation model of
//! `examples/longmemeval.rs`.
//!
//! **One index per conversation, every configuration scored on it** — the same
//! structure as `examples/sweep_fusion.rs`, and for the same reason: a
//! byte-identical corpus under every row, so two rows cannot disagree because
//! one's index happened to build differently.
//!
//! Run: `cargo run --release --example locomo --
//!        --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json`
//!
//! **A bare run writes to `$TMPDIR`, never to `eval/`** — see `bench_common::out_md`.
//! This binary owns the whole of `eval/LOCOMO.md`, and a benchmark run must not be
//! able to silently replace a reviewed artifact; that has already destroyed real
//! content in this project twice.

use std::collections::BTreeMap;
use std::fs;

use memory_wire::recall::FusionWeights;

// The `--out-md` policy (a bare run must not be able to overwrite a committed
// `eval/` artifact) and the build-profile name the provenance line needs.
mod bench_common;

// The dev-set core, shared with `examples/select_fusion.rs`: the data shapes, the
// ingestion, the metric definitions and the aggregation. It lives outside `examples/`
// so there is exactly one copy — a second one is how a weight grid and a replication
// check end up describing two different corpora under one name. See its module docs.
#[path = "../eval/locomo_dev.rs"]
mod dev;

use dev::{git_head, loadavg, Agg, Corpus, Row, UPSTREAM_REF};

/// The grid: the weight on the token-overlap stream, with BM25 pinned at 1.0, the
/// agreement bonus at 0.0 and RRF `k` at 60, so `overlap` is the only axis that
/// moves. Row 0 is the **unfitted baseline** — equal weight, the configuration
/// every published number was measured under and the only clean comparison
/// `docs/EVALUATION_HYGIENE.md` §2.2 has. It is pinned here rather than read from
/// `FusionWeights`, for the reason `sweep_fusion` pins its control: a baseline that
/// moved with the default would stop being a baseline. `0.25` is the fitted value
/// under test. `0.00` is a diagnostic bound (BM25 alone), not a candidate.
const GRID: &[f64] = &[1.00, 0.75, 0.50, 0.25, 0.00];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let load_start = loadavg();
    let docs_path = bench_common::arg(&args, "--docs", "eval/data/locomo/documents.json");
    let queries_path = bench_common::arg(&args, "--data", "eval/data/locomo/queries.json");
    let n: usize = bench_common::arg(&args, "--n", "0").parse().unwrap_or(0);
    let seed: u64 = bench_common::arg(&args, "--seed", "42").parse().unwrap_or(42);
    let out_md = bench_common::out_md(&args, "LOCOMO.md");
    // Temp-defaulted like `sweep_fusion`'s: a grid's per-question dump is not this
    // suite's reviewable record, and two harnesses defaulting to one committed
    // path is the clobber waiting to happen.
    let out_json = match bench_common::flag(&args, "--out-json") {
        Some(p) => p,
        None => {
            let p = std::env::temp_dir().join("results_locomo.json");
            eprintln!("no --out-json given: writing per-question data to {}", p.display());
            p.display().to_string()
        }
    };
    if bench_common::profile() != "release" {
        eprintln!(
            "WARNING: not `--release`. The retrieval metrics are unaffected by profile — there \
             is no LLM and no wall clock in the table — but the artifact's provenance line will \
             say the profile, and it should be `release` to match every other committed artifact."
        );
    }

    // One bank per `user_id`, built once, one memory per document: the shared
    // ingestion, so this harness and `select_fusion` cannot be describing two
    // different corpora. See `eval/locomo_dev.rs`.
    let corpus = Corpus::load(&docs_path, &queries_path)?;
    let all_queries = &corpus.queries;
    let documents = &corpus.documents;
    let banks = &corpus.banks;
    let (bank_min, bank_max) = corpus.bank_range();

    // Shuffled for the progress log only. A query's index is fixed by its `user_id`
    // and every configuration is scored on that one index, so the metrics are
    // seed-independent; the artifact says so rather than let `--seed` imply otherwise.
    let mut order = bench_common::shuffle(all_queries.len(), seed);
    if n > 0 && n < order.len() {
        order.truncate(n);
    }
    let planned = dev::plan(all_queries, order);
    eprintln!(
        "locomo: {} documents in {} banks, {} queries, {} weights (overlap {:?}), seed {seed}; \
         loadavg at start {load_start}",
        documents.len(),
        banks.len(),
        planned.len(),
        GRID.len(),
        GRID
    );

    // ---- the pass ----------------------------------------------------------------
    let mut per_config: Vec<Vec<Row>> =
        GRID.iter().map(|_| Vec::with_capacity(planned.len())).collect();
    let mut per_cat: Vec<BTreeMap<String, Agg>> = GRID.iter().map(|_| BTreeMap::new()).collect();
    // The count of unanswerable queries is a property of the corpus, not of this
    // pass: `plan` dropped them, and the artifact reports how many.
    let skipped_no_gold = corpus.skipped_no_gold;
    let mut blank_answer = 0usize;
    let mut non_string_answer = 0usize;
    // How many documents the fused ranking actually returns per query. Reported,
    // because it is what makes R@20 a mid-list cut rather than a ceiling.
    let (mut pool_len_sum, mut pool_len_max) = (0usize, 0usize);
    let budget_tokens = corpus.budget_tokens;
    let start = std::time::Instant::now();

    for (qi, planned_q) in planned.iter().enumerate() {
        let q = planned_q.query;
        // Shape bookkeeping only. `gold_answers` is never scored — this harness does
        // retrieval, not answer generation — so these two counters exist so the
        // artifact can report what the field actually held instead of asserting it.
        let answers = &q.gold_answers;
        if !answers
            .iter()
            .any(|a| a.as_str().is_some_and(|s| !s.trim().is_empty()))
        {
            blank_answer += 1;
        }
        if answers.iter().any(|a| a.as_str().is_none()) {
            non_string_answer += 1;
        }
        let svc = &banks[&q.user_id];
        for (ci, weight) in GRID.iter().enumerate() {
            let weights = FusionWeights { overlap: *weight, ..FusionWeights::SHIPPED };
            let hits = svc.recall_with_weights(&q.user_id, &q.query, budget_tokens, &weights)?;
            let retrieved: Vec<String> = hits.into_iter().map(|h| h.memory.id).collect();
            let row = Row::new(q, &retrieved, &planned_q.gold);
            // The fused *membership* is weight-independent — the weights reorder the
            // list, they do not change which documents enter it — so this is counted
            // once per query rather than once per weight.
            if ci == 0 {
                pool_len_sum += retrieved.len();
                pool_len_max = pool_len_max.max(retrieved.len());
            }
            per_cat[ci].entry(row.category.clone()).or_default().add(&row);
            per_config[ci].push(row);
        }
        if (qi + 1) % 250 == 0 {
            eprintln!("  {}/{} ...", qi + 1, planned.len());
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    anyhow::ensure!(
        per_config.iter().all(|rows| !rows.is_empty()),
        "locomo: no query with a gold document was evaluated — refusing to publish a 0% table"
    );

    let totals: Vec<Agg> = per_config
        .iter()
        .map(|rows| {
            let mut a = Agg::default();
            for r in rows {
                a.add(r);
            }
            a
        })
        .collect();
    let base_rows = &per_config[0];
    let base = &totals[0];
    // Per-query movement, counted rather than summarised, so a row that wins on the
    // mean by losing half its questions and winning the other half big is visible
    // instead of hidden.
    let diffs: Vec<(usize, usize, usize)> = per_config
        .iter()
        .map(|rows| {
            let (mut up, mut down) = (0, 0);
            for (r, b) in rows.iter().zip(base_rows) {
                match r.r5.partial_cmp(&b.r5).unwrap_or(std::cmp::Ordering::Equal) {
                    std::cmp::Ordering::Greater => up += 1,
                    std::cmp::Ordering::Less => down += 1,
                    std::cmp::Ordering::Equal => {}
                }
            }
            (up, down, rows.len() - up - down)
        })
        .collect();
    let cats: Vec<String> = per_cat[0].keys().cloned().collect();

    // ---- artifact ----------------------------------------------------------------
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let load_end = loadavg();
    let n_eval = base.count;
    let grid_list = GRID
        .iter()
        .map(|w| format!("{w:.2}"))
        .collect::<Vec<_>>()
        .join(", ");
    let cat_list = cats
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let n_turns: usize = corpus.turns;
    let multi_gold = all_queries.iter().filter(|q| q.gold_ids.len() > 1).count();
    let single_gold = all_queries.iter().filter(|q| q.gold_ids.len() == 1).count();
    // The coverage ceiling, read off the equal-weight row's aggregate: R@pool is
    // identical for every weight (the weights reorder the ranking, they do not change
    // which documents reach it), so row 0 is the honest reference.
    let pool_hit = totals[0].pool();
    let pool_miss = ((1.0 - pool_hit / 100.0) * n_eval as f64).round() as usize;
    let pool_miss_pct = 100.0 - pool_hit;
    let pool_mean = if n_eval == 0 {
        0.0
    } else {
        pool_len_sum as f64 / n_eval as f64
    };
    // The measured consequence of the pool number, stated rather than left for the
    // reader to infer. Both branches are written out because the one that fires is a
    // result, not a caveat.
    let pool_verdict = if pool_miss == 0 {
        "**Measured: zero.** Every gold document in every evaluated query shares at least one \
token with its question and reaches the fused ranking, so coverage is not the binding constraint \
on this suite and every delta below is **ordering, not matching** — the same shape of claim \
`eval/RESULTS.md` makes about its 200-row pool. A regression that lost the gold document \
entirely would show up here as a fall in `R@pool`, not as a small drift in R@5."
            .to_string()
    } else {
        format!(
            "**Measured: {pool_miss} of {n_eval}** queries never get their gold document into the \
ranking at any K, so the R@K columns below understate what a perfect reranker could reach by \
exactly that fraction, and every delta should be read under that ceiling. A reranking gain \
cannot recover those {pool_miss} questions; a content or matching change could."
        )
    };
    // The replication verdict, derived from the measured cells rather than written by
    // hand, so a re-run on a different corpus cannot leave a stale conclusion sitting
    // above fresh numbers. `R@pool` is excluded from the test: it is coverage, and the
    // weights cannot change it.
    let fitted = GRID
        .iter()
        .position(|w| (*w - 0.25).abs() < f64::EPSILON)
        .expect("the grid pins the shipped weight, so this row always exists");
    let win = |f: fn(&Agg) -> f64| f(&totals[fitted]) - f(&totals[0]);
    let (d_r1, d_r5, d_r10, d_ndcg, d_mrr) = (
        win(Agg::r1),
        win(Agg::r5),
        win(Agg::r10),
        win(Agg::ndcg10),
        win(Agg::mrr),
    );
    let checks: [(&str, f64); 5] = [
        ("R@1", d_r1),
        ("R@5", d_r5),
        ("R@10", d_r10),
        ("NDCG@10", d_ndcg),
        ("MRR", d_mrr),
    ];
    let lost: Vec<&str> = checks
        .iter()
        .filter(|(_, d)| *d < 0.0)
        .map(|(name, _)| *name)
        .collect();
    let lost_list = lost.join(", ");
    let verdict = if lost.is_empty() {
        format!(
            "**`overlap: 0.25` beats the unfitted `1.00` on every ordering metric, on {n_eval} \
queries that never chose it. The effect replicated on independent data.**\n\n\
             R@1 {d_r1:+.1}pp · R@5 {d_r5:+.1}pp · R@10 {d_r10:+.1}pp · NDCG@10 {d_ndcg:+.1}pp · \
MRR {d_mrr:+.1}pp, with R@pool unchanged at {pool:.1}% because the weights reorder the ranking \
and cannot change which documents reach it.\n\n\
             That is the answer `docs/EVALUATION_HYGIENE.md` §3.2 was waiting for, and it is \
**not** the size of the effect on LongMemEval: there `0.25` was worth +4.2pp R@5 and +4.8pp \
NDCG@10 over equal weight, here it is worth {d_r5:+.1}pp and {d_ndcg:+.1}pp. The direction \
replicates; the magnitude does not transfer, which is what a single-parameter fit on 500 \
questions should be expected to do.\n\n\
             **What this does not do.** It changes no shipped value. `0.25` may now be described \
as *replicated on independent data* rather than *provisional*, which is a demotion of the \
existing caveat and not a licence to re-tune: the grid above is a measurement, and picking a \
new weight off it would be a selection that has to be recorded as one in `docs/CONSISTENCY.md` \
under the counting rule. **This harness did not pick a weight and must not be used to pick one \
without saying so out loud.**",
            pool = totals[0].pool(),
        )
    } else {
        format!(
            "**`overlap: 0.25` does not beat the unfitted `1.00` on {lost_list} on {n_eval} \
queries that never chose it. The effect did not replicate on independent data.**\n\n\
             R@1 {d_r1:+.1}pp · R@5 {d_r5:+.1}pp · R@10 {d_r10:+.1}pp · NDCG@10 {d_ndcg:+.1}pp · \
MRR {d_mrr:+.1}pp.\n\n\
             Per `docs/EVALUATION_HYGIENE.md` §3.2 the pre-registered consequence is the one \
written there: it was an artifact of 500 test-set questions, and the unfitted `1.00` stands. \
The regression it bought on LongMemEval is a real trade-off this project chose, and it was \
chosen on the test set — that is the finding, and it is not softened by the fact that other \
metrics moved the right way.",
        )
    };

    let mut md = format!(
        "# LoCoMo retrieval results (memory-wire) — the dev set\n\n\
         ## Provenance\n\n\
         | | |\n|---|---|\n\
         | Commit under test | `{commit}` |\n\
         | Command | `cargo run --release --example locomo -- --docs {docs} --data {queries} \
--out-md eval/LOCOMO.md --out-json eval/results_locomo.json` |\n\
         | Run | {date}, from `--{profile}`, on {host} |\n\
         | Load at start / at end | `{load_start}` / `{load_end}` |\n\
         | Dataset | LoCoMo (Snap Research) via the canonical distribution \
`vectorize-io/agent-memory-benchmark`, `data/locomo/locomo10/`, pinned at upstream commit \
`{upstream}` and sha256-verified by `eval/download.sh` |\n\
         | Corpus | 272 documents, 1,540 queries, 10 conversations — fetched by \
`./eval/download.sh`, **not committed** (`eval/data/` is gitignored) |\n\
         | **LLM involvement** | **none.** No model, no judge, no answer generation, no network \
call of any kind. `gold_answers` is read for shape and never scored. |\n\n\
         The retrieval metrics do not depend on machine load and there is **no latency column** \
in this artifact, so the load row is recorded for completeness only — the same reason \
`eval/SWEEP_FUSION.md` carries no timing.\n\n\
         ## The question\n\n\
         `overlap: 0.25` in `src/recall.rs` was selected by sweeping 46 configurations against \
LongMemEval-S's 500 questions — that is, fitted to the test set, which `AGENTS.md` §1 and \
`docs/EVALUATION_HYGIENE.md` §1 forbid. **This artifact answers one question: does 0.25 beat \
the unfitted equal weight 1.00 on data that never chose it?** Either answer is worth more \
than any new model. Neither is used here to pick a weight: this is a replication check, and a \
row that wins here is evidence, not a licence to re-tune.\n\n\
         ## Comparability — read before quoting any number from this file\n\n\
         **A `recall_any@K` from this harness is not a LoCoMo score.** LoCoMo's standard metric \
is LLM-judged *answer* accuracy: a model reads retrieved context and is graded on whether its \
generated answer matches a gold answer. This harness never generates an answer and never \
judges one, so it cannot produce that number and must not be placed beside one. Any published \
LoCoMo figure — including the 92.0% in `README.md`, which is Hindsight's LLM-judged answer \
accuracy from a `rag` run with two Gemini calls in the loop — is a different measurement on a \
different scale. The two answer different questions and neither bounds the other. **Do not \
place a number below next to a published LoCoMo number.**\n\n\
         What *is* comparable is the internal comparison: the `0.25` row against the unfitted \
`1.00` row, on the same corpus, the same index, the same harness, the same run. The \
`R@5 / R@10 / NDCG@10 / MRR` columns use the same definitions and units as `eval/RESULTS.md` \
(`examples/longmemeval.rs`) and are printed together for that reason — but the suites differ in \
document granularity (LoCoMo: one document per conversation session, {bank_min}–{bank_max} per \
bank; LongMemEval: ~48 per bank), question style and question language, so a LoCoMo percentage \
is not a LongMemEval percentage and **the absolute levels are not comparable either. Only the \
within-table deltas are.**\n\n\
         ## Method\n\n\
         One bank per `user_id` (the conversation, and the isolation unit), built once: one memory \
per LoCoMo document, `id` = document id, content = that document's dialogue turns joined one per \
line. Each query is answered against **its own `user_id`'s bank and no other** — the isolation \
model `examples/longmemeval.rs` uses. Every weight is scored on one shared index per bank, so \
two rows cannot disagree because one's index happened to build differently.\n\n\
         **Read the `R@pool` column before any other.** Each bank holds {bank_min}–{bank_max} \
documents, under both recall's 200-row candidate-pool window and BM25's `LIMIT 50`, so the \
*store* hands recall the whole bank. But the fused list is the union of the two streams, and both \
of them **drop a document that matches no query token** — `fts_match_query` ORs the query tokens \
and `rank_candidates` retains `score > 0.0`. A gold document that shares no token with its \
question is therefore unreachable at any K. `R@pool` is recall over the *whole* returned list, so \
it measures exactly that: the {pool_miss} queries of {n_eval} ({pool_miss_pct:.1}%) whose gold \
document never entered the ranking. That is the coverage ceiling for this suite, and it is the \
analogue of the note in `eval/RESULTS.md` that the 200-row pool never binds on LongMemEval. \
{pool_verdict}\n\n\
         **So R@20 is a mid-list cut here, not a ceiling, and it should not be read as one.** The \
fused ranking returns a mean of {pool_mean:.1} documents per query (max {pool_max}), so a top-20 \
cut lands inside a 19–32-document list and misses a gold document sitting at rank 25. The \
discriminating columns are **R@1, R@5 and NDCG@10** — ordering, which is exactly what the fusion \
weight controls. R@20 is printed because the LongMemEval table prints it and the two are meant to \
be read together. This suite cannot detect a store-pool-bound or BM25-truncation regression; \
`eval/BENCH_RECALL_CURVE.md` and `tests/scale.rs` price that.\n\n\
         The metrics are deterministic and **seed-independent** — a query's index is fixed by its \
`user_id`, so `--seed` changes only the order of the progress log. Re-running this binary \
reproduces every per-question value in `eval/results_locomo.json` bit-for-bit.\n\n\
         ## The grid\n\n\
         {grid_list} — the weight on the token-overlap stream, with BM25 pinned at 1.0, the \
agreement bonus at 0.0 and RRF `k` at 60, so `overlap` is the only axis that moves. `1.00` is \
**equal weight**: the configuration every published number in `README.md` and `eval/RESULTS.md` \
was measured under, and the only clean comparison `docs/EVALUATION_HYGIENE.md` §2.2 has. `0.25` \
is the fitted value under test. `0.00` is a diagnostic bound (BM25 alone) and **not a \
candidate**: a zero weight drops the overlap stream's candidates while BM25 truncates at 50 in \
SQL, so it is only correct on a corpus this small — one of the reasons `overlap: 0.0` was \
rejected on LongMemEval. Every `Δ` is against the `1.00` row.\n\n\
         ## Aggregate, every weight\n\n\
         | overlap | R@1 | ΔR@1 | R@5 | ΔR@5 | R@10 | R@20 | NDCG@10 | ΔNDCG | MRR | ΔMRR | R@pool | R@5 up/down/same | n |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
        commit = git_head(),
        docs = docs_path,
        queries = queries_path,
        profile = bench_common::profile(),
        host = bench_common::host(),
        upstream = UPSTREAM_REF,
        load_start = load_start,
        load_end = load_end,
        bank_min = bank_min,
        bank_max = bank_max,
        grid_list = grid_list,
        pool_miss = pool_miss,
        pool_miss_pct = pool_miss_pct,
        pool_mean = pool_mean,
        pool_max = pool_len_max,
        pool_verdict = pool_verdict,
    );
    for (i, weight) in GRID.iter().enumerate() {
        let t = &totals[i];
        let (up, down, same) = diffs[i];
        md.push_str(&format!(
            "| {weight:.2} | {:.1}% | {:+.1} | {:.1}% | {:+.1} | {:.1}% | {:.1}% | {:.1}% | \
{:+.1} | {:.1}% | {:+.1} | {:.1}% | {up}/{down}/{same} | {} |\n",
            t.r1(),
            t.r1() - base.r1(),
            t.r5(),
            t.r5() - base.r5(),
            t.r10(),
            t.r20(),
            t.ndcg10(),
            t.ndcg10() - base.ndcg10(),
            t.mrr(),
            t.mrr() - base.mrr(),
            t.pool(),
            t.count
        ));
    }

    // ---- the answer, placed directly under the numbers it is derived from ------
    md.push_str(&format!(
        "\n## The answer\n\n{verdict}\n\n\
         Per-query movement on R@5 for the `0.25` row against the `1.00` row: **{up} improved, \
{down} regressed, {same} unchanged** of {n_eval}. A win on the mean with a small improving set is \
a different claim from a win that moves most of the suite, and both numbers are here so the \
reader can tell which one this is.\n",
        up = diffs[fitted].0,
        down = diffs[fitted].1,
        same = diffs[fitted].2,
    ));

    // Per-category, for every weight. A matrix per metric rather than a table per
    // weight: the point of this table is the side-by-side, and five near-identical
    // tables is how a per-category regression gets missed. Every cell carries its n.
    for (title, f) in [
        ("R@1", Agg::r1 as fn(&Agg) -> f64),
        ("R@5", Agg::r5),
        ("NDCG@10", Agg::ndcg10),
    ] {
        md.push_str(&format!("\n## {title}, per `meta.category`\n\n| overlap |"));
        for c in &cats {
            md.push_str(&format!(" {c} |"));
        }
        md.push_str(" overall |\n|---|");
        for _ in 0..=cats.len() {
            md.push_str("---|");
        }
        md.push('\n');
        let empty = Agg::default();
        for (i, weight) in GRID.iter().enumerate() {
            md.push_str(&format!("| {weight:.2} |"));
            for c in &cats {
                let a = per_cat[i].get(c).unwrap_or(&empty);
                let b = per_cat[0].get(c).unwrap_or(&empty);
                md.push_str(&format!(" {:.1}% ({:+.1}, n={}) |", f(a), f(a) - f(b), a.count));
            }
            md.push_str(&format!(" {:.1}% |\n", f(&totals[i])));
        }
    }

    // The counts, because "a percentage without its denominator is not acceptable in
    // this project" (AGENTS.md §3).
    md.push_str(&format!(
        "\n## Counts\n\n\
         | | |\n|---|---|\n\
         | documents indexed | {docs_n}, in {banks_n} banks ({bank_min}–{bank_max} per bank) |\n\
         | documents returned per query by the fused ranking | mean {pool_mean:.1}, max {pool_max} |\n\
         | dialogue turns decoded | {turns} |\n\
         | queries in `queries.json` | {queries_n} |\n\
         | **queries evaluated — the n in every table above** | **{n_eval}** |\n\
         | dropped: `gold_ids` empty | {skipped} |\n\
         | queries with a blank `gold_answers` (shape-checked only, never scored) | {blank} |\n\
         | queries whose `gold_answers` holds a bare JSON number, not a string | {nonstr} |\n\
         | conversations (`user_id`) | {banks_n} |\n\
         | wall clock for the scoring pass | {elapsed:.1}s |\n\n",
        docs_n = documents.len(),
        banks_n = banks.len(),
        turns = n_turns,
        queries_n = all_queries.len(),
        n_eval = n_eval,
        skipped = skipped_no_gold,
        blank = blank_answer,
        nonstr = non_string_answer,
        pool_mean = pool_mean,
        pool_max = pool_len_max,
    ));
    if skipped_no_gold > 0 {
        md.push_str(&format!(
            "**The n above is {n_eval}, not {queries_n}.** {skipped} queries carry an empty \
`gold_ids` list: they have a gold *answer* but no gold *document*, so no document could be \
retrieved and every `recall_any@K` would be 0 by construction. They are excluded from every \
denominator and reported here rather than silently kept — keeping them would understate every \
row by a flat {pct:.2}pp carrying no measurement at all. Every remaining `gold_ids` value \
resolves to a document in the query's own bank; none names a document from another \
conversation, so per-bank isolation costs nothing measurable here.\n\n",
            n_eval = n_eval,
            queries_n = all_queries.len(),
            skipped = skipped_no_gold,
            pct = skipped_no_gold as f64 / all_queries.len() as f64 * 100.0,
        ));
    }

    md.push_str(&format!(
        "## Data shape, as measured (not as assumed)\n\n\
         | field | observed |\n|---|---|\n\
         | `documents[].content` | a JSON **string** holding a list of dialogue turns — {turns} of \
them across {docs_n} documents — parsed a second time by the harness |\n\
         | `queries[].meta` | already a JSON **object** (`category`, `sample_id`, `speaker_a`, \
`speaker_b`, `query_timestamp`) — **not** an encoded string |\n\
         | `queries[].gold_ids` | already a JSON **array of strings** — **not** an encoded string |\n\
         | `queries[].gold_answers` | an array whose elements are **not uniformly strings**: \
{nonstr} of {queries_n} queries carry a bare JSON number (a year, a count) rather than a quoted \
string, so the harness types it `serde_json::Value`. Never scored — this is a retrieval harness |\n\
         | `gold_ids` per query | 1 for {single_gold} queries, 2–15 for {multi_gold}, 0 for the \
{skipped} dropped above |\n\
         | `meta.category` values | {cat_list} |\n\n\
         **Two of the three fields the P−1 plan expected to be double-encoded (`meta`, \
`gold_ids`) are not** — in this distribution they are ordinary decoded JSON values, and only \
`content` needs a second parse. A third surprise sits beside them: `gold_answers` is not a list \
of strings either, because a numeric answer (a year, a count) is emitted unquoted. The harness \
types `content` as a `String`, `meta` and `gold_ids` as their natural types, and `gold_answers` \
as `serde_json::Value` — so a distribution that *does* double-encode the first two, or quotes the \
third differently, fails loudly at the serde layer rather than silently scoring an empty gold \
set. All three assumptions were checked against the bytes before a line of the harness was \
written, not discovered by a run.\n\n\
         ## Per-question data\n\n\
         `eval/results_locomo.json` carries `query_id`, `category` and all six retrieval values \
for every evaluated query under every weight, so any cell above can be re-derived. The `R@5 \
up/down/same` column in the aggregate table counts individual queries against the `overlap=1.00` \
row.\n\n\
         Wrote `{out_md}` and `{out_json}`. This binary owns the whole of this file: do not \
hand-edit it, and note that a bare run writes to `$TMPDIR` instead — updating the committed \
artifact means naming `--out-md eval/LOCOMO.md`.\n",
        turns = n_turns,
        docs_n = documents.len(),
        single_gold = single_gold,
        multi_gold = multi_gold,
        skipped = skipped_no_gold,
        nonstr = non_string_answer,
        queries_n = all_queries.len(),
        cat_list = cat_list,
    ));
    fs::write(&out_md, &md)?;

    let json: Vec<serde_json::Value> = GRID
        .iter()
        .enumerate()
        .map(|(i, weight)| {
            let role = if (*weight - 1.0).abs() < f64::EPSILON {
                "unfitted baseline: equal weight, the configuration every published number was measured under"
            } else if (*weight - 0.25).abs() < f64::EPSILON {
                "shipped value under test: fitted on LongMemEval-S's 500 questions"
            } else if weight.abs() < f64::EPSILON {
                "diagnostic bound: BM25 only, not a candidate"
            } else {
                "intermediate grid point"
            };
            serde_json::json!({
                "overlap": weight,
                "role": role,
                "queries": per_config[i].iter().map(|r| serde_json::json!({
                    "query_id": r.query_id,
                    "category": r.category,
                    "recall_any_at_1": r.r1,
                    "recall_any_at_5": r.r5,
                    "recall_any_at_10": r.r10,
                    "recall_any_at_20": r.r20,
                    "ndcg_at_10": r.ndcg10,
                    "mrr": r.mrr,
                    "gold_in_pool": r.pool,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    fs::write(&out_json, serde_json::to_string(&json)?)?;

    // ---- stdout ------------------------------------------------------------------
    // The table on stdout too: an artifact nobody reads is a log, and this is what the
    // accept/reject decision is made from.
    println!("{md}");

    let fitted = GRID
        .iter()
        .position(|w| (*w - 0.25).abs() < f64::EPSILON)
        .expect("the grid pins the shipped weight, so this row always exists");
    let t = &totals[fitted];
    println!(        "REPLICATION CHECK — overlap=0.25 vs unfitted overlap=1.00 on LoCoMo (n={n_eval}, \
{docs} documents, {banks} banks). Retrieval-only: no LLM.\n  \
R@1     {a_r1:.1}%  vs {b_r1:.1}%  ({d_r1:+.1}pp)\n  \
R@5     {a_r5:.1}%  vs {b_r5:.1}%  ({d_r5:+.1}pp)\n  \
R@10    {a_r10:.1}%  vs {b_r10:.1}%  ({d_r10:+.1}pp)\n  \
R@20    {a_r20:.1}%  vs {b_r20:.1}%  ({d_r20:+.1}pp)\n  \
NDCG@10 {a_n:.1}%  vs {b_n:.1}%  ({d_n:+.1}pp)\n  \
MRR     {a_m:.1}%  vs {b_m:.1}%  ({d_m:+.1}pp)\n  \
R@pool  {a_p:.1}%  vs {b_p:.1}%  ({d_p:+.1}pp)\n  \
per-query R@5: {up} improved, {down} regressed, {same} unchanged",
        n_eval = n_eval,
        docs = documents.len(),
        banks = banks.len(),
        a_r1 = t.r1(),
        b_r1 = base.r1(),
        d_r1 = t.r1() - base.r1(),
        a_r5 = t.r5(),
        b_r5 = base.r5(),
        d_r5 = t.r5() - base.r5(),
        a_r10 = t.r10(),
        b_r10 = base.r10(),
        d_r10 = t.r10() - base.r10(),
        a_r20 = t.r20(),
        b_r20 = base.r20(),
        d_r20 = t.r20() - base.r20(),
        a_n = t.ndcg10(),
        b_n = base.ndcg10(),
        d_n = t.ndcg10() - base.ndcg10(),
        a_m = t.mrr(),
        b_m = base.mrr(),
        d_m = t.mrr() - base.mrr(),
        a_p = t.pool(),
        b_p = base.pool(),
        d_p = t.pool() - base.pool(),
        up = diffs[fitted].0,
        down = diffs[fitted].1,
        same = diffs[fitted].2,
    );
    eprintln!("wrote {out_md} + {out_json}");
    Ok(())
}
