# Plan: clean out-of-sample R@5 above 95.2%

Written 2026-09-28 after a source-level audit of both competitors
(`docs/EVALUATION_HYGIENE.md` §1 makes the three-set discipline binding; §2.2
records the one violation already found). Supersedes nothing.
`docs/PERFORMANCE_PLAN.md` remains the phase list; this is the competitive target
and the evidence for it.

## 0. The bar is not what it looked like

### 0.1 What agentmemory's 95.2% actually is

From `_audit/agentmemory/src/state/hybrid-search.ts:198-231` and
`benchmark/longmemeval-bench.ts:189-198`:

- **BM25** (k1=1.2, b=0.75, Lucene IDF at `search-index.ts:120`) **plus two
  lexical extras we do not have**: synonym expansion at weight 0.7
  (`search-index.ts:105-110`, table at `synonyms.ts:3-40`) and a prefix-match
  pass at `idf × 0.5` (`search-index.ts:137-160`).
- **Plus a 384-d dense stream** — `Xenova/all-MiniLM-L6-v2`, local ONNX, q8
  (`providers/embedding/local.ts:10,40-44`). Stored in a plain in-process `Map`
  and searched by **exact brute-force cosine, no ANN index**
  (`vector-index.ts:92-120`).
- Fused by **weighted RRF**, `RRF_K = 60` (`hybrid-search.ts:20`), weights
  `bm25 0.4 / vector 0.6`, **plus a 5% cross-stream agreement bonus**
  (`AGREEMENT_BONUS = 0.05`, applied as `1 + 0.05 × (matchedStreams − 1)`).
- **Graph weight 0.0 and the cross-encoder disabled** in the benchmark call
  (`longmemeval-bench.ts:196-197`).

**The weights were never swept.** They are constructor defaults
(`hybrid-search.ts:30-32`), overridable only by env, and
`benchmark/LONGMEMEVAL.md` documents no tuning procedure, no ablation of the
agreement bonus, and no negative results. Their `real-embeddings-eval.ts:375,383,391`
enumerates three configs on a **15-question in-house corpus**, not LongMemEval.

> **95.2% is a single unvalidated point estimate from one hand-set
> configuration.** That is not a ceiling. It is a lower bound on what weighted
> RRF plus one small local embedding model achieves, produced by a system that
> never tried to be better on this benchmark.

### 0.2 What hindsight's 92.0% is — and is not comparable to ours

`_audit/amb/results-manifest.json`, the run record for the figure:

```
"accuracy": 0.9201, "total_queries": 1540, "correct": 1417,
"mean_precision": null, "mean_recall": null,
"avg_context_tokens": 36235.4, "avg_retrieve_time_ms": 964.1
```

`mean_recall` is **null**. The 92.0% is
P(an answer LLM wrote a correct answer | the context it was handed), graded by a
**third** LLM (`_audit/amb/src/memory_bench/judge.py:5-33`), over
`budget: "high"` = **1,000 candidate memories** (`memory/hindsight.py:740-747`
→ `config.py:1931`) at ~36k tokens of context. The only LLM-free mode in that
harness is `RetrievalMode`, wired solely to PrecisionMemBench, never to LoCoMo.
The harness is pinned to the literal string `main` (`AMB_REF`), unpinned.

`_audit/amb/external_results.json` shows the field's judge models are
incompatible and the choice moves the number: one system scores 0.9169 with a
GPT-4o-mini judge and 0.8812 with a different backbone — a 3.6pp swing from the
instrument alone.

> **Do not build a plan to close a gap to 92.0%. It is a different measurement,
> not a lower one.** An exhaustive grep for `recall@|ndcg|mrr@|hit@` across both
> repos returns only a code comment and an unrelated connection-pool metric:
> **no retrieval-recall number exists for Hindsight anywhere in the source we
> hold.**

### 0.3 Their fusion is the same as ours, and they argue against weighted RRF

`fusion.py:85` is the entire fusion: `rrf_scores[doc_id] += 1.0 / (k + rank)`,
`k = 60`, **no weights**. And `recall_boost.py:29-50` documents *removing* them
as a measured finding:

> with k=60 the whole 300-candidate window spans `1/61 → 1/360`, a factor of 5.9;
> any weight above that makes the sort **lexicographic** (boosted arm first, rank
> a tiebreak). Their `high` was `w = 7`; the boosted arm filled all 300 reranker
> slots and "no semantic-only candidate ever reached the cross-encoder
> (**measured: recall@20 0.97 → 0.40**)".

**This kills the move I was about to propose.** The obvious read of §0.1 is "copy
their 0.4/0.6 instead of our 1.0/0.25." Their own source says score-space weights
above the RRF spread degrade to lexicographic sorting and cost 57pp of recall@20.
The mitigating difference is that our 0.25 is a *tiebreaker* regime and their 7 was
a *primary* regime, so the finding may not transfer — but that is an argument for
**testing weights on the dev set, not for copying a number.** `docs/PERFORMANCE_PLAN.md`
P1 already schedules the `k` sweep; add the weight-ratio question to it there.

### 0.4 Neither benchmark number came from the write side

