# LoCoMo lever A/B — H1a, H1b, H1c, H3 (dev set, single pre-registered values)

## Provenance

| | |
|---|---|
| Commit under test | `84580d577cb3d0299da37ca9636081b1f3d30a2d` |
| Command | `cargo run --release --example locomo_levers -- --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json --out-md eval/LOCOMO_LEVERS.md --out-json eval/results_locomo_levers.json` |
| Run | 2026-10-14, from `release`, on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| Load at start / at end | `37.62 31.92 28.57 16/6112 1428829` / `35.49 31.87 28.64 24/6099 1431741` |
| Dataset | LoCoMo (Snap Research) via the canonical distribution `vectorize-io/agent-memory-benchmark`, `data/locomo/locomo10/`, pinned at upstream commit `decbb07f4f9899deac28a76293564cf263872652` and sha256-verified by `eval/download.sh`; corpus is date-aware since H0 (each memory carries its own `timestamp`) |
| **LLM involvement** | **none.** Retrieval metrics only, no network call. |
| Queries | 1531 evaluated (9 dropped, empty gold set) |

The shipped arm must equal `eval/LOCOMO.md`'s `0.25` row; the harness is deterministic and there is no latency column, so the load row is for completeness only.

## Aggregate, every lever vs shipped

| lever | R@1 | R@5 | R@10 | R@20 | recall_all@10 | NDCG@10 | MRR | R@pool | Δall10 | all10 up/down | lists unchanged |
|---|---|---|---|---|---|---|---|---|---|---|---|
| shipped | 60.4% | 86.0% | 94.2% | 98.0% | 83.0% | 73.6% | 71.8% | 100.0% | +0.0pp | 0/0 | 0/1531 |
| h1a keyword_scope ExactAndPrefix | 63.4% | 87.7% | 93.9% | 98.6% | 83.5% | 75.3% | 74.0% | 100.0% | +0.5pp | 34/26 | 42/1531 |
| h1b overlap_scope BestSentence | 57.0% | 86.8% | 93.6% | 98.1% | 82.6% | 72.5% | 69.9% | 100.0% | -0.3pp | 14/19 | 0/1531 |
| h1c bm25_magnitude 1.0 | 60.4% | 86.2% | 94.1% | 98.0% | 82.8% | 73.6% | 71.7% | 100.0% | -0.2pp | 4/7 | 249/1531 |
| h3 recency 1.0 TemporalQueriesOnly | 48.9% | 78.5% | 92.2% | 98.0% | 80.4% | 65.6% | 62.2% | 100.0% | -2.5pp | 8/47 | 1025/1531 |

## recall_all@10 per category

| lever | multi-hop | open-domain | single-hop | temporal | overall (n 1531) |
|---|---|---|
| shipped | 56.2% | 97.0% | 43.1% | 88.4% | 83.0% |
| h1a keyword_scope ExactAndPrefix | 53.9% (-2.2) | 97.0% (+0.0) | 46.6% (+3.6) | 88.4% (+0.0) | 83.5% |
| h1b overlap_scope BestSentence | 56.2% (+0.0) | 96.6% (-0.5) | 43.8% (+0.7) | 87.5% (-0.9) | 82.6% |
| h1c bm25_magnitude 1.0 | 57.3% (+1.1) | 96.9% (-0.1) | 42.3% (-0.7) | 88.1% (-0.3) | 82.8% |
| h3 recency 1.0 TemporalQueriesOnly | 56.2% (+0.0) | 95.8% (-1.2) | 41.6% (-1.4) | 80.6% (-7.8) | 80.4% |

## R@1 per category

| lever | multi-hop | open-domain | single-hop | temporal | overall (n 1531) |
|---|---|---|
| shipped | 37.1% | 67.8% | 49.5% | 57.2% | 60.4% |
| h1a keyword_scope ExactAndPrefix | 38.2% (+1.1) | 71.0% (+3.2) | 50.2% (+0.7) | 61.9% (+4.7) | 63.4% |
| h1b overlap_scope BestSentence | 25.8% (-11.2) | 66.5% (-1.3) | 39.1% (-10.3) | 56.2% (-0.9) | 57.0% |
| h1c bm25_magnitude 1.0 | 33.7% (-3.4) | 68.5% (+0.7) | 47.0% (-2.5) | 58.4% (+1.2) | 60.4% |
| h3 recency 1.0 TemporalQueriesOnly | 34.8% (-2.2) | 60.5% (-7.3) | 47.0% (-2.5) | 23.8% (-33.4) | 48.9% |

