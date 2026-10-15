//! H1a/H1b/H1c/H3 — pre-registered, single-value lever tests on the LoCoMo dev set.
//!
//! ## Why this exists
//!
//! H1.0 (`examples/locomo_ranks.rs`, `CONSISTENCY.md` §36) settled that 162 of 261
//! `recall_all@10` misses hold their worst gold at rank 11–20 — within reach of a
//! reordering lever — so `docs/handoff/RND_PLAN.md` H1a–c are worth *measuring*, and
//! H3 is a falsification test now that H0 made the corpus date-aware. This harness
//! runs the shipped configuration as an in-run control plus one pre-registered
//! value per lever — no sweeps, no selection: every value below was written down in
//! the plan before this file existed.
//!
//! ## What is measured
//!
//! Per lever: the full metric vector (`recall_any@{1,5,10,20}`, `recall_all@10`,
//! NDCG@10, MRR, R@pool) against the shipped control, per category, with per-query
//! up/down counts and a count of queries whose whole retrieved list is unchanged.
//! The shipped arm must reproduce `eval/LOCOMO.md`'s `0.25` row — it is the same
//! corpus, same granularity, same weights — or nothing below is readable.
//!
//! The gates (from the plan, rendered from the measured numbers so a re-run cannot
//! leave a stale verdict above fresh data):
//!
//! - **H1a–c** — `recall_all@10` **+> 1 pp** over shipped, `recall_all@10`
//!   up-count > down-count, no category regression on R@1 / R@5 / NDCG@10 (the
//!   harness is deterministic, so any regression is real), R@20 not down.
//! - **H3** — temporal-category `recall_all@10` and R@1 up, every *non-temporal*
//!   query's retrieved list bit-identical to shipped (the
//!   `TemporalQueriesOnly` contract, asserted at run time: a non-temporal query
//!   whose list moved fails the run).
//!
//! ## Run
//!
//! ```text
//! cargo run --release --example locomo_levers -- \
//!   --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json
//! ```
//!
//! A bare run writes to `$TMPDIR`, never to `eval/` (`bench_common::out_md`).

mod bench_common;

#[path = "../eval/locomo_dev.rs"]
mod dev;

use std::collections::BTreeMap;

use memory_wire::recall::{is_temporal_query, FusionWeights, KeywordScope, OverlapScope, RecencyPolicy};

