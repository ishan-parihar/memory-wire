# Performance audit

Measured at `59eeb85`. Evidence for every number here is in `eval/` — the
artifact named in each section, its commit, and its provenance line. Claims
without a harness behind them do not belong in this file.

Load note, because it invalidates half of what you might otherwise conclude from
this repo: this machine runs unrelated background HPO jobs and sat at
**loadavg 22–90 on 24 cores** for most of the work below. **Latency and RSS
figures measured here are not comparable to anything and are not quoted.** Quality
metrics are load-independent and are what this audit is about.

---

## 1. The finding that reframes everything

**Retrieval pool coverage on LongMemEval-S is 500/500. Every single question's
answer is already inside the pool memory-wire builds.**

`eval/ORACLE_RERANK.md`, from `examples/oracle_rerank.rs`. Per question it
reconstructs the exact pool recall builds — `recall_inputs` → `rank_candidates` →
`rrf_fuse` with `FusionWeights::SHIPPED`, the same three primitives in the same
order — and locates the first gold row in it.

| first gold rank | 1 | 2–5 | 6–10 | 11–20 | 21–50 | 51–200 | not in pool |
|---|---|---|---|---|---|---|---|
| questions | 419 | 67 | 7 | 5 | 2 | 0 | **0** |

So the oracle — a perfect reranker over the pool we already build — scores
**100.0% at every k**:

| k | current | oracle | Δ |
|---|---|---|---|
| 1 | 83.8% | 100.0% | **+16.2pp** |
| 2 | 90.2% | 100.0% | +9.8pp |
| 5 | 97.2% | 100.0% | **+2.8pp** |
| 10 | 98.6% | 100.0% | +1.4pp |
| 20 | 99.6% | 100.0% | +0.4pp |
| 50 | 99.8% | 100.0% | +0.2pp |

**The answer to "is this the best this architecture can do" is no, and the
remaining headroom is entirely in ordering, not matching.** Not one question on
this suite needs a vector arm, an LLM, or a wider pool.

Two things that does *not* license:

- **An oracle is a ceiling computed with the gold labels.** It sizes the prize.
  It does not predict that any reranker closes it, and 100.0% is unreachable by
  anything that does not peek at the answers.
- **A suite with zero coverage failures cannot validate a new retrieval signal.**
  LongMemEval-S has no coverage failure for a vector arm to fix, so it can neither
  support nor refute one. That is a gap in the benchmark, not a verdict on
  vectors.

## 2. Where the deficit actually is, by category

Current R@5 misses are 14 questions out of 500, and every one is already
retrieved:

| question type | n | R@5 | oracle | at rank 6–20 | at rank 21+ | not in pool |
|---|---|---|---|---|---|---|
| single-session-preference | 30 | 86.7% | 100.0% | 4 | 0 | 0 |
| temporal-reasoning | 133 | 96.2% | 100.0% | 4 | 1 | 0 |
| multi-session | 133 | 97.0% | 100.0% | 3 | 1 | 0 |
| single-session-user | 70 | 98.6% | 100.0% | 1 | 0 | 0 |
| single-session-assistant | 56 | 100.0% | 100.0% | 0 | 0 | 0 |
| knowledge-update | 78 | 100.0% | 100.0% | 0 | 0 | 0 |

- The `single-session-preference` deficit that motivated the E1 reweighting is
  **4/4 recoverable by reordering alone.**
- `single-session-assistant` and `knowledge-update` have **nothing left to gain** —
  they are already at R@5 = 100%.

## 3. The number nobody was tracking

**R@1 is 83.8% and it is not a column in `eval/RESULTS.md`, not a column in the E1
sweep, and not mentioned in the README.**

That matters more than R@5 for how this thing is actually used. An agent asking
"what does the user prefer?" gets the right memory within its top 5 **97.2%** of
the time, but at the very top only **83.8%** of the time. If the agent reads one
memory, or reads until the first plausible one, it is wrong one time in six.

It is also where the headroom is: **+16.2pp**, five times the R@5 prize.

Unknown: how much of the current 83.8% is the E1 reweighting's doing. The E1
sweep recorded R@5/R@10/R@20/NDCG@10/MRR and not R@1, so the pre-E1 R@1 was
never captured. Recovering it is a one-line change to the sweep harness
(`eval/SWEEP_FUSION.md` has the grid) and should be the first thing anyone does,
because without it nobody can say whether R@1 is improving or has been flat.

## 4. Gains achieved, and what they cost

Retrieval, from `eval/RESULTS.md` and `eval/SWEEP_FUSION.md`:

| | before E1 | shipped | change |
|---|---|---|---|
| R@5 | 93.0% | 97.2% | **+4.2pp** |
| R@10 | 97.4% | 98.6% | +1.2pp |
| R@20 | 99.6% | 99.6% | 0.0 |
| NDCG@10 | 83.5% | 88.2% | **+4.7pp** |
| MRR | 83.9% | 89.2% | +5.3pp |

