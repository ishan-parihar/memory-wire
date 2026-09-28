# Learned re-ranking: audit and plan

Status: proposal, nothing implemented. Supersedes nothing.
Author: audit of 2026-09-28, from `eval/ORACLE_RERANK.md` and the registry.
Constraint: the single ~8.4 MB static binary, offline, no model download stays.

## 1. Is machine learning the right tool here?

Yes, and more precisely than the question was posed. The problem is not
"ranking is hard". It is a **learning-to-rank problem with 500 queries × ~47
candidate rows**, where every answer is already retrieved.

| k | current | perfect rerank | gap |
|---|---|---|---|
| 1 | 83.8% | 100.0% | **+16.2pp** |
| 5 | 97.2% | 100.0% | +2.8pp |
| 20 | 99.6% | 100.0% | +0.4pp |

Where the 500 first-gold ranks actually fall: 419 at rank 1, 67 at ranks 2–5,
**7 at 6–10, 5 at 11–20, 2 at 21–50, 0 at 51–200, 0 not in the pool.**

So the target is precise: **81 questions where the gold row is in the pool and
outranked.** Fourteen of them sit at ranks 6–20, which is the band a reranker
exists to fix. No amount of retrieval work moves these, because retrieval is
not what is failing.

## 2. The data-size question, answered before it is asked

Two facts decide the whole shape of this plan.

**The effective sample size is 500, not 23,000.** There are ~500 positives and
~23,000 negatives, but the negatives within a query are exchangeable — the
quantity that generalises is the *query*, and query-level error bounds converge
as 1/√n in queries. The published guidance for LambdaMART-class rankers is
*tens of thousands* of queries (TREC Deep Learning). We are one to two orders
of magnitude short of where that method is documented to work.

**LambdaMART's core advantage is mathematically void on our data.** LambdaRank
weights each pair's gradient by ΔNDCG. With exactly one relevant document per
query, DCG@k is `1/log2(1+rank)` inside the top k and exactly 0 outside it. So
for any pair where both documents sit outside the top k, ΔNDCG is **0** — no
gradient. For a pair straddling the boundary, ΔNDCG is the **same constant**
every time. The `1/|1 − ΔNDCG|` discount that distinguishes LambdaMART from
RankNet is therefore constant, and LambdaMART collapses to **pairwise logistic
regression with no discount weighting**.

That is a derivation from the metric definition, not an empirical result, and it
should be confirmed numerically before it is relied on. But if it holds, the
conclusion is firm: **do not reach for LambdaMART. Reach for a plain pairwise
logistic ranker over a handful of features.** A 321-parameter model on 500
queries will overfit regardless of what it is called.

## 3. Options, honestly costed

| Approach | Inference-time dependency | Install-size impact | Verdict |
|---|---|---|---|
| Linear weights over existing signals | none — ~10 `f32`s and a dot product | ~40 bytes | **do this first** |
| GBDT exported to flat arrays | none — ~60-line hand-written scorer | JSON embedded via `include_str!` | viable, second |
| Small MLP (10→16→8→1) | none — forward pass is matmuls | 1,284 B of `f32` | probably same as linear, at more risk |
| Cross-encoder | ONNX Runtime + weights | +23–91 MB model, +108 MB runtime | **disqualified** |

### 3.1 The Rust GBDT crate situation is bad, and irrelevant

Every maintained option is either abandoned or drags in C++:
`lightgbm` 0.2.3 was published **2021-02-11** (5.5 years stale) and builds the
whole LightGBM C++ library through `cmake` + `libclang`; `xgboost` 0.1.4 dates
from **2019-03-05**; `boosters` 0.1.0 from 2023 and unmaintained; `forust` is a
2016 name whose successor `forust-ml` 0.5.0 is alive but whose ranking-objective
support is unverified; `sooboost-core` 0.2.0 (2026-09-01) is a clean pure-Rust
GBDT with **no ranking objective**; `sequoia-boost` 0.2.0 (2026-08-16) does
claim LambdaMART but has 3 stars, 178 downloads, and describes itself as largely
AI-generated.

**None of that needs to be true.** LightGBM's `dump_model` JSON and XGBoost's
`save_model` JSON both encode every tree as flat
`(split_feature, threshold, decision_type, left_child, right_child, leaf_value)`.
Train in Python, embed the JSON, walk it in ~60 lines of Rust. No crate, no
C++, no inference runtime — strictly better than any of the above.

### 3.2 The cross-encoder is disqualified, not merely unattractive

