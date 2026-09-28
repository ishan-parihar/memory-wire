# Scale benchmark — LongMemEval-style, 2026-09-27 run

> **This file is unchanged by the 2026-09-28 re-pin, on purpose.** Its two
> latency figures are a *debug*-profile `cargo test` target and the note below
> records that the build profile of one pair of them is unverified. Re-measuring
> one number in a file whose companion number is still unresolved would not
> resolve anything, so the whole pair is left exactly as recorded. For the
> current release-profile picture — the 10k store, the 4-connection read pool,
> FTS5 `detail=none`, and every number that moved on 2026-09-28 — read
> `docs/CONSISTENCY.md` §12 and the `eval/BENCH_*.md` artifacts. The R@1/R@5
> rows here remain true and are unaffected by any of it.

Harness: `memory-wire/tests/scale.rs` (in-process, in-memory SQLite + FTS5 BM25
fused with overlap-rank via RRF k=60). Pins: `docs/VERSIONS.md`.

**Build profile of every latency in this file: DEBUG.** This harness is a
`cargo test` target, so `cargo test` compiles it in the default profile and
never in `--release`. That includes the `p95 < 500 ms` assertion at
`tests/scale.rs:79` — that gate is a debug-profile gate, and it is the only
latency assertion in the repo. `cargo test --release` is the only way to get a
release-profile reading out of this harness.

Do not compare these numbers against the `--release` figures in
`eval/SCALE_SWEEP.md`, `eval/CODING_LIFE.md` or `eval/RESULTS.md`. A debug run
reports the same retrieval metrics at roughly 2–5× the latency, which is why
every committed `eval/` artifact states its profile in its own header.

> **Unresolved: this file and `docs/CONSISTENCY.md` disagree on the profile of
> one pair of numbers.** The `13–17 ms / 16–23 ms` p50/p95 below are recorded
> here as 3 debug runs. The *same* figures appear in `docs/CONSISTENCY.md` §6 in
> its **"Measured (release)"** column, against a `45 / 48 ms` debug baseline.
> Both records are left exactly as written — neither number has been
> re-measured, and inventing a replacement would be worse than the ambiguity.
> Until someone re-runs this harness under both profiles, treat the build
> profile of these two specific figures as unverified. The R@1/R@5 rows are
> unaffected: they are deterministic and profile-independent.

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
- Recall path uses FTS5 BM25 + overlap fusion, and those two streams are all a
  **default** build has. That qualifier now carries weight: the `embed` feature
  (off by default, `default = []`) brings in `src/vector.rs`, which *does* have
  an embedder, a `memory_vectors` column and a third branch in the query path,
  over vendored int8 MiniLM weights. It changes no ranking — `FusionWeights::vector`
  ships at `0.0` — so a default build compiles none of it and every number in
  this file remains a default-build number. `src/embed.rs` remains the
  dependency-free cosine/rank kernel.
- To truly exceed: vendor the LongMemEval-S set, store vectors at retain,
  3-stream RRF + cross-encoder rerank, then publish the side-by-side scorecard
  here.
