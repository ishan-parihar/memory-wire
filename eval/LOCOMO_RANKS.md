# LoCoMo — where the missed gold sessions sit (H1.0)

## Provenance

| | |
|---|---|
| Commit under test | `84580d577cb3d0299da37ca9636081b1f3d30a2d` |
| Command | `cargo run --release --example locomo_ranks -- --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json --out-md eval/LOCOMO_RANKS.md --out-json eval/results_locomo_ranks.json` |
| Run | 2026-10-14, from `release`, on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| Granularity | session: one memory per document |
| Weights | `FusionWeights::SHIPPED`: bm25 1.0, overlap 0.25, agreement 0.0, k 60, recency 0.0 — the shipped configuration, the `0.25` row of `eval/LOCOMO.md` |
| Load at start / at end | `19.34 12.81 13.65 22/6614 491664` / `19.72 12.99 13.70 21/6617 492037` |
| Dataset | LoCoMo (Snap Research) via the canonical distribution `vectorize-io/agent-memory-benchmark`, `data/locomo/locomo10/`, pinned at upstream commit `decbb07f4f9899deac28a76293564cf263872652` and sha256-verified by `eval/download.sh` |
| **LLM involvement** | **none.** No model, no judge, no answer generation, no network call of any kind. |
| Queries | 1531 evaluated (9 dropped, empty gold set) |

The rank distribution is deterministic — same corpus, same index, same weights, same ranks — and there is no latency column, so the load row is recorded for completeness only.

## The question

`eval/LOCOMO.md` at this configuration serves every gold session to 83.0% of the 1531 evaluated queries (`recall_all@10`, measured on this run), at least one to 94.2% (`recall_any@10`, same run). A reordering lever can only promote a session already within reach, so this artifact answers one pre-registered question (**H1.0**, `docs/handoff/RND_PLAN.md`): for every miss, where does its **worst-ranked** gold session sit? It selects nothing and tunes nothing; it sizes the problem.

## Answer — worst-ranked gold per miss query

| worst gold | 11–15 | 16–20 | 21+ | absent | n | % of misses |
|---|---|---|---|---|---|---|
|  | 89 | 73 | 99 | 0 | 261 | 100.0% |

**Mostly within reach.** 162 of 261 misses hold their worst gold at rank 11–20 (62.1%), so a reordering lever (H1a–c) has something to promote and is worth building; 99 sit at 21+ and bound what reordering can recover.

## By category

| category | evaluated | misses | miss rate | 11–15 | 16–20 | 21+ | absent |
|---|---|---|---|---|---|---|---|
| multi-hop | 89 | 39 | 43.8% | 12 | 7 | 20 | 0 |
| open-domain | 841 | 25 | 3.0% | 14 | 4 | 7 | 0 |
| single-hop | 281 | 160 | 56.9% | 50 | 53 | 57 | 0 |
| temporal | 320 | 37 | 11.6% | 13 | 9 | 15 | 0 |

## By gold-set size

| gold sessions | evaluated | misses | miss rate | 11–15 | 16–20 | 21+ | absent |
|---|---|---|---|---|---|---|---|
| 1 | 1203 | 69 | 5.7% | 29 | 13 | 27 | 0 |
| 2 | 202 | 90 | 44.6% | 33 | 31 | 26 | 0 |
| 3 | 67 | 49 | 73.1% | 17 | 14 | 18 | 0 |
| ≥4 | 59 | 53 | 89.8% | 10 | 15 | 28 | 0 |

## Every missed gold session

The tables above are per query; a 3-gold query with two golds served contributes **one** row there. This table is per session: each of the 374 gold sessions a miss query left outside its top 10 contributes one row, same bins.

| missed gold session | 11–15 | 16–20 | 21+ | absent | n |
|---|---|---|---|---|---|
| rank | 146 | 113 | 115 | 0 | 374 |

## Context, so a rank is read against the right denominators

- Bank sizes: 19–32 documents, median 29.0 across 10 banks.
- Fused-list length on the 261 miss queries: min 19, median 29.0, max 32 — equal to the bank size on 261 of 261 misses, so the fused ranking held the whole bank every time it mattered here: no document was dropped for matching no query token, a gold session outside the list (`absent`) never occurs on this corpus, and `rank 21` is a rank inside a 19–32-document list, not a pool cut.
- Ranks are positions in the fused list, the same list `eval/LOCOMO.md` cuts at 10.
- Per-question data: `eval/results_locomo_ranks.json` (query id, category, per-gold rank), so every aggregate here is re-derivable.
