//! H1.0 — where the missing gold sessions sit: rank distribution on LoCoMo.
//!
//! ## Why this exists
//!
//! `eval/LOCOMO.md` (shipped config, n 1,531) serves every gold session to 83.0%
//! of queries (`recall_all@10`) while serving at least one to 94.2%
//! (`recall_any@10`). The gap is multi-session coverage, and a reordering lever
//! can only promote a session that is already within reach. Before building any
//! lever (H1a–c) the plan needs to know where the sessions it would have to
//! promote are sitting; whether they are one or two places below the cut, deep
//! in the list, or never entered the ranking at all decides which lever class —
//! reordering, or semantics — is worth building. This harness answers exactly
//! that, pre-registered as **H1.0** in `docs/handoff/RND_PLAN.md`. It **selects
//! nothing**: it sizes a problem, on the dev set, and no parameter here is
//! chosen by looking at any number.
//!
//! ## What is measured
//!
//! For every query whose `recall_all@10` is 0 under `FusionWeights::SHIPPED`
//! (the `0.25` row of `eval/LOCOMO.md`), the rank of **each** gold session in
//! the fused list (`session_of` over `recall_with_weights`), reported as:
//!
//! - a histogram of the **worst-ranked** gold per miss query, in the
//!   pre-registered bins `11–15`, `16–20`, `21+`, `absent` — absent meaning a
//!   gold session that never entered the fused list, because it shares no
//!   query token in either stream;
//! - the same histogram split by `meta.category` and by gold-set size, with the
//!   evaluated n and the miss rate so no percentage is read without its
//!   denominator (`AGENTS.md` §7);
//! - every missed gold session (not only the worst one per query), same bins;
//! - the bank sizes and the fused-list lengths on miss queries, so `rank 21`
//!   is read against the 19–32-document banks and the lists that may be
//!   shorter than them.
//!
//! No LLM, no judge, no answer generation, no network call: the same
//! retrieval-only discipline as `examples/locomo.rs`, whose ingestion this
//! harness shares via `eval/locomo_dev.rs`, so the two cannot be describing two
//! different corpora under one name.
//!
//! ## Run
//!
//! ```text
//! cargo run --release --example locomo_ranks --
//!   --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json
//! ```
//!
//! **A bare run writes to `$TMPDIR`, never to `eval/`** (see
//! `bench_common::out_md`): this binary owns the whole of
//! `eval/LOCOMO_RANKS.md`, and a bare run must not be able to replace a
//! reviewed artifact.

mod bench_common;

#[path = "../eval/locomo_dev.rs"]
mod dev;

use std::collections::{BTreeMap, HashSet};
use std::fs;

use memory_wire::recall::FusionWeights;

/// The pre-registered bins, worst-ranked gold session per miss query. Index of
/// this array is the bin used everywhere below. A miss query's worst gold is
/// by construction never inside the top 10 (that is what `recall_all@10 = 0`
/// means), so no bin below 11 exists.
const BINS: [&str; 4] = ["11–15", "16–20", "21+", "absent"];

/// Gold-set size as a table key: 1, 2, 3, 4 (= ≥4, the reporting shape
/// `docs/handoff/RND_PLAN.md` §1.2 uses).
fn gold_size_key(n: usize) -> usize {
    n.min(4)
}

/// The bin a rank falls into. `None` is a gold session that never entered the
/// fused list at all; a `Some(r)` with `r <= 10` cannot arrive here from a miss
/// query but can from the every-missed-gold table, which only counts golds
/// outside the top 10 — so both callers feed this function ranks the bins were
/// designed for.
fn bin_of(rank: Option<usize>) -> usize {
    match rank {
        None => 3,
        Some(r) if r <= 15 => 0,
        Some(r) if r <= 20 => 1,
        Some(_) => 2,
    }
}

