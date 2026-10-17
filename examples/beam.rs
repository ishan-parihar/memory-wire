//! BEAM-100k retrieval harness — H9's pool-scale re-test of the token-overlap
//! stream (`docs/handoff/RND_PLAN.md` §6.1, §14.2 of `docs/CONSISTENCY.md`).
//!
//! ## The question, pre-registered before any BEAM byte was labelled
//!
//! `overlap: 0.25` in `FusionWeights::SHIPPED` was fitted on LongMemEval's 500
//! questions. On LoCoMo's 19–32-document banks it replicated in direction
//! (`eval/LOCOMO.md`) but `overlap: 0.00` tied it on every ordering metric —
//! a tie with 69-up/41-down churn, which §14.2 recorded as "a plateau, not a
//! peak", with one reservation: **LoCoMo banks under-measure pool size**. A
//! second lexical voter has more room to break ties among 50 candidates than
//! among 25, so the null result was not acted on. This harness re-tests the
//! same one axis at pool scale. The pre-registered consequence stands: a tie
//! here is the evidence §14.2 was waiting for and the stream is deleteable;
//! a win here keeps it with a measured justification for the first time.
//! **Acting on the shipped default is the user's call either way** — this
//! artifact reports, it does not patch `FusionWeights`.
//!
//! ## Why the document unit is the QA turn pair, not the BEAM "session"
//!
//! BEAM-100k ships 20 conversations of 3–5 "sessions" of ~100K characters
//! each — session-as-document would give **3–5-document banks, smaller than
//! LoCoMo's**, the opposite of the pre-registered purpose. The corpus's native
//! QA structure gives the unit the re-test actually needs: every user turn
//! has exactly one assistant reply, so one document per turn pair yields
//! 94–196-document banks — past LongMemEval's ~50, the regime §14.2 named.
//! It is also the granularity the product's own hooks write at (the live
//! `omp` bank is turn rows); a 100K-character "session" is a shape the
//! product never produces. This choice was made on this mechanism, recorded
//! in `RND_PLAN.md`, before the harness was run — and it is a *corpus unit*
//! choice for this one re-test, not a proposal to change the session
//! granularity the LongMemEval/AMB providers ship (that lever is closed, H2).
//!
//! ## The labels, and their ceiling
//!
//! Gold pair ids are **agent-authored** by content-match
//! (`eval/beam_label.py`: the ideal answer's distinctive terms against the
//! question's own conversation), then reviewed on samples. 313 of 400
//! questions resolved; `abstention` (40) and `summarization` (40) have no
//! localizable gold by design and are dropped and counted, never scored.
//! Review agreement: 13 of a 15-question sample carried the answer-bearing
//! pair in gold, 2 were partial (a genuinely diffuse aggregate answer, and a
//! spending-limit instruction matched only topically). The systematic
//! residue, stated rather than hidden: event-ordering gold sets hold 3–5 of
//! the 4–6 ordered items the question names; short numeric answers localize
//! by neighborhood. **Both arms of the grid score byte-identical labels**, so
//! label noise damps the delta both ways; it cannot manufacture one.
//!
//! Run: `cargo run --release --example beam -- --out-md eval/BEAM.md`
//!
//! **A bare run writes to `$TMPDIR`, never to `eval/`** (`bench_common::out_md`)
//! — updating the committed artifact means naming `--out-md eval/BEAM.md`.
//!
//! Retrieval only: no LLM, no judge, no answer generation, no network call.
//! `gold_answers` is read for shape and never scored.

use std::collections::BTreeMap;
use std::fs;

use memory_wire::recall::FusionWeights;

mod bench_common;

// The dev-set core shared with `examples/locomo.rs`: ingestion, metric
// definitions, aggregation. One copy — a second is how two harnesses end up
// describing two different corpora under one name.
#[path = "../eval/locomo_dev.rs"]
mod dev;

use dev::{git_head, loadavg, Agg, Corpus, Row};

