//! H4 — build the fixed 200-question LoCoMo subset for the reader-drowning run.
//!
//! ## Why this exists
//!
//! H4 (`docs/handoff/RND_PLAN.md`) needs a LoCoMo subset where gold coverage is
//! *fixed* while the served-set size grows: only questions whose **every** gold
//! session sits inside the shipped configuration's top 3, so at k = 3, 5 and 10
//! the reader always has all the gold it needs and the only variable is how much
//! distraction rides along. Accuracy falling across k on that subset would give
//! the precision levers (TODO #17) a dev set that exhibits the failure; accuracy
//! flat would close #17 as "not reproduced on independent data".
//!
//! ## The rule, pre-registered
//!
//! Eligibility: `recall_all@3` = 1 under `FusionWeights::SHIPPED`, session
//! granularity — the same list AMB's `rag` mode serves from. Selection: 200
//! queries, stratified proportionally over `meta.category`, drawn by a seeded
//! permutation (seed 42, `bench_common::shuffle` — the same generator
//! `examples/locomo.rs` uses) over the id-sorted eligible list per category,
//! with rounding reconciled to exactly 200. The manifest this writes is the
//! committed record; `eval/amb/h4_build_subset.py` derives the actual AMB input
//! file from it deterministically.
//!
//! ## Run
//!
//! ```text
//! cargo run --release --example locomo_h4_subset -- --out eval/h4_subset.json
//! ```

mod bench_common;

#[path = "../eval/locomo_dev.rs"]
mod dev;

use std::collections::BTreeMap;

use memory_wire::recall::FusionWeights;

/// The fixed subset size from the plan.
const N_SUBSET: usize = 200;
/// The seed, matching the corpus's own convention (`locomo` uses 42).
const SEED: u64 = 42;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let docs_path = bench_common::arg(&args, "--docs", "eval/data/locomo/documents.json");
    let queries_path = bench_common::arg(&args, "--data", "eval/data/locomo/queries.json");
    let out = bench_common::arg(&args, "--out", "locomo_h4_subset.json");

    let corpus = dev::Corpus::load(&docs_path, &queries_path)?;
    let planned = dev::plan(&corpus.queries, 0..corpus.queries.len());

    // Eligible: every gold id inside the top 3 of the shipped fused list.
    let mut eligible: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut evaluated_per_cat: BTreeMap<String, usize> = BTreeMap::new();
    for pq in &planned {
        let q = pq.query;
        let svc = &corpus.banks[&q.user_id];
        let hits = svc.recall_with_weights(&q.user_id, &q.query, corpus.budget_tokens, &FusionWeights::SHIPPED)?;
        let retrieved = dev::session_of(hits.into_iter().map(|h| h.memory.id));
        let top3: std::collections::HashSet<&str> =
            retrieved.iter().take(3).map(String::as_str).collect();
        let row = dev::Row::new(q, &retrieved, &pq.gold);
        *evaluated_per_cat.entry(row.category.clone()).or_default() += 1;
        if pq.gold.iter().all(|g| top3.contains(g.as_str())) {
            eligible.entry(row.category).or_default().push(q.id.clone());
        }
    }

    let total_eligible: usize = eligible.values().map(Vec::len).sum();
    eprintln!(
        "locomo_h4_subset: {} evaluated, {} eligible (all gold in top 3); per category: {:?}",
        planned.len(),
        total_eligible,
        eligible.iter().map(|(k, v)| (k, v.len())).collect::<Vec<_>>()
    );
    anyhow::ensure!(
        total_eligible >= N_SUBSET,
        "locomo_h4_subset: only {total_eligible} eligible queries, cannot draw {N_SUBSET}"
    );

    // Proportional allocation with largest-remainder reconciliation to exactly
    // N_SUBSET — deterministic given the eligible counts, no tie-breaking luck.
    let mut alloc: Vec<(String, usize)> = Vec::new();
    let mut exact_sum = 0usize;
    for (cat, ids) in &eligible {
        let exact = N_SUBSET as f64 * ids.len() as f64 / total_eligible as f64;
        let floor = exact.floor() as usize;
        alloc.push((cat.clone(), floor));
        exact_sum += floor;
    }
    let mut remainder: Vec<(usize, f64, String)> = alloc
        .iter()
        .enumerate()
        .map(|(i, (cat, floor))| {
            let ids_len = eligible[cat].len();
            (
                i,
                N_SUBSET as f64 * ids_len as f64 / total_eligible as f64 - *floor as f64,
                cat.clone(),
            )
        })
        .collect();
    // Descending fractional part, then category name for a stable tie order.
    remainder.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.2.cmp(&b.2))
    });
    let mut left = N_SUBSET - exact_sum;
    for (i, _, _) in &remainder {
        if left == 0 {
            break;
        }
        alloc[*i].1 += 1;
        left -= 1;
    }

    // The draw itself: id-sorted eligible list per category, seeded permutation,
    // first n. Same generator and seed as the corpus harness, so the subset is
    // reproducible from this file alone.
    let mut chosen: Vec<(String, String)> = Vec::new();
    for (cat, n) in &alloc {
        let ids = &eligible[cat];
        let order = bench_common::shuffle(ids.len(), SEED);
        for &i in order.iter().take(*n) {
            chosen.push((cat.clone(), ids[i].clone()));
        }
    }
    chosen.sort_by(|a, b| a.1.cmp(&b.1));

    let manifest = serde_json::json!({
        "rule": "recall_all@3 = 1 under FusionWeights::SHIPPED, session granularity; \
                 200 proportional per meta.category, largest-remainder reconciliation, \
                 seeded permutation (seed 42, bench_common::shuffle) over the id-sorted \
                 eligible list",
        "n_subset": N_SUBSET,
        "seed": SEED,
        "evaluated_per_category": evaluated_per_cat,
        "eligible_per_category": eligible.iter().map(|(k, v)| (k.clone(), v.len())).collect::<BTreeMap<String, usize>>(),
        "allocated_per_category": alloc.iter().cloned().collect::<BTreeMap<String, usize>>(),
        "queries": chosen.iter().map(|(cat, id)| serde_json::json!({
            "id": id,
            "category": cat,
        })).collect::<Vec<_>>(),
    });
    std::fs::write(&out, serde_json::to_string_pretty(&manifest)?)?;
    eprintln!(
        "locomo_h4_subset: wrote {} — {} queries, allocation {:?}",
        out,
        chosen.len(),
        alloc
    );
    Ok(())
}
