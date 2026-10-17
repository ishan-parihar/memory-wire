# BEAM-100k retrieval results (memory-wire) — the pool-scale re-test

## Provenance

| | |
|---|---|
| Commit under test | `c919f13a2a75760e48c54daa4a2248c5cb99d917` |
| Command | `cargo run --release --example beam -- --docs eval/data/beam/documents.json --data eval/data/beam/queries.json --out-md eval/BEAM.md --out-json eval/results_beam.json` |
| Run | 2026-10-16, from `--release`, on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| Load at start / at end | `71.38 79.50 92.95 57/7421 3649759` / `71.12 78.77 92.35 95/7371 3663284` |
| Dataset | BEAM 100K (ICLR 2026) via HF `Mohammadta/BEAM` split `100K`, fetched by the pinned OmniMemEval `prepare_beam.py`; **CC BY-SA 4.0, run from the gitignored corpus, never vendored**; the labels digest `eval/beam_labels_100k.json` names the input sha256 |
| Document unit | **QA turn pair** (user turn + its assistant reply), 94–196 per bank — see the module docs and §6.1 for why the BEAM "session" (3–5 per conversation, ~100K chars) is a shape the product never writes |
| Occurrence times | each pair carries its turn's `time_anchor`, so every memory is dated; the shipped recency weight is 0.0 |
| **LLM involvement** | **none.** No model, no judge, no answer generation, no network call. `gold_answers` (the upstream `ideal_response`) is read for shape and never scored. |

The retrieval metrics do not depend on machine load and there is **no latency column**, so the load row is recorded for completeness only. The box was not quiet when this ran.

## The question

Pre-registered in `docs/handoff/RND_PLAN.md` §6.1: does `overlap: 0.25` beat the stream-dropped `0.00` at pool scale — the reservation `docs/CONSISTENCY.md` §14.2 recorded when the LoCoMo replication showed a tie? **A tie here means the stream is deleteable; a win means it finally has a measured justification. Acting on `FusionWeights::SHIPPED` is the user's call either way.** The tie bar was fixed before the run: every ordering metric within ±1pp.

## Comparability — read before quoting any number from this file

**Nothing here is a BEAM score.** BEAM's standard metric is LLM-judged nugget recall/precision against a reference set this artifact never builds. This harness measures `recall_any@K` over agent-authored gold pair ids — a different measurement on a different scale, produced by different labelling. Do not place a number below next to any published BEAM figure. What *is* comparable is the internal comparison: every row against every other, same corpus, same index, same labels, same run.

The labels are agent-authored by content-match (`eval/beam_label.py`) on 313 of 400 questions; 80 (`abstention`, `summarization`) have no localizable gold by design; 7 resolved to nothing. Review agreement on a 15-question sample: 13 carried the answer-bearing pair, 2 were partial — the authoring-weakness caveat sits beside every number in this file, and **both arms score byte-identical labels, so label noise dampens the delta both ways; it cannot manufacture one**.

## Method

One bank per conversation, one memory per QA turn pair, built once; every weight scored on that one index. At 94–196 documents per bank the pool regime §14.2 named is finally live: recall's 200-row candidate window covers the bank, **BM25 truncates at 50 in SQL**, and the fused list is the union of the streams — so the fused pool mean is 99.5 documents (max 100). **`R@pool` behaves differently per arm.** Between the keep-stream arms (`1.00`, `0.25`) the weights only reorder, so the pool is shared and their deltas are pure ordering — in those arms 0 of 313 questions miss the pool. The stream-dropped `0.00` arm can lose pool membership outright: a gold pair that only the overlap stream surfaces sits below BM25's 50-row SQL cut and is unreachable at every K. Its `R@pool` in the grid is therefore the coverage price of deletion, and the grid's Δ column prices it explicitly.

## The grid

The weight on the token-overlap stream; every other weight at `FusionWeights::SHIPPED`. `1.00` is the unfitted equal-weight control; `0.25` the fitted shipped value; `0.00` the stream-dropped bound — the deletion candidate. Δ columns are against `1.00`.

| overlap | R@1 | ΔR@1 | R@5 | ΔR@5 | R@10 | R@20 | recall_all@10 | NDCG@10 | ΔNDCG | MRR | ΔMRR | R@pool | R@5 up/down/same | n |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1.00 | 42.5% | +0.0 | 67.1% | +0.0 | 77.6% | 90.7% | 21.1% | 42.3% | +0.0 | 54.1% | +0.0 | 100.0% | 0/0/313 | 313 |
| 0.25 | 39.9% | -2.6 | 63.6% | -3.5 | 76.4% | 87.2% | 21.1% | 41.2% | -1.1 | 51.9% | -2.2 | 100.0% | 8/19/286 | 313 |
| 0.00 | 36.4% | -6.1 | 62.3% | -4.8 | 74.8% | 85.9% | 20.4% | 39.3% | -3.0 | 48.7% | -5.4 | 95.5% | 13/28/272 | 313 |

