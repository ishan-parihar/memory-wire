# Fusion-weight selection grid (memory-wire) — the dev set

## Provenance

| | |
|---|---|
| Commit under test | `87f726926ac7d8a0fe91b93edf7dfb169350258a` |
| Working tree at run time | **7 path(s) had uncommitted changes when this ran**, so the commit above does NOT describe the binary that produced these numbers. Paths: README.md, docs/NEXT_ITERATION.md, eval/ANSWER_QUALITY.md, eval/results_answer_quality.json, examples/select_fusion.rs, plugin/skills/memory-wire/SKILL.md, RELEASE_NOTES_v0.4.0.md |
| Command | `cargo run --release --example select_fusion -- --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json --rounds 2 --budget-seconds 0 --axes vector --out-md /tmp/VECTOR_GRID_FIXED.md --out-json /tmp/VECTOR_GRID_FIXED.json` |
| Run | 2026-09-28, from `--release`, on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| Load at start / at end | `200.29 190.37 111.82 15/9561 1441692` / `66.80 89.77 90.43 80/9615 1512185` |
| Dataset | LoCoMo (Snap Research) via the canonical distribution `vectorize-io/agent-memory-benchmark`, `data/locomo/locomo10/`, pinned at upstream commit `decbb07f4f9899deac28a76293564cf263872652` and sha256-verified by `eval/download.sh` |
| Corpus | 272 documents, 1540 queries, 10 conversations — fetched by `./eval/download.sh`, **not committed** (`eval/data/` is gitignored) |
| **LLM involvement** | **none.** No model, no judge, no answer generation, no network call. `gold_answers` is read for shape and never scored. |
| LongMemEval involvement | **none.** Selection happens on LoCoMo and only on LoCoMo (`AGENTS.md` §1). The 500 test questions are untouched by this binary. |

The retrieval metrics do not depend on machine load and there is **no recall-latency column** in this artifact, so the load row is recorded for completeness — the same reason `eval/LOCOMO.md` carries no timing. The wall clock in "Runtime" below is *harness throughput*, not a latency measurement, and it is reported next to the load line for that reason.

## Control — the shipped configuration, re-measured here

The walk starts at `FusionWeights::SHIPPED`, so its first row is the control this harness is validated against: the numbers `eval/LOCOMO.md` already committed for `overlap: 0.25`. It reproduces them.

| metric | measured here | `eval/LOCOMO.md` | Δ |
|---|---|---|---|
| R@1 | 60.4% | 60.4% | +0.0 |
| R@5 | 86.0% | 86.0% | -0.0 |
| R@10 | 94.2% | 94.2% | -0.0 |
| R@20 | 98.0% | 98.0% | -0.0 |
| NDCG@10 | 73.6% | 73.6% | +0.0 |
| MRR | 71.8% | 71.8% | -0.0 |
| n | 1531 | 1531 | 0 |

> **The control row above is measured at a fitted value.** `overlap: 0.25` won 46 configurations scored on LongMemEval-S's own 500 questions, so it was chosen by looking at the test set (`AGENTS.md` §1, `docs/EVALUATION_HYGIENE.md` §2.1). It is reproduced here because this harness starts there, and because the replication on independent data is established in `eval/LOCOMO.md` — not because the number is a generalisation estimate. The other rows in this file are at values that were fitted to nothing.

## The dense arm — vector coverage, checked before any row was scored

`vector::retain` is the only writer of `memory_vectors`, and it runs at **retain time**. The dev corpus ingests through `store.put`, which writes no vector, so a sweep over the shared ingestion would have scored a dense stream over banks with **no vectors at all** — and `vector::vector_stream` returns an empty stream for a vectorless bank *by design*, so the whole `vector` axis would have read as a null for the wrong reason. Every document is therefore embedded here, before the sweep, and the run **aborts rather than publishing a grid** if the counts disagree.

The count below is read back through `Store::bank_vectors`, which **decodes each blob and silently omits an unreadable one** — so this is the number of *usable* vectors, not the number of writes that claimed to succeed.

| bank | `memories` rows | decodable vectors | complete |
|---|---|---|---|
| `conv-26` | 19 | 19 | **yes** |
| `conv-30` | 19 | 19 | **yes** |
| `conv-41` | 32 | 32 | **yes** |
| `conv-42` | 29 | 29 | **yes** |
| `conv-43` | 29 | 29 | **yes** |
| `conv-44` | 28 | 28 | **yes** |
| `conv-47` | 31 | 31 | **yes** |
| `conv-48` | 30 | 30 | **yes** |
| `conv-49` | 25 | 25 | **yes** |
| `conv-50` | 30 | 30 | **yes** |