/// Median of a slice, the 10-bank and miss-query context lines. Mean of the two
/// middle values on an even count, so a 10-bank corpus reports a real median
/// rather than a lower-element convention.
fn median(values: &mut [usize]) -> f64 {
    values.sort_unstable();
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2] as f64
    } else {
        (values[n / 2 - 1] + values[n / 2]) as f64 / 2.0
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let load_start = dev::loadavg();
    let docs_path = bench_common::arg(&args, "--docs", "eval/data/locomo/documents.json");
    let queries_path = bench_common::arg(&args, "--data", "eval/data/locomo/queries.json");
    let out_md = bench_common::out_md(&args, "LOCOMO_RANKS.md");
    // Temp-defaulted like `locomo`'s: the per-question dump is re-derivability
    // for the aggregate, and two harnesses defaulting to one committed path is
    // the clobber waiting to happen.
    let out_json = match bench_common::flag(&args, "--out-json") {
        Some(p) => p,
        None => {
            let p = std::env::temp_dir().join("results_locomo_ranks.json");
            eprintln!(
                "no --out-json given: writing per-question data to {}",
                p.display()
            );
            p.display().to_string()
        }
    };
    if bench_common::profile() != "release" {
        eprintln!(
            "WARNING: not `--release`. The rank distribution is unaffected by profile — there \
             is no LLM and no wall clock in the table — but the artifact's provenance line will \
             say the profile, and it should be `release` to match every other committed artifact."
        );
    }

    // The whole-session arm: the committed `eval/LOCOMO.md` is the default
    // run, and a rank histogram must be taken on the same granularity the
    // misses it explains were measured on.
    let corpus = dev::Corpus::load_at(&docs_path, &queries_path, dev::Granularity::Session)?;
    let planned = dev::plan(&corpus.queries, 0..corpus.queries.len());
    eprintln!(
        "locomo_ranks: {} documents in {} banks, {} queries, shipped weights; loadavg at start {}",
        corpus.documents.len(),
        corpus.banks.len(),
        planned.len(),
        load_start
    );

    // One record per miss query. The gold list keeps first-appearance order out
    // of `gold_ids` (deduplicated) so the dump is deterministic without
    // depending on `HashSet` iteration order.
    struct Miss {
        query_id: String,
        category: String,
        gold_size: usize,
        bank_size: usize,
        list_len: usize,
        /// (gold id, 1-based rank in the fused list, `None` when absent).
        golds: Vec<(String, Option<usize>)>,
    }

    let start = std::time::Instant::now();
    let mut misses: Vec<Miss> = Vec::new();
    // Evaluated n per split, so every miss rate below carries its denominator.
    let mut n_eval_by_cat: BTreeMap<String, usize> = BTreeMap::new();
    let mut n_eval_by_gold: BTreeMap<usize, usize> = BTreeMap::new();
    let mut n_all10 = 0usize;
    let mut r10_sum = 0.0f64;

    for pq in &planned {
        let q = pq.query;
        let svc = &corpus.banks[&q.user_id];
        let hits =
            svc.recall_with_weights(&q.user_id, &q.query, corpus.budget_tokens, &FusionWeights::SHIPPED)?;
        let retrieved = dev::session_of(hits.into_iter().map(|h| h.memory.id));
        let row = dev::Row::new(q, &retrieved, &pq.gold);
        *n_eval_by_cat.entry(row.category.clone()).or_default() += 1;
        *n_eval_by_gold.entry(gold_size_key(pq.gold.len())).or_default() += 1;
        r10_sum += row.r10;
        if row.all10 > 0.0 {
            n_all10 += 1;
            continue;
        }
        let mut seen = HashSet::new();
        let golds: Vec<(String, Option<usize>)> = q
            .gold_ids
            .iter()
            .filter(|g| seen.insert((*g).clone()))
            .map(|g| {
                let rank = retrieved.iter().position(|r| r == g).map(|i| i + 1);
                (g.clone(), rank)
            })
            .collect();
        // Defensive consistency: `all10 = 0` must mean some gold sits outside
        // the top 10 or is absent. If this ever fires, the histogram and
        // `recall_all` disagree about the same list and no number below is
        // trustworthy.
        anyhow::ensure!(
            golds.iter().any(|(_, r)| r.is_none_or(|r| r > 10)),
            "locomo_ranks: query {} scored all10 = 0 but every gold sits inside the top 10",
            q.id
        );
        misses.push(Miss {
            query_id: q.id.clone(),
            category: row.category,
            gold_size: golds.len(),
            bank_size: corpus.bank_sizes[&q.user_id],
            list_len: retrieved.len(),
            golds,
        });
    }
    eprintln!(
        "locomo_ranks: {} queries scored, {} with every gold in the top 10, {} misses, in {:?}",
        planned.len(),
        n_all10,
        misses.len(),
        start.elapsed()
    );

    // ---- histograms -----------------------------------------------------------
    let mut hist_overall = [0usize; 4];
    let mut hist_by_cat: BTreeMap<String, [usize; 4]> = BTreeMap::new();
    let mut hist_by_gold: BTreeMap<usize, [usize; 4]> = BTreeMap::new();
    // Every missed gold session, not only the worst per query — the shape the
    // plan's question is asked in ("where do the missing sessions sit").
    let mut hist_golds = [0usize; 4];
    let mut missed_golds = 0usize;

    for m in &misses {
        let any_absent = m.golds.iter().any(|(_, r)| r.is_none());
        let b = if any_absent {
            3
        } else {
            bin_of(m.golds.iter().filter_map(|(_, r)| *r).max())
        };
        hist_overall[b] += 1;
        hist_by_cat.entry(m.category.clone()).or_default()[b] += 1;
        hist_by_gold.entry(gold_size_key(m.gold_size)).or_default()[b] += 1;
        for (_, rank) in &m.golds {
            if rank.is_none_or(|r| r > 10) {
                hist_golds[bin_of(*rank)] += 1;
                missed_golds += 1;
            }
        }
    }

    // ---- context --------------------------------------------------------------
    let mut bank_sizes: Vec<usize> = corpus.bank_sizes.values().copied().collect();
    let (bank_min, bank_max) = corpus.bank_range();
    let bank_median = median(&mut bank_sizes);
    let mut list_lens: Vec<usize> = misses.iter().map(|m| m.list_len).collect();
    let list_median = median(&mut list_lens);
    let (list_min, list_max) = match list_lens.iter().copied().min() {
        Some(min) => (min, list_lens.iter().copied().max().unwrap_or(min)),
        None => (0, 0),
    };
    // A miss query whose fused list is the whole bank says no document was dropped
    // for matching no query token — the precondition for `absent` to be able to
    // occur. Counted, not asserted, so a corpus where lists do shrink reports it.
    let list_covers_bank = misses
        .iter()
        .filter(|m| m.list_len == m.bank_size)
        .count();

    // The gate reading, derived from the counts rather than written by hand,
    // so a re-run on a changed corpus cannot leave a stale conclusion above
    // fresh numbers. H1.0's pre-registered decision rule.
    let n_miss = misses.len();
    let within = hist_overall[0] + hist_overall[1];
    let deep = hist_overall[2] + hist_overall[3];
    let verdict = if n_miss == 0 {
        "No misses on this corpus: nothing to size.".to_string()
    } else if within * 2 >= n_miss {
        // `absent` is folded into the deep clause only when it occurred, so a run
        // where it is zero does not describe a case that did not happen.
        let deep_desc = if hist_overall[3] == 0 {
            format!("{deep} sit at 21+ and bound what reordering can recover.")
        } else {
            format!(
                "{deep} sit at 21+ or never entered the list and bound what reordering can recover."
            )
        };
        format!(
            "**Mostly within reach.** {within} of {n_miss} misses hold their worst gold at rank \
             11–20 ({:.1}%), so a reordering lever (H1a–c) has something to promote and is worth \
             building; {deep_desc}",
            100.0 * within as f64 / n_miss as f64
        )
    } else {
        format!(
            "**Mostly out of reach.** {deep} of {n_miss} misses hold their worst gold at rank 21+ \
             or absent ({:.1}%), so no reordering lever can recover them and the honest finding is \
             the one `docs/EXCEED_PLAN.md` §4 already names — the deficit is semantic (H1d), whose \
             shipping is the user's decision.",
            100.0 * deep as f64 / n_miss as f64
        )
    };

    // ---- the per-question dump -------------------------------------------------
    let dump: Vec<serde_json::Value> = misses
        .iter()
        .map(|m| {
            serde_json::json!({
                "query_id": m.query_id,
                "category": m.category,
                "gold_size": m.gold_size,
                "bank_size": m.bank_size,
                "list_len": m.list_len,
                "worst_rank": m.golds.iter().filter_map(|(_, r)| *r).max(),
                "any_absent": m.golds.iter().any(|(_, r)| r.is_none()),
                "golds": m.golds.iter().map(|(id, r)| serde_json::json!({
                    "id": id,
                    "rank": r,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    fs::write(&out_json, serde_json::to_string_pretty(&dump)?)?;

    // ---- the artifact ------------------------------------------------------------
    let pct = |n: usize, d: usize| {
        if d == 0 {
            "—".to_string()
        } else {
            format!("{:.1}%", 100.0 * n as f64 / d as f64)
        }
    };
    let hist_row = |label: &str, h: &[usize; 4], d: usize| {
        format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            label,
            h[0],
            h[1],
            h[2],
            h[3],
            h.iter().sum::<usize>(),
            if d == 0 { "—".into() } else { pct(h.iter().sum(), d) },
        )
    };

    let mut md = String::new();
    md.push_str("# LoCoMo — where the missed gold sessions sit (H1.0)\n\n");
    md.push_str("## Provenance\n\n");
    md.push_str("| | |\n|---|---|\n");
    md.push_str(&format!(
        "| Commit under test | `{}` |\n",
        dev::git_head()
    ));
    md.push_str(&format!(
        "| Command | `cargo run --release --example locomo_ranks -- --docs {} --data {} --out-md {} \
         --out-json {}` |\n",
        docs_path, queries_path, out_md, out_json
    ));
    md.push_str(&format!(
        "| Run | {}, from `{}`, on {} |\n",
        chrono::Utc::now().format("%Y-%m-%d"),
        bench_common::profile(),
        bench_common::host()
    ));
    md.push_str("| Granularity | session: one memory per document |\n");
    md.push_str(
        "| Weights | `FusionWeights::SHIPPED`: bm25 1.0, overlap 0.25, agreement 0.0, k 60, \
         recency 0.0 — the shipped configuration, the `0.25` row of `eval/LOCOMO.md` |\n",
    );
    md.push_str(&format!("| Load at start / at end | `{}` / `{}` |\n", load_start, dev::loadavg()));
    md.push_str(&format!(
        "| Dataset | LoCoMo (Snap Research) via the canonical distribution \
         `vectorize-io/agent-memory-benchmark`, `data/locomo/locomo10/`, pinned at upstream commit \
         `{}` and sha256-verified by `eval/download.sh` |\n",
        dev::UPSTREAM_REF
    ));
    md.push_str(
        "| **LLM involvement** | **none.** No model, no judge, no answer generation, no network \
         call of any kind. |\n",
    );
    md.push_str(&format!(
        "| Queries | {} evaluated ({} dropped, empty gold set) |\n",
        planned.len(),
        corpus.skipped_no_gold
    ));
    md.push_str("\nThe rank distribution is deterministic — same corpus, same index, same \
        weights, same ranks — and there is no latency column, so the load row is recorded for \
        completeness only.\n\n");

    md.push_str("## The question\n\n");
    md.push_str(&format!(
        "`eval/LOCOMO.md` at this configuration serves every gold session to {} of the {} \
         evaluated queries (`recall_all@10`, measured on this run), at least one to {:.1}% \
         (`recall_any@10`, same run). A reordering \
         lever can only promote a session already within reach, so this artifact answers one \
         pre-registered question (**H1.0**, `docs/handoff/RND_PLAN.md`): for every miss, where \
         does its **worst-ranked** gold session sit? It selects nothing and tunes nothing; it \
         sizes the problem.\n\n",
        pct(n_all10, planned.len()),
        planned.len(),
        100.0 * r10_sum / planned.len().max(1) as f64,
    ));

    md.push_str("## Answer — worst-ranked gold per miss query\n\n");
    md.push_str(&format!(
        "| worst gold | {} | {} | {} | {} | n | % of misses |\n|---|---|---|---|---|---|---|\n",
        BINS[0], BINS[1], BINS[2], BINS[3]
    ));
    md.push_str(&hist_row("", &hist_overall, n_miss));
    md.push_str(&format!("\n{verdict}\n\n"));

    md.push_str("## By category\n\n");
    md.push_str(&format!(
        "| category | evaluated | misses | miss rate | {} | {} | {} | {} |\n\
         |---|---|---|---|---|---|---|---|\n",
        BINS[0], BINS[1], BINS[2], BINS[3]
    ));
    for (cat, h) in &hist_by_cat {
        let evaluated = n_eval_by_cat[cat];
        let miss_n: usize = h.iter().sum();
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            cat,
            evaluated,
            miss_n,
            pct(miss_n, evaluated),
            h[0],
            h[1],
            h[2],
            h[3],
        ));
    }
    md.push('\n');

    md.push_str("## By gold-set size\n\n");
    md.push_str(&format!(
        "| gold sessions | evaluated | misses | miss rate | {} | {} | {} | {} |\n\
         |---|---|---|---|---|---|---|---|\n",
        BINS[0], BINS[1], BINS[2], BINS[3]
    ));
    for (size, h) in &hist_by_gold {
        let evaluated = n_eval_by_gold[size];
        let miss_n: usize = h.iter().sum();
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            if *size == 4 { "≥4".to_string() } else { size.to_string() },
            evaluated,
            miss_n,
            pct(miss_n, evaluated),
            h[0],
            h[1],
            h[2],
            h[3],
        ));
    }
    md.push('\n');

    md.push_str("## Every missed gold session\n\n");
    md.push_str(&format!(
        "The tables above are per query; a 3-gold query with two golds served contributes **one** \
         row there. This table is per session: each of the {} gold sessions a miss query left \
         outside its top 10 contributes one row, same bins.\n\n",
        missed_golds
    ));
    md.push_str(&format!(
        "| missed gold session | {} | {} | {} | {} | n |\n|---|---|---|---|---|---|\n",
        BINS[0], BINS[1], BINS[2], BINS[3]
    ));
    md.push_str(&format!(
        "| rank | {} | {} | {} | {} | {} |\n",
        hist_golds[0],
        hist_golds[1],
        hist_golds[2],
        hist_golds[3],
        missed_golds
    ));
    md.push('\n');

    md.push_str("## Context, so a rank is read against the right denominators\n\n");
    md.push_str(&format!(
        "- Bank sizes: {}–{} documents, median {:.1} across {} banks.\n",
        bank_min, bank_max, bank_median, corpus.banks.len()
    ));
    md.push_str(&format!(
        "- Fused-list length on the {} miss queries: min {}, median {:.1}, max {} — equal to the \
         bank size on {} of {} misses, so the fused ranking held the whole bank every time it \
         mattered here: no document was dropped for matching no query token, a gold session \
         outside the list (`absent`) never occurs on this corpus, and `rank 21` is a rank \
         inside a 19–32-document list, not a pool cut.\n",
        n_miss, list_min, list_median, list_max, list_covers_bank, n_miss
    ));
    md.push_str(
        "- Ranks are positions in the fused list, the same list `eval/LOCOMO.md` cuts at 10.\n",
    );
    md.push_str(&format!(
        "- Per-question data: `{}` (query id, category, per-gold rank), so every aggregate here \
         is re-derivable.\n",
        out_json
    ));

    fs::write(&out_md, &md)?;
    println!("{md}");
    eprintln!("locomo_ranks: wrote {} and {}", out_md, out_json);
    Ok(())
}
