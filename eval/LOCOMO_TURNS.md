# LoCoMo retrieval results (memory-wire) — the dev set

## Provenance

| | |
|---|---|
| Commit under test | `84580d577cb3d0299da37ca9636081b1f3d30a2d` |
| Command | `cargo run --release --example locomo -- --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json --out-md eval/LOCOMO.md --out-json eval/results_locomo.json` |
| Run | 2026-10-14, from `--release`, on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| Granularity | **turn** (`--turns`): one memory per dialogue turn, hits pooled to their session at its best turn (MaxP). An experiment arm, not the committed whole-session run |
| Occurrence times | each memory carries its corpus `timestamp` (`api::parse_created_at`), not the ingest clock — the corpus has been date-aware since 2026-10-14 (H0, `docs/handoff/RND_PLAN.md`); the shipped recency weight is 0.0, so dated ingestion moved no number in this file (control: the full metric vector was bit-identical) |
| Load at start / at end | `18.45 12.09 13.45 18/6618 489523` / `19.34 12.81 13.65 25/6619 491663` |
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

**Read the `R@pool` column before any other.** Each bank holds 19–32 documents, under both recall's 200-row candidate-pool window and BM25's `LIMIT 50`, so the *store* hands recall the whole bank. But the fused list is the union of the two streams, and both of them **drop a document that matches no query token** — `fts_match_query` ORs the query tokens and `rank_candidates` retains `score > 0.0`. A gold document that shares no token with its question is therefore unreachable at any K. `R@pool` is recall over the *whole* returned list, so it measures exactly that: the 13 queries of 1531 (0.8%) whose gold document never entered the ranking. That is the coverage ceiling for this suite, and it is the analogue of the note in `eval/RESULTS.md` that the 200-row pool never binds on LongMemEval. **Measured: 13 of 1531** queries never get their gold document into the ranking at any K, so the R@K columns below understate what a perfect reranker could reach by exactly that fraction, and every delta should be read under that ceiling. A reranking gain cannot recover those 13 questions; a content or matching change could.

**So R@20 is a mid-list cut here, not a ceiling, and it should not be read as one.** The fused ranking returns a mean of 23.3 documents per query (max 31), so a top-20 cut lands inside a 19–32-document list and misses a gold document sitting at rank 25. The discriminating columns are **R@1, R@5 and NDCG@10** — ordering, which is exactly what the fusion weight controls. R@20 is printed because the LongMemEval table prints it and the two are meant to be read together. This suite cannot detect a store-pool-bound or BM25-truncation regression; `eval/BENCH_RECALL_CURVE.md` and `tests/scale.rs` price that.

The metrics are deterministic and **seed-independent** — a query's index is fixed by its `user_id`, so `--seed` changes only the order of the progress log. Re-running this binary reproduces every per-question value in `eval/results_locomo.json` bit-for-bit.

## The grid

1.00, 0.75, 0.50, 0.25, 0.00 — the weight on the token-overlap stream, with BM25 pinned at 1.0, the agreement bonus at 0.0 and RRF `k` at 60, so `overlap` is the only axis that moves. `1.00` is **equal weight**: the configuration every published number in `README.md` and `eval/RESULTS.md` was measured under, and the only clean comparison `docs/EVALUATION_HYGIENE.md` §2.2 has. `0.25` is the fitted value under test. `0.00` is a diagnostic bound (BM25 alone) and **not a candidate**: a zero weight drops the overlap stream's candidates while BM25 truncates at 50 in SQL, so it is only correct on a corpus this small — one of the reasons `overlap: 0.0` was rejected on LongMemEval. Every `Δ` is against the `1.00` row.

## Aggregate, every weight

| overlap | R@1 | ΔR@1 | R@5 | ΔR@5 | R@10 | R@20 | NDCG@10 | ΔNDCG | MRR | ΔMRR | R@pool | R@5 up/down/same | n |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1.00 | 50.2% | +0.0 | 79.4% | +0.0 | 90.8% | 98.0% | 66.2% | +0.0 | 63.1% | +0.0 | 99.2% | 0/0/1531 | 1531 |
| 0.75 | 51.0% | +0.8 | 80.9% | +1.4 | 91.4% | 98.0% | 67.1% | +0.9 | 64.0% | +0.9 | 99.2% | 27/5/1499 | 1531 |
| 0.50 | 53.0% | +2.7 | 82.0% | +2.6 | 91.4% | 98.2% | 68.3% | +2.1 | 65.5% | +2.3 | 99.2% | 61/21/1449 | 1531 |
| 0.25 | 54.6% | +4.4 | 83.0% | +3.6 | 92.2% | 98.1% | 69.7% | +3.5 | 67.1% | +3.9 | 99.2% | 91/36/1404 | 1531 |
| 0.00 | 55.3% | +5.1 | 83.0% | +3.6 | 92.4% | 97.9% | 70.0% | +3.8 | 67.5% | +4.4 | 98.8% | 116/61/1354 | 1531 |

## The answer

**`overlap: 0.25` beats the unfitted `1.00` on every ordering metric, on 1531 queries that never chose it. The effect replicated on independent data.**

R@1 +4.4pp · R@5 +3.6pp · R@10 +1.4pp · NDCG@10 +3.5pp · MRR +3.9pp, with R@pool unchanged at 99.2% because the weights reorder the ranking and cannot change which documents reach it.

That is the answer `docs/EVALUATION_HYGIENE.md` §3.2 was waiting for, and it is **not** the size of the effect on LongMemEval: there `0.25` was worth +4.2pp R@5 and +4.8pp NDCG@10 over equal weight, here it is worth +3.6pp and +3.5pp. The direction replicates; the magnitude does not transfer, which is what a single-parameter fit on 500 questions should be expected to do.