**272 of 272 rows, 10 banks, 100% covered.** Each document is embedded from the same `doc.text()` the lexical arm indexes — the dialogue turns joined one per line — so both streams rank the same content and the sweep measures the weight rather than a difference in what was indexed. `vector::retain` was deliberately *not* used for the write: it routes through `MemoryService::retain_doc` and `redact_pii`, which would replace the store's own text and leave the two arms ranking different strings.

**The other function's lexical half is the same function.** Rows with `vector != 0.0` are scored through `vector::recall`; the control row is scored through `MemoryService::recall_with_weights`, the path `eval/LOCOMO.md` was measured on. Those are two different functions, so every `vector` row would differ from the control in **two** ways at once — the added dense stream *and* any lexical drift — if the lexical halves disagreed. They do not: The lexical half of `vector::recall` at `vector: 0.0` scores the same six metrics and the same n as the service path over 1531 queries, to within one f64 ulp. The two functions are interchangeable on the lexical streams, so a `vector` row differs from the control in the dense stream and nothing else.

## How much does the dense stream *reach*? The number this axis is for

One question, asked with the dense stream **alone** — both lexical streams dropped, `bm25: 0.0, overlap: 0.0, vector: 1.00` — against the same gold, the same banks, the same 100%-covered index. It is a **diagnostic, not a candidate configuration**: nobody ships a dense-only arm on a corpus whose lexical arm already has 100% R@pool, and this row is not eligible for the walk.

| stream | R@pool | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR | n |
|---|---|---|---|---|---|---|---|---|
| **dense alone** (`vector: 1.00`, no lexical stream) | 100.0% | 31.3% | 62.4% | 79.8% | 93.9% | 50.3% | 46.3% | 1531 |
| fused at the control weight (`vector: 0.00`, lexical only) | 100.0% | 60.4% | 86.0% | 94.2% | 98.0% | 73.6% | 71.8% | 1531 |

**The dense stream reaches exactly as much as the lexical fusion does, and ranks it further down.** R@pool 100.0% against the fusion's 100.0% — the same, to the decimal — at R@5 62.4% against 86.0%. A stream that puts the same gold documents in the list and much further down it is a **reordering** signal on this corpus, not a **recall** signal. That reframes what the weight sweep below is for: it is not measuring whether the dense arm can find things the lexical arm misses, because on this corpus it reaches nothing the lexical arm misses, and a weight that made the dense arm dominant would be buying reordering of a pool the lexical streams already had.

**R@pool is measured over the whole returned list, not a top-k**, which is what makes it answer "is retrieval or coverage the binding constraint" — the question R@20 cannot answer on a corpus whose banks are 19–32 documents. It is the reach number this axis needs, and it is the reason a `vector` weight that *helps* here would be helping with order rather than with recall.


## What this artifact is, and what it is not

**It is a grid.** 19 configurations were scored, 7 of them distinct, on 1531 queries that never chose any of these values. Every row below carries the full metric set and its `n`.

**It is not a selection.** There is no recommended configuration here, no best-configuration row, and the tables are in *evaluation order* rather than sorted by score precisely so that nothing in this file can be read as a ranking. The walk has to move to *some* value to make progress, and where it moved is a consequence of the objective declared below — but choosing what to ship is a decision for a human, made on this evidence and recorded in `docs/CONSISTENCY.md` under the counting rule in `docs/EVALUATION_HYGIENE.md` §3.3. **A sweep that emitted a recommendation would be the exact failure this whole exercise exists to prevent**, and the run that produced `overlap: 0.25` is the cautionary example: 46 configurations scored on the test set, a maximum taken, and a +4.2pp effect that reproduced at +1.4pp on independent data (`docs/EVALUATION_HYGIENE.md` §2.1).

**A `recall_any@K` from this harness is not a LoCoMo score.** LoCoMo's standard metric is LLM-judged *answer* accuracy, and this harness never generates or judges an answer. It is a different measurement on a different scale and must not be placed beside a published LoCoMo figure. What *is* comparable is every difference between two rows of the table below: same corpus, same index, same harness, same run. The suites differ from LongMemEval in document granularity (LoCoMo: 19–32 per bank; LongMemEval: ~48), question style and question language, so **the absolute levels are not comparable either — only the within-table deltas are.**

