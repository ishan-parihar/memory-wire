# Scale benchmark — LongMemEval-style, 2026-09-27 run

Harness: `memory-wire/tests/scale.rs` (in-process, debug build, in-memory
SQLite + FTS5 BM25 fused with overlap-rank via RRF k=60).
Pins: `docs/VERSIONS.md`.

This is the only suite in the repo whose corpus is larger than the 200-row recall
candidate pool, which makes it the regression probe for that bound. It is
deliberately adversarial about it: the 100 signal memories are inserted *first*
and the 4,900 distractors after, so at query time every signal sits outside the
"newest 200 rows" window and can only enter the pool through the BM25 top-50 union
clause. A pool that lost its union with the hits, or a BM25 arm that regressed,
would show up here as a drop below 20/20.

## Results (5,000 memories: 100 signal × 4 anchors + 4,900 distractors)

| Metric | memory-wire | Competitor reference |
|---|---|---|
| R@1 (20 queries, 4 categories) | **20/20 (100%)** | — (official LongMemEval-S: agentmemory R@5 95.2%; hindsight SOTA, exact R@1 n/a here) |
| R@5 | **20/20 (100%)** | agentmemory 95.2% R@5 on real LongMemEval-S |
| recall p50 / p95 | **13–17 ms / 16–23 ms** (3 debug runs, 2026-09-27) | hindsight ~1–2 s daemon ops (model-loaded); agentmemory local BM25 ms-range |
| categories covered | single-fact, multi-hop terms, technical, policy/temporal | full LongMemEval: 6 categories incl. knowledge-update + time-sensitive |

## Honest scope

- Corpus is **synthetic anchors + distractors**, not the official LongMemEval
  dataset (115k-token histories, annotated QA). 100% here means the BM25+RRF
  plumbing holds at 5k scale with a 16–23 ms p95 — it does **not** claim SOTA on
  the real benchmark.
- The latency row moved 45 ms / 48 ms → 13–17 ms / 16–23 ms between the two runs
  with **no change to R@1 or R@5**, both still 20/20. That is the bounded
  candidate pool: recall used to score every row in the bank, and now scores at
  most 200 plus the BM25 hits, so a 5,000-row bank costs roughly what a 200-row
  one does. Quality held while cost fell, which is the result worth having.
- Recall path uses FTS5 BM25 + overlap fusion, and those two streams are all
  this build has. `src/embed.rs` holds the dependency-free cosine/rank kernel a
  vector stream would consume; there is no embedder, no vector column, and no
  vector branch in the query path, so a vector stream needs embedding-at-retain
  plus storage before it can be fused at all.
- To truly exceed: vendor the LongMemEval-S set, store vectors at retain,
  3-stream RRF + cross-encoder rerank, then publish the side-by-side scorecard
  here.