This is the finding that answers "where are we lacking the abilities."

- **agentmemory's eval adapter** ingests **one session = one `POST /remember` =
  one memory row = one BM25 entry + one embedding**
  (`eval/runner/adapters/agentmemory.ts:43-58`). No extraction, no
  consolidation, no observations, no forgetting. 95.2% came entirely from the
  read path.
- **hindsight's benchmark runner** has `wait_consolidation: bool = False` at
  **every** level — `benchmark_runner.py:921, 1018, 1194, 1269, 1349` — and **no
  call site in the repo passes `True`**. The worker poller that would run
  consolidation only starts `if wait_consolidation:` (`:1085`). In their scored
  runs, **observations did not exist in the index at evaluation time.**
- The only ranking-visible write artifact in either codebase is
  `proof_count`, worth **±5%** multiplicative (`reranking.py:302`,
  `alpha = 0.1`), and it requires their LLM consolidation worker to exist.

**Therefore: do not rebuild the consolidation ladder to chase a number.** We
deleted ours as dead code, and this audit says that was right — no competitor can
show a win from it on this benchmark. The write-side gap is real as a *product*
gap and irrelevant as a *benchmark* gap. `docs/CONSISTENCY.md` §14 records the
blunt answer.

## 1. Where our own loss actually is

Measured from `eval/results.json` at `0bfb530`, recovering each question's exact
first-gold rank from `MRR = 1/rank` (single gold session, so MRR inverts exactly):

| rank of first gold | count | share |
|---|---|---|
| **1 (hit)** | **419** | **83.8%** |
| 2 | 32 | 6.4% |
| 3 | 16 | 3.2% |
| 4–5 | 19 | 3.8% |
| 6–10 | 7 | 1.4% |
| 11–20 | 5 | 1.0% |
| >20 | 1 | 0.2% |

- **67 of the 81 R@1 misses have gold at rank 2–5.** A perfect reranker over the
  pool we already assemble takes **R@1 from 83.8% to 97.2%**.
- **13 questions (2.6%) have gold at rank ≥6**; **1 lacks gold entirely**.
- **36 of the 67 recoverable misses are single-gold questions** — exactly one
  right session among ~50. Multi-gold questions hedge by construction and hit
  R@1 more often (88.0% vs **76.1%**).
- Misses are **not** a temporal problem: the largest recoverable bucket is
  single-gold (36), then temporal-reasoning (21). Gold sessions that lose are
  1.12× longer than gold sessions that win, so length normalisation contributes
  but is not the story.

> **The entire competitive game is reordering a five-item pool.** We are not
> missing recall. We are missing a tiebreaker.

## 2. The plan

Every phase: **decide on mechanism → select on LoCoMo → measure LongMemEval
once → record in `docs/CONSISTENCY.md` §14.** No parameter is chosen by looking at
LongMemEval. Each phase ships default-off and inert until selected, exactly like
the four mechanisms already built.

### Phase A — rank-seeded scoring (0 bytes, ~1 day)

The one idea genuinely worth copying, and it is Hindsight's *no-cross-encoder*
path (`reranking.py:227-261`): when the reranker is absent they re-seed the base
score **from rank** so bounded multiplicative features modulate a meaningful base
instead of replacing it.

We already compute and discard most of this:

1. **BM25 magnitude.** FTS5's `bm25()` returns a real number; we use it in
   `ORDER BY` and keep only rank. A rank-7 row with a far better BM25 score than a
   marginal rank-1 row is exactly what rank-only RRF cannot see. Already built
   (`src/recall.rs` side-channel, default 0.0) — needs enabling and selection.
2. **Agreement prior.** `combined = rrf × (1 + β × (streams_matched − 1))`.
   `FusionWeights` already carries an `agreement` field at **0.0**. 12 bytes.
3. **Bounded multiplicative features** over signals we already hold, each capped
   (Hindsight uses ±5% for proof_count; recency/temporal ±20%).

Mechanism argument, stated before measuring: these are *signal restorations*, not
new parameters — the values exist and are discarded today.

Gate: LoCoMo must improve; no LongMemEval category may regress; R@20 must not
regress. Then LongMemEval once.

### Phase B — lexical paraphrase (0 bytes, ~2 days)

The gap vectors would close is paraphrase, and it is concentrated in
`single-session-preference` (our R@1 43.3%, agentmemory's BM25-only R@5 60.0% →
hybrid 83.3%). Two lexical mechanisms, both free:

1. **Prefix matching.** agentmemory credits every term with the query term as a
   prefix at `idf × 0.5` (`search-index.ts:137-160`). SQLite FTS5 gives this as
   `token*` inside `MATCH`. Nearly free; catches morphological variants
   (migrate/migration/migrating) that BM25's exact-token match discards.
2. **Synonym expansion** at a down-weighted term (`search-index.ts:105-110`).
   agentmemory's table is 63 hardcoded developer-domain lines. Ours must be
   justified on mechanism, not swept — and a synonym table selected against
   LoCoMo is still a fitted artefact, so it ships as a *user-supplied* config
   value, not a hardcoded table. memory-wire already has per-bank JSON config;
   this is a key in it.