## Method

One bank per `user_id`, built once, one memory per LoCoMo document, `id` = document id, content = that document's dialogue turns joined one per line. Each query is answered against **its own `user_id`'s bank and no other**. Every configuration is scored on that one shared index, so two rows cannot disagree because one's index happened to build differently. The ingestion and the metric definitions are literally the same code as `examples/locomo.rs` — `eval/locomo_dev.rs`, shared by both — so a number here and a number in `eval/LOCOMO.md` mean the same measurement.

### The two passes

The run is two passes over the same axes, in this order, and both are in the table below.

**Pass 1 — one axis at a time, from the shipped start point.** Each axis's full candidate list is measured with every other coordinate held at the shipped default. This is the conventional sweep, and it is here precisely because of what it cannot show: every row of it sits in the one regime where the axes are independent. It is also the control for pass 2 — when a coordinate's pass-1 rows and pass-2 rows disagree, the difference *is* the interaction, measured rather than argued.

**Pass 2 — coordinate descent, 2 round(s)**, from the same shipped start point, over the axes in this order:

| # | axis | values tried | what it is |
|---|---|---|---|
| 1 | `overlap` | 0.00, 0.10, 0.25, 0.50, 0.75, 1.00 | the token-overlap stream's weight; the control, first, because the three-set discipline in `AGENTS.md` §1 exists over this one value |
| 2 | `k` | 5.00, 10.00, 20.00, 40.00, 60.00, 120.00 | the RRF constant, `docs/PERFORMANCE_PLAN.md` P1; **borrowed, never fitted** (`docs/EVALUATION_HYGIENE.md` §2.6) |
| 3 | `bm25_magnitude` | 0.00, 0.25, 0.50, 1.00, 2.00, 4.00 | the BM25 score FTS5 already computed and the rank-only RRF form discards; **diagnostic bound `4.00`**, see the note below |
| 4 | `agreement` | 0.00, 0.05, 0.10, 0.25, 0.50 | the cross-stream bonus; agentmemory applies `1 + 0.05 × (matchedStreams − 1)` |
| 5 | `recency` | 0.00, 0.10, 0.25, 0.50, 1.00 | a third stream over `memories.created_at`; **swept last**, on its own merits — see the recency caveat |
| 6 | `recency_half_life_days` | 1.00, 7.00, 30.00, 90.00 | the recency decay; skipped entirely while `recency == 0.0`, because a zero weight drops the stream before the half-life is read |

| 7 | `vector` | 0.00, 0.10, 0.25, 0.50, 0.75, 1.00, 1.50 | the dense-vector stream (slot 3); **present only in an `--features embed` build**, and **carrying `1.50` as a diagnostic bound past the point where the dense arm becomes the sort key rather than a tiebreaker** — see the note below

One round sweeps every axis once in that order; each move is re-evaluated against the current values of all the others, which is the whole reason for the method: **the interaction is the thing being measured.** `docs/EXCEED_PLAN.md` §0.3 records Hindsight's own `recall_boost.py:29-50` finding that score-space weights above the RRF spread degenerate into lexicographic sorting and cost `recall@20 0.97 → 0.40`; a one-axis-at-a-time sweep holds every other weight at its inert default, which is the single regime in which that collapse is invisible. `bm25_magnitude: 4.00` is carried in the grid on purpose so the collapse is *measured* here rather than imported as a citation.

**The objective, declared.** The walk maximises, in this order: **R@5, then NDCG@10, then MRR, then R@1, then R@10, then R@20.** R@5 leads because `docs/EXCEED_PLAN.md` §4 states the bar in R@5 terms; NDCG@10 and MRR follow because they are the ordering metrics the P1 gate refuses to regress; R@1 follows because the audit's 16.2pp oracle gap is an R@1 gap. **This is an instrument, not a claim about which metric matters** — a different order would walk a different path and would be equally defensible, which is why the order is printed here rather than hidden in a comparator. A move happens only on a **strict** improvement, so an exact tie leaves the walk where it is and a plateau cannot make it drift.

### Determinism and the absence of a confidence interval

