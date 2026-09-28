# memory-wire 0.4.0

**The answer loop got 20 points better, and it was a default value.**

## The headline

`DEFAULT_RECALL_BUDGET` was **2,000 tokens**. The median LongMemEval session is
**2,626 tokens**. A recall budget below the size of a stored memory cannot return
a median memory: the trim truncates the top hit and skips everything below it.
Raised to 8,000 — 3× the median, so three sessions fit whole.

| end-to-end answer accuracy, 25 questions | before | after |
|---|---|---|
| retrieval-conditioned | 40.0% | **60.0%** |
| closed-book control (no memory at all) | 8.3% | 8.3% |
| **delta** | **+31.7pp** | **+51.7pp** |
| gold session present in served context | 92.0% | 92.0% |
| … answered **wrong** despite gold served | **52.0%** | **32.0%** |

Read the last two rows together. Retrieval did not change — the gold session was
already being found and served 92.0% of the time in both runs. What changed is
that the answerer stopped being starved of the evidence it had already been given.

**`eval/RESULTS.md` cannot see this defect at all.** Its harness passes an
explicit 100,000-token budget, so the default is structurally invisible to it.
That is how weeks of retrieval work left a 2× gap between what recall found and
what the consumer received. `tests/recall_budget.rs` now asserts a gold row
survives the trim.

Caveats, not buried: n=25 with 2 row errors, so the closed-book cell is
provisional under the harness's own rule; the judge is a local free-tier model,
not LongMemEval's official grader. It sizes an effect. It does not establish a
rate.

## Retrieval: where we honestly stand

| | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR |
|---|---|---|---|---|---|---|
| **memory-wire** | **83.8%** | **97.2%** | 98.6% | **99.6%** | **88.2%** | **89.2%** |
| agentmemory BM25+Vector | — | 95.2% | 98.6% | 99.4% | 87.9% | 88.2% |
| agentmemory BM25-only | — | 86.2% | 94.6% | 98.6% | 73.0% | 71.5% |

Those are the fitted numbers, and the clean ones are **93.0 / 97.4 / 99.6 /
83.5 / 83.9** at equal weight. `overlap: 0.25` was chosen by sweeping 46
configurations against these 500 questions. On 1,531 LoCoMo queries that never
chose it, the weight is worth **+1.4pp R@5, not +4.2pp** — direction replicated,
magnitude not. The honest out-of-sample estimate is **≈94.4% R@5: behind
agentmemory's hybrid, not 2.0pp ahead of it.** Both rows are in the README.

Two things the source audit established that are worth more than the number:

- **Our FTS5 BM25 alone scores 97.0% R@5** — 10.8pp above agentmemory's
  BM25-only 86.2%, and above their *full hybrid* 95.2%. Their +9.0pp from adding
  vectors is largely repair of their own weak lexical arm, not a capability we
  lacked. Their own doc says the same: BM25+Vector 95.2% "nearly matches" pure
  vector search 96.6%.
- **Hindsight publishes no retrieval-recall number at all.** Its one LoCoMo
  figure, 92.0%, is LLM-judged *answer* accuracy from a `rag` run with two Gemini
  calls in the loop; the run record has `mean_recall: null`. An exhaustive grep
  for `recall@|ndcg|pool_size` across both repos returns one code comment.

## Dense vectors: built, measured, shipped off

`--features embed` adds a 23 MB int8 all-MiniLM-L6-v2 arm as a third fusion
stream. A coordinate descent over its weight on the LoCoMo dev set swept
0.00 → 1.50. The arm is **reordering-only** — R@20 rises monotonically while R@1
falls, and dense alone reaches *less* than the lexical fusion (R@5 62.4% vs
86.0%). Best honest contribution: **+1.4pp R@5 at 0.25, with NDCG@10 and MRR
moving backward**, against +53.8 MB and a fourth shared library. At 1.50 the
up/down ratio inverts (97/216).

So `FusionWeights::vector` ships at **0.0** in every build. There is a real
alternative reading — NDCG@10 and MRR both peak at 0.10, where R@1, NDCG@10 and
MRR improve together for 18 up / 6 down — and choosing between them is a
product decision about how many results a caller reads. It is left unmade.

## Also in this release

- **A real bug in the cosine kernel.** `rank_by_cosine` discarded any candidate
  with cosine ≤ 0 rather than ranking it last, deleting a document whose
  similarity to the query was −0.013. Dense-arm R@pool 99.9347% → 100.0%.
- **`recallSynonyms` was dead on the product path.** An explicit recall budget
  discarded the synonym set, and both HTTP and MCP always pass one — so no bank
  could ever have used it.
- **The embed build is now self-contained.** It failed to start on a minimal root
  because it dynamically linked `libstdc++`. A new `build.rs` resolves a static
  `libstdc++.a` into rustc's existing `-Wl,-Bstatic` group, so `--as-needed`
  declines to record a `DT_NEEDED` at all. It now runs real ONNX inference inside
  a minimal root. The default build is byte-identical with `build.rs` moved
  aside. The 4th `NEEDED` on the embed build is `ld-linux-x86-64.so.2`, reachable
  only through `ld.so` since glibc 2.34 — traced by replaying the link line, and
  benign.
- **Four retrieval mechanisms built, all default-off**, each mutation-tested
  inert: BM25-magnitude tie-breaker (the fusion was discarding a score FTS5
  already computes), a recency stream, sentence-level overlap, and a
  temporal-query classifier.
- **`created_at` is plumbed from `haystack_dates`.** The harness passed `None`,
  so every row was stamped at insert and a recency stream would have ranked by
  insertion order.
- Gate: **291 tests, clippy `-D warnings` clean, 0 rustdoc warnings.**

## The standing invitation, stated plainly

The vector arm is inert in every build and costs nothing to leave off — but it
is a flag someone can flip without reading this. It is off *by measurement*,
not by oversight.

## Reproduce

```bash
cargo test --locked
cargo run --release --example longmemeval -- --out-md eval/RESULTS.md
cargo run --release --example answer_quality -- --n 25 --out-md eval/ANSWER_QUALITY.md
```

Every number above is in a committed artifact under `eval/`, and each artifact
carries its own provenance. `docs/CONSISTENCY.md` is the append-only record of
what was measured, on what, at what load, and what was rejected.