/// The levers under test, one pre-registered value each. Values come from
/// `docs/handoff/RND_PLAN.md` §H1/H3, written there before this harness; nothing
/// here is chosen by looking at a number this harness produces. The first entry is
/// the shipped control and must run first — everything is differenced against it.
fn levers() -> Vec<(&'static str, FusionWeights)> {
    vec![
        ("shipped", FusionWeights::SHIPPED),
        // H1a: BM25 sees every query token as a quoted prefix, `"token"*`. The
        // mechanism: English morphology makes exact-token matching miss
        // migrate/migration variants. `KeywordScope`'s rustdoc records that a
        // prefix expanding past one index term scores ~1e-6, so the realistic
        // hope is small and the run decides.
        (
            "h1a keyword_scope ExactAndPrefix",
            FusionWeights {
                keyword_scope: KeywordScope::ExactAndPrefix,
                ..FusionWeights::SHIPPED
            },
        ),
        // H1b: the overlap stream scores the single densest segment instead of
        // the whole document, so one strong sentence is not diluted by session
        // length — the documented `BestSentence` rationale.
        (
            "h1b overlap_scope BestSentence",
            FusionWeights {
                overlap_scope: OverlapScope::BestSentence,
                ..FusionWeights::SHIPPED
            },
        ),
        // H1c: the BM25-magnitude stream at the RRF-sized term weight 1.0. The
        // one axis with prior partial evidence (`eval/SELECTION.md` walked it on
        // R@5 before `recall_all@10` existed); one value, no scan.
        (
            "h1c bm25_magnitude 1.0",
            FusionWeights {
                bm25_magnitude: 1.0,
                ..FusionWeights::SHIPPED
            },
        ),
        // H3: the recency stream at the RRF-sized 1.0, gated to queries
        // `is_temporal_query` reads as time-denoting. On a corpus whose
        // timestamps were ingest clock until H0 this was a measurement of
        // nothing; now it is a falsification test — LoCoMo temporal questions
        // ask *when a specific event happened*, so a recency prior may not help.
        // Half-life stays the shipped placeholder (`RECENCY_HALF_LIFE_PLACEHOLDER_DAYS`).
        (
            "h3 recency 1.0 TemporalQueriesOnly",
            FusionWeights {
                recency: 1.0,
                recency_policy: RecencyPolicy::TemporalQueriesOnly,
                ..FusionWeights::SHIPPED
            },
        ),
    ]
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let load_start = dev::loadavg();
    let docs_path = bench_common::arg(&args, "--docs", "eval/data/locomo/documents.json");
    let queries_path = bench_common::arg(&args, "--data", "eval/data/locomo/queries.json");
    let out_md = bench_common::out_md(&args, "LOCOMO_LEVERS.md");
    let out_json = match bench_common::flag(&args, "--out-json") {
        Some(p) => p,
        None => {
            let p = std::env::temp_dir().join("results_locomo_levers.json");
            eprintln!("no --out-json given: writing per-question data to {}", p.display());
            p.display().to_string()
        }
    };
    if bench_common::profile() != "release" {
        eprintln!(
            "WARNING: not `--release`; the metric vector is deterministic so the numbers \
             stand, but the provenance line names the profile and every other committed \
             artifact was taken from `release`."
        );
    }

    let corpus = dev::Corpus::load(&docs_path, &queries_path)?;
    let planned = dev::plan(&corpus.queries, 0..corpus.queries.len());
    let levers = levers();
    eprintln!(
        "locomo_levers: {} documents in {} banks, {} queries, {} levers; loadavg at start {}",
        corpus.documents.len(),
        corpus.banks.len(),
        planned.len(),
        levers.len(),
        load_start
    );

    let mut agg: Vec<dev::Agg> = levers.iter().map(|_| dev::Agg::default()).collect();
    let mut per_cat: Vec<BTreeMap<String, dev::Agg>> =
        levers.iter().map(|_| BTreeMap::new()).collect();
    // Per query, per lever: the metric row, so the committed JSON re-derives every
    // aggregate in this artifact. The shipped arm's retrieved lists are kept too.
    let mut all_rows: Vec<Vec<dev::Row>> = Vec::with_capacity(planned.len());
    let mut base_lists: Vec<Vec<String>> = Vec::with_capacity(planned.len());
    // Per lever: up/down counts for recall_all@10 and R@5, and how many queries'
    // whole retrieved list is unchanged.
    let mut all10_updown: Vec<(usize, usize)> = levers.iter().map(|_| (0, 0)).collect();
    let mut r5_updown: Vec<(usize, usize)> = levers.iter().map(|_| (0, 0)).collect();
    let mut identical: Vec<usize> = levers.iter().map(|_| 0).collect();
    // H3's contract, as the mechanism defines it: `RecencyPolicy::TemporalQueriesOnly`
    // is gated by the *grammatical* detector `is_temporal_query`, not by LoCoMo's
    // category label — the detector fires on "when"/"day"/"session" questions in
    // every category. The plan's "non-temporal queries bit-identical" therefore
    // means detector-negative queries; a detector-negative query whose list
    // moved fails the run. The detector's cross-category spillover is itself
    // part of what H3 measures and is reported, not asserted away.
    let mut h3_violations: Vec<String> = Vec::new();
    let mut h3_fired = 0usize;
    let mut h3_fired_by_cat: BTreeMap<String, usize> = BTreeMap::new();
    let mut h3_n_nontemporal = 0usize;
    let mut h3_identical_nontemporal = 0usize;

    let start = std::time::Instant::now();
    for pq in &planned {
        let q = pq.query;
        let svc = &corpus.banks[&q.user_id];
        let mut this_rows: Vec<dev::Row> = Vec::with_capacity(levers.len());
        let mut this_lists: Vec<Vec<String>> = Vec::with_capacity(levers.len());
        for (li, (_, weights)) in levers.iter().enumerate() {
            let hits = svc.recall_with_weights(&q.user_id, &q.query, corpus.budget_tokens, weights)?;
            let retrieved = dev::session_of(hits.into_iter().map(|h| h.memory.id));
            let row = dev::Row::new(q, &retrieved, &pq.gold);
            agg[li].add(&row);
            per_cat[li].entry(row.category.clone()).or_default().add(&row);
            this_rows.push(row);
            this_lists.push(retrieved);
        }
        let base_row = &this_rows[0];
        let fires = is_temporal_query(&q.query);
        if fires {
            h3_fired += 1;
            *h3_fired_by_cat.entry(base_row.category.clone()).or_default() += 1;
        } else {
            h3_n_nontemporal += 1;
        }
        for (li, _) in levers.iter().enumerate().skip(1) {
            if this_rows[li].all10 > base_row.all10 {
                all10_updown[li].0 += 1;
            } else if this_rows[li].all10 < base_row.all10 {
                all10_updown[li].1 += 1;
            }
            if this_rows[li].r5 > base_row.r5 {
                r5_updown[li].0 += 1;
            } else if this_rows[li].r5 < base_row.r5 {
                r5_updown[li].1 += 1;
            }
            if this_lists[li] == this_lists[0] {
                identical[li] += 1;
            }
            // H3 runs as the last lever; a detector-negative query whose list
            // moved breaks the TemporalQueriesOnly contract.
            if li == levers.len() - 1 {
                let same = this_lists[li] == this_lists[0];
                if !fires {
                    if same {
                        h3_identical_nontemporal += 1;
                    } else {
                        h3_violations.push(q.id.clone());
                    }
                }
            }
        }
        base_lists.push(this_lists.remove(0));
        all_rows.push(this_rows);
    }
    eprintln!(
        "locomo_levers: {} queries x {} levers in {:?}; loadavg at end {}",
        planned.len(),
        levers.len(),
        start.elapsed(),
        dev::loadavg()
    );
    if !h3_violations.is_empty() {
        // The control failed: report and refuse to render verdicts over it.
        anyhow::bail!(
            "h3 control: {} detector-negative queries changed their retrieved list under \
             TemporalQueriesOnly, first {}: the policy leaks — nothing in this artifact \
             is readable until that is fixed",
            h3_violations.len(),
            h3_violations[0]
        );
    }

    // ---- per-question dump ------------------------------------------------------
    let lever_names: Vec<&str> = levers.iter().map(|(n, _)| *n).collect();
    let dump: Vec<serde_json::Value> = planned
        .iter()
        .enumerate()
        .map(|(qi, pq)| {
            let rows = &all_rows[qi];
            let levers_json: serde_json::Map<String, serde_json::Value> = lever_names
                .iter()
                .zip(rows.iter())
                .map(|(name, r)| {
                    (
                        name.to_string(),
                        serde_json::json!({
                            "r1": r.r1, "r5": r.r5, "r10": r.r10, "r20": r.r20,
                            "all10": r.all10, "ndcg10": r.ndcg10, "mrr": r.mrr,
                        }),
                    )
                })
                .collect();
            serde_json::json!({
                "query_id": pq.query.id,
                "category": rows[0].category,
                "shipped_retrieved": base_lists[qi],
                "levers": levers_json,
            })
        })
        .collect();
    std::fs::write(&out_json, serde_json::to_string_pretty(&dump)?)?;

    // ---- gate verdicts, derived from the measurements ---------------------------
    let cats: Vec<String> = per_cat[0].keys().cloned().collect();
    let gate_h1 = |li: usize| -> String {
        let d_all10 = agg[li].all10() - agg[0].all10();
        let d_r20 = agg[li].r20() - agg[0].r20();
        let (up, down) = all10_updown[li];
        let mut regressions: Vec<String> = Vec::new();
        for cat in &cats {
            let l = &per_cat[li][cat];
            let b = &per_cat[0][cat];
            for (metric, lv, bv) in [
                ("R@1", l.r1(), b.r1()),
                ("R@5", l.r5(), b.r5()),
                ("NDCG@10", l.ndcg10(), b.ndcg10()),
            ] {
                if lv < bv {
                    regressions.push(format!("{cat} {metric} {bv:.1}->{lv:.1}"));
                }
            }
        }
        let pass = d_all10 > 1.0 && up > down && regressions.is_empty() && d_r20 >= 0.0;
        format!(
            "{} — `recall_all@10` {:+.1}pp (up {up}/down {down}), R@20 {:+.1}pp, category \
             regressions: {}",
            if pass { "**PASS**" } else { "**FAIL**" },
            d_all10,
            d_r20,
            if regressions.is_empty() {
                "none".to_string()
            } else {
                regressions.join("; ")
            },
        )
    };
    let gate_h3 = |li: usize| -> String {
        let t = &per_cat[li]["temporal"];
        let t0 = &per_cat[0]["temporal"];
        let d_all10 = t.all10() - t0.all10();
        let d_r1 = t.r1() - t0.r1();
        // The plan's letter says "other categories unchanged"; the policy's
        // mechanism (grammatical detection, not category labels) makes that
        // clause unsatisfiable — the stream fires on lexically temporal queries
        // in every category. The honored contract is the detector's negative
        // set; the cross-category spillover is reported in the verdict text.
        let spillover: Vec<String> = h3_fired_by_cat
            .iter()
            .filter(|(c, _)| c.as_str() != "temporal")
            .map(|(c, n)| format!("{c} {n}"))
            .collect();
        let pass = d_all10 > 0.0 && d_r1 > 0.0 && h3_identical_nontemporal == h3_n_nontemporal;
        format!(
            "{} — temporal `recall_all@10` {:+.1}pp, temporal R@1 {:+.1}pp; detector fired on \
             {} queries (category temporal: {}, spillover: {}); detector-negative lists \
             identical {}/{} (asserted at run time)",
            if pass { "**PASS**" } else { "**FAIL**" },
            d_all10,
            d_r1,
            h3_fired,
            h3_fired_by_cat.get("temporal").copied().unwrap_or(0),
            spillover.join(", "),
            h3_identical_nontemporal,
            h3_n_nontemporal,
        )
    };

    // ---- artifact -----------------------------------------------------------------
    let mut md = String::new();
    md.push_str("# LoCoMo lever A/B — H1a, H1b, H1c, H3 (dev set, single pre-registered values)\n\n");
    md.push_str("## Provenance\n\n| | |\n|---|---|\n");
    md.push_str(&format!("| Commit under test | `{}` |\n", dev::git_head()));
    md.push_str(&format!(
        "| Command | `cargo run --release --example locomo_levers -- --docs {} --data {} --out-md {} \
         --out-json {}` |\n",
        docs_path, queries_path, out_md, out_json
    ));
    md.push_str(&format!(
        "| Run | {}, from `{}`, on {} |\n",
        chrono::Utc::now().format("%Y-%m-%d"),
        bench_common::profile(),
        bench_common::host()
    ));
    md.push_str(&format!("| Load at start / at end | `{}` / `{}` |\n", load_start, dev::loadavg()));
    md.push_str(&format!(
        "| Dataset | LoCoMo (Snap Research) via the canonical distribution \
         `vectorize-io/agent-memory-benchmark`, `data/locomo/locomo10/`, pinned at upstream commit \
         `{}` and sha256-verified by `eval/download.sh`; corpus is date-aware since H0 \
         (each memory carries its own `timestamp`) |\n",
        dev::UPSTREAM_REF
    ));
    md.push_str("| **LLM involvement** | **none.** Retrieval metrics only, no network call. |\n");
    md.push_str(&format!(
        "| Queries | {} evaluated ({} dropped, empty gold set) |\n\n",
        planned.len(),
        corpus.skipped_no_gold
    ));
    md.push_str("The shipped arm must equal `eval/LOCOMO.md`'s `0.25` row; the harness is \
        deterministic and there is no latency column, so the load row is for completeness only.\n\n");

    md.push_str("## Aggregate, every lever vs shipped\n\n");
    md.push_str(
        "| lever | R@1 | R@5 | R@10 | R@20 | recall_all@10 | NDCG@10 | MRR | R@pool | \
         Δall10 | all10 up/down | lists unchanged |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for (li, (name, _)) in levers.iter().enumerate() {
        let a = &agg[li];
        md.push_str(&format!(
            "| {} | {:.1}% | {:.1}% | {:.1}% | {:.1}% | {:.1}% | {:.1}% | {:.1}% | {:.1}% | \
             {:+.1}pp | {}/{} | {}/{} |\n",
            name,
            a.r1(),
            a.r5(),
            a.r10(),
            a.r20(),
            a.all10(),
            a.ndcg10(),
            a.mrr(),
            a.pct(a.pool),
            a.all10() - agg[0].all10(),
            all10_updown[li].0,
            all10_updown[li].1,
            identical[li],
            planned.len(),
        ));
    }
    md.push('\n');

    for (metric, get) in [
        ("recall_all@10", 0usize),
        ("R@1", 1),
        ("R@5", 2),
        ("NDCG@10", 3),
    ] {
        md.push_str(&format!("## {} per category\n\n", metric));
        md.push_str(&format!(
            "| lever | {} | overall (n {}) |\n|---|---|---|\n",
            cats.join(" | "),
            planned.len()
        ));
        for (li, (name, _)) in levers.iter().enumerate() {
            let cells: Vec<String> = cats
                .iter()
                .map(|c| {
                    let (l, b) = (&per_cat[li][c], &per_cat[0][c]);
                    let (lv, bv) = match get {
                        0 => (l.all10(), b.all10()),
                        1 => (l.r1(), b.r1()),
                        2 => (l.r5(), b.r5()),
                        _ => (l.ndcg10(), b.ndcg10()),
                    };
                    if li == 0 {
                        format!("{:.1}%", lv)
                    } else {
                        format!("{:.1}% ({:+.1})", lv, lv - bv)
                    }
                })
                .collect();
            let overall = match get {
                0 => agg[li].all10(),
                1 => agg[li].r1(),
                2 => agg[li].r5(),
                _ => agg[li].ndcg10(),
            };
            md.push_str(&format!("| {} | {} | {:.1}% |\n", name, cells.join(" | "), overall));
        }
        md.push('\n');
    }

    md.push_str("## Gates (pre-registered, derived from the tables above)\n\n");
    for (li, _) in levers.iter().enumerate().skip(1) {
        let (name, _) = levers[li];
        if name.starts_with("h3") {
            md.push_str(&format!("- **{}**: {}\n", name, gate_h3(li)));
        } else {
            md.push_str(&format!("- **{}**: {}\n", name, gate_h1(li)));
        }
    }
    md.push_str(
        "\nA LoCoMo null closes the lever on the dev set (`docs/handoff/CLOSED.md`); a pass \
         earns a single LongMemEval measurement, counted in `docs/CONSISTENCY.md`.\n",
    );
    md.push_str(&format!(
        "\nPer-question data (shipped rows + retrieved lists): `{}`.\n",
        out_json
    ));

    std::fs::write(&out_md, &md)?;
    println!("{md}");
    eprintln!("locomo_levers: wrote {} and {}", out_md, out_json);
    Ok(())
}