**What this does not do.** It changes no shipped value. `0.25` may now be described as *replicated on independent data* rather than *provisional*, which is a demotion of the existing caveat and not a licence to re-tune: the grid above is a measurement, and picking a new weight off it would be a selection that has to be recorded as one in `docs/CONSISTENCY.md` under the counting rule. **This harness did not pick a weight and must not be used to pick one without saying so out loud.**

Per-query movement on R@5 for the `0.25` row against the `1.00` row: **91 improved, 36 regressed, 1404 unchanged** of 1531. A win on the mean with a small improving set is a different claim from a win that moves most of the suite, and both numbers are here so the reader can tell which one this is.

## R@1, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 27.0% (+0.0, n=89) | 59.6% (+0.0, n=841) | 32.0% (+0.0, n=281) | 48.1% (+0.0, n=320) | 50.2% |
| 0.75 | 25.8% (-1.1, n=89) | 61.1% (+1.5, n=841) | 31.7% (-0.4, n=281) | 48.4% (+0.3, n=320) | 51.0% |
| 0.50 | 29.2% (+2.2, n=89) | 63.3% (+3.7, n=841) | 32.7% (+0.7, n=281) | 50.3% (+2.2, n=320) | 53.0% |
| 0.25 | 31.5% (+4.5, n=89) | 64.9% (+5.4, n=841) | 35.6% (+3.6, n=281) | 50.6% (+2.5, n=320) | 54.6% |
| 0.00 | 32.6% (+5.6, n=89) | 65.3% (+5.7, n=841) | 38.8% (+6.8, n=281) | 50.0% (+1.9, n=320) | 55.3% |

## R@5, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 51.7% (+0.0, n=89) | 85.5% (+0.0, n=841) | 71.5% (+0.0, n=281) | 78.1% (+0.0, n=320) | 79.4% |
| 0.75 | 55.1% (+3.4, n=89) | 87.3% (+1.8, n=841) | 73.0% (+1.4, n=281) | 78.1% (+0.0, n=320) | 80.9% |
| 0.50 | 53.9% (+2.2, n=89) | 88.3% (+2.9, n=841) | 74.4% (+2.8, n=281) | 80.0% (+1.9, n=320) | 82.0% |
| 0.25 | 55.1% (+3.4, n=89) | 89.2% (+3.7, n=841) | 75.1% (+3.6, n=281) | 81.6% (+3.4, n=320) | 83.0% |
| 0.00 | 59.6% (+7.9, n=89) | 89.7% (+4.2, n=841) | 75.8% (+4.3, n=281) | 78.4% (+0.3, n=320) | 83.0% |

## recall_all@10, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 53.9% (+0.0, n=89) | 93.6% (+0.0, n=841) | 32.7% (+0.0, n=281) | 86.9% (+0.0, n=320) | 78.7% |
| 0.75 | 52.8% (-1.1, n=89) | 94.5% (+1.0, n=841) | 34.5% (+1.8, n=281) | 86.6% (-0.3, n=320) | 79.4% |
| 0.50 | 52.8% (-1.1, n=89) | 94.9% (+1.3, n=841) | 35.2% (+2.5, n=281) | 87.2% (+0.3, n=320) | 79.9% |
| 0.25 | 57.3% (+3.4, n=89) | 95.4% (+1.8, n=841) | 36.7% (+3.9, n=281) | 87.5% (+0.6, n=320) | 80.7% |
| 0.00 | 53.9% (+0.0, n=89) | 95.7% (+2.1, n=841) | 37.7% (+5.0, n=281) | 87.5% (+0.6, n=320) | 80.9% |

## NDCG@10, per `meta.category`

| overlap | multi-hop | open-domain | single-hop | temporal | overall |
|---|---|---|---|---|---|
| 1.00 | 40.1% (+0.0, n=89) | 76.0% (+0.0, n=841) | 45.3% (+0.0, n=281) | 66.2% (+0.0, n=320) | 66.2% |
| 0.75 | 40.3% (+0.1, n=89) | 77.3% (+1.3, n=841) | 45.6% (+0.3, n=281) | 66.9% (+0.6, n=320) | 67.1% |
| 0.50 | 42.6% (+2.4, n=89) | 78.6% (+2.7, n=841) | 46.1% (+0.8, n=281) | 67.8% (+1.5, n=320) | 68.3% |
| 0.25 | 45.4% (+5.2, n=89) | 80.0% (+4.0, n=841) | 47.8% (+2.5, n=281) | 68.6% (+2.4, n=320) | 69.7% |
| 0.00 | 45.7% (+5.6, n=89) | 80.4% (+4.4, n=841) | 48.8% (+3.5, n=281) | 68.2% (+1.9, n=320) | 70.0% |

## Counts

| | |
|---|---|
| documents indexed | 272, in 10 banks (19–32 per bank) |
| documents returned per query by the fused ranking | mean 23.3, max 31 |
| dialogue turns decoded | 5882 |
| queries in `queries.json` | 1540 |
| **queries evaluated — the n in every table above** | **1531** |
| dropped: `gold_ids` empty | 9 |
| queries with a blank `gold_answers` (shape-checked only, never scored) | 6 |
| queries whose `gold_answers` holds a bare JSON number, not a string | 6 |
| conversations (`user_id`) | 10 |
| wall clock for the scoring pass | 25.8s |

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

Wrote `eval/LOCOMO_TURNS.md` and `eval/results_locomo_turns.json`. This binary owns the whole of this file: do not hand-edit it, and note that a bare run writes to `$TMPDIR` instead — updating the committed artifact means naming `--out-md eval/LOCOMO.md`.
