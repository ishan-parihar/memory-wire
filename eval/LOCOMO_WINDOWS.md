# LoCoMo retrieval results (memory-wire) — the dev set

## Provenance

| | |
|---|---|
| Commit under test | `84580d577cb3d0299da37ca9636081b1f3d30a2d` |
| Command | `cargo run --release --example locomo -- --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json --out-md eval/LOCOMO.md --out-json eval/results_locomo.json` |
| Run | 2026-10-14, from `--release`, on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| Granularity | **window of 2 turns** (`--windows 2`): one memory per window of 2 consecutive turns, stride 1, hits pooled to their session at its best window (MaxP). An experiment arm (H2, `docs/handoff/RND_PLAN.md`), not the committed whole-session run |
| Occurrence times | each memory carries its corpus `timestamp` (`api::parse_created_at`), not the ingest clock — the corpus has been date-aware since 2026-10-14 (H0, `docs/handoff/RND_PLAN.md`); the shipped recency weight is 0.0, so dated ingestion moved no number in this file (control: the full metric vector was bit-identical) |
| Load at start / at end | `19.72 12.99 13.70 27/6618 492039` / `26.98 15.61 14.55 29/6742 504813` |
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

**Read the `R@pool` column before any other.** Each bank holds 19–32 documents, under both recall's 200-row candidate-pool window and BM25's `LIMIT 50`, so the *store* hands recall the whole bank. But the fused list is the union of the two streams, and both of them **drop a document that matches no query token** — `fts_match_query` ORs the query tokens and `rank_candidates` retains `score > 0.0`. A gold document that shares no token with its question is therefore unreachable at any K. `R@pool` is recall over the *whole* returned list, so it measures exactly that: the 20 queries of 1531 (1.3%) whose gold document never entered the ranking. That is the coverage ceiling for this suite, and it is the analogue of the note in `eval/RESULTS.md` that the 200-row pool never binds on LongMemEval. **Measured: 20 of 1531** queries never get their gold document into the ranking at any K, so the R@K columns below understate what a perfect reranker could reach by exactly that fraction, and every delta should be read under that ceiling. A reranking gain cannot recover those 20 questions; a content or matching change could.

**So R@20 is a mid-list cut here, not a ceiling, and it should not be read as one.** The fused ranking returns a mean of 20.3 documents per query (max 27), so a top-20 cut lands inside a 19–32-document list and misses a gold document sitting at rank 25. The discriminating columns are **R@1, R@5 and NDCG@10** — ordering, which is exactly what the fusion weight controls. R@20 is printed because the LongMemEval table prints it and the two are meant to be read together. This suite cannot detect a store-pool-bound or BM25-truncation regression; `eval/BENCH_RECALL_CURVE.md` and `tests/scale.rs` price that.

The metrics are deterministic and **seed-independent** — a query's index is fixed by its `user_id`, so `--seed` changes only the order of the progress log. Re-running this binary reproduces every per-question value in `eval/results_locomo.json` bit-for-bit.

## The grid

1.00, 0.75, 0.50, 0.25, 0.00 — the weight on the token-overlap stream, with BM25 pinned at 1.0, the agreement bonus at 0.0 and RRF `k` at 60, so `overlap` is the only axis that moves. `1.00` is **equal weight**: the configuration every published number in `README.md` and `eval/RESULTS.md` was measured under, and the only clean comparison `docs/EVALUATION_HYGIENE.md` §2.2 has. `0.25` is the fitted value under test. `0.00` is a diagnostic bound (BM25 alone) and **not a candidate**: a zero weight drops the overlap stream's candidates while BM25 truncates at 50 in SQL, so it is only correct on a corpus this small — one of the reasons `overlap: 0.0` was rejected on LongMemEval. Every `Δ` is against the `1.00` row.

## Aggregate, every weight

| overlap | R@1 | ΔR@1 | R@5 | ΔR@5 | R@10 | R@20 | NDCG@10 | ΔNDCG | MRR | ΔMRR | R@pool | R@5 up/down/same | n |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1.00 | 54.1% | +0.0 | 81.8% | +0.0 | 92.0% | 98.2% | 69.1% | +0.0 | 66.3% | +0.0 | 98.7% | 0/0/1531 | 1531 |
| 0.75 | 55.3% | +1.1 | 83.3% | +1.5 | 92.1% | 98.2% | 69.9% | +0.9 | 67.3% | +1.0 | 98.7% | 26/3/1502 | 1531 |
| 0.50 | 56.6% | +2.5 | 84.8% | +3.1 | 92.7% | 98.2% | 71.1% | +2.0 | 68.6% | +2.4 | 98.7% | 57/10/1464 | 1531 |
| 0.25 | 59.3% | +5.2 | 86.2% | +4.4 | 93.1% | 98.3% | 72.7% | +3.6 | 70.6% | +4.4 | 98.7% | 91/23/1417 | 1531 |
| 0.00 | 62.1% | +7.9 | 86.1% | +4.3 | 93.7% | 97.8% | 74.3% | +5.3 | 72.6% | +6.4 | 97.9% | 108/42/1381 | 1531 |

## The answer

**`overlap: 0.25` beats the unfitted `1.00` on every ordering metric, on 1531 queries that never chose it. The effect replicated on independent data.**

R@1 +5.2pp · R@5 +4.4pp · R@10 +1.1pp · NDCG@10 +3.6pp · MRR +4.4pp, with R@pool unchanged at 98.7% because the weights reorder the ranking and cannot change which documents reach it.