## The answer

**The deletion question answers 'keep': the stream is not deleteable at pool scale.** `0.25` over `0.00` on 313 questions: R@1 +3.5pp, R@5 +1.3pp, R@10 +1.6pp, NDCG@10 +1.9pp, MRR +3.2pp (per-query R@5: 7 up / 11 down for the dropped arm). The dropped arm also pays a coverage price: R@pool 95.5% vs 100.0% — a gold pair that only the overlap stream surfaces sits below BM25's 50-row SQL cut and is unreachable at every K without it. §14.2's reservation was right: on 19–32-document LoCoMo banks the second lexical voter could only reorder ties; on 94–196-document banks it also carries coverage, and the ordering deltas above are its work.

**The control comparison — `0.25` against the unfitted `1.00` — points against the fitted value.** R@1 -2.6pp, R@5 -3.5pp, NDCG@10 -1.1pp, MRR -2.2pp, per-query R@5 8 up / 19 down for the fitted arm. For scale: LoCoMo gave the fitted value +1.4pp R@5 over equal weight (`eval/LOCOMO.md`); this corpus moves the other way. Two dev corpora, two directions, neither dominant beyond its own noise — the fitted value's out-of-sample edge exists on one and inverts on the other.

**What this settles and what it does not.** Settled, by measurement: the token-overlap stream stays — the pre-registered deletion consequence did not fire, and the stream has its first pool-scale justification (ordering, plus the coverage the R@pool row prices). Not settled: the weight. `1.00` leads here, `0.25` led on LoCoMo; picking a value off either table is a dev-set selection that must be recorded as one in `docs/CONSISTENCY.md`, and **`FusionWeights::SHIPPED` is the user's to change** (`docs/handoff/README.md`). This artifact reports both comparisons and patches nothing.

The deletion comparison, per-query R@5 of `0.00` against `0.25`: **7 improved, 11 regressed, 295 unchanged** of 313. The fitted-vs-control movement (`0.25` against `1.00`): 8 improved, 19 regressed, 286 unchanged.

## R@1, per BEAM dimension

| overlap | contradiction_resolution | event_ordering | information_extraction | instruction_following | knowledge_update | multi_session_reasoning | preference_following | temporal_reasoning | overall |
|---|---|---|---|---|---|---|---|---|---|
| 1.00 | 92.5% (+0.0, n=40) | 18.4% (+0.0, n=38) | 47.5% (+0.0, n=40) | 12.5% (+0.0, n=40) | 50.0% (+0.0, n=38) | 40.5% (+0.0, n=37) | 17.5% (+0.0, n=40) | 60.0% (+0.0, n=40) | 42.5% |
| 0.25 | 87.5% (-5.0, n=40) | 18.4% (+0.0, n=38) | 42.5% (-5.0, n=40) | 10.0% (-2.5, n=40) | 44.7% (-5.3, n=38) | 43.2% (+2.7, n=37) | 15.0% (-2.5, n=40) | 57.5% (-2.5, n=40) | 39.9% |
| 0.00 | 85.0% (-7.5, n=40) | 13.2% (-5.3, n=38) | 37.5% (-10.0, n=40) | 5.0% (-7.5, n=40) | 39.5% (-10.5, n=38) | 43.2% (+2.7, n=37) | 15.0% (-2.5, n=40) | 52.5% (-7.5, n=40) | 36.4% |

## R@5, per BEAM dimension

| overlap | contradiction_resolution | event_ordering | information_extraction | instruction_following | knowledge_update | multi_session_reasoning | preference_following | temporal_reasoning | overall |
|---|---|---|---|---|---|---|---|---|---|
| 1.00 | 97.5% (+0.0, n=40) | 52.6% (+0.0, n=38) | 75.0% (+0.0, n=40) | 27.5% (+0.0, n=40) | 76.3% (+0.0, n=38) | 73.0% (+0.0, n=37) | 45.0% (+0.0, n=40) | 90.0% (+0.0, n=40) | 67.1% |
| 0.25 | 97.5% (+0.0, n=40) | 36.8% (-15.8, n=38) | 65.0% (-10.0, n=40) | 25.0% (-2.5, n=40) | 78.9% (+2.6, n=38) | 70.3% (-2.7, n=37) | 45.0% (+0.0, n=40) | 90.0% (+0.0, n=40) | 63.6% |
| 0.00 | 97.5% (+0.0, n=40) | 31.6% (-21.1, n=38) | 60.0% (-15.0, n=40) | 32.5% (+5.0, n=40) | 78.9% (+2.6, n=38) | 67.6% (-5.4, n=37) | 45.0% (+0.0, n=40) | 85.0% (-5.0, n=40) | 62.3% |