Gate as Phase A.

### Phase C — the temporal work, re-aimed (0 bytes, ~1 day)

Defer, and **re-aim at single-gold rather than temporal**. `created_at` is now
real (`haystack_dates` → `created_at`, 0 per-question diffs), so the recency
stream is finally measurable — but the recoverable misses are not primarily
temporal, and the agentmemory "f2–f5 at 27% with token recency vs 14% without"
figure I previously cited **does not exist in the audit clone** (exhaustive
search of all `.md`/`.ts`/`.json`; every hit is a hex question id or a CHANGELOG
line about a heap gauge). `Consistency.md` §14.4 records the withdrawal. The
temporal classifier and recency stream stay default-off until Phase A and B have
had their selection, and then they are selected on their own merits — not on the
strength of a citation that does not exist.

### Phase D — vectors, funded only on evidence (~1 week, 90 MB)

Only if A and B both come back null on LoCoMo. The honest cost statement:

- `Xenova/all-MiniLM-L6-v2`, 384-d, q8 ONNX, **~80–90 MB** — not the 280 MB I
  estimated earlier; Hindsight's default cross-encoder is the same 6-layer
  MiniLM.
- **No ANN index is needed at our scale.** agentmemory ships exact brute-force
  cosine over a `Map` and gets 95.2%. With ~50 documents per bank a linear scan
  is microseconds.
- What it costs: a **model download on first use**, which breaks the offline,
  one-file property the product is built around — one file to copy, three shared
  libraries (`libgcc_s.so.1`, `libm.so.6`, `libc.so.6`), no daemon, no database
  server, nothing to install. It is not a *static* binary; `readelf -d` lists
  exactly those three `NEEDED` entries and nothing else
  (`docs/CONSISTENCY.md` §16.6). **The citation to
  `docs/PERFORMANCE_AUDIT.md` that earlier revisions of this line carried was
  wrong — that file never states the property**, so nothing is being broken that
  document promised. It is still a real product trade and the user's call, not
  mine.

Gate: must beat the best of A/B/C on LoCoMo by more than the run-to-run spread,
or it does not ship.

**Not a cross-encoder, at any point.** Hindsight's is optional by design (three
separate off-switches: per-request `enable_reranking`, per-bank
`DEFAULT_ENABLE_RERANKING`, provider-level `provider_name == "rrf"` passthrough),
it is **on by default but contributes nothing to any published number**, and
**they publish no measured recall for their own local model.** The only
reranker number anywhere in their repo (`recall@1 0.94 vs 0.87`) is for a
third-party *hosted* model they do not ship. Buying an unmeasured, config-gated
component to match an unmeasured delta is not a plan.

### Phase E — granularity, kept off the benchmark (product, not score)

Sub-dividing a long session into topical units multiplies independent chances
that one ranks top-5 — the one genuine write-side lever, and the reason both
systems' numbers are read-side. **But `recall_any@K` is defined over
session-as-document.** Changing the unit changes the corpus and the numbers stop
being comparable to agentmemory's, Hindsight's, or our own history. So it ships
as a product capability with its own harness, and never inside the LongMemEval
comparison.

## 3. What is explicitly not being done, and why

| Not doing | Because |
|---|---|
| Copying agentmemory's 0.4/0.6 weights | `recall_boost.py:41` documents a measured collapse (recall@20 0.97 → 0.40) when weights exceed the RRF spread. Tested on the dev set, not copied. |
| A cross-encoder reranker | Optional even to Hindsight, unmeasured by them, breaks the single-binary promise. |
| Rebuilding the consolidation ladder | Both competitors measured with it **off** (`wait_consolidation: False`; 1-session-1-row adapter). No benchmark win is available. |
| Proof-count-style popularity boost | ±5% in Hindsight, needs an LLM worker, and re-ranks by popularity. Cheap to add, near-worthless. |
| A recency term aimed at the temporal slice | The recoverable misses are single-gold, not temporal. The citation that motivated it does not exist. |
| Cross-bank retrieval | Hindsight's banks are as isolated as ours and its accuracy comes from ~1000 candidates *inside* one bank. We are not losing recall to isolation. |
| Targeting hindsight's 92.0% | A different instrument: LLM-judged answer accuracy, ~1000-deep pool, third-party judge, unpinned ref. |

## 4. The bar this plan has to clear

**A configuration selected on LoCoMo, measured once on LongMemEval-S, scoring
R@5 above 95.2% with no parameter chosen against those 500 questions.**

Current standing: 93.0% clean (equal weight), 97.2% fitted (not clean), ≈94.4%
best out-of-sample estimate. A clean 95.2%+ is a real gap of roughly 1.2pp from
the honest estimate, and the decomposition says it is reachable by reordering
alone — 67 questions sit at rank 2–5 and the machinery to move them is mostly
already built and inert.

If Phases A–C all come back null, the deficit is paraphrase that lexical features
cannot reach, Phase D is the only remaining move, and the honest choice is
vectors-with-a-download versus accepting ≈94–95%. That is a legitimate place to
stop, and it is a far better answer than six weeks of ranking experiments.
