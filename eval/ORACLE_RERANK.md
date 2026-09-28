# Oracle-rerank ceiling for the shipped recall pool (memory-wire)

Measured at `3957f10a00717ce6309f1dac5fa1012d4a06273c` plus the uncommitted `examples/oracle_rerank.rs`.

```bash
cargo run --release --example oracle_rerank -- \
  --data eval/data/longmemeval_s_cleaned.json --out-md eval/ORACLE_RERANK.md
```

## Provenance

| | |
|---|---|
| Date | 2026-09-28 |
| Commit | `3957f10a00717ce6309f1dac5fa1012d4a06273c` |
| Build profile | `release` |
| Toolchain | rustc 1.98.0 (88d9e12ae 2026-08-18) |
| Machine | AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| `/proc/loadavg` at run | `22.40 21.72 32.29 12/7704 1938393` |
| Questions | 500 (seed 42, none dropped) |
| `FusionWeights::SHIPPED` | `bm25: 1.0`, `overlap: 0.25`, `agreement: 0.0`, `k: 60` |
| Stream caps in force | FTS5 BM25 `50`, token-overlap `200`, store window `200` |
| Dataset | `xiaowu0162/longmemeval-cleaned`, `longmemeval_s_cleaned.json` |
| Dataset sha256 | `d6f21ea9d60a0d56f34a05b609c79c88a451d2ae03597821ea3d5a9678c3a442` |
| Dataset revision | unpinned — `eval/download.sh` resolves `main`, so the content hash is the identifier |

The load figure is context for the run, not a caveat on anything below: every number here is a retrieval-quality metric, which does not move with machine load. **No latency or RSS figure is published in this artifact** — this machine runs background jobs, and a timing number taken under that load is an anecdote wearing a decimal point.

## Method

