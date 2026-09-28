# LoCoMo retrieval results (memory-wire) — the dev set

## Provenance

| | |
|---|---|
| Commit under test | `ebb4e832d6a61c8e051541709c17f4275f84bcd5` |
| Command | `cargo run --release --example locomo -- --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json --out-md eval/LOCOMO.md --out-json eval/results_locomo.json` |
| Run | 2026-09-28, from `--release`, on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| Load at start / at end | `44.68 42.62 36.95 36/6808 2767588` / `42.37 42.29 36.97 18/6842 2771599` |
| Dataset | LoCoMo (Snap Research) via the canonical distribution `vectorize-io/agent-memory-benchmark`, `data/locomo/locomo10/`, pinned at upstream commit `decbb07f4f9899deac28a76293564cf263872652` and sha256-verified by `eval/download.sh` |
| Corpus | 272 documents, 1,540 queries, 10 conversations — fetched by `./eval/download.sh`, **not committed** (`eval/data/` is gitignored) |
| **LLM involvement** | **none.** No model, no judge, no answer generation, no network call of any kind. `gold_answers` is read for shape and never scored. |

The retrieval metrics do not depend on machine load and there is **no latency column** in this artifact, so the load row is recorded for completeness only — the same reason `eval/SWEEP_FUSION.md` carries no timing.

## The question

`overlap: 0.25` in `src/recall.rs` was selected by sweeping 46 configurations against LongMemEval-S's 500 questions — that is, fitted to the test set, which `AGENTS.md` §1 and `docs/EVALUATION_HYGIENE.md` §1 forbid. **This artifact answers one question: does 0.25 beat the unfitted equal weight 1.00 on data that never chose it?** Either answer is worth more than any new model. Neither is used here to pick a weight: this is a replication check, and a row that wins here is evidence, not a licence to re-tune.

## Comparability — read before quoting any number from this file

**A `recall_any@K` from this harness is not a LoCoMo score.** LoCoMo's standard metric is LLM-judged *answer* accuracy: a model reads retrieved context and is graded on whether its generated answer matches a gold answer. This harness never generates an answer and never judges one, so it cannot produce that number and must not be placed beside one. Any published LoCoMo figure — including the 92.0% in `README.md`, which is Hindsight's LLM-judged answer accuracy from a `rag` run with two Gemini calls in the loop — is a different measurement on a different scale. The two answer different questions and neither bounds the other. **Do not place a number below next to a published LoCoMo number.**

What *is* comparable is the internal comparison: the `0.25` row against the unfitted `1.00` row, on the same corpus, the same index, the same harness, the same run. The `R@5 / R@10 / NDCG@10 / MRR` columns use the same definitions and units as `eval/RESULTS.md` (`examples/longmemeval.rs`) and are printed together for that reason — but the suites differ in document granularity (LoCoMo: one document per conversation session, 19–32 per bank; LongMemEval: ~48 per bank), question style and question language, so a LoCoMo percentage is not a LongMemEval percentage and **the absolute levels are not comparable either. Only the within-table deltas are.**

## Method

One bank per `user_id` (the conversation, and the isolation unit), built once: one memory per LoCoMo document, `id` = document id, content = that document's dialogue turns joined one per line. Each query is answered against **its own `user_id`'s bank and no other** — the isolation model `examples/longmemeval.rs` uses. Every weight is scored on one shared index per bank, so two rows cannot disagree because one's index happened to build differently.

**Read the `R@pool` column before any other.** Each bank holds 19–32 documents, under both recall's 200-row candidate-pool window and BM25's `LIMIT 50`, so the *store* hands recall the whole bank. But the fused list is the union of the two streams, and both of them **drop a document that matches no query token** — `fts_match_query` ORs the query tokens and `rank_candidates` retains `score > 0.0`. A gold document that shares no token with its question is therefore unreachable at any K. `R@pool` is recall over the *whole* returned list, so it measures exactly that: the 0 queries of 1531 (0.0%) whose gold document never entered the ranking. That is the coverage ceiling for this suite, and it is the analogue of the note in `eval/RESULTS.md` that the 200-row pool never binds on LongMemEval. **Measured: zero.** Every gold document in every evaluated query shares at least one token with its question and reaches the fused ranking, so coverage is not the binding constraint on this suite and every delta below is **ordering, not matching** — the same shape of claim `eval/RESULTS.md` makes about its 200-row pool. A regression that lost the gold document entirely would show up here as a fall in `R@pool`, not as a small drift in R@5.

**So R@20 is a mid-list cut here, not a ceiling, and it should not be read as one.** The fused ranking returns a mean of 27.7 documents per query (max 32), so a top-20 cut lands inside a 19–32-document list and misses a gold document sitting at rank 25. The discriminating columns are **R@1, R@5 and NDCG@10** — ordering, which is exactly what the fusion weight controls. R@20 is printed because the LongMemEval table prints it and the two are meant to be read together. This suite cannot detect a store-pool-bound or BM25-truncation regression; `eval/BENCH_RECALL_CURVE.md` and `tests/scale.rs` price that.

