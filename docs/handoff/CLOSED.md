# Closed levers — measured and rejected (do not re-propose without new evidence)

AGENTS §8 forbids re-litigating these. Each row names the measurement that closed it;
if you think new evidence reopens one, state what is new **before** you spend anything.
Detail lives in the cited documents; this is the index a cheaper agent can scan.

| lever | verdict | closed by |
|---|---|---|
| dense vectors **as a fusion-weight lever** (H1d) | measured 2026-10-15, gate FAILED: `recall_all@10` +1.8pp best (vec 0.75) bought at −9.9pp R@1; deep-miss rescue 4/99; below-gate at low weights. Re-opening needs a new mechanism, not a re-run (`CONSISTENCY.md` §41) | `eval/SELECTION_VECTOR_AXIS_ALL10.md` |
| dense vectors **as a default** | declined: no coverage failure to fix on LongMemEval (R@20 99.6%, pool 500/500); costs ~3× artifact size, 4–7× idle RSS, 19–30× write path. The arm exists behind `--features embed`, weight 0.0. Reopening is the **user's** call — now informed by the H1d measurement above | `docs/PERFORMANCE_PLAN.md` "Explicitly not doing"; `docs/EXCEED_PLAN.md` Phase D |
| cross-encoder reranker | disqualified: breaks the single-binary position, unmeasured even by Hindsight who ship it optional | `docs/RERANKING_PLAN.md` §3.2 |
| graph stream; consolidation / extraction ladder | both competitors measure with it off; no benchmark win available | `docs/EXCEED_PLAN.md` §3 |
| Porter stemming (E2) | flat on top of E1; temporal-reasoning −2.26 pp; removed | `docs/NEXT_ITERATION.md` E2 |
| IDF-weighted overlap (E3) | the hypothesis it would fix the recall curve was disproved; removed | `docs/NEXT_ITERATION.md` E3 |
| distinct-term-coverage stream (E4) | algebraically identical to raising the overlap weight; removed | `docs/NEXT_ITERATION.md` E4 |
| `cache_size` / `mmap` tuning | no gain, +15–17% RSS; refused | `docs/PERFORMANCE_PLAN.md` |
| the token-overlap stream's *fitted weight* as evidence | `overlap 0.25` was chosen on LongMemEval (46 configs): fitted, not earned. Not "closed" as a value, but closed as a claim | `docs/EVALUATION_HYGIENE.md` §2.1 |
| a **hardcoded** synonym table | maintenance + licensing; the user-supplied `recallSynonyms` config is the supported form | `docs/EXCEED_PLAN.md` §3 |
| score thresholds on auto-injection | measured; the levers are composition, caps, labels, tags — not a cut-off | `docs/CONSISTENCY.md` §32; `docs/OPEN_HOOK_RECALL_RELEVANCE.md` |
| `agreement` prior (`FusionWeights::agreement`) | moved **0 of 1,531** LoCoMo questions at 0.50 (and 0 of 500 on LongMemEval) | `eval/SELECTION.md` reading 1 |
| `k` in RRF (`k = 60`, inherited from Hindsight) | flat: R@5 spans 86.0–86.1% across k ∈ {5…120} at the shipped overlap | `eval/SELECTION.md` reading 3 |
| `bm25_magnitude` as a big win | does not collapse (the competitor warning does not transfer) but R@5 only 86.0→86.3% at 4.00 — small. *`recall_all@10` was not measured then*; H1c reopens only that metric — **re-opened 2026-10-14 and closed again**: `recall_all@10` 83.0→82.8% (4 up / 7 down), multi-hop R@1 −3.4pp (n 1,531) | `eval/SELECTION.md` reading 2; H1c: `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| `keyword_scope: ExactAndPrefix` (H1a) | best single-lever R@1 gain on the dev set (R@1 +3.0pp, MRR +2.2pp, NDCG +1.7pp) but `recall_all@10` +0.5pp < the +1pp gate and single-hop R@5 −0.4pp — real, deterministic. Gate failed as pre-registered; no LongMemEval consultation | `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| `overlap_scope: BestSentence` (H1b) | `recall_all@10` −0.3pp, R@1 −3.4pp overall, multi-hop R@1 −11.3pp (n 1,531). Concentrating overlap on one dense segment loses more than it gains | `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| recency 1.0 `TemporalQueriesOnly` on the dated corpus (H3) | falsified: temporal `recall_all@10` −7.8pp, temporal R@1 −33.4pp; control exact (1,025/1,025 detector-negative lists bit-identical). LoCoMo temporal questions ask *when an event happened*, not *what is newest*. Recency stays 0.0; the dated corpus (H0) remains for date-*aware* mechanisms that are not a recency prior | `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| intermediate write granularity, windows of W=2 turns (H2) | R@1 −1.1pp, `recall_all@10` −0.4pp vs sessions, and R@pool 100.0→98.7% — windowing pushes banks past the candidate-pool ceiling, so some gold is unreachable at any K. With `Turns` (§35) also worse, granularity is closed: sessions stay | `eval/LOCOMO_WINDOWS.md`, `docs/CONSISTENCY.md` §37 |
| **turn-level writes, session scored at best turn** (this cycle) | worse on every aggregate on LoCoMo at the shipped config: `recall_all@10` 83.0→80.7%, R@1 60.4→54.6%, R@5 86.0→83.0%, NDCG@10 73.6→69.7% (n 1,531); per-query `recall_all@10` 42 up / 76 down. One run, not tuned | `eval/LOCOMO_TURNS.md`, `docs/CONSISTENCY.md` §35 |
| a recency stream *measured on the old LoCoMo ingest* | invalid, not rejected: every LoCoMo `created_at` was the harness clock (0.006 s spread), so R@1 24.8% at `recency 1.0` is an ingest-order artifact. Re-test only after H0 | `eval/SELECTION.md` "recency caveat" |
| cross-bank retrieval | banks are isolated by design; Hindsight's accuracy comes from ~1000 candidates *inside* one bank | `docs/EXCEED_PLAN.md` §3 |
| targeting Hindsight's 92.0% | a different instrument (LLM-judged accuracy, third-party judge, ~1000-deep pool) | `docs/EXCEED_PLAN.md` §0.2 |

## Things that look like findings but are instrument artifacts

Do not cite these as memory-wire properties:

- **17.0% LongMemEval `answer_quality`**: a small-reader floor, undated prompt (STATE §2).
  *The dated re-measure (2026-10-14, harness fixed per TODO #16) reads 18.6% conditioned /
  6.0% closed-book — a different instrument (dates in the prompt), recorded beside, never
  compared as better/worse (`CONSISTENCY.md` §39). Neither number is comparable to any
  published LongMemEval figure: judge, prompt and truncation are all this harness's own.*
- **84.1% AMB LoCoMo**: free `small-stack` reader and judge; not comparable to any
  published LoCoMo score; the date handling predates the `created_at` round-trip.
- **Latency / RSS / throughput in the baseline**: taken at load 11–28 on 24 cores.
- **Relevance-battery junk-injected 0/9**: the live bank `omp` grew; the code did not change.
- **PrecisionMemBench 14/77**: 77 cases, measure-only — *superseded 2026-10-14 by the H6 capability change: with scope-tag filters honoured the adapter measures 19/77, precision 0.24 (`eval/amb/PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz`). The pre-change 14/77 stays here as the historical no-filter reading.*