That is the answer `docs/EVALUATION_HYGIENE.md` §3.2 was waiting for, and it is **not** the size of the effect on LongMemEval: there `0.25` was worth +4.2pp R@5 and +4.8pp NDCG@10 over equal weight, here it is worth +4.4pp and +3.6pp. The direction replicates; the magnitude does not transfer, which is what a single-parameter fit on 500 questions should be expected to do.

**What this does not do.** It changes no shipped value. `0.25` may now be described as *replicated on independent data* rather than *provisional*, which is a demotion of the existing caveat and not a licence to re-tune: the grid above is a measurement, and picking a new weight off it would be a selection that has to be recorded as one in `docs/CONSISTENCY.md` under the counting rule. **This harness did not pick a weight and must not be used to pick one without saying so out loud.**

Per-query movement on R@5 for the `0.25` row against the `1.00` row: **91 improved, 23 regressed, 1417 unchanged** of 1531. A win on the mean with a small improving set is a different claim from a win that moves most of the suite, and both numbers are here so the reader can tell which one this is.

## R@1, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 25.8% (+0.0, n=89) | 65.0% (+0.0, n=841) | 40.6% (+0.0, n=281) | 45.3% (+0.0, n=320) | 54.1% |
| 0.75 | 24.7% (-1.1, n=89) | 66.3% (+1.3, n=841) | 42.0% (+1.4, n=281) | 46.2% (+0.9, n=320) | 55.3% |
| 0.50 | 23.6% (-2.2, n=89) | 67.9% (+2.9, n=841) | 44.1% (+3.6, n=281) | 47.2% (+1.9, n=320) | 56.6% |
| 0.25 | 25.8% (+0.0, n=89) | 70.7% (+5.7, n=841) | 44.1% (+3.6, n=281) | 51.9% (+6.6, n=320) | 59.3% |
| 0.00 | 32.6% (+6.7, n=89) | 73.5% (+8.4, n=841) | 45.2% (+4.6, n=281) | 55.0% (+9.7, n=320) | 62.1% |

## R@5, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 55.1% (+0.0, n=89) | 88.3% (+0.0, n=841) | 74.4% (+0.0, n=281) | 78.4% (+0.0, n=320) | 81.8% |
| 0.75 | 56.2% (+1.1, n=89) | 90.0% (+1.7, n=841) | 75.1% (+0.7, n=281) | 80.3% (+1.9, n=320) | 83.3% |
| 0.50 | 61.8% (+6.7, n=89) | 91.2% (+2.9, n=841) | 78.3% (+3.9, n=281) | 80.3% (+1.9, n=320) | 84.8% |
| 0.25 | 64.0% (+9.0, n=89) | 92.4% (+4.0, n=841) | 78.6% (+4.3, n=281) | 82.8% (+4.4, n=320) | 86.2% |
| 0.00 | 64.0% (+9.0, n=89) | 92.6% (+4.3, n=841) | 77.2% (+2.8, n=281) | 82.8% (+4.4, n=320) | 86.1% |

## recall_all@10, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 52.8% (+0.0, n=89) | 95.6% (+0.0, n=841) | 40.9% (+0.0, n=281) | 87.8% (+0.0, n=320) | 81.5% |
| 0.75 | 51.7% (-1.1, n=89) | 96.0% (+0.4, n=841) | 40.9% (+0.0, n=281) | 88.1% (+0.3, n=320) | 81.6% |
| 0.50 | 52.8% (+0.0, n=89) | 96.8% (+1.2, n=841) | 40.2% (-0.7, n=281) | 88.8% (+0.9, n=320) | 82.2% |
| 0.25 | 53.9% (+1.1, n=89) | 97.1% (+1.5, n=841) | 41.3% (+0.4, n=281) | 88.8% (+0.9, n=320) | 82.6% |
| 0.00 | 52.8% (+0.0, n=89) | 97.6% (+2.0, n=841) | 42.7% (+1.8, n=281) | 89.4% (+1.6, n=320) | 83.2% |

## NDCG@10, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 41.7% (+0.0, n=89) | 79.9% (+0.0, n=841) | 49.9% (+0.0, n=281) | 64.9% (+0.0, n=320) | 69.1% |
| 0.75 | 41.3% (-0.4, n=89) | 81.0% (+1.1, n=841) | 50.5% (+0.5, n=281) | 66.0% (+1.0, n=320) | 69.9% |
| 0.50 | 42.3% (+0.6, n=89) | 82.3% (+2.4, n=841) | 51.0% (+1.1, n=281) | 67.1% (+2.1, n=320) | 71.1% |
| 0.25 | 44.3% (+2.6, n=89) | 84.1% (+4.2, n=841) | 51.5% (+1.6, n=281) | 69.2% (+4.3, n=320) | 72.7% |
| 0.00 | 47.0% (+5.3, n=89) | 85.7% (+5.8, n=841) | 52.1% (+2.2, n=281) | 71.6% (+6.7, n=320) | 74.3% |

## Counts

| | |
|---|---|
| documents indexed | 272, in 10 banks (19–32 per bank) |
| documents returned per query by the fused ranking | mean 20.3, max 27 |
| dialogue turns decoded | 5882 |
| queries in `queries.json` | 1540 |
| **queries evaluated — the n in every table above** | **1531** |
| dropped: `gold_ids` empty | 9 |
| queries with a blank `gold_answers` (shape-checked only, never scored) | 6 |
| queries whose `gold_answers` holds a bare JSON number, not a string | 6 |
| conversations (`user_id`) | 10 |
| wall clock for the scoring pass | 40.3s |

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

Wrote `eval/LOCOMO_WINDOWS.md` and `eval/results_locomo_windows.json`. This binary owns the whole of this file: do not hand-edit it, and note that a bare run writes to `$TMPDIR` instead — updating the committed artifact means naming `--out-md eval/LOCOMO.md`.