The metrics here are **deterministic and seed-independent**: a query's index is fixed by its `user_id`, every configuration is scored on that one index, and `--seed` changes only the order of the progress log. Re-running this binary reproduces every cell below bit-for-bit. That is why **no confidence interval is reported and none should be**: there is no run-to-run variance in these numbers to put an interval around. The only stochastic quantity in the run is wall clock, which is throughput, not a measurement. A sweep that *were* stochastic at this size would run once and be reported without a CI, per `AGENTS.md` §3 — but this one is not that case, and the distinction is worth stating rather than glossing.

### Limitations — read before drawing anything from this grid

1. **Coordinate descent is greedy and axis-ordered, and can therefore miss interactions.** It can walk into a local optimum whose basin depends on the walk order; it never revisits a combination it has left; and a plateau can hide a descent beside it. 2 round(s) is a floor imposed by exactly that weakness, not a claim that it is enough. A different walk order, or a full factorial grid, could land somewhere else. **No cell in this file is a claim about the global optimum of the weight space.**
2. **One corpus.** Every number is LoCoMo, one snapshot of one dataset. A weight that helps here is evidence *about this corpus*; generalization is what `docs/EVALUATION_HYGIENE.md` §3.4 requires and what only a held-out set can supply.
3. **R@20 is a mid-list cut, not a ceiling.** The fused ranking returns a mean of 27.7 documents per query, so a top-20 cut lands inside a 19–32 document list. The discriminating columns are R@1, R@5 and NDCG@10 — ordering, which is what a fusion weight controls.
4. **The recency axis is measured, but what it measures is this harness.** See the caveat below.
5. **Two `FusionWeights` fields were not swept**, and a reader should not assume they are inert: `overlap_scope` (whole-document vs per-segment overlap scoring) and `recency_policy` (`Always` vs `TemporalQueriesOnly`). Both are shipped at their inert defaults and neither is in `docs/EXCEED_PLAN.md` Phases A–B's scope; sweeping them is a separate, separately-motivated job.

### The recency caveat, measured

`Corpus::load` writes every LoCoMo document with `created_at: None`, and the store stamps ingest time on insert — **LoCoMo documents carry no date of their own for the recency stream to read.** The widest `created_at` spread observed across any bank after ingestion was **0.006 s**, which means `recency_rank` is ordering a bank whose documents all carry approximately the same timestamp, and its documented tiebreak (descending decay, then id ascending) is doing the work. **So a delta on the `recency` axis here is a delta attributable to *ingest order*, not to recency as a retrieval signal, and must not be read as evidence for the mechanism.** This is stated because it is easy to miss: the axis produces plausible-looking numbers, and those numbers are about the harness. It is also why `recency` is swept **last** — `docs/EXCEED_PLAN.md` Phase C: agentmemory has no recency term anywhere in its retrieval path, and the "f2–f5 at 27% with token recency" figure that originally motivated this lever **does not exist in their repo** (exhaustive search; the withdrawal is recorded in `docs/CONSISTENCY.md` §14.4). There is no external citation suggesting this axis will win, and this artifact does not supply one.

## Every configuration, in evaluation order

Two passes, and the distinction matters. **`axis-at-shipped`** varies one axis and holds every other at the shipped default — the conventional one-at-a-time sweep, which is the regime in which interactions are invisible. **`walk`** is coordinate descent: each candidate is measured against the *current* values of all the other axes. The control row is `control`.

