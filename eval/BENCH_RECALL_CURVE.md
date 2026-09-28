# Recall quality and latency vs bank size (memory-wire)

Run 2026-09-28 from `--profile=release` on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs.

One fixed query set of 32 queries — identical text, identical gold ids at every size — recalled at a 2000-token budget through `MemoryService::recall`, which is the HTTP and MCP path with its 100-result cap and budget trim in place. Sizes 1000 / 10000 / 50000 / 100000; 3 latency passes per size; seed 42.

Corpus: 32 gold rows plus distractors, mixed by a seeded LCG permutation so gold rows land throughout insertion order rather than at the front. Each gold row's unique nonce token is what its query repeats; the rest of every row — gold and distractor alike — comes from the four-topic generator `examples/scale_sweep.rs` uses, so the FTS stream has real topical competition at each size and the corpus vocabulary stays comparable with `eval/SCALE_SWEEP.md`.

**The candidate pool is measured, not assumed.** `Gold in window` counts gold rows inside the `ORDER BY rowid DESC LIMIT 200` window (exact: rowids are assigned in write order). `Gold in BM25` counts gold rows the FTS stream returns at `LIMIT 50`, read through the store's own public `keyword_search_fts`. `Gold in pool` is the union — the candidate set fusion could choose from, and therefore the ceiling on R@1.

| Memories | Signal | Distractor | Signal share | Gold in window | Gold in BM25 | Gold in pool | R@1 | R@5 | p50 us | p95 us | Build |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1000 | 32 | 968 | 3.200% | 3 | 32 | 32 | 78.1% | 100.0% | 1090 | 1414 | 0.2s |
| 10000 | 32 | 9968 | 0.320% | 0 | 32 | 32 | 75.0% | 100.0% | 7019 | 9399 | 3.2s |
| 50000 | 32 | 49968 | 0.064% | 0 | 32 | 32 | 81.2% | 100.0% | 38366 | 45614 | 23.1s |
| 100000 | 32 | 99968 | 0.032% | 0 | 32 | 32 | 75.0% | 100.0% | 79205 | 104752 | 49.9s |

**Quality does not degrade with size, on this corpus.** Every gold row is in the BM25 top-50 at every size, so the pool never loses an answer and R@5 is 100% throughout. R@1 sits at 75.0-81.2% with no trend: at 32 queries one query is 3.1 percentage points, and the spread across the whole ladder is smaller than that. R@1 and R@5 are deterministic for a given seed, so a movement in them is a changed corpus or a changed ranking, never measurement noise.

**Latency does grow with size, and the bounded pool does not stop it.** Median recall goes 1090 us at 1000 memories to 79205 us at 100000 — 73x for a 100x larger bank, close to linear in the index. The pool bound is real, but it bounds the *fusion* stage: the candidate set provably never exceeds 200 + 50 rows, so the overlap rank, RRF and the budget trim do not grow with the bank. What still grows is the FTS5 `MATCH` scan behind it, which returns 50 rows by walking postings, and a `LIMIT` does not bound a scan. This corpus makes that cost as visible as it can: there are only four topic sentences among the distractors, so every query token matches roughly a quarter of the bank and IDF weighting has almost no rare term to exploit except the gold rows' nonce. A corpus with a realistic vocabulary puts more rare terms in the query and shortens the traversal, so read the *shape* of this curve — bounded fusion, unbounded scan — as the finding, and the absolute microseconds as specific to this generator. It is the number to re-measure after any change to the read path.

**The gap this closes.** `eval/RESULTS.md` records that on LongMemEval-S the 200-row pool "never binds" — that suite indexes 38-62 sessions per question, so every session is scored on every query and the suite cannot detect a pool-bound regression on its own. Here the window holds 3 of 32 gold rows at 1000 memories and 0 at 100000; R@1 moves 78.1% -> 75.0% and R@5 100.0% -> 100.0% across the ladder. Recall quality as a bank grows past 10k is a number this repository had never measured.

Every requested size was built and measured.

## What this does not measure

- Recall quality on human text. Generated strings and a nonce token are not LongMemEval sessions: R@1 here says a retrieval pipeline finds an identifiable needle in a haystack of a given size, not that it answers natural multi-session questions. `eval/RESULTS.md` stays the human-text number; this does not replace it.
- The tag-filtered recall path, `reflect`, the lifecycle routes, or non-default budgets.
- Concurrent readers — recall latency here is single-threaded; `examples/bench_concurrency.rs` is where contention is measured.
- Write cost. The corpus is built with `Store::put` outside every timed region; `eval/BENCH_WRITE.md` prices the write path.
- First-touch cost. Latency is the median of warm passes on an already-built store; `examples/bench_coldstart.rs` reports the first query separately.

## Limitations

- R@k is binary per query, so at 32 queries one query is 3.1 percentage points. A move of one or two points is one query, not a trend.
- Distractors repeat one of four topic sentences, so a 100k bank is a repetition of a small vocabulary. Real text has a longer tail, and BM25's IDF weighting over four topic words is not the same as over a large one. The consequence is stated where it matters: the *pool* behaviour measured here transfers to real text, the latency magnitude does not, and the R@k scores may not either.
- A gold row's nonce is unique by construction, so a defect in the BM25 stream surfaces as R@1 loss here. A defect that only affects rows sharing every term with a competitor will not.
- Build time grows with size and a large size can take minutes. Build seconds are in the table so a slow row is visible instead of skipped.