## recall_all@10, per BEAM dimension

| overlap | contradiction_resolution | event_ordering | information_extraction | instruction_following | knowledge_update | multi_session_reasoning | preference_following | temporal_reasoning | overall |
|---|---|---|---|---|---|---|---|---|---|
| 1.00 | 35.0% (+0.0, n=40) | 5.3% (+0.0, n=38) | 25.0% (+0.0, n=40) | 2.5% (+0.0, n=40) | 31.6% (+0.0, n=38) | 5.4% (+0.0, n=37) | 15.0% (+0.0, n=40) | 47.5% (+0.0, n=40) | 21.1% |
| 0.25 | 37.5% (+2.5, n=40) | 2.6% (-2.6, n=38) | 15.0% (-10.0, n=40) | 5.0% (+2.5, n=40) | 39.5% (+7.9, n=38) | 5.4% (+0.0, n=37) | 12.5% (-2.5, n=40) | 50.0% (+2.5, n=40) | 21.1% |
| 0.00 | 35.0% (+0.0, n=40) | 0.0% (-5.3, n=38) | 12.5% (-12.5, n=40) | 5.0% (+2.5, n=40) | 36.8% (+5.3, n=38) | 5.4% (+0.0, n=37) | 20.0% (+5.0, n=40) | 47.5% (+0.0, n=40) | 20.4% |

## NDCG@10, per BEAM dimension

| overlap | contradiction_resolution | event_ordering | information_extraction | instruction_following | knowledge_update | multi_session_reasoning | preference_following | temporal_reasoning | overall |
|---|---|---|---|---|---|---|---|---|---|
| 1.00 | 76.9% (+0.0, n=40) | 20.5% (+0.0, n=38) | 43.4% (+0.0, n=40) | 14.3% (+0.0, n=40) | 52.2% (+0.0, n=38) | 40.8% (+0.0, n=37) | 26.1% (+0.0, n=40) | 63.5% (+0.0, n=40) | 42.3% |
| 0.25 | 76.0% (-0.9, n=40) | 18.8% (-1.7, n=38) | 38.8% (-4.6, n=40) | 14.4% (+0.1, n=40) | 53.3% (+1.1, n=38) | 39.7% (-1.2, n=37) | 24.7% (-1.3, n=40) | 63.4% (-0.1, n=40) | 41.2% |
| 0.00 | 74.3% (-2.6, n=40) | 15.3% (-5.1, n=38) | 36.6% (-6.8, n=40) | 12.5% (-1.8, n=40) | 51.0% (-1.3, n=38) | 37.4% (-3.4, n=37) | 24.4% (-1.7, n=40) | 62.4% (-1.0, n=40) | 39.3% |

## Counts

| | |
|---|---|
| QA turn-pair documents | 2866, in 20 banks (94–196 per bank) |
| fused documents per query | mean 99.5, max 100 |
| questions in `queries.json` | 400 |
| **questions evaluated — the n in every table above** | **313** |
| dropped: no localizable gold by design (`abstention` 40, `summarization` 40) | 80 |
| dropped: content-match resolved nothing | 7 |
| wall clock for the scoring pass | 21.1s |

## Data shape, as measured (not as assumed)

| field | observed |
|---|---|
| upstream `probing_questions` | a **Python-repr string**, parsed by the generator with `ast.literal_eval` — upstream's own parser does the same (`beam_search.py:69`) |
| upstream `chat` | 3–5 sessions per conversation, 46–132 turns each, ~100K chars — the assumption in H9's registration ("27–50-doc banks at session granularity") was wrong, measured before labelling and corrected by the pair unit |
| turns | every user turn has exactly one assistant reply: 2866 pairs, `94`–`196` per bank |
| `time_anchor` | per turn, `%B-%d-%Y` ("March-15-2024"), parsed by the generator into the pair's `timestamp`; every pair dated |

Wrote `eval/BEAM.md` and `eval/results_beam.json`. This binary owns the whole of this file: do not hand-edit it, and a bare run writes to `$TMPDIR` instead — updating the committed artifact means naming `--out-md eval/BEAM.md`.