| # | pass | round | axis swept | configuration | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR | R@pool | R@5 vs shipped | n |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | control | 0 | — (start point) | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.00 | 60.4% | 86.0% | 94.2% | 98.0% | 73.6% | 71.8% | 100.0% | 0/0/1531 | 1531 |
| 1 | axis-at-shipped | 0 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.10 | 60.8% | 86.7% | 94.1% | 98.0% | 74.2% | 72.3% | 100.0% | 17/5/1509 | 1531 |
| 2 | axis-at-shipped | 0 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.25 | 57.2% | 87.4% | 94.4% | 98.3% | 73.0% | 70.4% | 100.0% | 33/11/1487 | 1531 |
| 3 | axis-at-shipped | 0 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.50 | 53.4% | 87.1% | 94.6% | 98.5% | 71.1% | 67.7% | 100.0% | 58/40/1433 | 1531 |
| 4 | axis-at-shipped | 0 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.75 | 50.5% | 84.5% | 94.6% | 98.9% | 69.2% | 65.1% | 100.0% | 72/95/1364 | 1531 |
| 5 | axis-at-shipped | 0 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=1.00 | 48.1% | 82.0% | 94.4% | 99.0% | 67.4% | 62.8% | 100.0% | 79/140/1312 | 1531 |
| 6 | axis-at-shipped | 0 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=1.50 | 45.4% | 78.2% | 90.9% | 99.2% | 64.2% | 60.0% | 100.0% | 97/216/1218 | 1531 |
| 7 | walk | 1 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.10 | 60.8% | 86.7% | 94.1% | 98.0% | 74.2% | 72.3% | 100.0% | 17/5/1509 | 1531 |
| 8 | walk | 1 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.25 | 57.2% | 87.4% | 94.4% | 98.3% | 73.0% | 70.4% | 100.0% | 33/11/1487 | 1531 |
| 9 | walk | 1 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.50 | 53.4% | 87.1% | 94.6% | 98.5% | 71.1% | 67.7% | 100.0% | 58/40/1433 | 1531 |
| 10 | walk | 1 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.75 | 50.5% | 84.5% | 94.6% | 98.9% | 69.2% | 65.1% | 100.0% | 72/95/1364 | 1531 |
| 11 | walk | 1 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=1.00 | 48.1% | 82.0% | 94.4% | 99.0% | 67.4% | 62.8% | 100.0% | 79/140/1312 | 1531 |
| 12 | walk | 1 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=1.50 | 45.4% | 78.2% | 90.9% | 99.2% | 64.2% | 60.0% | 100.0% | 97/216/1218 | 1531 |
| 13 | walk | 2 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.00 | 60.4% | 86.0% | 94.2% | 98.0% | 73.6% | 71.8% | 100.0% | 0/0/1531 | 1531 |
| 14 | walk | 2 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.10 | 60.8% | 86.7% | 94.1% | 98.0% | 74.2% | 72.3% | 100.0% | 17/5/1509 | 1531 |
| 15 | walk | 2 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.50 | 53.4% | 87.1% | 94.6% | 98.5% | 71.1% | 67.7% | 100.0% | 58/40/1433 | 1531 |
| 16 | walk | 2 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.75 | 50.5% | 84.5% | 94.6% | 98.9% | 69.2% | 65.1% | 100.0% | 72/95/1364 | 1531 |
| 17 | walk | 2 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=1.00 | 48.1% | 82.0% | 94.4% | 99.0% | 67.4% | 62.8% | 100.0% | 79/140/1312 | 1531 |
| 18 | walk | 2 | vector | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=1.50 | 45.4% | 78.2% | 90.9% | 99.2% | 64.2% | 60.0% | 100.0% | 97/216/1218 | 1531 |

## What the grid shows

Five readings of the cells above, each computed from them. **None of them is a selection** — they are what the measurements say, not what should ship.

1. **The `agreement` axis moved 0 of 1531 questions at its largest value, and the walk never moved it.** The axis was not run in this pass. The shipped rustdoc already records the same null on LongMemEval at the same magnitudes ("moved zero of 500 questions"); this is the replication on independent data, and it is why the field stays at `0.0`.
2. **`bm25_magnitude` does not collapse on this corpus, even at 0.00.** R@5 0.0% against the control's 86.0%, R@1 0.0% against 60.4%, R@20 0.0% against 98.0% (n=1531). `docs/EXCEED_PLAN.md` §0.3 warns that score-space weights above the RRF spread degenerate into lexicographic sorting and cost `recall@20 0.97 → 0.40`; **that warning does not transfer to this mechanism**, and the reason is the algebra on the field: the term is min-max normalised to `[0,1]` over the query's own BM25 stream and denominated in `1/(k+1)`, so it is bounded by construction and cannot outvote a rank-1 hit on its own. This is a bounded-term null, not a general licence to use large weights — the unbounded `w · magnitude` form the rustdoc rejected is a different mechanism and was not measured here.
3. **The `k` axis was not run in this pass, so this reading has no result.** A run that sweeps one axis has nothing to say about the axes it skipped, and a span printed for one of them would be the fold's own seed value, not a measurement.
4. **`recency` is destructive on this corpus and the numbers are about the harness, not about recency.** At `recency: 0.00`, R@1 falls to 0.0% from 60.4% and R@5 to 0.0% from 86.0% (n=1531); the walk never moved it. Read the recency caveat above before drawing anything from this: every `created_at` here is the harness's own ingest clock, spread over 0.006 s, so this is a measurement of *ingest order* and must not be cited as evidence for or against a recency stream.
5. **The walk's own grid is degenerate after its first move, and the artifact says so rather than letting three axes look dead.** The walk accepted 1 move(s) across 2 round(s), the first being `vector` 0.00 -> 0.25 in round 1. After it, the 0 configurations it measured at `overlap: 0.00` with `recency: 0.00` — spanning `k` n/a, `bm25_magnitude` n/a, `agreement` n/a — produced **0 distinct metric vectors**. That is not noise and not a bug: with the overlap stream dropped there is one stream, so the agreement bonus has no id that two streams found, and the plain RRF score `w/(k+rank)` is already monotone in rank — so neither `k` nor the BM25 magnitude can reorder anything, at any magnitude. (`recency` is the one axis still live there, and it is live for a different reason: a recency stream covers the whole bank, so it changes which documents reach the ranking, not only their order.) **This is precisely the blind spot of a one-at-a-time sweep that `docs/EXCEED_PLAN.md` §0.3 predicts, reproduced here from the other direction**, and it is why pass 1 exists: the same three axes measured at the shipped `overlap: 0.25` are *not* flat (§2 and §3 above). A reader who looked only at the walk rows would conclude `k`, `bm25_magnitude` and `agreement` are dead levers. They are not — they are inert **in that regime**, which is a statement about the regime.