`cross-encoder/ms-marco-MiniLM-L-6-v2` is 22.7M parameters: **91 MB** as fp32
ONNX, 23 MB int8. `ort` needs ONNX Runtime, whose static library is
**108 MB** uncompressed. `bge-reranker-base` is 278M parameters / 1.1 GB — that
alone is Hindsight-scale, which is the thing we exist to avoid.

The decisive number is latency. Measured on a 2019 i7-9750H, MiniLM-L6 int8
reranking **50 documents takes 578 ms** (fp32: 773 ms). Our pool is ~47 rows. So
the cheapest viable cross-encoder adds **roughly 600–800 ms to every recall**,
against a current 1–6 ms. That is a 100× latency regression to chase +16.2pp at
R@1, and it breaks the single-binary claim by 3–11×.

## 4. Features worth fitting a model over

All of these are already computed or one line away — no new query, no new index.

1. **`1/(k + rank_bm25)`** — the current BM25 RRF term.
2. **`1/(k + rank_overlap)`** — the current overlap RRF term.
3. **BM25 score magnitude** — FTS5's `bm25()` returns a real number and **we
   currently discard it**, keeping only the rank. A row at rank 7 with a far
   better BM25 score than a marginal row at rank 1 is precisely the case that
   RRF's rank-only form cannot see. **This is the strongest Tier-0 hypothesis in
   the whole document, and it costs nothing to test.**
4. **Distinct-token match fraction** — the overlap stream's raw count ignores how
   many distinct query tokens matched.
5. **Row length in tokens** — BM25 already length-normalises, but a residual
   length feature often still carries signal.
6. **Provenance** — found by BM25 only / overlap only / both.
7. **Row age** — Hindsight's temporal arm exists for a reason, and the cheapest
   published evidence that *structured* signal helps ordering is Zep's finding
   that a temporal knowledge graph beat pure embeddings on long-conversational
   memory.
8. **Rank delta** between the two streams.
9. **Tag match** between query and row.
10. **Bigram/phrase overlap** — would require restoring FTS positional data
    (`detail=column`), which `detail=none` deliberately removed. Costs index
    size, so it is a real trade rather than a free add.

## 5. Protocol — the part that decides whether any of this is honest

The only labelled data we have is the 500 LongMemEval questions. Fitting on them
and then reporting LongMemEval numbers produces fiction. Non-negotiable:

1. **Split by query, before anything else.** 5-fold cross-validation where the
   *query* is the fold unit, never the row. Feature selection happens inside the
   fold. Report mean ± std across folds.
2. **Pre-register the gate before fitting**, in this document, so the gate cannot
   be moved after seeing the result.
3. **Report paired per-question deltas** — how many questions flip miss→hit and
   hit→miss — not only the aggregate.
4. **No category may regress** even if the aggregate improves.
5. **A null result is a real result.** If nothing beats tuned RRF on held-out
   folds, that is the finding: the deficit is semantic, this architecture is at
   its ceiling, and we stop.

**Pre-registered gate.** Must beat RRF on the mean held-out fold, must not
regress any of the six question categories, and must not regress R@20. Anything
less is reverted and recorded.

## 6. Negative evidence, sought deliberately

- **Pooling bias.** A reranker trained on BM25-pooled labels largely learns to
  reproduce BM25, because the pool is BM25-dominated. Our pool is exactly that.
  Expect small gains and treat larger ones as a bug until explained.
- **Short-document saturation.** Documented in the LTR literature: gains
  saturate on short documents. Our candidates are memory chunks — short.
- **Scale mismatch.** TREC Deep Learning reports BM25 → neural re-ranker moving
  MRR@10 from 0.230 to 0.395. Encouraging, and the strongest public evidence
  that a reranker is the right lever — but on ~43,000 queries, not 500. It
  should inform the direction, not the expectation.

## 7. Recommendation

Run **Tier 0 first**: a pairwise logistic ranker over features 1–5, 7 and 8,
fitted offline, exported as a dozen constants, cross-validated by query. Start
with **feature 3 alone** — restore BM25 magnitude as a tie-breaker above the
RRF score — because it is a one-line change with a specific mechanistic
hypothesis behind it and it needs no model at all.

If Tier 0 cannot beat RRF on held-out folds, the honest conclusion is that the
remaining 16.2pp is semantic, and that means this architecture is finished. That
is a genuinely useful answer: it would stop us spending effort here and settle
the vectors question with evidence instead of argument.