In plain terms: the wrong memory in the top 5 went from 1 in 13 questions to
1 in 36.

Resource use, from `docs/CONSISTENCY.md` §1.1 and `eval/BENCH_RECALL_CURVE.md`:

| | before | after |
|---|---|---|
| commit p50, four write paths | — | **7.2–10.1× faster** (`synchronous=NORMAL`) |
| `get` p50 @1k | 12.8 µs | 4.2 µs (`prepare_cached`) |
| FTS index, 10k memories | 434,176 B | 262,144 B (**−39.6%**, `detail=none`) |
| 10k store on disk | 3,194,880 B | 3,022,848 B (−5.4%) |

**The costs, stated plainly:**

- `eval/BENCH_RECALL_CURVE.md` — the synthetic needle suite dropped from R@5
  100% at every size to **96.9 / 84.4 / 90.6 / 90.6%** at 1k/10k/50k/100k. The
  gain in §4 came at the cost of that. We believe the trade is right (real
  questions beat synthetic ones) but it is a trade, not a free win.
- `eval/CODING_LIFE.md` — R@5 100% → 96.7%, which ties the grep baseline rather
  than beating it. Hit rate remains 100%.
- `synchronous=NORMAL` trades durability: a power loss or OS crash can lose the
  last few seconds of transactions. A process crash cannot, and the database is
  never corrupt either way.

## 5. Structural limits found, not yet addressed

**The token budget binds before the pool does.** From `eval/ORACLE_RERANK.md`: the
store window is the whole bank (38/48/62 rows min/median/max) so the "newest 200
∪ BM25 top-50" description never engages on this suite — and the 100k budget
leaves a caller a median of **35 of 47** ranked rows, on all 500 questions. It
costs nothing at R@5/10/20 (0 hits lost, 0 questions diverge before rank 20), but
**a caller never sees the deep candidates a reranker would reorder.** Any reranker
has to sit before the budget trim, not after it.

**Two fusion parameters have never been swept.** The RRF constant is `k = 60` and
has been 60 since the first commit. `k` controls how sharply the top of the
ranking separates from the tail; at 60 the score difference between rank 1 and
rank 5 is small. This is pure arithmetic on signals that already exist and it has
never been tried.

**An unresolved inconsistency in the E1 record, which P1 must settle.** The E1
agent reported that an agreement bonus "moved 0 of 500 questions at every
magnitude up to 0.50 — both streams return the same documents in the same order."
But RRF with weights (1, 0.25) over two *identical* rankings is
`1.25/(k+r)` — monotone in `r`, hence the same order, hence **R@k could not have
moved**. R@5 moved 93.0 → 97.2. Both statements cannot be true. Either the
streams differ (and the agreement metric measures something narrower than rank
identity), or the bonus was inert for a different reason. Until this is
reconciled, the two retrieval streams' behaviour is not actually understood, and
every fusion decision rests on it.

## 6. Benchmark surface, as it stands

Present: `longmemeval` (500 q, 5 metrics), `coding_life` (15 q), `scale_sweep`
(240→100k), `soak` (concurrency + 5xx + p99), `bench_recall_curve`,
`bench_concurrency`, `bench_write`, `bench_footprint`, `bench_coldstart`,
`sweep_fusion` (the 500-q weight grid), `oracle_rerank` (ceiling).

Absent, and each absence has now cost something:

- **R@1 as a tracked column** anywhere.
- **A suite with a coverage failure**, so no new retrieval signal can be
  validated. §1 shows why this is the binding constraint on evaluation now.
- **Any latency measurement on a quiet machine.** Every committed timing number
  in `eval/` was taken at loadavg 22–90 and is a range, not a value. The idle-box
  re-measurement that was done (`bench_recall_curve` 1,015/6,129/34,999/71,331 µs
  at 1k/10k/50k/100k, spread ±2–6% instead of the loaded 2.9×) is the only
  trustworthy latency set in the repo, and it is not the committed default.
- **A cross-suite regression gate.** Nothing runs `longmemeval` in CI — correctly,
  for quota reasons — so a retrieval regression can only be caught by a human
  running the harness.

## 7. Honest summary

The architecture is not at its ceiling. There is a precisely located, measured
**+16.2pp at R@1 and +2.8pp at R@5 sitting inside pools we already build**, and
**none of it requires better matching** — which is the finding that makes a
cross-encoder or a vector arm a question about ordering rather than about
retrieval, and therefore much cheaper to answer than it looked an hour ago.

What is genuinely unknown: whether any reranker closes the gap. The oracle says
the gap is there. It does not say it is reachable, and the only honest next step
is to measure a cheap reranker against it before spending anything on a model.