The metrics are deterministic and **seed-independent** — a query's index is fixed by its `user_id`, so `--seed` changes only the order of the progress log. Re-running this binary reproduces every per-question value in `eval/results_locomo.json` bit-for-bit.

## The grid

1.00, 0.75, 0.50, 0.25, 0.00 — the weight on the token-overlap stream, with BM25 pinned at 1.0, the agreement bonus at 0.0 and RRF `k` at 60, so `overlap` is the only axis that moves. `1.00` is **equal weight**: the configuration every published number in `README.md` and `eval/RESULTS.md` was measured under, and the only clean comparison `docs/EVALUATION_HYGIENE.md` §2.2 has. `0.25` is the fitted value under test. `0.00` is a diagnostic bound (BM25 alone) and **not a candidate**: a zero weight drops the overlap stream's candidates while BM25 truncates at 50 in SQL, so it is only correct on a corpus this small — one of the reasons `overlap: 0.0` was rejected on LongMemEval. Every `Δ` is against the `1.00` row.

## Aggregate, every weight

| overlap | R@1 | ΔR@1 | R@5 | ΔR@5 | R@10 | R@20 | NDCG@10 | ΔNDCG | MRR | ΔMRR | R@pool | R@5 up/down/same | n |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1.00 | 54.8% | +0.0 | 84.5% | +0.0 | 93.3% | 98.0% | 70.7% | +0.0 | 68.0% | +0.0 | 100.0% | 0/0/1531 | 1531 |
| 0.75 | 57.0% | +2.2 | 85.8% | +1.2 | 94.0% | 98.0% | 72.0% | +1.2 | 69.5% | +1.4 | 100.0% | 22/3/1506 | 1531 |
| 0.50 | 59.0% | +4.2 | 86.2% | +1.7 | 93.8% | 98.0% | 72.9% | +2.1 | 70.7% | +2.7 | 100.0% | 39/13/1479 | 1531 |
| 0.25 | 60.4% | +5.6 | 86.0% | +1.4 | 94.2% | 98.0% | 73.6% | +2.9 | 71.8% | +3.7 | 100.0% | 57/35/1439 | 1531 |
| 0.00 | 60.4% | +5.6 | 86.3% | +1.8 | 94.1% | 98.2% | 73.6% | +2.9 | 71.6% | +3.6 | 100.0% | 69/41/1421 | 1531 |

## The answer

**`overlap: 0.25` beats the unfitted `1.00` on every ordering metric, on 1531 queries that never chose it. The effect replicated on independent data.**

R@1 +5.6pp · R@5 +1.4pp · R@10 +0.8pp · NDCG@10 +2.9pp · MRR +3.7pp, with R@pool unchanged at 100.0% because the weights reorder the ranking and cannot change which documents reach it.

That is the answer `docs/EVALUATION_HYGIENE.md` §3.2 was waiting for, and it is **not** the size of the effect on LongMemEval: there `0.25` was worth +4.2pp R@5 and +4.8pp NDCG@10 over equal weight, here it is worth +1.4pp and +2.9pp. The direction replicates; the magnitude does not transfer, which is what a single-parameter fit on 500 questions should be expected to do.

**What this does not do.** It changes no shipped value. `0.25` may now be described as *replicated on independent data* rather than *provisional*, which is a demotion of the existing caveat and not a licence to re-tune: the grid above is a measurement, and picking a new weight off it would be a selection that has to be recorded as one in `docs/CONSISTENCY.md` under the counting rule. **This harness did not pick a weight and must not be used to pick one without saying so out loud.**

Per-query movement on R@5 for the `0.25` row against the `1.00` row: **57 improved, 35 regressed, 1439 unchanged** of 1531. A win on the mean with a small improving set is a different claim from a win that moves most of the suite, and both numbers are here so the reader can tell which one this is.

## R@1, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 33.7% (+0.0, n=89) | 62.4% (+0.0, n=841) | 45.2% (+0.0, n=281) | 49.1% (+0.0, n=320) | 54.8% |
| 0.75 | 32.6% (-1.1, n=89) | 65.3% (+2.9, n=841) | 46.6% (+1.4, n=281) | 50.9% (+1.9, n=320) | 57.0% |
| 0.50 | 33.7% (+0.0, n=89) | 67.2% (+4.8, n=841) | 48.8% (+3.6, n=281) | 53.4% (+4.4, n=320) | 59.0% |
| 0.25 | 37.1% (+3.4, n=89) | 67.8% (+5.4, n=841) | 49.5% (+4.3, n=281) | 57.2% (+8.1, n=320) | 60.4% |
| 0.00 | 32.6% (-1.1, n=89) | 68.5% (+6.1, n=841) | 47.0% (+1.8, n=281) | 58.4% (+9.4, n=320) | 60.4% |