Per question: a fresh in-memory index, one memory per haystack session, query = question text — the same build `examples/longmemeval.rs` uses, not re-implemented. The pool is then constructed in-process from the **same three primitives** `MemoryService::recall` calls, in the same order, with the same weights: `Store::recall_inputs` (the store's bounded window ∪ its BM25 hits), `rank_candidates` over that window, and `rrf_fuse` with `FusionWeights::SHIPPED`. No re-query, no re-rank, no approximation, no question trimmed to make anything fit.

**Two lists, because they are not the same length.** The fusion's ranked output is the **pool** — what a reranker would be handed. What `MemoryService::recall` returns is the **served** list: the same rows after the token-budget trim and the 100-result cap. Oracle metrics are computed on the pool; current metrics on the served list, because that is what this build ships and what `eval/results.json` scored. The difference between the two is measured below, not assumed away.

**The pool is checked against what recall serves.** Per question the harness calls the shipped `MemoryService::recall` and compares ids in order and each fused score. That is what makes "the pool recall built" measured rather than claimed — the two stream caps in `src/api.rs` are mirrored as constants here, so drift in either surfaces as a mismatch instead of quietly measuring a different pool.

- Max absolute fused-score difference, served vs pool: **0.0** (0.0 is exact agreement)
- Served list shorter than the pool: **500** of 500 questions (budget trim; first 5: gpt4_483dd43c (39 of 52); f685340e_abs (31 of 43); 5a4f22c0 (35 of 49); 545bd2b5 (33 of 43); f35224e0 (36 of 53))
- Served list stops agreeing with the pool before rank 20: **0** of 500
- Per-question `recall_any@5/10/20` vs the committed `eval/results.json`: **0** mismatches over 500 compared rows

Every recall metric below is a function of one number per question: the 1-based rank of the **first** gold row, or `None` if no gold row is in the list. "Any gold in the top k" is exactly `first_gold <= k`, so there is no second place for the answer to hide.

## Pool size

The pool is described in the code as "newest 200 rows ∪ BM25 top-50". That is a bound, not a size, and on this suite the bank is the whole haystack, so the real distribution is:

| Rows per question | min / median / max |
|---|---|
| Haystack sessions indexed (the bank) | 38 / 48 / 62 |
| Store candidate window read | 38 / 48 / 62 |
| **Fusion pool (the oracle's list)** | **38 / 47 / 62** |
| Served after the token-budget trim (current) | 26 / 35 / 47 |

The pool is bounded by the bank, not by the 200-row window: the deepest rank a gold row can hold here is **62**, so the `51-200` bucket holds only ranks 51–62 and the per-k "pool smaller than k" counts below are the same fact stated per cutoff. **The candidate window is not what binds on this suite — the token budget is.** The served list is shorter than the pool on **500 of 500** questions, because a 48-session haystack does not fit in the 100000-token budget the harness passes. That costs nothing at R@5/10/20 (0 questions diverge before rank 20), but it does mean a caller never sees the bottom of the ranking, which is the region a reranker would be drawing its deep candidates from.

## First-gold-rank histogram — all 500 questions

Where the first gold row sat inside the **pool**. This is the load-bearing table: a reranker can only move a gold row that is already in the pool, so everything below rank 5 is the entire rerankable prize, and the `not in pool` mass is the part no reordering can reach.

| First gold rank in pool | Questions | Share |
|---|---|---|
| not in pool | 0 | 0.0% |
| 1 | 419 | 83.8% |
| 2-5 | 67 | 13.4% |
| 6-10 | 7 | 1.4% |
| 11-20 | 5 | 1.0% |
| 21-50 | 2 | 0.4% |
| 51-200 | 0 | 0.0% |
| **total** | **500** | **100.0%** |

`pool smaller than k`, per cutoff — the questions whose pool cannot reach k at all, which is what caps oracle R@k at the deep end:

| k | Questions with pool < k | Share |
|---|---|---|
| 1 | 0 | 0.0% |
| 2 | 0 | 0.0% |
| 5 | 0 | 0.0% |
| 10 | 0 | 0.0% |
| 20 | 0 | 0.0% |
| 50 | 367 | 73.4% |
| 200 | 500 | 100.0% |

`pool smaller than k` is stated here rather than as a bucket in the histogram, because the two would double-count: a first-gold rank cannot exceed the pool length, so a gold row past the pool end **is** `not in pool`. Read together the two tables say the whole thing — the histogram says how deep the recoverable mass sits, the per-k counts say which cutoffs the pool is even tall enough to express.

**Questions whose gold set is empty** (the "evidence resolves to no session" shape): **0** of 500. They would be kept in the denominator and scored as a miss at every k, never dropped. Both denominators are reported and are identical, so no figure in this artifact depends on the choice: **500** of 500 questions have a non-empty gold set.

## Oracle R@k vs current R@k

**Oracle R@k** — a hit if any gold row appears in the **pool's** top k. It is the score a *perfect* reranker over this pool would post: a reranker may reorder candidates but cannot invent one. **Current R@k** — what this build posts today over the **served** list, recomputed here from that same pool before the trim. The delta is the size of the prize.

| k | current R@k | oracle R@k | delta | published R@k (`eval/RESULTS.md`) |
|---|---|---|---|---|
| 1 | 83.8% | 100.0% | +16.2pp | not reported |
| 2 | 90.2% | 100.0% | +9.8pp | not reported |
| 5 | 97.2% | 100.0% | +2.8pp | 97.2% |
| 10 | 98.6% | 100.0% | +1.4pp | 98.6% |
| 20 | 99.6% | 100.0% | +0.4pp | 99.6% |
| 50 | 99.8% | 100.0% | +0.2pp | not reported |
| 200 | 99.8% | 100.0% | +0.2pp | not reported |

Recomputed here: current **97.2% / 98.6% / 99.6%**, oracle **100.0% / 100.0% / 100.0%** at R@5 / R@10 / R@20, over 500 questions.

The committed `eval/RESULTS.md` row reads **97.2% / 98.6% / 99.6%**. They agree — and so does every per-question value: all `recall_any@5/10/20` recomputed from the served list matched `eval/results.json` row for row.

## The three-way split — the 14 questions recall currently misses at R@5

| Where the gold row actually is | Questions | Share of all 500 | Share of the 14 misses | Fixable by |
|---|---|---|---|---|
| Rank 6–20 in the pool | 12 | 2.4% | 85.7% | reranking alone |
| Rank 21+ in the pool | 2 | 0.4% | 14.3% | reranking a deeper pool |
| Not in the pool at all | 0 | 0.0% | 0.0% | better **matching** only |
| In pool at ≤5, trimmed out of the served list | 0 | 0.0% | 0.0% | budget, not ranking |
| **total misses** | **14** | **2.8%** | **100.0%** | |

In plain language: of the questions recall gets wrong at rank 5 today, **12** have a gold row sitting between rank 6 and 20 — inside the pool, below the cutoff, reachable by reordering and by nothing else. **2** have a gold row at rank 21 or deeper: still inside the pool, so still reachable by a reranker handed the whole pool, but not by anything operating on a top-20 window. **0** have no gold row in the pool at all, which is the one category no reordering can touch and the only one a different kind of match could recover. **0** were ranked into the top 5 and then dropped at the token-budget trim — a budget fact rather than a ranking fact, and the reason a three-way split needs a fourth row: those questions would otherwise have been counted as a reranking failure.

**12** questions — **2.4%** of the suite — are the entire addressable market for a reranker that reorders the pool recall already builds.

## Per question type

The same categories, cut by the dataset's own `question_type`. This suite's R@5 deficit is known to sit in `single-session-preference` and `single-session-assistant`, so the question each row answers is: is that category's shortfall rerankable, or is it a matching failure?

| Type | n | R@5 | oracle R@5 | Δ | in pool 6–20 | in pool 21+ | absent | trimmed |
|---|---|---|---|---|---|---|---|---|
| knowledge-update | 78 | 100.0% | 100.0% | +0.0pp | 0 | 0 | 0 | 0 |
| multi-session | 133 | 97.0% | 100.0% | +3.0pp | 3 | 1 | 0 | 0 |
| single-session-assistant | 56 | 100.0% | 100.0% | +0.0pp | 0 | 0 | 0 | 0 |
| single-session-preference | 30 | 86.7% | 100.0% | +13.3pp | 4 | 0 | 0 | 0 |
| single-session-user | 70 | 98.6% | 100.0% | +1.4pp | 1 | 0 | 0 | 0 |
| temporal-reasoning | 133 | 96.2% | 100.0% | +3.8pp | 4 | 1 | 0 | 0 |
| **overall** | **500** | **97.2%** | **100.0%** | **+2.8pp** | **12** | **2** | **0** | **0** |

## What this means

- **Oracle R@5 is 100.0%, not 99.6%.** The gold row is inside the pool recall builds for **all 500** questions — 0 land in `not in pool`. The 99.6% is the *current* R@20, and it understates pool coverage because the pool runs far deeper than 20 (38 / 47 / 62 rows, min/median/max). A perfect reranker over the pool this build already assembles would post R@5 = 100.0%, against today's 97.2%.
- **The entire R@5 deficit is rerankable: 12 of the misses sit at rank 6–20, 2 at rank 21+.** Every question recall currently misses at rank 5 has its gold row in the pool, between ranks 6 and 50. Nothing is lost to pool coverage, so on this suite nothing at R@5 is a matching failure.
- **The largest single prize is at rank 1, not at rank 5.** Current R@1 is 83.8% against a 100.0% oracle — **16.2pp**, against 2.8pp at R@5. 419 questions already rank the gold row first; the 81 that do not are what a reranker would have to move, and that is a larger prize than the R@5 headline.
- **The recoverable mass is concentrated.** `single-session-preference` supplies 4 of the 12 rank-6–20 misses on only 30 questions (R@5 86.7% against a 100.0% oracle), and `temporal-reasoning` 5 of them on 133. `knowledge-update` and `single-session-assistant` are already at 100% R@5 and have nothing left to gain.
- **This suite cannot speak to better *matching*, in either direction.** With 0 questions missing from the pool there is no coverage failure here for any new retrieval signal to fix. The pool is the entire 38 / 48 / 62 row haystack — `recall_inputs` reads every row on this suite — so pool coverage is unconstrained by construction, and no figure here supports or refutes a vector arm or an LLM.
- **The candidate list is not the binding constraint; the serving budget is.** The pool holds 38 / 47 / 62 rows while the 100,000-token budget leaves a caller 26 / 35 / 47, on all 500 questions. No question diverges before rank 20, so the trim cost 0 R@5 hits — but the deep candidates a reranker would be reordering are exactly the ones the budget keeps out of the response.