## R@5 per category

| lever | multi-hop | open-domain | single-hop | temporal | overall (n 1531) |
|---|---|---|
| shipped | 67.4% | 91.8% | 79.4% | 81.6% | 86.0% |
| h1a keyword_scope ExactAndPrefix | 69.7% (+2.2) | 93.7% (+1.9) | 79.0% (-0.4) | 84.4% (+2.8) | 87.7% |
| h1b overlap_scope BestSentence | 66.3% (-1.1) | 92.7% (+1.0) | 80.4% (+1.1) | 82.5% (+0.9) | 86.8% |
| h1c bm25_magnitude 1.0 | 66.3% (-1.1) | 91.7% (-0.1) | 79.7% (+0.4) | 82.8% (+1.2) | 86.2% |
| h3 recency 1.0 TemporalQueriesOnly | 66.3% (-1.1) | 87.9% (-3.9) | 76.2% (-3.2) | 59.4% (-22.2) | 78.5% |

## NDCG@10 per category

| lever | multi-hop | open-domain | single-hop | temporal | overall (n 1531) |
|---|---|---|
| shipped | 50.0% | 82.6% | 54.9% | 72.8% | 73.6% |
| h1a keyword_scope ExactAndPrefix | 50.0% (+0.0) | 84.5% (+1.9) | 55.4% (+0.4) | 75.6% (+2.7) | 75.3% |
| h1b overlap_scope BestSentence | 45.4% (-4.5) | 82.1% (-0.5) | 52.5% (-2.4) | 72.1% (-0.7) | 72.5% |
| h1c bm25_magnitude 1.0 | 49.1% (-0.8) | 82.8% (+0.1) | 54.5% (-0.5) | 73.3% (+0.4) | 73.6% |
| h3 recency 1.0 TemporalQueriesOnly | 49.0% (-1.0) | 77.8% (-4.8) | 53.1% (-1.9) | 49.2% (-23.7) | 65.6% |

## Gates (pre-registered, derived from the tables above)

- **h1a keyword_scope ExactAndPrefix**: **FAIL** — `recall_all@10` +0.5pp (up 34/down 26), R@20 +0.6pp, category regressions: single-hop R@5 79.4->79.0
- **h1b overlap_scope BestSentence**: **FAIL** — `recall_all@10` -0.3pp (up 14/down 19), R@20 +0.1pp, category regressions: multi-hop R@1 37.1->25.8; multi-hop R@5 67.4->66.3; multi-hop NDCG@10 50.0->45.4; open-domain R@1 67.8->66.5; open-domain NDCG@10 82.6->82.1; single-hop R@1 49.5->39.1; single-hop NDCG@10 54.9->52.5; temporal R@1 57.2->56.2; temporal NDCG@10 72.8->72.1
- **h1c bm25_magnitude 1.0**: **FAIL** — `recall_all@10` -0.2pp (up 4/down 7), R@20 +0.0pp, category regressions: multi-hop R@1 37.1->33.7; multi-hop R@5 67.4->66.3; multi-hop NDCG@10 50.0->49.1; open-domain R@5 91.8->91.7; single-hop R@1 49.5->47.0; single-hop NDCG@10 54.9->54.5
- **h3 recency 1.0 TemporalQueriesOnly**: **FAIL** — temporal `recall_all@10` -7.8pp, temporal R@1 -33.4pp; detector fired on 506 queries (category temporal: 287, spillover: multi-hop 9, open-domain 179, single-hop 31); detector-negative lists identical 1025/1025 (asserted at run time)

A LoCoMo null closes the lever on the dev set (`docs/handoff/CLOSED.md`); a pass earns a single LongMemEval measurement, counted in `docs/CONSISTENCY.md`.

Per-question data (shipped rows + retrieved lists): `eval/results_locomo_levers.json`.