## R@5, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 67.4% (+0.0, n=89) | 90.7% (+0.0, n=841) | 77.2% (+0.0, n=281) | 79.4% (+0.0, n=320) | 84.5% |
| 0.75 | 67.4% (+0.0, n=89) | 91.4% (+0.7, n=841) | 79.7% (+2.5, n=281) | 81.2% (+1.9, n=320) | 85.8% |
| 0.50 | 64.0% (-3.4, n=89) | 92.2% (+1.4, n=841) | 80.1% (+2.8, n=281) | 82.2% (+2.8, n=320) | 86.2% |
| 0.25 | 67.4% (+0.0, n=89) | 91.8% (+1.1, n=841) | 79.4% (+2.1, n=281) | 81.6% (+2.2, n=320) | 86.0% |
| 0.00 | 66.3% (-1.1, n=89) | 91.7% (+1.0, n=841) | 80.4% (+3.2, n=281) | 83.1% (+3.8, n=320) | 86.3% |

## NDCG@10, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 48.9% (+0.0, n=89) | 79.9% (+0.0, n=841) | 53.1% (+0.0, n=281) | 68.0% (+0.0, n=320) | 70.7% |
| 0.75 | 48.2% (-0.8, n=89) | 81.4% (+1.5, n=841) | 54.0% (+0.9, n=281) | 69.4% (+1.4, n=320) | 72.0% |
| 0.50 | 48.1% (-0.8, n=89) | 82.4% (+2.4, n=841) | 54.7% (+1.6, n=281) | 70.8% (+2.8, n=320) | 72.9% |
| 0.25 | 50.0% (+1.0, n=89) | 82.6% (+2.7, n=841) | 54.9% (+1.8, n=281) | 72.8% (+4.8, n=320) | 73.6% |
| 0.00 | 49.1% (+0.2, n=89) | 82.7% (+2.8, n=841) | 54.3% (+1.2, n=281) | 73.4% (+5.4, n=320) | 73.6% |

## Counts

| | |
|---|---|
| documents indexed | 272, in 10 banks (19–32 per bank) |
| documents returned per query by the fused ranking | mean 27.7, max 32 |
| dialogue turns decoded | 5882 |
| queries in `queries.json` | 1540 |
| **queries evaluated — the n in every table above** | **1531** |
| dropped: `gold_ids` empty | 9 |
| queries with a blank `gold_answers` (shape-checked only, never scored) | 6 |
| queries whose `gold_answers` holds a bare JSON number, not a string | 6 |
| conversations (`user_id`) | 10 |
| wall clock for the scoring pass | 20.8s |

**The n above is 1531, not 1540.** 9 queries carry an empty `gold_ids` list: they have a gold *answer* but no gold *document*, so no document could be retrieved and every `recall_any@K` would be 0 by construction. They are excluded from every denominator and reported here rather than silently kept — keeping them would understate every row by a flat 0.58pp carrying no measurement at all. Every remaining `gold_ids` value resolves to a document in the query's own bank; none names a document from another conversation, so per-bank isolation costs nothing measurable here.

## Data shape, as measured (not as assumed)

| field | observed |
|---|---|
| `documents[].content` | a JSON **string** holding a list of dialogue turns — 5882 of them across 272 documents — parsed a second time by the harness |
| `queries[].meta` | already a JSON **object** (`category`, `sample_id`, `speaker_a`, `speaker_b`, `query_timestamp`) — **not** an encoded string |
| `queries[].gold_ids` | already a JSON **array of strings** — **not** an encoded string |
| `queries[].gold_answers` | an array whose elements are **not uniformly strings**: 6 of 1540 queries carry a bare JSON number (a year, a count) rather than a quoted string, so the harness types it `serde_json::Value`. Never scored — this is a retrieval harness |
| `gold_ids` per query | 1 for 1203 queries, 2–15 for 328, 0 for the 9 dropped above |
| `meta.category` values | `multi-hop`, `open-domain`, `single-hop`, `temporal` |

**Two of the three fields the P−1 plan expected to be double-encoded (`meta`, `gold_ids`) are not** — in this distribution they are ordinary decoded JSON values, and only `content` needs a second parse. A third surprise sits beside them: `gold_answers` is not a list of strings either, because a numeric answer (a year, a count) is emitted unquoted. The harness types `content` as a `String`, `meta` and `gold_ids` as their natural types, and `gold_answers` as `serde_json::Value` — so a distribution that *does* double-encode the first two, or quotes the third differently, fails loudly at the serde layer rather than silently scoring an empty gold set. All three assumptions were checked against the bytes before a line of the harness was written, not discovered by a run.

## Per-question data

`eval/results_locomo.json` carries `query_id`, `category` and all six retrieval values for every evaluated query under every weight, so any cell above can be re-derived. The `R@5 up/down/same` column in the aggregate table counts individual queries against the `overlap=1.00` row.

Wrote `eval/LOCOMO.md` and `eval/results_locomo.json`. This binary owns the whole of this file: do not hand-edit it, and note that a bare run writes to `$TMPDIR` instead — updating the committed artifact means naming `--out-md eval/LOCOMO.md`.
