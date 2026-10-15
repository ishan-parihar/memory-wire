# Recall quality and latency vs bank size (memory-wire)

Run 2026-10-13 from `--profile=release` on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs.

One fixed query set of 32 queries — identical text, identical gold ids at every size — recalled at a 2000-token budget through `MemoryService::recall` (shipped) or `MemoryService::recall_with_weights` (every other row), which is the same code path with the weights as an argument. Sizes 1000 / 10000 / 50000 / 100000; 3 latency passes per size, **interleaved across configurations**; seed 42; pool 200 ∪ BM25 50.

One bank is built per size and every configuration is scored on it, so two rows cannot disagree because their index happened to build differently. The passes are interleaved rather than run configuration-after-configuration so that a load spike lands on all of them in the same pass — that makes the latency columns comparable *to each other* and to nothing else. **The absolute microseconds still need an idle box; the R@1/R@5 columns do not.** Load at start `16.68 14.64 12.92 20/6965 3619525`, at end `16.68 14.64 12.92 22/6965 3619525`.

Corpus: 32 gold rows plus distractors, mixed by a seeded LCG permutation so gold rows land throughout insertion order rather than at the front. Each gold row's unique nonce token is what its query repeats; the rest of every row — gold and distractor alike — comes from the four-topic generator `examples/scale_sweep.rs` uses, so the FTS stream has real topical competition at each size and the corpus vocabulary stays comparable with `eval/SCALE_SWEEP.md`. Every query is **six tokens**: five from its topic sentence, repeated across a quarter of the bank, plus the unique nonce. So a distractor of the matching topic matches **five** of a query's six tokens and the gold row matches **all six** — the count stream separates them by one, and only term rarity separates them by five.

**The candidate pool is measured, not assumed.** `in window` counts gold rows inside the `ORDER BY rowid DESC LIMIT 200` window (exact: rowids are assigned in write order). `in BM25` counts gold rows the FTS stream returns at `LIMIT 50`, read through the store's own public `keyword_search_fts`. `in pool` is the union — the candidate set fusion could choose from, and therefore the ceiling on R@1.

| Memories | configuration | signal % | in window | in BM25 | in pool | R@1 | R@5 | p50 us | p95 us | Build |
|---|---|---|---|---|---|---|---|---|---|---|
| 1000 | shipped | 3.200% | 3 | 32 | 32 | 71.9% | 96.9% | 1187 | 1653 | 0.3s |
| 1000 | raw overlap=0.50 | 3.200% | 3 | 32 | 32 | 71.9% | 96.9% | 1197 | 1597 | 0.3s |
| 1000 | raw overlap=0.75 | 3.200% | 3 | 32 | 32 | 71.9% | 100.0% | 1173 | 1797 | 0.3s |
| 1000 | raw overlap=1.00 | 3.200% | 3 | 32 | 32 | 78.1% | 100.0% | 1162 | 1528 | 0.3s |
| 1000 | raw overlap=2.00 | 3.200% | 3 | 32 | 32 | 84.4% | 100.0% | 1187 | 1492 | 0.3s |
| 10000 | shipped | 0.320% | 0 | 32 | 32 | 75.0% | 84.4% | 6724 | 9053 | 3.3s |
| 10000 | raw overlap=0.50 | 0.320% | 0 | 32 | 32 | 75.0% | 90.6% | 6705 | 9415 | 3.3s |
| 10000 | raw overlap=0.75 | 0.320% | 0 | 32 | 32 | 75.0% | 100.0% | 6681 | 8908 | 3.3s |
| 10000 | raw overlap=1.00 | 0.320% | 0 | 32 | 32 | 75.0% | 100.0% | 6762 | 8668 | 3.3s |
| 10000 | raw overlap=2.00 | 0.320% | 0 | 32 | 32 | 84.4% | 100.0% | 6729 | 9694 | 3.3s |
| 50000 | shipped | 0.064% | 0 | 32 | 32 | 75.0% | 90.6% | 38391 | 52502 | 20.4s |
| 50000 | raw overlap=0.50 | 0.064% | 0 | 32 | 32 | 75.0% | 96.9% | 38478 | 60804 | 20.4s |
| 50000 | raw overlap=0.75 | 0.064% | 0 | 32 | 32 | 78.1% | 100.0% | 38417 | 54073 | 20.4s |
| 50000 | raw overlap=1.00 | 0.064% | 0 | 32 | 32 | 81.2% | 100.0% | 38463 | 58340 | 20.4s |
| 50000 | raw overlap=2.00 | 0.064% | 0 | 32 | 32 | 87.5% | 100.0% | 38404 | 54689 | 20.4s |
| 100000 | shipped | 0.032% | 0 | 32 | 32 | 71.9% | 90.6% | 84578 | 142561 | 38.4s |
| 100000 | raw overlap=0.50 | 0.032% | 0 | 32 | 32 | 71.9% | 96.9% | 82431 | 146679 | 38.4s |
| 100000 | raw overlap=0.75 | 0.032% | 0 | 32 | 32 | 71.9% | 100.0% | 83475 | 141648 | 38.4s |
| 100000 | raw overlap=1.00 | 0.032% | 0 | 32 | 32 | 75.0% | 100.0% | 85225 | 125630 | 38.4s |
| 100000 | raw overlap=2.00 | 0.032% | 0 | 32 | 32 | 81.2% | 100.0% | 84003 | 153408 | 38.4s |