## Round by round

Each row is one accepted move. A coordinate whose candidates all failed to beat the incumbent strictly produced no row — that is a recorded null result, not an omission: the grid above holds the measurement for every candidate that was tried and rejected.

| round | axis | from | to | Δ over all six metrics | R@5 questions improved |
|---|---|---|---|---|---|
| 1 | vector | 0.00 | 0.25 | R@1 -3.3 · R@5 +1.4 · R@10 +0.2 · R@20 +0.3 · NDCG@10 -0.6 · MRR -1.4 | 33 |

| round | coordinates swept | configuration at round end | Δ over the round |
|---|---|---|---|
| 1 | 1 | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.25 | R@1 -3.3 · R@5 +1.4 · R@10 +0.2 · R@20 +0.3 · NDCG@10 -0.6 · MRR -1.4 |
| 2 | 1 | ov=0.25 · k=60 · mag=0.00 · agr=0.00 · rec=0.00 · hl=30d · vec=0.25 | R@1 +0.0 · R@5 +0.0 · R@10 +0.0 · R@20 +0.0 · NDCG@10 +0.0 · MRR +0.0 |

## Per `meta.category` — the shipped configuration and the walk's endpoint

Two rows, and the labels matter. **Row 1 is what ships today** (`overlap: 0.25`, a *fitted* value — see the caveat in the Control section above). **Row 2 is the coordinate-descent endpoint, which is NOT a selected configuration**: it is wherever this particular walk, in this particular order, under this particular declared objective, stopped being able to improve. It has not been chosen by a human, it has not been checked against anything, and it is not a recommendation. It is reported because the per-category shape is where a reordering win or loss shows up, and an aggregate that holds can be two categories moving in opposite directions.

### R@1

| row | multi-hop | open-domain | single-hop | temporal | overall | n |
|---|---|---|---|---|---|
| shipped (`overlap: 0.25`) | 37.1% (n=89) | 67.8% (n=841) | 49.5% (n=281) | 57.2% (n=320) | 60.4% | 1531 |
| coordinate-descent endpoint — **not a selected configuration** | 29.2% (n=89) | 64.6% (n=841) | 47.7% (n=281) | 53.8% (n=320) | 57.2% | 1531 |

### R@5

| row | multi-hop | open-domain | single-hop | temporal | overall | n |
|---|---|---|---|---|---|
| shipped (`overlap: 0.25`) | 67.4% (n=89) | 91.8% (n=841) | 79.4% (n=281) | 81.6% (n=320) | 86.0% | 1531 |
| coordinate-descent endpoint — **not a selected configuration** | 69.7% (n=89) | 92.5% (n=841) | 82.2% (n=281) | 83.4% (n=320) | 87.4% | 1531 |

### R@10