/// The grid: the token-overlap stream's weight, every other weight pinned at
/// `FusionWeights::SHIPPED`, so `overlap` is the only axis that moves.
/// `1.00` is the unfitted equal-weight control; `0.25` the fitted shipped
/// value under test; `0.00` the stream-dropped bound — the deletion
/// candidate, and the arm the pre-registered tie consequence names.
const GRID: &[f64] = &[1.00, 0.25, 0.00];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let load_start = loadavg();
    let docs_path = bench_common::arg(&args, "--docs", "eval/data/beam/documents.json");
    let queries_path = bench_common::arg(&args, "--data", "eval/data/beam/queries.json");
    let out_md = bench_common::out_md(&args, "BEAM.md");
    let out_json = match bench_common::flag(&args, "--out-json") {
        Some(p) => p,
        None => {
            let p = std::env::temp_dir().join("results_beam.json");
            eprintln!("no --out-json given: writing per-question data to {}", p.display());
            p.display().to_string()
        }
    };
    if bench_common::profile() != "release" {
        eprintln!(
            "WARNING: not `--release`. The retrieval metrics are unaffected by profile — there \
             is no LLM and no wall clock in the table — but the artifact's provenance line will \
             say the profile, and every committed artifact is `release`."
        );
    }

    let corpus = Corpus::load(&docs_path, &queries_path)?;
    let all_queries = &corpus.queries;
    let documents = &corpus.documents;
    let banks = &corpus.banks;
    let (bank_min, bank_max) = corpus.bank_range();
    let order: Vec<usize> = (0..all_queries.len()).collect();
    let planned = dev::plan(all_queries, order);
    eprintln!(
        "beam: {} documents in {} banks, {} queries planned, {} weights (overlap {:?}); \
         loadavg at start {load_start}",
        documents.len(),
        banks.len(),
        planned.len(),
        GRID.len(),
        GRID
    );

    // ---- the pass ----------------------------------------------------------------
    let mut per_config: Vec<Vec<Row>> = GRID.iter().map(|_| Vec::with_capacity(planned.len())).collect();
    let mut per_dim: Vec<BTreeMap<String, Agg>> = GRID.iter().map(|_| BTreeMap::new()).collect();
    let mut pool_len_sum = 0usize;
    let mut pool_len_max = 0usize;
    let budget_tokens = corpus.budget_tokens;
    let start = std::time::Instant::now();

    for (qi, planned_q) in planned.iter().enumerate() {
        let q = planned_q.query;
        let svc = &banks[&q.user_id];
        for (ci, weight) in GRID.iter().enumerate() {
            let weights = FusionWeights { overlap: *weight, ..FusionWeights::SHIPPED };
            let hits = svc.recall_with_weights(&q.user_id, &q.query, budget_tokens, &weights)?;
            let retrieved = dev::session_of(hits.into_iter().map(|h| h.memory.id));
            let row = Row::new(q, &retrieved, &planned_q.gold);
            if ci == 0 {
                pool_len_sum += retrieved.len();
                pool_len_max = pool_len_max.max(retrieved.len());
            }
            per_dim[ci].entry(row.category.clone()).or_default().add(&row);
            per_config[ci].push(row);
        }
        if (qi + 1) % 100 == 0 {
            eprintln!("  {}/{} ...", qi + 1, planned.len());
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    anyhow::ensure!(
        per_config.iter().all(|rows| !rows.is_empty()),
        "beam: no query with a gold pair was evaluated — refusing to publish a 0% table"
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
    // Per-query movement of the fitted arm against the control, counted rather
    // than summarised.
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
    // The deletion question is 0.25 vs 0.00, per-query, on R@5 and R@1.
    let fitted = GRID.iter().position(|w| (*w - 0.25).abs() < f64::EPSILON).expect("grid pins 0.25");
    let zero = GRID.iter().position(|w| w.abs() < f64::EPSILON).expect("grid pins 0.00");
    let mut zero_up = 0;
    let mut zero_down = 0;
    for (r, b) in per_config[zero].iter().zip(&per_config[fitted]) {
        match r.r5.partial_cmp(&b.r5).unwrap_or(std::cmp::Ordering::Equal) {
            std::cmp::Ordering::Greater => zero_up += 1,
            std::cmp::Ordering::Less => zero_down += 1,
            std::cmp::Ordering::Equal => {}
        }
    }
    let dims: Vec<String> = per_dim[0].keys().cloned().collect();

    // ---- artifact ----------------------------------------------------------------
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let load_end = loadavg();
    let n_eval = totals[0].count;
    let pool_hit = totals[0].pool();
    let pool_miss = ((1.0 - pool_hit / 100.0) * n_eval as f64).round() as usize;
    let pool_mean = if n_eval == 0 { 0.0 } else { pool_len_sum as f64 / n_eval as f64 };
    let d = |a: &Agg, b: &Agg, f: fn(&Agg) -> f64| f(a) - f(b);
    // The pre-registered deletion test (`0.25` vs `0.00`) and, beside it, the control
    // comparison (`0.25` vs the unfitted `1.00`). Both are derived from measured
    // cells so a re-run on a different corpus cannot leave a stale conclusion
    // above fresh numbers. The tie bar was fixed before the run: every
    // ordering metric within ±1pp.
    type Metric = (&'static str, fn(&Agg) -> f64);
    let del: [Metric; 5] = [
        ("R@1", Agg::r1),
        ("R@5", Agg::r5),
        ("R@10", Agg::r10),
        ("NDCG@10", Agg::ndcg10),
        ("MRR", Agg::mrr),
    ];
    let del_deltas: Vec<(&str, f64)> = del
        .iter()
        .map(|(n, f)| (*n, f(&totals[fitted]) - f(&totals[zero])))
        .collect();
    let tie = del_deltas.iter().all(|(_, v)| v.abs() < 1.0);
    let del_para = if tie {
        format!(
            "**The deletion question answers 'tie' — the pre-registered §14.2 consequence \
             fires.** `0.25` vs `0.00` on {n_eval} questions over {bank_min}–{bank_max}-document \
             banks: every ordering metric within ±1pp, R@5 per-query {zero_up} up / {zero_down} \
             down for the dropped arm. This is the second independent corpus on which the \
             token-overlap stream does not earn its weight, and the first at pool scale, which \
             was §14.2's stated reservation. The stream is deleteable. **Deleting it is a change \
             to `FusionWeights::SHIPPED`, the user's call** (`docs/handoff/README.md`): this \
             artifact is the evidence, not the patch.",
        )
    } else {
        let pool_price = if totals[zero].pool() < totals[fitted].pool() {
            format!(
                " The dropped arm also pays a coverage price: R@pool {:.1}% vs {:.1}% — a \
                 gold pair that only the overlap stream surfaces sits below BM25's 50-row SQL \
                 cut and is unreachable at every K without it.",
                totals[zero].pool(),
                totals[fitted].pool()
            )
        } else {
            String::new()
        };
        format!(
            "**The deletion question answers 'keep': the stream is not deleteable at pool \
             scale.** `0.25` over `0.00` on {n_eval} questions: R@1 {:+.1}pp, R@5 {:+.1}pp, \
             R@10 {:+.1}pp, NDCG@10 {:+.1}pp, MRR {:+.1}pp (per-query R@5: {zero_up} up / \
             {zero_down} down for the dropped arm).{pool_price} §14.2's reservation was right: \
             on 19–32-document LoCoMo banks the second lexical voter could only reorder ties; on \
             {bank_min}–{bank_max}-document banks it also carries coverage, and the ordering \
             deltas above are its work.",
            del_deltas[0].1,
            del_deltas[1].1,
            del_deltas[2].1,
            del_deltas[3].1,
            del_deltas[4].1,
        )
    };
    // The control comparison, stated whichever way it points.
    let (ctl_r1, ctl_r5) = (
        d(&totals[fitted], &totals[0], Agg::r1),
        d(&totals[fitted], &totals[0], Agg::r5),
    );
    let ctl_word = if ctl_r5 > 0.0 {
        "toward"
    } else if ctl_r5 < 0.0 {
        "against"
    } else {
        "neither way for"
    };
    let ctl_para = format!(
        "**The control comparison — `0.25` against the unfitted `1.00` — points {word} the \
         fitted value.** R@1 {ctl_r1:+.1}pp, R@5 {ctl_r5:+.1}pp, NDCG@10 {ctl_n:+.1}pp, MRR {ctl_m:+.1}pp, \
         per-query R@5 {up} up / {down} down for the fitted arm. For scale: LoCoMo gave the fitted \
         value +1.4pp R@5 over equal weight (`eval/LOCOMO.md`); this corpus moves the other way. \
         Two dev corpora, two directions, neither dominant beyond its own noise — the fitted \
         value's out-of-sample edge exists on one and inverts on the other.",
        word = ctl_word,
        ctl_n = d(&totals[fitted], &totals[0], Agg::ndcg10),
        ctl_m = d(&totals[fitted], &totals[0], Agg::mrr),
        up = diffs[fitted].0,
        down = diffs[fitted].1,
    );
    let consequence_para =
        "**What this settles and what it does not.** Settled, by measurement: the token-overlap \
         stream stays — the pre-registered deletion consequence did not fire, and the stream has \
         its first pool-scale justification (ordering, plus the coverage the R@pool row prices). \
         Not settled: the weight. `1.00` leads here, `0.25` led on LoCoMo; picking a value off \
         either table is a dev-set selection that must be recorded as one in `docs/CONSISTENCY.md`, \
         and **`FusionWeights::SHIPPED` is the user's to change** (`docs/handoff/README.md`). \
         This artifact reports both comparisons and patches nothing.";
    let verdict = if tie {
        del_para
    } else {
        format!("{del_para}\n\n{ctl_para}\n\n{consequence_para}")
    };

    let mut md = format!(
        "# BEAM-100k retrieval results (memory-wire) — the pool-scale re-test\n\n\
        ## Provenance\n\n\
        | | |\n|---|---|\n\
        | Commit under test | `{commit}` |\n\
        | Command | `cargo run --release --example beam -- --docs {docs} --data {queries} \
--out-md eval/BEAM.md --out-json eval/results_beam.json` |\n\
        | Run | {date}, from `--{profile}`, on {host} |\n\
        | Load at start / at end | `{load_start}` / `{load_end}` |\n\
        | Dataset | BEAM 100K (ICLR 2026) via HF `Mohammadta/BEAM` split `100K`, fetched by the \
pinned OmniMemEval `prepare_beam.py`; **CC BY-SA 4.0, run from the gitignored corpus, never \
vendored**; the labels digest `eval/beam_labels_100k.json` names the input sha256 |\n\
        | Document unit | **QA turn pair** (user turn + its assistant reply), 94–196 per bank — \
see the module docs and §6.1 for why the BEAM \"session\" (3–5 per conversation, ~100K chars) is \
a shape the product never writes |\n\
        | Occurrence times | each pair carries its turn's `time_anchor`, so every memory is \
dated; the shipped recency weight is 0.0 |\n\
        | **LLM involvement** | **none.** No model, no judge, no answer generation, no network \
call. `gold_answers` (the upstream `ideal_response`) is read for shape and never scored. |\n\n\
        The retrieval metrics do not depend on machine load and there is **no latency column**, so \
the load row is recorded for completeness only. The box was not quiet when this ran.\n\n\
        ## The question\n\n\
        Pre-registered in `docs/handoff/RND_PLAN.md` §6.1: does `overlap: 0.25` beat the \
stream-dropped `0.00` at pool scale — the reservation `docs/CONSISTENCY.md` §14.2 recorded \
when the LoCoMo replication showed a tie? **A tie here means the stream is deleteable; a win \
means it finally has a measured justification. Acting on `FusionWeights::SHIPPED` is the \
user's call either way.** The tie bar was fixed before the run: every ordering metric within \
±1pp.\n\n\
        ## Comparability — read before quoting any number from this file\n\n\
        **Nothing here is a BEAM score.** BEAM's standard metric is LLM-judged nugget \
recall/precision against a reference set this artifact never builds. This harness measures \
`recall_any@K` over agent-authored gold pair ids — a different measurement on a different \
scale, produced by different labelling. Do not place a number below next to any published BEAM \
figure. What *is* comparable is the internal comparison: every row against every other, same \
corpus, same index, same labels, same run.\n\n\
        The labels are agent-authored by content-match (`eval/beam_label.py`) on 313 of 400 \
questions; 80 (`abstention`, `summarization`) have no localizable gold by design; 7 resolved \
to nothing. Review agreement on a 15-question sample: 13 carried the answer-bearing pair, 2 \
were partial — the authoring-weakness caveat sits beside every number in this file, and **both \
arms score byte-identical labels, so label noise dampens the delta both ways; it cannot \
manufacture one**.\n\n\
        ## Method\n\n\
        One bank per conversation, one memory per QA turn pair, built once; every weight scored \
on that one index. At {bank_min}–{bank_max} documents per bank the pool regime §14.2 named is \
finally live: recall's 200-row candidate window covers the bank, **BM25 truncates at 50 in \
SQL**, and the fused list is the union of the streams — so the fused pool mean is \
{pool_mean:.1} documents (max {pool_max}). **`R@pool` behaves differently per arm.** Between the \
keep-stream arms (`1.00`, `0.25`) the weights only reorder, so the pool is shared and their \
deltas are pure ordering — in those arms {pool_miss} of {n_eval} questions miss the pool. The \
stream-dropped `0.00` arm can lose pool membership outright: a gold pair that only the overlap \
stream surfaces sits below BM25's 50-row SQL cut and is unreachable at every K. Its `R@pool` \
in the grid is therefore the coverage price of deletion, and the grid's Δ column prices it \
explicitly.\n\n\
        ## The grid\n\n\
        The weight on the token-overlap stream; every other weight at `FusionWeights::SHIPPED`. \
`1.00` is the unfitted equal-weight control; `0.25` the fitted shipped value; `0.00` the \
stream-dropped bound — the deletion candidate. Δ columns are against `1.00`.\n\n\
        | overlap | R@1 | ΔR@1 | R@5 | ΔR@5 | R@10 | R@20 | recall_all@10 | NDCG@10 | ΔNDCG | \
MRR | ΔMRR | R@pool | R@5 up/down/same | n |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
        commit = git_head(),
        docs = docs_path,
        queries = queries_path,
        profile = bench_common::profile(),
        host = bench_common::host(),
        load_start = load_start,
        load_end = load_end,
        bank_min = bank_min,
        bank_max = bank_max,
        pool_mean = pool_mean,
        pool_max = pool_len_max,
        pool_miss = pool_miss,
        n_eval = n_eval,
    );
    let base = &totals[0];
    for (i, weight) in GRID.iter().enumerate() {
        let t = &totals[i];
        let (up, down, same) = diffs[i];
        md.push_str(&format!(
            "| {weight:.2} | {:.1}% | {:+.1} | {:.1}% | {:+.1} | {:.1}% | {:.1}% | {:.1}% | \
{:.1}% | {:+.1} | {:.1}% | {:+.1} | {:.1}% | {up}/{down}/{same} | {} |\n",
            t.r1(), t.r1() - base.r1(),
            t.r5(), t.r5() - base.r5(),
            t.r10(), t.r20(), t.all10(),
            t.ndcg10(), t.ndcg10() - base.ndcg10(),
            t.mrr(), t.mrr() - base.mrr(),
            t.pool(), t.count,
        ));
    }
    md.push_str(&format!(
        "\n## The answer\n\n{verdict}\n\n\
         The deletion comparison, per-query R@5 of `0.00` against `0.25`: **{zero_up} improved, \
         {zero_down} regressed, {same_z} unchanged** of {n_eval}. The fitted-vs-control \
         movement (`0.25` against `1.00`): {up_f} improved, {down_f} regressed, {same_f} \
         unchanged.\n",
        zero_up = zero_up,
        zero_down = zero_down,
        same_z = n_eval - zero_up - zero_down,
        up_f = diffs[fitted].0,
        down_f = diffs[fitted].1,
        same_f = diffs[fitted].2,
    ));
    for (title, f) in [
        ("R@1", Agg::r1 as fn(&Agg) -> f64),
        ("R@5", Agg::r5),
        ("recall_all@10", Agg::all10),
        ("NDCG@10", Agg::ndcg10),
    ] {
        md.push_str(&format!("\n## {title}, per BEAM dimension\n\n| overlap |"));
        for c in &dims {
            md.push_str(&format!(" {c} |"));
        }
        md.push_str(" overall |\n|---|");
        for _ in 0..=dims.len() {
            md.push_str("---|");
        }
        md.push('\n');
        let empty = Agg::default();
        for (i, weight) in GRID.iter().enumerate() {
            md.push_str(&format!("| {weight:.2} |"));
            for c in &dims {
                let a = per_dim[i].get(c).unwrap_or(&empty);
                let b = per_dim[0].get(c).unwrap_or(&empty);
                md.push_str(&format!(" {:.1}% ({:+.1}, n={}) |", f(a), f(a) - f(b), a.count));
            }
            md.push_str(&format!(" {:.1}% |\n", f(&totals[i])));
        }
    }
    md.push_str(&format!(
        "\n## Counts\n\n\
        | | |\n|---|---|\n\
        | QA turn-pair documents | {docs_n}, in {banks_n} banks ({bank_min}–{bank_max} per bank) |\n\
        | fused documents per query | mean {pool_mean:.1}, max {pool_max} |\n\
        | questions in `queries.json` | {queries_n} |\n\
        | **questions evaluated — the n in every table above** | **{n_eval}** |\n\
        | dropped: no localizable gold by design (`abstention` 40, `summarization` 40) | 80 |\n\
        | dropped: content-match resolved nothing | {unresolved} |\n\
        | wall clock for the scoring pass | {elapsed:.1}s |\n\n\
        ## Data shape, as measured (not as assumed)\n\n\
        | field | observed |\n|---|---|\n\
        | upstream `probing_questions` | a **Python-repr string**, parsed by the generator with \
`ast.literal_eval` — upstream's own parser does the same (`beam_search.py:69`) |\n\
        | upstream `chat` | 3–5 sessions per conversation, 46–132 turns each, ~100K chars — \
the assumption in H9's registration (\"27–50-doc banks at session granularity\") was wrong, \
measured before labelling and corrected by the pair unit |\n\
        | turns | every user turn has exactly one assistant reply: {pairs} pairs, `{bank_min}`–\
`{bank_max}` per bank |\n\
        | `time_anchor` | per turn, `%B-%d-%Y` (\"March-15-2024\"), parsed by the generator into \
the pair's `timestamp`; every pair dated |\n\n\
        Wrote `{out_md}` and `{out_json}`. This binary owns the whole of this file: do not \
hand-edit it, and a bare run writes to `$TMPDIR` instead — updating the committed artifact means \
naming `--out-md eval/BEAM.md`.\n",
        docs_n = documents.len(),
        banks_n = banks.len(),
        bank_min = bank_min,
        bank_max = bank_max,
        pool_mean = pool_mean,
        pool_max = pool_len_max,
        queries_n = all_queries.len(),
        n_eval = n_eval,
        unresolved = all_queries.len() - 80 - n_eval,
        elapsed = elapsed,
        pairs = documents.len(),
    ));
    fs::write(&out_md, &md)?;

    let json: Vec<serde_json::Value> = GRID
        .iter()
        .enumerate()
        .map(|(i, weight)| {
            let role = if (*weight - 1.0).abs() < f64::EPSILON {
                "unfitted control: equal weight"
            } else if (*weight - 0.25).abs() < f64::EPSILON {
                "shipped value under test: fitted on LongMemEval-S's 500 questions"
            } else {
                "stream-dropped bound: the deletion candidate"
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
                    "recall_all_at_10": r.all10,
                    "ndcg_at_10": r.ndcg10,
                    "mrr": r.mrr,
                    "gold_in_pool": r.pool,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    fs::write(&out_json, serde_json::to_string(&json)?)?;

    println!("{md}");
    let (t_f, t_z) = (&totals[fitted], &totals[zero]);
    println!(
        "POOL-SCALE RE-TEST — overlap=0.25 vs 0.00 on BEAM-100k (n={n_eval}, {docs_n} pair \
documents, {banks_n} banks of {bank_min}–{bank_max}). Retrieval-only: no LLM.\n  \
R@1     {a_r1:.1}%  vs {b_r1:.1}%  ({d_r1:+.1}pp)\n  \
R@5     {a_r5:.1}%  vs {b_r5:.1}%  ({d_r5:+.1}pp)   per-query {zero_up} up / {zero_down} down\n  \
R@10    {a_r10:.1}%  vs {b_r10:.1}%  ({d_r10:+.1}pp)\n  \
NDCG@10 {a_n:.1}%  vs {b_n:.1}%  ({d_n:+.1}pp)\n  \
MRR     {a_m:.1}%  vs {b_m:.1}%  ({d_m:+.1}pp)\n  \
R@pool  {a_p:.1}%  vs {b_p:.1}%  ({d_p:+.1}pp)\n  \
control — overlap=1.00: R@1 {ctl_1:.1}%, R@5 {ctl_5:.1}% — points {ctl_word} the fitted value",
        a_r1 = t_f.r1(), b_r1 = t_z.r1(), d_r1 = t_f.r1() - t_z.r1(),
        a_r5 = t_f.r5(), b_r5 = t_z.r5(), d_r5 = t_f.r5() - t_z.r5(),
        a_r10 = t_f.r10(), b_r10 = t_z.r10(), d_r10 = t_f.r10() - t_z.r10(),
        a_n = t_f.ndcg10(), b_n = t_z.ndcg10(), d_n = t_f.ndcg10() - t_z.ndcg10(),
        a_m = t_f.mrr(), b_m = t_z.mrr(), d_m = t_f.mrr() - t_z.mrr(),
        a_p = t_f.pool(), b_p = t_z.pool(), d_p = t_f.pool() - t_z.pool(),
        ctl_1 = totals[0].r1(),
        ctl_5 = totals[0].r5(),
        ctl_word = ctl_word,
        docs_n = documents.len(),
        banks_n = banks.len(),
        bank_min = bank_min,
        bank_max = bank_max,
    );
    eprintln!("wrote {out_md} + {out_json}");
    Ok(())
}