## Per size, every configuration

**1000 memories** (32 gold rows, 3.200% signal share, build 0.3s)

| configuration | BM25 | overlap | R@1 | R@5 | p50 us | p95 us |
|---|---|---|---|---|---|---|
| shipped | 1.00 | 0.25 | 71.9% | 96.9% | 1187 | 1653 |
| raw overlap=0.50 | 1.00 | 0.50 | 71.9% | 96.9% | 1197 | 1597 |
| raw overlap=0.75 | 1.00 | 0.75 | 71.9% | 100.0% | 1173 | 1797 |
| raw overlap=1.00 | 1.00 | 1.00 | 78.1% | 100.0% | 1162 | 1528 |
| raw overlap=2.00 | 1.00 | 2.00 | 84.4% | 100.0% | 1187 | 1492 |

**10000 memories** (32 gold rows, 0.320% signal share, build 3.3s)

| configuration | BM25 | overlap | R@1 | R@5 | p50 us | p95 us |
|---|---|---|---|---|---|---|
| shipped | 1.00 | 0.25 | 75.0% | 84.4% | 6724 | 9053 |
| raw overlap=0.50 | 1.00 | 0.50 | 75.0% | 90.6% | 6705 | 9415 |
| raw overlap=0.75 | 1.00 | 0.75 | 75.0% | 100.0% | 6681 | 8908 |
| raw overlap=1.00 | 1.00 | 1.00 | 75.0% | 100.0% | 6762 | 8668 |
| raw overlap=2.00 | 1.00 | 2.00 | 84.4% | 100.0% | 6729 | 9694 |

**50000 memories** (32 gold rows, 0.064% signal share, build 20.4s)

| configuration | BM25 | overlap | R@1 | R@5 | p50 us | p95 us |
|---|---|---|---|---|---|---|
| shipped | 1.00 | 0.25 | 75.0% | 90.6% | 38391 | 52502 |
| raw overlap=0.50 | 1.00 | 0.50 | 75.0% | 96.9% | 38478 | 60804 |
| raw overlap=0.75 | 1.00 | 0.75 | 78.1% | 100.0% | 38417 | 54073 |
| raw overlap=1.00 | 1.00 | 1.00 | 81.2% | 100.0% | 38463 | 58340 |
| raw overlap=2.00 | 1.00 | 2.00 | 87.5% | 100.0% | 38404 | 54689 |

**100000 memories** (32 gold rows, 0.032% signal share, build 38.4s)

| configuration | BM25 | overlap | R@1 | R@5 | p50 us | p95 us |
|---|---|---|---|---|---|---|
| shipped | 1.00 | 0.25 | 71.9% | 90.6% | 84578 | 142561 |
| raw overlap=0.50 | 1.00 | 0.50 | 71.9% | 96.9% | 82431 | 146679 |
| raw overlap=0.75 | 1.00 | 0.75 | 71.9% | 100.0% | 83475 | 141648 |
| raw overlap=1.00 | 1.00 | 1.00 | 75.0% | 100.0% | 85225 | 125630 |
| raw overlap=2.00 | 1.00 | 2.00 | 81.2% | 100.0% | 84003 | 153408 |


**The pool held every gold row at every size and every configuration**, so R@1 and R@5 below are ranking results and nothing was lost to the candidate window. R@k is deterministic for a given seed, so a movement in them is a changed corpus or a changed ranking, never measurement noise.

**3.1 percentage points per query.** A move of one or two points in R@1 or R@5 is one question, not a trend.

Every requested size was built and measured.

## What this does not measure

- Recall quality on human text. Generated strings and a nonce token are not LongMemEval sessions: R@1 here says a retrieval pipeline finds an identifiable needle in a haystack of a given size, not that it answers natural multi-session questions. `eval/RESULTS.md` stays the human-text number; this does not replace it.
- The tag-filtered recall path, `reflect`, the lifecycle routes, or non-default budgets.
- Concurrent readers — recall latency here is single-threaded; `examples/bench_concurrency.rs` is where contention is measured.
- Write cost. The corpus is built with `Store::put` outside every timed region; `eval/BENCH_WRITE.md` prices the write path.
- First-touch cost. Latency is the median of warm passes on an already-built store; `examples/bench_coldstart.rs` reports the first query separately.

## Limitations

- R@k is binary per query, so at 32 queries one query is 3.1 percentage points.
- Distractors repeat one of four topic sentences, so a 100k bank is a repetition of a small vocabulary. Real text has a longer tail, and BM25's IDF weighting over four topic words is not the same as over a large one. The consequence is stated where it matters: the *pool* behaviour measured here transfers to real text, the latency magnitude does not, and the R@k scores may not either.
- A gold row's nonce is unique by construction, so a defect in the BM25 stream surfaces as R@1 loss here. A defect that only affects rows sharing every term with a competitor will not.
- Build time grows with size and a large size can take minutes. Build seconds are in the table so a slow row is visible instead of skipped.