| row | multi-hop | open-domain | single-hop | temporal | overall | n |
|---|---|---|---|---|---|
| shipped (`overlap: 0.25`) | 78.7% (n=89) | 97.0% (n=841) | 94.7% (n=281) | 90.6% (n=320) | 94.2% | 1531 |
| coordinate-descent endpoint — **not a selected configuration** | 78.7% (n=89) | 97.4% (n=841) | 93.6% (n=281) | 91.6% (n=320) | 94.4% | 1531 |

### R@20

| row | multi-hop | open-domain | single-hop | temporal | overall | n |
|---|---|---|---|---|---|
| shipped (`overlap: 0.25`) | 91.0% (n=89) | 99.2% (n=841) | 98.9% (n=281) | 95.9% (n=320) | 98.0% | 1531 |
| coordinate-descent endpoint — **not a selected configuration** | 93.3% (n=89) | 99.2% (n=841) | 99.3% (n=281) | 96.6% (n=320) | 98.3% | 1531 |

### NDCG@10

| row | multi-hop | open-domain | single-hop | temporal | overall | n |
|---|---|---|---|---|---|
| shipped (`overlap: 0.25`) | 50.0% (n=89) | 82.6% (n=841) | 54.9% (n=281) | 72.8% (n=320) | 73.6% | 1531 |
| coordinate-descent endpoint — **not a selected configuration** | 49.3% (n=89) | 81.6% (n=841) | 56.5% (n=281) | 71.9% (n=320) | 73.0% | 1531 |

### MRR

| row | multi-hop | open-domain | single-hop | temporal | overall | n |
|---|---|---|---|---|---|
| shipped (`overlap: 0.25`) | 50.5% (n=89) | 78.2% (n=841) | 62.7% (n=281) | 68.8% (n=320) | 71.8% | 1531 |
| coordinate-descent endpoint — **not a selected configuration** | 47.1% (n=89) | 76.6% (n=841) | 63.2% (n=281) | 67.0% (n=320) | 70.4% | 1531 |

## Per `meta.category` across the `vector` axis

The control, plus **the two weights chosen by a rule, not by score**: the largest swept weight that did **not** lose R@5 against the control, and the largest swept weight. On a corpus where the dense arm turns out to be inert at every weight these two collapse onto one row and the table says so. `n` is printed per cell, because a category percentage without its denominator is not a number.

| weight | metric | multi-hop | open-domain | single-hop | temporal | overall | n |
|---|---|---|---|---|---|---|---|
| `vector: 0.00` — **the control** | R@1 | 37.1% (n=89) | 67.8% (n=841) | 49.5% (n=281) | 57.2% (n=320) | 60.4% | 1531 |
| `vector: 0.00` — **the control** | R@5 | 67.4% (n=89) | 91.8% (n=841) | 79.4% (n=281) | 81.6% (n=320) | 86.0% | 1531 |
| `vector: 0.00` — **the control** | R@10 | 78.7% (n=89) | 97.0% (n=841) | 94.7% (n=281) | 90.6% (n=320) | 94.2% | 1531 |
| `vector: 0.00` — **the control** | R@20 | 91.0% (n=89) | 99.2% (n=841) | 98.9% (n=281) | 95.9% (n=320) | 98.0% | 1531 |
| `vector: 0.00` — **the control** | NDCG@10 | 50.0% (n=89) | 82.6% (n=841) | 54.9% (n=281) | 72.8% (n=320) | 73.6% | 1531 |
| `vector: 0.00` — **the control** | MRR | 50.5% (n=89) | 78.2% (n=841) | 62.7% (n=281) | 68.8% (n=320) | 71.8% | 1531 |
| `vector: 0.10` | R@1 | 32.6% (n=89) | 68.4% (n=841) | 49.1% (n=281) | 59.1% (n=320) | 60.8% | 1531 |
| `vector: 0.10` | R@5 | 68.5% (n=89) | 91.9% (n=841) | 80.8% (n=281) | 83.4% (n=320) | 86.7% | 1531 |
| `vector: 0.10` | R@10 | 78.7% (n=89) | 96.8% (n=841) | 94.3% (n=281) | 90.9% (n=320) | 94.1% | 1531 |
| `vector: 0.10` | R@20 | 91.0% (n=89) | 99.3% (n=841) | 98.9% (n=281) | 95.6% (n=320) | 98.0% | 1531 |
| `vector: 0.10` | NDCG@10 | 49.8% (n=89) | 83.0% (n=841) | 55.8% (n=281) | 73.8% (n=320) | 74.2% | 1531 |
| `vector: 0.10` | MRR | 49.1% (n=89) | 78.8% (n=841) | 62.9% (n=281) | 70.0% (n=320) | 72.3% | 1531 |
| `vector: 1.50` | R@1 | 24.7% (n=89) | 49.3% (n=841) | 41.6% (n=281) | 44.1% (n=320) | 45.4% | 1531 |
| `vector: 1.50` | R@5 | 60.7% (n=89) | 79.2% (n=841) | 85.4% (n=281) | 74.1% (n=320) | 78.2% | 1531 |
| `vector: 1.50` | R@10 | 77.5% (n=89) | 91.3% (n=841) | 95.7% (n=281) | 89.4% (n=320) | 90.9% | 1531 |
| `vector: 1.50` | R@20 | 95.5% (n=89) | 99.4% (n=841) | 100.0% (n=281) | 98.8% (n=320) | 99.2% | 1531 |
| `vector: 1.50` | NDCG@10 | 43.2% (n=89) | 69.2% (n=841) | 56.0% (n=281) | 64.0% (n=320) | 64.2% | 1531 |
| `vector: 1.50` | MRR | 40.3% (n=89) | 63.0% (n=841) | 59.7% (n=281) | 58.0% (n=320) | 60.0% | 1531 |

## Counts

| | |
|---|---|
| documents indexed | 272, in 10 banks (19–32 per bank) |
| dialogue turns decoded | 5882 |
| queries in `queries.json` | 1540 |
| **queries evaluated — the n in every table above** | **1531** |
| dropped: `gold_ids` empty (gold answer, no gold document) | 9 |
| `gold_ids` per query | 1 for 1203 queries, 2–15 for 328, 0 for the 9 dropped |
| `meta.category` values | `multi-hop`, `open-domain`, `single-hop`, `temporal` |
| widest `created_at` spread in any bank, after ingestion | 0.006 s |

**The n above is 1531, not 1540.** 9 queries carry an empty `gold_ids`: they have a gold *answer* but no gold *document*, so no document could be retrieved and every `recall_any@K` would be 0 by construction. They are excluded from every denominator and reported here rather than silently kept — keeping them would understate every row by a flat 0.58pp carrying no measurement at all.

## Runtime

| | |
|---|---|
| configurations scored | 19 (7 distinct) |
| of which: control / axis-at-shipped / walk | 1 / 6 / 12 |
| coordinates per axis, all six axes | 7 candidate values |
| descent rounds requested / completed | 2 / 2 |
| **total wall clock, both passes** | **366.5s** |
| mean per configuration | 19.3s |
| budget cap | none (`--budget-seconds 0`): both passes ran to completion |
| loadavg at start / at end | `200.29 190.37 111.82 15/9561 1441692` / `66.80 89.77 90.43 80/9615 1512185` |

**This is throughput, not recall latency, and it is not comparable to any latency figure in this repo.** It is 1531 recalls plus metric computation per configuration, measured on a box whose `loadavg` was 200.29 190.37 111.82 15/9561 1441692 at the start and 66.80 89.77 90.43 80/9615 1512185 at the end — a box that is not quiet, and on which every per-request figure in `eval/` is a range at best (`AGENTS.md` §3). Nothing here should be quoted as a latency.

## Reproduce

```bash
cargo run --release --example select_fusion -- \
  --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json --rounds 2 --budget-seconds 0 \
  --out-md eval/SELECTION.md --out-json eval/results_selection.json
```

`/tmp/VECTOR_GRID_FIXED.json` carries every configuration's aggregate, its per-category breakdown, and the per-query R@5 vector, so any cell above — including every `up/down/same` count — can be re-derived. This binary owns the whole of this file: **do not hand-edit it**, and note that a bare run writes to `$TMPDIR` instead. Updating the committed artifact means naming `--out-md eval/SELECTION.md`, per `eval/README.md`.

## The one thing to do with this file

Read the grid, then take the decision **to a human**, on the evidence, and record it in `docs/CONSISTENCY.md` with the date, the parameter, the size of the search space and the delta — the counting rule in `docs/EVALUATION_HYGIENE.md` §3.3. Then, and only then, measure LongMemEval **once** and report whatever it says. The whole reason this harness exists is that the last time a sweep chose a value, it chose it by looking at the test set and called the result an improvement; that is a failure this project is now structured to make impossible rather than to apologise for after the fact.
