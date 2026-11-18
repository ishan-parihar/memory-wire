# CONTEXT PACK — context-pack-2026-11-18-round2

## READING CONTRACT

One message, one file: every input this round needs is below. Do not open repository files to re-read
what is already quoted here — that spends your tool calls and adds nothing.

- Cite by anchor: `# SECTION <NAME>` or `# FILE: <path> (lines a,b)`. A finding that rests on a figure
  must name its anchor and keep the figure's evidence class (`[LIVE]`, `[CERT]`, `[IN-SAMPLE]`,
  `[PRIOR]`, `[UNCONFIRMED]`). A figure with no anchor is an assumption: label it one.
- Lens families, where the pack carries them, are instruments, not vocabulary. Naming a lens earns
  nothing; the move the lens forces is the finding.
- If a number you need is absent, say which one and continue under a stated assumption. The report is
  judged on structure, arithmetic and falsifiers, not on filling gaps with plausible values.


# SECTION A-STATE

# FILE: README.md (lines 323,363)

## The wider field, checked 2026-10-15

The rows above are the only head-to-head retrieval numbers in this space: the
same 500 questions under the same harness method. Everything below is either
measured-by-us through a harness whose commit is pinned (and marked), or the
competitor's self-report (and marked that way instead). The rule is `AGENTS.md`
§12: a number that cannot be traced to a committed artifact is not a number,
it is marketing.

| | **memory-wire v0.8.0** | **agentmemory** | **Hindsight** (vectorize-io) | **MemOS** (MemTensor) | **Honcho** (plastic-labs) |
|---|---|---|---|---|---|
| install | one static binary; a musl build with `DT_NEEDED=0` and no glibc floor | Node runtime + detached engine + worker | Python image + Postgres/pg0 + optional ONNX reranker | Python 3.11, Docker/Helm multi-service stack | Python/FastAPI server or managed service |
| idle footprint | **~10.7 MiB RSS · 8.8 MiB binary · 3 libs** | ~6 MB heap @1k → **316 MB @50k** | **0.8–1.0 GB** | container stack (not measured by us) | server process (not measured by us) |
| LongMemEval-S retrieval, same protocol (measured by us) | R@5 **93.0% clean unfitted** · 97.2% fitted (marked) · ≈94.4% out-of-sample est · R@1 **83.8%** | hybrid 95.2% · BM25-only 86.2% (their harness, tuning undisclosed) | **no comparable number exists** — their published figures are LLM-judged answers | — | — |
| LoCoMo judged accuracy (AMB `rag`) | **84.1%** (n 1540) — free-tier small reader, 15.2k ctx, **26.5 ms** retrieve | — | 92.0% — Gemini-class answerer, 36k ctx, 964 ms retrieve. **Different answerer ⇒ not comparable** to ours | — | self-reported 89.9% |
| PrecisionMemBench (LLM-free, AMB retrieval) | **19/77, precision 0.24**, recall 0.95 — no identity machinery | — | **66/77, precision 0.85**, recall 1.00 — *with* scope filters + identity-extraction LLM at ingest | — | — |
| self-update | `memory-wire update` — sha256-verified, rollback kept, live-verified on a second box | manual | manual | container redeploy | server redeploy |
| license | MIT | MIT | MIT | Apache-2.0 | **AGPL-3.0** — embedding cost for proprietary stacks |
| eval evidence discipline | 4 test-set consultations, all measurement-only; every number traceable to a committed artifact | none disclosed | — | — | — |

MemOS and Honcho publish judged numbers (MemOS a "35.24% token savings" claim;
Honcho [90.4% LongMemEval-S, 89.9% LoCoMo](https://plasticlabs.ai/blog/research/Benchmarking-Honcho))
that no independent harness has reproduced; treat them as claims until run
under a common one. The audit plan that would do exactly that — plus the
mechanism-level screen of all four architectures — is
[`docs/COMPETITOR_GAPS_PLAN.md`](docs/COMPETITOR_GAPS_PLAN.md); the benchmark
inventory and every comparability rule lives in
[`docs/BENCHMARK_LANDSCAPE.md`](docs/BENCHMARK_LANDSCAPE.md).

**Where memory-wire wins, measured:** the only clean head-to-head in the field
(+6.8pp R@5 over agentmemory's BM25-only arm, 93.0 vs 86.2); footprint 30–100×
lighter idle than either parent; ~283 µs puts and 26.5 ms end-to-end retrieve
against Hindsight's 964 ms on the same mode; the only verified self-updating
fleet story; MIT with no AGPL embedding cost.

**Where memory-wire trails, named:** agentmemory's hybrid by ≈0.8pp honest
(≈94.4 vs 95.2 — the dense weight, measured to cost −9.9pp R@1 for +1.8pp
coverage); Hindsight's PrecisionMemBench precision 0.24 vs 0.85, because they
run an identity-extraction LLM at ingest and we deliberately do not — the
gap is priced, recorded, and its lever is on the TODO register, not hidden.


# FILE: docs/ARCHITECTURE.md (lines 1,85)

# Architecture

**What this document is.** The shape of the whole system on one page, written
for someone who will not read the code. If you want the full API surface,
`docs/` holds it. If you want to know what this thing *is*, this is the page.

---

## The one sentence

memory-wire gives an AI agent a memory that survives the conversation, keeps
each project's memories separate from every other project's, and does it in a
binary small enough to ship as a download rather than an install.

---

## The parts

A Rust library with five modules behind one service, plus a binary that
exposes the same service three ways. They are separate because they fail
differently and are measured separately.

| Module | What it is | Why it is its own module |
|---|---|---|
| `api` | `MemoryService` — retain, recall, reflect. The only thing a caller touches. | The interface is the contract; changing it is a breaking change, so it sits apart from every implementation. |
| `store` | SQLite persistence, and an in-memory variant for tests and zero-setup use. | Storage is swappable by design, and the in-memory store exists so a test never touches a file on disk. |
| `recall` | Bank-isolated retrieval: FTS5 BM25 plus reciprocal-rank fusion. | Ranking is the part that is benchmarked, so it is the part that must be measured in isolation. |
| `capture` | What gets kept automatically while a coding agent works. | Capture and recall meet at `api` and nowhere else; coupling them would make retention policy a side effect of search. |
| `embed` | Optional dense-vector arm — 23 MB of int8 weights, behind a cargo feature and absent from a default build. | A feature that doubles the binary has to be optional at compile time, not just at runtime. |
| `memory` | Bank model, isolation rules, and PII redaction on write. | The privacy guarantee is a boundary, and a boundary belongs where it can be audited in one place. |

---

## How a memory gets kept

```
 an agent working in one terminal writes something worth keeping
        |
 capture: what is worth keeping is decided here, not by the caller
        |
 memory: the entry is redacted on write and assigned to a bank
        |
 store: one row, in this bank, in one SQLite file
```

---

## How a memory comes back

```
 a query arrives, with the bank it may look in
        |
 recall: FTS5 BM25 and RRF fusion rank within that bank only
        |
 the budget is applied before the bytes are returned
        |
 the caller gets the ranked set, each entry stamped with its own date
```

The budget is applied at the end and not the beginning, which is what keeps a
recall from returning something the caller did not ask for.

---

## The boundaries

- **One service, three doors.** HTTP, an MCP server over stdio, and the CLI all
  reach the same `MemoryService`. There is no second implementation of recall.
- **Banks never mix.** A recall names its bank. There is no cross-bank fallback,
  because a memory from one project appearing in another is the failure this
  exists to prevent.
- **Redaction happens on write, not on read.** A secret is not stored, so it
  cannot be recalled.
- **The daemon is a deployment, not a dependency.** No background process is
  required; `memory-wire daemon start` runs the same binary in the background
  when you want it to outlive a terminal.

---

## What this respects

The design integrates two upstream approaches — biomimetic retain/recall/reflect
and coding-agent auto-capture — and the credit is recorded: `docs/AUDIT.md`
holds the teardowns of both, and the benchmark claims live in `eval/` with the
datasets they were run on.

# SECTION B-LAW

# FILE: AGENTS.md (lines 26,63)

## 1. What this project is

`memory-wire` is a single-binary agent memory service. A bank-isolated SQLite
store with FTS5 full-text search, exposed over HTTP and MCP, with hooks that
capture conversation prose into it.

It exists to be **the cheap option**. The competitive position is deliberate and
should not be eroded casually:

| | memory-wire | Hindsight | agentmemory |
|---|---|---|---|
| install | one static binary, no daemon required | Python image + Postgres/pg0, optional ONNX reranker | Node runtime, detached engine + worker |
| idle footprint | ~10.7 MiB RSS | 0.8–1.0 GB | ~6 MB heap at 1k, 316 MB at 50k |
| default build deps | 3 shared libraries | — | — |

Three properties are load-bearing and each has been paid for:

- **The default build must not gain a dependency or grow materially.** It is
  10.25 MiB at `v0.7.0` with `DT_NEEDED` = 3 (`eval/baseline/2026-10-13-v0.7.0/BENCH_FOOTPRINT.md`;
  the 9.7 MiB this line used to state was an earlier build). Verify `ldd` after touching anything
  that links.
- **Retrieval reports are the record.** The competitor whose retrieval number we
  can actually compare against is agentmemory, on LongMemEval-S, and we lead it
  on the honest (unfitted) number. Do not publish a number you cannot trace to
  a committed artifact.
- **Retrieval is a reordering problem, not a coverage problem.** The candidate
  pool contains the gold memory in 500 of 500 LongMemEval questions. Work on
  ordering; do not spend effort on recall/coverage mechanisms. (Pool membership is
  not the same as the *top-k a reader is served*: on LoCoMo, questions with several
  gold sessions miss at k = 10 — `recall_all@10` 83.0% vs `recall_any@10` 94.2% —
  and that is still an ordering problem inside the pool. See `docs/handoff/RND_PLAN.md`.)

The parent projects are `rohitg00/agentmemory` and the Hindsight repositories,
audited read-only under `_audit/`. This is a deliberate Tier-C extraction: the
~3k LOC retrieval core is ported, the ~40k LOC surround is intentionally out.
**Full parent parity is not the goal** — pursuing it means rebuilding the
parents.


# FILE: AGENTS.md (lines 203,281)

## 6. The evaluation set is not a training set

This is the rule that matters most here, and the one most easily broken by
accident.

`eval/RESULTS.md` holds LongMemEval-S results over 500 questions. Those 500
questions are an **instrument for measuring**, never an objective to optimise
against.

**Never do any of the following and then describe the result as an improvement:**

- sweep a parameter, weight, threshold, or feature configuration against the
  LongMemEval numbers and keep the winner
- add, remove, or reorder a retrieval signal because it moved R@5 / R@10 /
  NDCG@10 on LongMemEval
- re-run a change, observe a delta, and iterate until the delta is positive
- quote a LongMemEval number as evidence that a component is "better" when the
  component was chosen by looking at that number

**A change selected by looking at the evaluation set is fitted, not earned.**
When that has happened — and it has — say so in the commit message and in
`docs/CONSISTENCY.md`, mark the value provisional, and treat re-validation on
independent data as outstanding work.

| Allowed | Not allowed |
|---|---|
| Measuring on LongMemEval and reporting the number | Choosing a value by comparing LongMemEval numbers |
| Fixing a bug that LongMemEval exposed | Tuning until a metric stops complaining |
| Deciding a change on a mechanistic argument, then *observing* the metric | Deciding a change on the metric |
| Selecting against a **development** set that is not LongMemEval | Using the test partition as a dev set |
| Reporting that a change did not help | Reverting a change because a benchmark "regressed" while keeping an equal-or-worse alternative that scored better |

### The three-set discipline

- **dev** — a labelled set that is *not* LongMemEval. All selection happens here.
  `eval/data/locomo/` is the dev corpus (independent source, ~1530 usable
  queries, gold document ids). See `docs/EVALUATION_HYGIENE.md`.
- **test** — LongMemEval-S. Consulted for **measurement and release acceptance
  only**. Never for selection.
- **synthetic** — `examples/bench_recall_curve.rs` and `tests/scale.rs` generate
  their own corpora with fixed seeds. Useful for scaling and regression
  assertions, but we author the distractors, so it is weak evidence about
  ranking quality. Do not tune against it either.
- **measure-only** — every other external set (PrecisionMemBench, BEAM,
  agent-memory-bench, …). Same rule as test: report, never select. Any harness
  that runs LongMemEval (OAMB, MemBase, memorybench) *is* a test consultation.
  Roles are assigned in `docs/BENCHMARK_LANDSCAPE.md` before a set's first number.

**A constant changed in response to a bug is derived from mechanism, then
measured exactly once.** Never iterate until a metric improves — that yields a
better number and a worthless one. `DEFAULT_RECALL_BUDGET` is the worked example:
derived as 3× the measured median LongMemEval session, measured once, and the
result reported whether it moved or not.

### Count the consultations

Every change that is *selected* on the test set burns it. `docs/CONSISTENCY.md`
records the count. If a component's parameters were fitted on LongMemEval, it
carries a `provisional:` marker in the code and in the docs until an independent
set confirms it.

## 7. Artifacts and measurement

- A number in prose must exist in a committed artifact under `eval/`, or the
  prose must say where it came from and that it is not in a committed artifact.
- When a doc and an artifact disagree, **the artifact wins** — except for a
  historical record that deliberately describes a specific dated build, which
  stays and keeps its date.
- Never hand-edit a generated artifact under `eval/`. Fix the generator.
- `docs/CONSISTENCY.md` is the append-only record of what was measured, on what,
  at what load, and what was rejected. Add to it. Do not rewrite it.
- **Latency and RSS are only meaningful on a quiet box.** Check `loadavg` before
  and after and record it. At load > 20 on 24 cores a number is a range at best
  and must be labelled as one. Several published figures in this repo's history
  were load artifacts and are documented as such.
- Sweeps that are infeasible to repeat (≥ 30 min) run once and are reported
  without a CI, not three times.
- Do not report a percentage without the denominator and the n.


# FILE: AGENTS.md (lines 313,327)

## 10. Output conventions for agents

The CLI and the HTTP surface are read by other agents, so:

- A bare invocation shows **live state**, not a route map. Reference material
  belongs behind `--help`.
- A command's output is its answer. A list endpoint states the total; an empty
  result says so definitively rather than returning a bare `[]` that is
  indistinguishable from a failure.
- Content is truncated with the original length and the fetch command to get the
  whole thing.
- An unknown flag exits non-zero and **lists the valid ones in the same
  response**, so a correction costs one turn instead of two.
- MCP capabilities are advertised only when the server actually implements them.


# SECTION C-EVIDENCE

# FILE: docs/handoff/CLOSED.md (lines 1,45)

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

# FILE: docs/handoff/STATE.md (lines 169,227)

## 2. Current numbers (every one traces to a committed artifact)

Baseline directory: **`eval/baseline/2026-10-13-v0.7.0/`** (see its `README.md` and
`PROVENANCE.md`). Load was 11–28 on 24 cores throughout: retrieval metrics are
load-independent, **every latency/throughput/RSS figure is range-quality.**

### Retrieval (free, deterministic)

| | R@1 | R@5 | R@10 | NDCG@10 | n | artifact |
|---|---|---|---|---|---|---|
| LongMemEval-S, all (**test**) | 83.8% | 97.2% | 98.6% | 88.2% | 500 | `LONGMEMEVAL.md` |
| LongMemEval-S, 470 (abstention excluded) | 84.0% | 97.2% | 98.5% | 88.5% | 470 | same |
| LoCoMo (**dev**), `overlap 0.25` shipped | 60.4% | 86.0% | 94.2% | 73.6% | 1,531 | `LOCOMO.md`, repo `eval/LOCOMO.md` |
| LoCoMo `recall_all@10` (new), shipped | — | — | **83.0%** | — | 1,531 | `eval/LOCOMO.md` |
| LoCoMo `recall_any@10` for contrast | | | 94.2% | | 1,531 | same |
| BEAM-100k (**dev**, pool-scale re-test §47): 1.00 / 0.25 / 0.00 | 42.5 / 39.9 / 36.4 | 67.1 / 63.6 / 62.3 | 77.6 / 76.4 / 74.8 | 42.3 / 41.2 / 39.3 | 313 | `eval/BEAM.md` |
| Frozen relevance battery (§48): auto gold 16/17, junk-injected 0/9, search-reachable 17/17; H8 gate green (43% less text) | | | | | 21 | `eval/relevance/BATTERY-2026-10-16-frozen.txt` |
| PrecisionMemBench (measure-only) | total 14/77, active 3/43, precision 0.17, recall 0.95 | | | | 77 | `precisionmembench.json.gz` |

Honesty note: `overlap 0.25` was chosen on LongMemEval (46 configurations) and is
**fitted** (`docs/EVALUATION_HYGIENE.md` §2.1). The honest unfitted LongMemEval R@5 is
93.0% (equal weight); 97.2% is the fitted number. Do not quote 97.2% as evidence that
anything is "better".

New 2026-10-14 (**H1.0**, dev set, no LLM — `eval/LOCOMO_RANKS.md`, `CONSISTENCY.md` §36):
of the 261 `recall_all@10` misses, the worst-ranked gold session sits at 11–15 / 16–20 /
21+ / absent = 89 / 73 / 99 / **0**. Every miss is an ordering position inside a complete
list (fused list = whole bank on 261/261); 62.1% within 11–20 ⇒ reordering levers (H1a–c)
are worth building, 99 misses at 21+ bound what they can recover.

### LLM-judged (reader and judge both aroute `small-stack`, temperature 0)

| | accuracy | n | artifact |
|---|---|---|---|
| `answer_quality`, LongMemEval-S (test), conditioned | 17.0% (85) | 500 | `ANSWER_QUALITY_LME.md`, `answer_quality_lme.json` |
| same, closed-book control | 6.0% (30) | 500 | same |
| AMB LoCoMo `rag` (measure-only) | **84.1%** (1,295) | 1,540 | `amb_locomo_rag.json.gz` |
| — open-domain / temporal / multi-hop / single-hop | 92.7% / 81.9% / 68.8% / 66.0% | 841 / 321 / 96 / 282 | same |
| — by gold coverage in the served top 10: all / some / none | 91.2% / 53.0% / 36.5% | 1,289 / 166 / 85 | derived from the same file |

How to read them (the full reasoning is `docs/CONSISTENCY.md` §34):

- The LongMemEval 17.0% is a **small-reader floor**, not memory quality: a gold session
  was served in 454/500 and ranked first in 419 of those, yet 368 of the 374 wrong
  answers were `I don't know`. Gold alone was answered 6/10, gold + 2 other sessions
  1/10 (10 items, diagnostic).
- On LoCoMo the reader does *not* refuse (12 of 245 wrong answers). Accuracy tracks
  whether **all** gold sessions were served — this is why `recall_all@10` exists.
- **Never read 84.1% against 17.0%.** Different corpus and bank size (LoCoMo banks hold
  19–32 sessions). Never place a retrieval number beside a judged one, or beside a
  number from a different reader/judge (`docs/BENCHMARK_LANDSCAPE.md` §6).

### Footprint and speed (range-quality — load 11–28)

Binary 10,745,080 B (10.25 MiB), `DT_NEEDED` 3; idle RSS 9.88 MB; cold start 13–16 ms;
retain 2,018 ops/s; recall p50 7.2 ms at 10k memories. **`bench_concurrency` failed its
2-client gate (1.30× vs 1.50×) under load but absolute throughput rose** — unreadable;
re-measure on a quiet box (`TODO.md` #14).


# FILE: eval/baseline/README.md (lines 21,89)

## What the battery contains, and what it deliberately leaves out

| phase | step | corpus / role | artifact |
|---|---|---|---|
| retrieval | `longmemeval` | LongMemEval-S, 500 q — **test** | `LONGMEMEVAL.md`, `longmemeval.json` |
| retrieval | `locomo` | LoCoMo, 1,531 q — **dev** | `LOCOMO.md`, `locomo.json` |
| retrieval | `coding_life`, `scale_sweep`, `recall_curve` | synthetic | `CODING_LIFE.md`, `SCALE_SWEEP.md`, `BENCH_RECALL_CURVE.md` |
| retrieval | `battery` | live bank `omp`, 10 labelled cases — **dev** (auto-injection) | `BATTERY.txt` |
| retrieval | `precisionmembench` | PMB via AMB, 77 cases — measure-only | `precisionmembench.json.gz` |
| perf | footprint, write, concurrency, coldstart, soak | synthetic | `BENCH_*.md`, `SOAK.md` |
| llm | `answer_quality` | LongMemEval-S, judged — **test** | `ANSWER_QUALITY_LME.md`, `answer_quality_lme.json` |
| llm | `amb_locomo_rag` | LoCoMo via AMB `rag`, judged — measure-only | `amb_locomo_rag.json.gz` |
| llm | (every call) | which model served it, retries, latency | `llm-calls.jsonl` |

**Left out on purpose, with the measurement that excludes each** (2026-10-13,
`docs/BENCHMARK_LANDSCAPE.md` §3). AMB `rag` hands the answerer the provider's
top 10 documents, and `eval/amb/memory_wire.py` stores each AMB document whole:

| corpus | docs/bank | top-10 tokens (median) | share of bank | why not in the baseline |
|---|---|---|---|---|
| LoCoMo | 29 | 15,870 | 34% | **included** — a real selection |
| BEAM 100k | 9 | 145,950 | 100% | top-10 *is* the whole history: a long-context test, not a memory test |
| PersonaMem 32k | 5 | 36,922 | 100% | same |
| BEAM 500k / 1m | 27 / 50 | ~248,000 | 37% / 20% | ~250k tokens of context per question on a free small model |
| PersonaMem 128k | 20 | 108,738 | 50% | ~109k tokens per question |
| LifeBench | 365 | 67,543 | 3% | meaningful selection, but ~135M input tokens for 2,003 q on a free tier |

They become measurable once the provider writes at the granularity the
product's own write path uses (the hooks retain turn pairs, not 25k-token
sessions) — `TODO.md`. Until then a number from them would describe the reader's
context window, not memory-wire.

## The LLM instrument, and why its numbers need their caveats

The `llm` phase routes through aroute's `small-stack` combo (free models:
`opencode-zen/space-bunny-free`, `poolside/laguna-s-2.1:free`,
`dots-studio/dots-3-note-preview:free` across cline / kilocode / openrouter, in
priority order) via `eval/llm_shim.py`. Measured while building it (2026-10-13,
aroute 0.1.12):

- **A combo is a fallback chain**, so the answering model is not fixed. aroute
  only routes combo names (a `provider/model` id is refused with
  `model_not_found`), so the model cannot be pinned; `llm-calls.jsonl` records the
  model that served each call, and a comparison between two baselines is only
  clean where the served-model mix matches.
- **aroute returned HTTP 200 with an `{"error": …502…}` body** (logged
  `outcome=ok`, no fallback) and **forwards every cline reply wrapped as
  `{"success": …, "data": {...}}`**. The shim retries the first and unwraps the
  second; both are aroute bugs, handed off in `docs/AROUTE_BUGS.md` (with two more:
  cline token headers read 0, and `opencode-zen` 403s on every call with no
  cooldown, so `space-bunny-free` served none of this baseline).
- **The free models ignore `response_format: json_schema`** and AMB's client sends
  no temperature; the shim states the schema in a system message and defaults
  `temperature` to 0.
- **A free tier has bad hours.** On 2026-10-13, 21:00–22:10, every provider hit
  transport failures. One shim call retried for about 20 minutes, AMB's OpenAI client
  timed out at 600 s, and AMB does not retry a timeout, so the whole run died after 384
  of 1,540 queries. The shim now stops at `--deadline 480` and returns a 502, which AMB
  does retry. `amb_rag` resumes from AMB's per-conversation saves
  (`target/amb-out/$TAG`, seeded from the committed `.json.gz`), so a dead step
  costs a restart, not the hours already spent.
- **The reader, not retrieval, dominates the judged number.** On the first 6
  LongMemEval questions (seed 42) memory-wire ranked a gold session **first in all
  6** and it was in the context every time; the small reader still answered
  `I don't know` on 4 of the 5 answerable ones. Read `answer_quality` beside
  `gold_in_context` / `first_gold_rank` in its JSON, never alone — and never
  beside a published accuracy produced with a frontier reader or the official
  judge.


# SECTION D-ASK

# FILE: TODO.md (lines 56,76)

### The hierarchical-grounded-memory cycle (2026-11-18)

Rows 21–28 come from `docs/audit/2026-11-18-surface-audit.md` and are
sequenced in `docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md`.
**Read the audit before touching any of them**: three external proposals
describing this work were wrong about the tree (54 MCP tools that are 4, WAL
and auth that shipped in v0.7.0, and a metric no hierarchy can move), and two
of their headline ideas are already closed by measurement. No row here changes
`FusionWeights::SHIPPED`.

| # | status | item | why | where | acceptance |
|---|---|---|---|---|---|
| 21 | open | **W0.1 — make `golden_install_shapes` hermetic.** It calls `run()`, which uses the real `binary_on_path`, so it fails on any host without `copilot` installed: `CopilotCli: unexpected outcome Skipped("not detected on this machine")`. The seam exists and 9 sibling tests use it (`apply(..., on_path, ...)`, `no_path` at `src/connect.rs:1309`). | `cargo test --locked` is **currently red on this machine** for an environmental reason, so every later row's gate is unreadable until this is fixed. The test's own helper comment says detection must not depend on what is installed. | `src/connect.rs` golden test | `PATH=/usr/bin:$PATH cargo test --locked` green on a host with no agent harnesses installed |
| 22 | open | **W0.2 — pin the redactor against code identifiers.** `src/capture.rs` tests cover OpenAI/GitHub/AWS/Telegram/JWT/PEM/bearer and nothing else; add UUIDs, git SHAs, file paths, semver, credential-free URLs, bare `user@host`. | F6: redaction runs on every write, so a false positive silently mangles a code snippet an agent stored. No test currently says which way each of those should go. | `src/capture.rs` tests | one case per shape, asserting what it must and must not become |
| 23 | open | **W1 — dimensional metadata.** Add `scope` / `aspect` / `lifecycle` / `trust` / `parent_id` / `superseded_by` to `memories` via the `migrate()` `has_column` path, threaded through a new `MemoryMeta` struct (not an 8th positional arg on `retain_doc_at`), with `recall` filters that narrow in SQL before ranking. | The genuinely missing half of every proposal: there is no way to separate a preference from an episodic turn, so nothing downstream can prioritize one. Uniform-default guard is part of the row: a test asserts the triple is not constant, or every filter is a silent no-op. | `src/store.rs` (`migrate`), `src/memory.rs`, `src/api.rs`, `src/recall.rs` | migration reads a pre-column DB; recall with no filters byte-identical to today; non-uniformity assertion passes |
| 24 | open | **W2 — mount profiles.** `read_only` and `max_trust` on `BankConfig`; writes to a `read_only` bank refused on all three surfaces; a write above the trust ceiling downgraded, not refused (the hook path is never-fail). | F7: an agent that ingested web content writes into the same bank it later reads as trusted rules. Cheapest real security win in the set — `BankConfig` already exists. | `src/api.rs` (`BankConfig`, `retain_doc_at`), `src/mcp.rs`, `src/hooks.rs` | `read_only` refuses on HTTP + MCP + hook; `untrusted_external` cannot land as `scope=policy`; a bank with no config is unchanged |
| 25 | open | **W3 — retroactive redaction.** `memory_versions` table (append-only) + `redact(memory_id, pattern)` rewriting current and historical content, reusing `capture::redact_pii`'s pattern set. | F6: a secret discovered after ingest cannot be scrubbed without destroying the row. `delete_memory` is the only removal path today. | `src/store.rs`, `src/api.rs`, `src/capture.rs` | a redacted secret absent from both tables; version rows and timestamps survive; a second redact is a no-op |
| 26 | open | **W4 — deterministic hygiene scan.** New `src/hygiene.rs`: delta cursor, near-duplicate, contradiction, relative-date, promotion and bloat candidates; `hygiene_scan` (read-only) + `hygiene_apply` (explicit) as a **5th MCP tool**. | F8: no relative-date or contradiction detection exists anywhere (`sweep.rs` is TTL-deletion only). This is the "Dreaming" substrate with no LLM, no dependency and no process — the caller's agent does the synthesis. | `src/hygiene.rs` (new), `src/mcp.rs`, `src/store.rs` | planted duplicate/contradiction/relative-date/oversized row each named and nothing else; scan deterministic across runs; ranking path untouched |
| 27 | open | **W5 — startup grounding capsule.** Per-bank, opt-in, bounded index of `active` `preference\|invariant\|trap\|map` rows at `policy\|global\|domain` scope; 40 lines / 2048 bytes; subtracted from the recall budget, not added to it; `scope_globs` path conditioning. | F10: the measured wins here were serving shape (dates +20.0pp) and budget (40.0→60.0%), so a startup index must cost ~500 tokens, not the 25 KiB the proposals asked for — that is 2–3× the whole recall budget before a memory is served. | `src/api.rs`, `src/hooks.rs`, `src/store.rs` | ≤40 lines / ≤2048 bytes at defaults; capsule + recall ≤ `DEFAULT_RECALL_BUDGET`; glob-scoped row appears only with a matching path; flag-off path byte-identical |
| 28 | open | **W6 — `connect --with-skills`.** `connect` writes hooks and MCP entries and never deploys `plugin/skills/memory-wire/SKILL.md` (`grep -c SKILL src/` = 0). Add the flag, idempotently, refusing to clobber a modified copy; update `SKILL.md` for the W1–W5 model. | A skill that no host receives is documentation, not integration. Same defensive-write discipline as every other `connect` write: atomic, re-read verified. | `src/connect.rs`, `plugin/skills/memory-wire/SKILL.md` | a fresh `HOME` gets the file once; a second run is a no-op; a locally modified copy is refused with a named reason |
| 29 | open | **W7 — the per-turn injection path, and a supersede verb.** Added after the 2026-11-18 external review (`research/audits/context-pack-2026-11-18-round1/`). Two halves: (a) `memory_supersede`/`memory_retract` so an MCP client can correct a stale memory instead of only appending a competing row (`delete_memory` is HTTP-only); (b) decide, do not guess, whether `recall_section` should branch on the score it already parses. | The one place the review found the plan optimising a subsystem that is already fine while a diagnosed per-turn defect sat outside it. **Verified at `36bb947`: the "score is discarded" bug is FIXED** — `ScoredHit` carries `score`, pinned by `the_score_should_survive_the_client_boundary` (`src/hooks.rs:1497`) — but `recall_section` never reads it. Do not re-file the discarded-score bug. | `src/hooks.rs` (`recall_section`), `src/mcp.rs` (5th tool), `src/api.rs` | supersede round-trips (default recall excludes it; `include_history` returns it with lineage); the 4-tool count becomes 5 with a documented reason; any score gate is a frozen-battery hypothesis, never a threshold |

# SECTION E-ANALYSIS

# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 1,482)

# Plan — hierarchical grounded memory for memory-wire

Dated-frozen: published **2026-11-18**. A plan that changes is a new plan.

**Read first:** `docs/audit/2026-11-18-surface-audit.md`. This plan is derived
from it; every item below names the finding (F1–F10) or the "genuinely unbuilt"
row it rests on. If the audit's facts are wrong, this plan is wrong.

---

## 0. What this plan is, and what it is not

Three upgrade proposals arrived (Claude hierarchical memory + Dreaming;
a `ponytail-audit` red-team of that; and an `operant` × `memory-wire`
integration blueprint). They were written from the README's top half at
`b478d15` and **describe a `memory-wire` that does not exist**: 54 MCP tools
that are 4, `WAL`/auth work that shipped in v0.7.0, and a promise to lift a
benchmark metric that its own design structurally cannot touch.

This plan keeps what survives contact with the code:

- the **dimensional memory model** (`scope` × `aspect` × `lifecycle` × `trust`),
  which is genuinely absent and genuinely useful;
- **mount-level read-only**, a real prompt-injection boundary;
- **retroactive redaction**, a real privacy gap;
- **deterministic hygiene** (duplicate / contradiction / relative-date /
  bloat), pure Rust, cursor-based — the "Dreaming" substrate without an LLM;
- a **bounded startup grounding capsule**, opt-in and budgeted;
- **`connect --with-skills`**, a real one-line gap.

It drops:

- "activate the four inert ranking mechanisms" — **closed by measurement** (F2);
- "lift `single-session-preference` R@1 from 43.3% to ≥80% via hierarchy" —
  **structurally impossible** on that instrument (F3);
- "add `w_tier · TierPrior` to the RRF score" — **algebraically wrong** (F4);
- "audit and prune the 54 tools" — there are 4 (F5);
- "WAL + busy_timeout + BEGIN IMMEDIATE + loopback bind" — **shipped** (F1).

### Non-negotiable invariants (carried, not restated)

1. **Zero daemon, low RSS.** The default build stays one static binary,
   ~8.8 MiB, ~10.7 MiB idle RSS, `DT_NEEDED` = 3, 0 background processes.
   Any item that would add a dependency, a shared library, or a mandatory
   process is rejected before it is designed.
2. **No LLM in the substrate.** Dreaming/semantic synthesis is the caller's
   job (`operant`/host agent). `memory-wire` exposes deterministic primitives
   only. No `reqwest`, no HTTP client, no subagent runner in the default build.
3. **Eval-gated, three-set discipline** (`AGENTS.md` §6). Selection on LoCoMo
   and the frozen relevance battery only. Every LongMemEval-touching run is a
   counted consultation (next would be #6). **No item in this plan is
   selected on a LongMemEval number.**
4. **Do not re-propose `CLOSED.md`.** Every ranking item here is additive to
   the schema or the serving path, not a weight change.

---

## 1. The shape of the work

Six capability items (W1–W6), ordered by risk and dependency, plus two
correctness fixes that must land first because they make every later gate
meaningful.

```
W0  correctness fixes (test hermeticity, redactor test gap)   [hours, unblocks gates]
W1  dimensional metadata (scope/aspect/lifecycle/trust)       [schema + write + read]
W2  mount profiles (read_only, trust ceiling)                 [config + 2 guards]
W3  retroactive redaction (id/version + pattern)              [version table] ✅
W4  deterministic hygiene scan + apply                        [new module + verb] ✅
W5  startup grounding capsule (bounded, opt-in)               [new read path] ✅
W6  connect --with-skills                                     [one flag]
```

W1 is the foundation: W2, W4 and W5 all read its columns. W3 is independent
and can run beside it. W6 is independent of everything.

**No item changes `FusionWeights::SHIPPED`.** If a later cycle wants a ranking
lever, it is a new hypothesis with its own pre-registration, not a line here.

---

## 2. Items

### W0 — Correctness fixes (do first; gates depend on them)

Rests on **F9** (non-hermetic golden test) and **F6** (redactor test gap).

**W0.1 — make `golden_install_shapes` hermetic.** The test calls `run()`,
which uses the real `binary_on_path`; on a host without `copilot` it fails
with `CopilotCli: unexpected outcome Skipped("not detected on this machine")`.
The seam already exists and nine other tests in the same module use it:
`apply(host, exe, home, uninstall, on_path, data_root, bank)` takes the PATH
probe as an argument, and `no_path` is defined at `src/connect.rs:1309` for
"detection cannot depend on what happens to be installed". Change the golden
scenario to call `apply(..., no_path, ...)` with a home that carries each
host's config dir, so detection is a function of the fixture.

**W0.2 — pin the redactor against code identifiers.** Add cases to
`src/capture.rs` tests: UUIDs, git SHAs, file paths, semver, URLs without
credentials, bare `user@host` strings. Assert what each should and should not
become. Rests on **F6**.

**Gate:** `PATH=/usr/bin:$PATH cargo test --locked` is green on a host with no
agent harnesses installed. `cargo clippy --all-targets --all-features -- -D
warnings` clean.

> **Current state, so nobody misreads the baseline:** on this machine the bin
> suite is **345 passed / 1 failed** and the four integration suites are
> **239 passed / 0 failed**. The one failure is this item's subject. The suite
> is red *now*, so "green" in the gate above is a target, not a description of
> today.

---

### W1 — Dimensional metadata (the foundation)

Rests on **"genuinely unbuilt" row 1**; avoids the **uniform-default trap**
(audit §4) and **F5** (4 tools, so extend not add).

**Schema.** Add to `memories`, via the existing `ALTER TABLE … ADD COLUMN`
migration path (`src/store.rs` `migrate()`, the `has_column` block), all
defaulted so a legacy database reads unchanged:

| column | type | meaning |
|---|---|---|
| `scope` | `TEXT NOT NULL DEFAULT 'local'` | `policy` \| `global` \| `domain` \| `local` \| `agent` \| `session` |
| `aspect` | `TEXT NOT NULL DEFAULT 'episodic'` | `preference` \| `invariant` \| `decision` \| `procedure` \| `trap` \| `fact` \| `map` \| `episodic` |
| `lifecycle` | `TEXT NOT NULL DEFAULT 'active'` | `candidate` \| `active` \| `quarantined` \| `superseded` \| `archived` |
| `trust` | `TEXT NOT NULL DEFAULT 'agent_inferred'` | `manual_policy` \| `human_approved` \| `agent_verified` \| `agent_inferred` \| `untrusted_external` |
| `parent_id` | `TEXT` | nullable FK-ish pointer to another memory (no FK: a dangling pointer must not fail a write) |
| `superseded_by` | `TEXT` | nullable pointer; the row stays readable but is filtered by default |

**Write path.** `Memory` gains the four fields; `retain` gains optional
parameters. `retain_doc_at` already carries **seven** arguments under an
`#[allow(clippy::too_many_arguments)] // one more optional field on the same
write` (`src/api.rs:541`), and the codebase's stated policy is to raise
`too-many-arguments-threshold` rather than keep adding allows — so an eighth
positional parameter is the wrong move. **Introduce a `MemoryMeta` struct**
carrying `(scope, aspect, lifecycle, trust, parent_id, superseded_by,
scope_globs)` and thread it through `retain_doc_at`. That is the same shape the
W1 recall filters and the W5 capsule need, so it is one type, not two.

**Read path.** `recall` gains optional filters (`scopes`, `aspects`,
`lifecycles`, `include_history`) that narrow *before* ranking, in SQL, exactly
as `exclude_tags` does today (`recall_filtered`). `superseded` and
`quarantined` are excluded by default; `include_history` opts in.

**Uniform-default guard.** A test asserts that across the fixture corpus the
`(scope, aspect, lifecycle)` triple is **not** constant — otherwise every
downstream filter is a silent no-op. This is the anti-pattern the audit calls
out by name.

**Gate:** unit + integration tests for migration from a pre-column database,
default-path filters, `include_history` round-trip, and the non-uniformity
assertion. No ranking change; recall output byte-identical when no filter is
passed (asserted).

---

### W2 — Mount profiles (read-only boundary)

Rests on **"genuinely unbuilt" row 2** and **F7**.

**Config.** `BankConfig` (the per-bank JSON object behind
`GET|PUT /banks/:id/config`) gains `read_only: bool` (default `false`) and
`max_trust: string` (default `untrusted_external`, i.e. no ceiling).

**Guard 1 — write refusal.** `retain_doc_at` (and the MCP `memory_retain`, and
the HTTP `retain`) refuse with the frozen error contract's existing
`400 invalid content` shape when the resolved bank is `read_only`. Rests on
`AGENTS.md` §9: the HTTP error contract is frozen; a new refusal must be a new
documented variant, not a silent behavior change.

**Guard 2 — trust ceiling.** A write whose `trust` exceeds the bank's
`max_trust` is refused or downgraded (choose downgrade, and document it: a
silent refuse breaks the never-fail hook path).

**Anti-uniformity.** Test that an untrusted mount cannot write
`scope=policy|global` or `aspect=invariant` — the exact escalation the
proposals name.

**Gate:** `read_only` mount rejects writes on all three surfaces; downgrade
path tested; zero change to a bank with no config.

---

### W3 — Retroactive redaction ✅ DONE (commit 052f3a6)

Rests on **"genuinely unbuilt" row 3** and **F6**.

**Schema.** `memory_versions(id, memory_id, op, content, content_sha256,
actor, created_at)` — append-only, written on every create/update/supersede/
delete/redact. This is also what makes `superseded_by` auditable.

**Operation.** `redact(memory_id, pattern)` rewrites the *current* content and
every historical version's content, replacing matches with
`[REDACTED:<name>]`, preserving the audit skeleton (`op`, `created_at`,
`actor`, `content_sha256` recomputed). A `redact` over a *pattern* reuses
`capture::redact_pii`'s regex set (one source of truth, so the two cannot
drift).

**Delete vs redact.** `delete_memory` stays as-is (hard delete). `redact` is
the history-preserving path.

**Gate:** a secret retained, then redacted by pattern, is gone from
`memories` **and** `memory_versions`, while the version rows and their
timestamps survive; `content_sha256` of a redacted version differs from the
original; a second `redact` is idempotent.

---

### W4 — Deterministic hygiene scan ✅ DONE (commit ad8472a)

Rests on **"genuinely unbuilt" row 4** and **F8**. This is the "Dreaming
substrate" — **no LLM, no new dependency, no new process**.

**New module `src/hygiene.rs`** with a cursor:

`hygiene_scan(bank, cursor_id, opts) -> HygieneReport`

- **delta**: rows created/updated since the cursor (the `cursor` is a
  `created_at` watermark, stored in `banks.config` as `hygieneCursor`).
- **near-duplicates**: Jaccard token similarity above a threshold within the
  same `(scope, aspect)` — candidate `MERGE`.
- **contradictions**: same `scope` + overlapping token set + differing
  `created_at` and a negation/contrast marker — candidate `SUPERSEDE`.
- **relative dates**: regex for `yesterday|today|tomorrow|last week|next
  week|currently|now|this sprint` on rows that lack an ISO date — candidate
  `ANCHOR` (a *proposal*, never an automatic rewrite).
- **promotion candidates**: the same `(scope='local'|'session')`+`aspect`
  pattern appearing in ≥2 sessions — candidate `PROMOTE`.
- **bloat**: rows over `max_record_bytes`, and banks over
  `max_hot_index_lines/bytes` — candidate `SPLIT`.

**Apply** is a separate, explicit verb — `hygiene_apply(batch,
expected_epoch)` — so the scan is read-only and cheap and the mutation is
reviewable. Rests on `AGENTS.md` §8: `ponytail` discipline says the smallest
complete change; scan and apply are two verbs because they fail differently.

**Determinism.** Every detector is a pure function of the rows; no clock, no
randomness. A test runs the scan twice and asserts byte-identical reports.

**Surface.** One MCP tool `memory_hygiene` (a **5th** tool, justified: it is
the only read that is not a recall), or an extension of `memory_reflect`.
Choose the 5th tool and document why 4→5 is correct here while 4→54 would not
be.

**Gate:** synthetic fixture with one planted duplicate, one contradiction, one
relative date, one oversized row → the report names exactly those four and
nothing else; scan is deterministic across runs; scan touches no ranking path.

---

### W5 — Startup grounding capsule ✅ DONE (commit 4540b19)

Rests on **"genuinely unbuilt" row 5** and **F10**.

**What it is.** A per-bank, bounded materialization of the `active` rows whose
`aspect` is `preference|invariant|trap|map` and whose `scope` is
`policy|global|domain`, one line each:

```
[global/preference] m_01a [2026-10-14] Never use em dashes in prose or chat replies
[domain:eng/memory-wire/trap] m_08f [2026-11-02] LongMemEval harness passes a 100k budget; tests/recall_budget.rs guards the 8k trim
```

**Bound.** `max_lines` default **40**, `max_bytes` default **2048** (~500
tokens). **Not** the proposals' 200/25 KiB: the audit's F10 shows the dominant
cost is already the 8,000-token recall budget, and a 25 KiB index is 2–3× that
before a single memory is served. The bound is derived, and a test pins it to
the serving budget rather than to a chosen constant.

**Budget accounting.** The capsule is **subtracted from** the recall budget
when both are served in one injection, so the total never exceeds
`DEFAULT_RECALL_BUDGET`.

**Path conditioning.** Rows may carry a `scope_globs` list (new, nullable,
comma-separated). A row with a non-empty glob set is **excluded from the
capsule** unless an `active_paths` argument matches one — the "conditional
rule" behaviour, without a new store.

**Opt-in.** Per-bank config flag, default **off**. Nothing injects it until a
bank asks.

**Gate:** a bank with 200 preference rows yields ≤40 lines and ≤2048 bytes;
the capsule subtracts from the budget (asserted sum ≤ budget); a glob-scoped
row is absent without a matching path and present with one; a bank with the
flag off serves byte-identical output to today.

---

### W6 — `connect --with-skills` ✅ DONE (commit 4793326)

Rests on **F5** and audit §4.

`connect` writes hooks and MCP entries and **never deploys
`plugin/skills/memory-wire/SKILL.md`** (`grep -c SKILL` = 0 across `src/`).
Add `--with-skills` that copies the skill into each wired host's skills
directory, idempotently, refusing to overwrite a modified copy (same
defensive-write discipline as every other `connect` write: atomic, re-read
verified, never clobbers a foreign file).

**Also update `SKILL.md`** to state the new model once W1–W5 land: what
`scope`/`aspect`/`lifecycle` mean, the "verify against the workspace before
acting" framing, and the L0→L1→L2 read order. Apply the proposals' "what, not
how" principle: describe what each scope is *for*, do not prescribe a capture
checklist.

**Gate:** a fresh `HOME` with one host; `--with-skills` writes the file once,
a second run is a no-op, and a locally modified copy is refused with a named
reason.

---

### W7 — The per-turn injection path, and a supersede verb ✅ W7a DONE (commit d34a0a5), W7b decided (commit eb95a0a) (added after the 2026-11-18 review)

Rests on **D-ASK**, verified at `36bb947`, and on the review's finding 4.

**What is actually true at HEAD, so this item is not built on a stale premise.**
`src/hooks.rs:660` `recall_at` returns `Option<Vec<ScoredHit>>`; `ScoredHit`
(`:496-507`) carries `content`, `created_at` **and** `score: Option<f64>`, and
`the_score_should_survive_the_client_boundary` (`:1497`) pins that the score
survives the wire. So the *evidence-gathering* half of D-ASK has landed. What
has **not**: `recall_section` (`:680-706`) reads content and date and **never
reads `score`** — its only consumer is a `tracing::debug!` whose own comment says
"Nothing branches on this and nothing is gated by it". **Do not re-file the
"score is discarded" bug; it is fixed.** The open defect is that the scale is
available and unused.

**Two sub-items, deliberately separate because they fail differently.**

**W7a — a supersede/retract verb on the MCP surface.** ✅ DONE (commit d34a0a5) Today `memory_retain`
can only append, so an agent that notices an outdated memory mid-turn has no way
to correct it; it can only add a competing row. `delete_memory` exists but is
HTTP-only, so an MCP client cannot reach it. Add one verb — `memory_supersede`
or `memory_retract` — that marks a row superseded (W1's `superseded_by`) rather
than deleting it, so the audit trail survives. This is the **5th tool**, and the
justification is the one the audit already accepted for W4: it is a verb no
existing tool expresses, not a second way to spell `retain`.

**W7b — decide, do not guess, whether `recall_section` should branch on the
score.** The honest options are (a) leave it ungated and say so, or (b) gate it
on something the dev set supports. A raw score cut is **forbidden**: per-query
min-max normalisation forces the top hit of any irrelevant set to 1.0 by
construction (`src/recall.rs:319-337`), so a cut on that field is close to
meaningless — the same reasoning that already killed the threshold idea in
`CLOSED.md`. If W7b is attempted at all, it is a pre-registered hypothesis on
the frozen relevance battery, never a constant picked off an observation.

**Gate:** a supersede verb round-trips (supersede → default recall excludes it
→ `include_history` returns it with its lineage); the 4-tool count becomes 5
with a documented reason; and if W7b is attempted, its gate is the frozen
battery, not an anecdote.

> **Why this item exists at all, said plainly:** it is the one place where the
> review found the plan optimising a subsystem that is already fine while a
> diagnosed per-turn defect sat outside it. The defect is real, it is on the
> path every agent turn takes, and W0–W6 as first written did not touch it.

---

## 3. Ordering and what each item costs

| item | depends on | risk | rough size | what it buys |
|---|---|---|---|---|
| **W0** | — | low | hours | a gate that means something |
| **W1** | W0 | **medium** — schema migration | days | the substrate for W2/W4/W5; queryable scopes |
| **W2** | W1 | low | days | prompt-injection boundary (the real security win) |
| **W3** | W0 | low | days | retroactive privacy |
| **W4** | W1 | **medium** — new module, new verb | days | "Dreaming" without an LLM |
| **W5** | W1 | medium — serving path | days | bounded grounding at ~500 tokens |
| **W6** | — | low | hours | skills actually reach the hosts |
| **W7** | W1 | **medium** — a new verb, and a policy decision | days | the per-turn path; an agent can correct a stale memory |

**Do not parallelize W1 and W4/W5.** W1's column shape is what W4's detectors
and W5's filter read; changing it after W4 lands is a second migration.

---

## 4. Gates / success criteria (fixed before the run)

> **What this plan cannot move, stated plainly (added after the 2026-11-18
> external review).** Every gate below is deliberately conservative: no ranking
> weight changes, unfiltered recall stays byte-identical, the flag-off paths stay
> byte-identical. The consequence is that **no LongMemEval-S metric and no LoCoMo
> metric can move because of W1–W5.** A per-question fresh index has no W5 capsule
> rows; W1 adds no unfiltered effect; W4 never runs during a benchmark recall; W2
> is a write guard. The metrics W1/W4 can move are **PrecisionMemBench** (if the
> adapter populates and queries the new filters) and the **frozen relevance
> battery**. W2 moves no metric in the record at all. All four can move footprint
> and latency **negatively**, which Gate 8 bounds.
>
> That is the correct trade — but it must be said out loud, or the plan reads as
> promising a retrieval gain it structurally cannot deliver.

1. **W0:** `PATH=/usr/bin:$PATH cargo test --locked` green on a host with no
   agent harnesses; clippy `-D warnings --all-targets --all-features` clean.
2. **W1:** migration from a pre-column DB reads back every row; recall with no
   filters is byte-identical to pre-W1; a `(scope, aspect, lifecycle)`
   non-uniformity assertion passes on the fixture corpus.
3. **W2:** a `read_only` bank refuses writes on all three surfaces; an
   `untrusted_external` write cannot land as `scope=policy`.
4. **W3:** a redacted secret is absent from both `memories` and
   `memory_versions`; the audit skeleton survives; a second redact is a no-op.
5. **W4:** the scan is deterministic across runs; it names planted
   duplicates/contradictions/relative-dates/oversized rows and nothing else;
   the ranking path is untouched (recall output identical).
6. **W5:** capsule ≤40 lines and ≤2048 bytes at the defaults; the capsule +
   recall sum ≤ `DEFAULT_RECALL_BUDGET`; a glob-scoped row appears only with a
   matching `active_paths`; the flag-off path is byte-identical.
7. **W6:** `--with-skills` is idempotent and refuses a modified copy.
8. **Cross-cutting, every item:** binary ≤ 9 MiB (from 8.8), idle RSS ≤ 12 MiB,
   `DT_NEEDED` = 3, `cargo doc --no-deps --all-features` 0 warnings.
   **Any item that moves these is redesigned, not merged.**
9. **No ranking change is merged without a dev-set measurement** (LoCoMo or
   the frozen battery). This plan ships **zero** ranking changes; if a later
   item needs one, it is a new pre-registered hypothesis with its own gate.

---

## 5. Cost

No new dependency, no new shared library, no LLM, no background process. The
default build stays a single static binary. The largest single item is the W1
migration; the riskiest is W4 (a new module with a new public verb, which
`AGENTS.md` §5 requires be documented with `///` on every public item).

**What this plan deliberately does not do** (and why):

- **Change any ranking weight.** `CLOSED.md` forbids re-proposing closed
  levers without new evidence, and the audit (F2) shows the four "inert"
  mechanisms were measured and failed.
- **Add a tier prior to the RRF score.** Algebraically wrong (F4); if a
  structural prior is ever wanted it is a bounded rank voter, its own
  hypothesis, its own gate.
- **Promise a `single-session-preference` R@1 lift.** Structurally impossible
  on that instrument (F3); the plan says so rather than shipping a metric it
  cannot move.
- **Touch LongMemEval to select anything.** `AGENTS.md` §6; the next
  consultation would be #6 and is release acceptance only.

---

## 6. Resume here

**All items complete.** W0–W7 shipped across 10 commits:

- W0 (a45ea07) — hermetic golden_install_shapes + redactor identifier tests
- W1 (e10ba0b) — dimensional schema: scope/aspect/lifecycle/trust/parent_id/superseded_by
- W2 (8eaeac3) — mount profiles: readOnly + maxTrust on BankConfig
- W3 (052f3a6) — retroactive redaction with version audit trail
- W4 (ad8472a) — deterministic hygiene scan module
- W5 (4540b19) — startup grounding capsule module
- W7a (d34a0a5) — supersede verb on the MCP surface (5th tool)
- W6 (4793326) — connect --with-skills + embedded SKILL.md
- W7b (eb95a0a) — decision doc: no raw score gate on recall_section

`docs/handoff/STATE.md` is updated when a unit of work lands, per
`AGENTS.md` §13.

### 6.1 What the 2026-11-18 external review changed (round 1)

A context-pack review round was run against this plan — Hydrogen at `xhigh`,
75 kB pack, 18,985-char reply. Pack, prompt, reply and the verification pass are
in **`research/audits/context-pack-2026-11-18-round1/`**. Four adoptions:

1. **W7 added** — the per-turn injection path and a supersede verb. The reviewer
   found the plan improving a subsystem that is already fine while a diagnosed
   per-turn defect sat outside it. That was correct and is the round's main win.
2. **Gate 4 now states what the plan cannot move** (§4 preamble). True by
   construction; the plan had never said it.
3. **D-ASK's `hooks.rs` snippet corrected in place** — the score is no longer
   discarded, and the doc now says so rather than inviting the next reader to
   re-file a fixed bug.
4. **Process fix:** pack the artefact under review **in full**. The round's pack
   omitted this plan's W0–W6 body to fit a byte budget, and the reviewer's gaps
   hardened into confident claims about code it could not see. That is my error,
   recorded in the verification pass §5.

Two reviewer claims were **refuted** and recorded as such: that `hooks.rs`
discards the score (it carries `ScoredHit.score`, pinned at `src/hooks.rs:1497`),
and that the provenance label is missing (it is present at `:682`).

# FILE: docs/audit/2026-11-18-surface-audit.md (lines 154,262)

## Part II — Findings, ranked `ponytail` order

Ranked **Correctness → Risk/Security → Concurrency → Tested → Fast → Lean**.

### F1 [CORRECTNESS] Three of four "unbuilt" Pillars are already shipped

**Evidence:** §2 above. WAL, busy_timeout, loopback bind + exposure warning,
spawn_blocking, WAL read pool, summary capsules, in-band dates, per-bank
capsules.

**Consequence:** a plan that begins with Batch 1 ("Concurrency, Panic
Telemetry & Auth") would re-derive v0.7.0/v0.8.0. The real frontier is the
**dimensional schema** (nothing exists) and the **serving shape** (dates +
capsules already proven; the surface-level adoption is a small patch).

### F2 [CORRECTNESS] The "activate the dormant 4" lever is closed with measurements

**Evidence:** `eval/LOCOMO_LEVERS.md` §Gates; `CLOSED.md`. All four failed
pre-registered gates on n=1,531.

**Consequence:** any plan item that says "activate sentence-level overlap" or
"activate the recency stream" is **re-proposing a rejected lever**. It needs
new evidence *stated before spending anything* (AGENTS §8), and the only new
evidence that would qualify is a **different mechanism** (e.g. a sub-document
index that changes what is ranked), not a re-run of the same weights.

### F3 [CORRECTNESS] The proposals' benchmark diagnosis is right, so the metric promise is wrong

**Evidence:** README methodology line + `examples/longmemeval.rs` shape; §3 above.

**Consequence:** **no hierarchy feature can lift `single-session-preference`
R@1.** The only levers that can touch that number on this instrument are:
(a) what `retain` writes, (b) how `recall` ranks a session-as-document, (c)
the serving budget/shape. Any plan whose Phase 2 gate is "R@1 43.3% → 80%" is
promising something its own design cannot deliver.

### F4 [CORRECTNESS] Additive structural priors over RRF are algebraically wrong

**Evidence:** RRF score magnitudes (own derivation, consistent with
`src/recall.rs` docs and `eval/SELECTION.md`).

**Consequence:** a tier/kind/trust prior must be a **rank voter inside the
fusion**, bounded like `bm25_magnitude`'s `u ∈ [0,1]` formulation, or an
intent-gated margin. Never a raw constant added to `1/(k+rank)`.

### F5 [CORRECTNESS] "54 MCP tools" is wrong by 13×, and the plan it seeded is a non-problem

**Evidence:** §1.

**Consequence:** the tool-surface work item collapses from "audit and prune 54
tools" to "**extend 4 schemas**; decide whether the new model needs a 5th".
This is a one-day item, not a pillar.

### F6 [RISK] Retroactive redaction is genuinely absent

**Evidence:** `capture::redact_pii` is write-time only; no `memory_versions`
table; `delete_memory` is the only removal path.

**Consequence:** a leaked secret discovered post-ingest cannot be scrubbed
from history without destroying the row. Real gap. Also: no test pins the
redactor against code/UUID false positives (`capture.rs` tests cover
OpenAI/GitHub/AWS/Telegram/JWT/PEM/bearer only).

### F7 [RISK] No mount-level read-only boundary

**Evidence:** bank isolation is by name; there is no `read_only` concept in
`src/`.

**Consequence:** an agent ingesting untrusted web content writes into the same
bank it later reads as trusted rules. The proposals' "prompt-injection
firewall via access segregation" is a **real** gap. Note it is cheap:
`BankConfig` already exists as a per-bank JSON object (`recallMaxTokens`,
`retainTags`) — a `read_only` flag there is a small, testable addition.

### F8 [RISK] No relative-date / contradiction hygiene

**Evidence:** §4. `src/sweep.rs` is TTL-only.

**Consequence:** "what did we decide last week" degrades silently; no
detector exists. This is the one hygiene capability that is *pure Rust, no LLM,
no new dependency*, and therefore fits the substrate boundary exactly.

### F9 [TESTED] One pre-existing test failure, environmental

**Evidence:** `connect::tests::golden_install_shapes` fails on the pristine
tree with `CopilotCli: unexpected outcome Skipped("not detected on this
machine")`. The golden test calls `run()`, which uses the **real** `PATH`
probe (`binary_on_path`), and `copilot` is not installed on this machine.

**Consequence:** the test is **not hermetic** — it contradicts its own
`no_path` helper's stated intent. This is a **real bug in the test**, worth
fixing (inject `no_path`), and it means "the gate is green" is currently
**false on this host**. Any upgrade plan that says "gated by `cargo test`" must
say "gated by `cargo test`, which is currently red for an environmental
reason".

### F10 [LEAN] The real cost axis is the 8,000-token budget, not the tool count

**Evidence:** `tests/recall_budget.rs`, `README.md:142-170`, H12 +12% context
growth. The measured wins this project has ever had on *quality* were serving
shape (dates, +20.0pp) and budget (40.0→60.0%).

**Consequence:** a startup `HotIndex` must be **subtracted from** the serving
budget, not added to it. A 4 KiB index beside an 8,000-token recall is a
~50% overhead on the dominant cost; that is why the plan caps it at ~1,000
tokens and makes it opt-in.

---


# FILE: docs/audit/2026-11-18-surface-audit.md (lines 263,283)

## Part III — What is genuinely unbuilt (the real frontier)

Six things survive the audit. Each is small, deterministic, and fits the
"substrate, not agent" boundary.

| # | capability | why it is not already here | size |
|---|---|---|---|
| 1 | **Dimensional metadata**: `scope` / `aspect` / `lifecycle` / `trust` on a memory | never existed | schema + migration + `retain`/`recall` params |
| 2 | **Mount profiles**: per-bank `read_only`, per-call trust ceiling | never existed; `BankConfig` is the natural home | one config field + two guards |
| 3 | **Retroactive redaction**: `redact(id, pattern)` across versions | no version table | new table + one op |
| 4 | **Hygiene scan**: duplicate/contradiction/relative-date/bloat candidates, pure Rust, cursor-based | `sweep.rs` is TTL-only | one new module + one MCP verb |
| 5 | **Startup grounding capsule**: bounded index of active rules, opt-in, TOON/compact | never existed; capsules exist per-*memory* not per-*bank* | new read path |
| 6 | **`connect` deploys `SKILL.md`** (`--with-skills`) | `connect` writes hooks/MCP only | one flag + one template copy |

Plus one **test bug** (F9) and one **redactor test gap** (F6).

Everything else in the three proposals is either **already shipped** (F1), or
**closed by measurement** (F2), or **cannot do what it promises** (F3).

---


# SECTION F-LENS

# FILE: README.md (lines 511,576)

## Connect, hooks, doctor

`memory-wire connect` installs the five lifecycle hooks into every agent host it
detects (claude-code, codex, copilot-cli) and the MCP server entry into the two
that speak it (cursor, opencode); pass a host to wire just that one, `--uninstall`
to prune, `--guidelines` to write the rules block into this project's agent files
instead. It is idempotent and never touches a hook it did not write — malformed
config is refused untouched rather than rewritten. (The file it writes is
re-serialised with sorted keys and two-space indent; foreign entries survive
intact, and whenever there was a file to copy, a timestamped backup of it is kept
under `$XDG_DATA_HOME/memory-wire/backups/` and its path is printed.)

```bash
memory-wire connect claude-code     # -> claude-code  wired         SessionStart, UserPromptSubmit, Stop, PreCompact, SessionEnd
memory-wire doctor --db /path/to/agents.db
```

`hook session-start|prompt|stop|pre-compact|session-end` is what those host
entries invoke. The first two print: a bank preamble plus a recall, and the
recall lines for a submitted prompt. The last three print **nothing** — their
value is entirely in what they retain. `stop` retains one compacted line naming
the transcript; `pre-compact` and `session-end` fire at the two moments a
conversation stops being available (`PreCompact` runs immediately before Claude
Code discards it to context compaction), so both read the transcript at
`transcript_path` and retain the conversation's own prose, capped at 2,000
characters, rather than a pointer to the file. All five are best-effort by
contract — a down server prints the local framing and exits 0, never an error in
front of the model. The preamble
reads the bank's `background`/`preamble`/`system_prompt` from
`GET /banks/:id/config` when that key exists — for the bank the hook resolves
from its own working directory. That bank is resolved in a fixed order:
`--bank <id>`, then `MEMORY_WIRE_BANK`, then `owner/repo` from the repository's
`origin` remote, then the git work tree's top-level name, then `memory-wire`.
The remote is read straight out of `.git/config` with no `git` process spawned;
`https://github.com/acme/api.git`, `git@github.com:acme/api.git` and
`ssh://git@host/acme/api.git` all resolve to bank `acme-api`. A repository with
no remote, or one whose remote names no `owner/repo`, falls back to the old
basename rule. Full order, and the `.git`-as-a-file handling for worktrees and
submodules: [`docs/BANK_IDENTITY.md`](docs/BANK_IDENTITY.md).

**Upgrading moves your bank name, and nothing migrates for you.** A remote-derived
id is a different id from the basename that produced the same memories before,
so an existing bank stays where it is and new sessions write to the new name.
`doctor` warns when it sees that case — the derived name differs from the old
one *and* a bank of the old name exists — and names both, but it stays
read-only and moves nothing. Keep reaching the old memories with
`memory-wire hook session-start --bank <old>` or `MEMORY_WIRE_BANK=<old>`. There
is deliberately no automatic migration and no alias: a wrong guess moves
memories between namespaces on the strength of a directory name, silently, and
that is worse than a name you have to type.

`doctor` is one screen: endpoint, bank, store size and health, bank/memory row
counts, server state. `server up` means `/health` answered 2xx **and** the body
was exactly `ok`, so a foreign process on the port cannot pass for the server.
`--strict` exits nonzero when the server or store is unusable, and a store whose
pages fail `PRAGMA integrity_check` is reported `unreadable` rather than counted.
It never creates the store it reports on — pointing `--db` at a nonexistent path
reports `missing` without creating the file — and on an *existing* store it folds
the WAL into the main file on the way past (best effort, `PASSIVE`, never
blocking), which is what makes a `cp` taken straight afterwards a usable backup.
Point it at the same file you gave `serve --db`, or the report describes a store
the server never touches.

- [Page index](docs/PAGE-INDEX.md) — every active doc and the question it owns.
- [Architecture](docs/ARCHITECTURE.md) — what memory-wire is made of, written for a reader who will not read the code.


# FILE: README.md (lines 654,700)

## MCP

The same five tools are served two ways. `memory-wire mcp` is stdio —
JSON-RPC 2.0, newline-delimited, stdout only for protocol, logs to stderr.
`--bank` sets the default bank, falling back to `$MEMORY_WIRE_BANK` and then
to `memory-wire`; a per-call `bank` argument overrides both.

A running `memory-wire serve` also answers MCP over HTTP at **`/mcp`** and at
**`/mcp/{bank}`**, which is what hosts that speak `url`-based MCP need — Codex
among them. On `/mcp` the bank comes from the tool argument exactly as on
stdio. On `/mcp/{bank}` the URL **pins** it: a call that also supplies a
different `bank` is rejected with a tool error rather than quietly landing in
the bank the caller did not name. `memory-wire connect codex` writes the
`[mcp_servers.memory-wire]` entry with a `url`.

The HTTP endpoint is as unauthenticated as the rest of the server, and rmcp
additionally refuses any request whose `Host` header is not loopback — so
binding beyond `127.0.0.1` is not enough on its own to expose it.

| Tool | Writes? | Arguments |
|---|---|---|
| `memory_retain` | yes | `content` (required), `bank?`, `context?`, `tags?` |
| `memory_recall` | no | `query` (required), `bank?`, `budget?`, `tags?`, `exclude_tags?` |
| `memory_reflect` | no | `query` (required), `bank?` |
| `memory_bank_config_get` | no | `bank?` |

Every tool declares `destructiveHint: false`; `memory_retain` is the only one
without `readOnlyHint: true` — a repeated retain of the same content in the same
bank writes nothing and returns the id that already holds it, but a repeated
retain of *different* content is a second memory. Each tool is a thin adapter onto
the same `MemoryService` methods
the HTTP routes call — retrieval, redaction, and budgeting are not reimplemented
for MCP, so the two surfaces cannot drift. An unknown tool name is a JSON-RPC
`MethodNotFound`; a tool that runs and fails returns `isError: true` with the same
opaque message the HTTP surface returns, never a crash.

```json
{ "mcpServers": { "memory-wire": {
  "command": "memory-wire", "args": ["mcp", "--bank", "my-project"] } } }
```

Bank-in-path scoping (`/mcp/{bank}`) **is** built for HTTP, and `connect` writes it
for Codex — the one host whose MCP entry is a `url` rather than an argv, so the
URL is the only place the bank can live. The rule is the same one that mode needs:
a bank named in the URL is pinned, and a call that also supplies a different one
is refused rather than silently served.


# FILE: README.md (lines 426,453)

## CLI

Nine subcommands in a default build, exactly as `--help` reports them (`help` is
clap's own and is not one of them). An `--features embed` build has a tenth,
`embed <text>`, which prints one 384-d vector as JSON; it is the reason the
weights survive LTO:

| Command | What it does |
|---|---|
| `info` | Audit + plan pointers (the default when no command is given) |
| `daemon` | `start` / `stop` / `status` for a backgrounded server — see below |
| `serve` | Start the HTTP server in the foreground — `--addr` (default `127.0.0.1:8888`), `--db`. Stops accepting and drains in-flight requests on SIGINT/SIGTERM |
| `connect` | Wire the hooks into agent hosts — optional `<agent>`, `--uninstall`, `--guidelines` |
| `hook` | Lifecycle hook the hosts invoke — `session-start`, `prompt`, `stop`, `pre-compact`, `session-end` |
| `doctor` | Endpoint, bank, store, and server health — `--db`, `--strict` |
| `mcp` | Serve MCP over stdio — `--bank`, `--db` |
| `sweep` | Delete memories past their bank's `ttl_days` — `--dry-run`, `--db` |
| `seed` | One-shot bank seeding from this repo's git history — `--commits <N>` (default 100, capped at 500), `--transcripts`, `--bank`, `--db` |

`--addr` defaults to loopback because there is no authentication: anything that
can reach the port can read and delete every bank. A non-loopback address still
binds, and says so once on stderr before it does.

If 8888 is already taken, `serve --addr 127.0.0.1:<free port>` and point your
client at that port; `GET /health` on it must return exactly `ok`. There is no
`backup` subcommand — a SQLite store is backed up with `sqlite3 "$DB" ".backup
'$DB.bak'"`, recipe and caveats in `INSTALL_FOR_AGENTS.md`.


# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180)

# Open: hook recall injects the best of an irrelevant set

Status: **OPEN — diagnosed, deliberately not fixed.** Recorded 2026-09-30 so it
can be picked up after the operant agentic-loop work.

## Symptom

On 2026-09-30, twice, the `UserPromptSubmit` hook injected roughly 10,000
characters of quantitative-trading cell-architecture notes into a conversation
about Telegram agentic-loop bugs. The injected rows were well-formed, correctly
attributed, and completely irrelevant:

> `# Cell-Architecture Audit Report — pre-deployment revision (2026-09-22)`
> … 263 audited cell-venues … Kelly allocation … RG-071 …

The second instance was a fragment of an unrelated agent session log:

> `Full battery, five separate stages:` … `All five stages green independently`

The cost is not only the wasted context. It is that the injection is
indistinguishable, at the point of receipt, from a genuinely relevant recall — so
it reads as *my memory system is telling me something true* rather than *my memory
system has no notion of relevant*.

## Mechanism

> **CORRECTION 2026-11-18 — the code block below is STALE. Do not file a bug
> against it.** It was written 2026-09-30. The score is **no longer discarded**:
> at `36bb947` (v0.10.0) `src/hooks.rs:660` `recall_at` returns
> `Option<Vec<ScoredHit>>`, `ScoredHit` (`:496-507`) carries `content`,
> `created_at` **and** `score: Option<f64>`, `recall_at_fmt` (`:583`) asks for
> `format` explicitly so the score is on the wire, and
> `the_score_should_survive_the_client_boundary` (`:1497`) pins it.
>
> **The open defect is narrower and still real:** `recall_section` (`:680-706`)
> reads `content` and `created_at` and **never reads `score`**. The only consumer
> of `score` is a `tracing::debug!` (`:640`) whose own comment says "Nothing
> branches on this and nothing is gated by it". So the evidence-gathering half
> of this document has **landed**; the policy half has not.
>
> Tracked as **W7** in
> `docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md`.

`src/hooks.rs:333-362`, the live path **as it was when this was written**.

```rust
fn recall_at(endpoint: &str, bank: &str, query: &str) -> Option<Vec<String>> {
    if query.trim().is_empty() { return None; }
    let body = json!({ "query": query, "budget": BUDGET }).to_string();
    …
    serde_json::from_str::<Vec<String>>(&resp.body).ok()   // ← score discarded
}

fn recall_section(bank: &str, hits: &[String]) -> String {
    let mut out = format!("\n### memory-wire recall — `{bank}`\n\n");
    for hit in hits.iter().take(MAX_LINES) {              // ← top-N, unconditionally
        out.push_str("- ");
        out.push_str(hit);
```

Three facts, each verified in the tree:

1. **The ranking is relative by design, and deliberately so.**
   `src/recall.rs:330-334` states it outright: the min-max normalised BM25 term
   means *"better BM25 than its competitors for this query"*, explicitly **not**
   *"confidently relevant"*, and it records that a hard confidence floor is
   forbidden by `AGENTS.md` §1 without a dev set. That reasoning is sound. Given
   a query with no good match, the best-of-a-bad-set is still returned — that is
   what a relative ranker does, and it is not a defect in the ranker.

2. **The evidence needed to judge is available and thrown away one line later.**
   The recall response is a scored structure; `src/api.rs:1903`
   (`recall_should_score_with_the_fused_rrf_value`) pins that the fused RRF score
   is on the wire. `recall_at` deserialises to `Vec<String>` — content only. The
   score is dropped **inside memory-wire's own code**, not by an external caller.

3. **The only gate is non-emptiness.** `recall_section` takes the first
   `MAX_LINES` hits unconditionally. Nothing between the server's ranking and the
   injected text asks whether the ranking was any good.

So the defect is not in the ranker and not in the caller. It is the seam inside
memory-wire: a **relative** ranking is consumed as though it were an **absolute**
one, and the scale that would distinguish the two is discarded before the
consumption.

## The same defect exists independently, twice

`operant`'s in-process provider reproduces it exactly, in the same shape, in a
different codebase:

- `crates/operant-core/src/memory_wire.rs:166-180` — `format_hits` iterates
  `&[ScoredMemory]` and emits only `hit.memory.content`. The struct is *named*
  `ScoredMemory`; the score is one field access away.
- `crates/operant-core/src/agent/run.rs:1749-1769` — the injected block is gated
  on `!is_empty()` and nothing else.

That second one is **not** the path that produced the observed injections — its
bank is `DEFAULT_BANK` and it emits `[memory]\n- …`, which does not match the
observed `### memory-wire recall — \`omp\`` header. Recording it because the same
wrong assumption was written twice independently, and a fix that lands in one
place will otherwise leave the other.

## What is NOT wrong, and must not be "fixed"

- **Do not add a score threshold to `recall`.** `AGENTS.md` §1 forbids acquiring
  one without a dev set, and the reasoning at `recall.rs:330-334` is correct. A
  hard-coded cut would be a fitted constant with no evaluation behind it — the
  exact failure mode this project has already paid for once in the fusion weights.
- **Do not treat "it returned something" as a bug in FTS5 or the BM25 arm.** Both
  are measured to work; the corpus is simply not about the query.

## Where the fix belongs

The evidence-gathering half is unambiguous and cheap:

- Stop discarding the score in `recall_at`; deserialise the scored shape that
  `api.rs` already returns, and pass it through to `recall_section`.
- Same change in operant's `format_hits`, which already receives `ScoredMemory`.

That change alone is a strict improvement — it makes the decision *possible*
without making it. It is also testable without a dev set: assert the score
survives the client boundary.

The policy half is **not** decidable without one, and should not be guessed:

- What score, below which, is "not worth injecting"? BM25 magnitude is
  per-query min-max normalised (`recall.rs:319-337`), so the top hit of a bad set
  scores `1.0` by construction. A cut on that field is close to meaningless
  **unless** the raw, un-normalised BM25 is also carried across the boundary.
- Whether the right cut is a score, a count, a budget, or a per-turn cost
  ceiling is a product decision with a token-cost trade-off.
- A dev set for this means labelled (query, relevant?) pairs over the real bank.
  Until one exists, the honest options are the current always-inject, or an
  explicit opt-out the user controls.

## Related, already fixed — the mirror image

0.5.1 fixed the *opposite* failure: a hook resolving to a bank with no memories
returned a bare `[]` and `doctor` reported a store nothing was serving. That was
**silent empty recall**. This is **silent irrelevant recall**. Both present to the
user as "the memory system did not help", and the 0.5.1 fix does nothing for this
one. Worth reading them together, because a system that has been hardened against
one half of a failure class should not be surprised by the other.

## Verification when fixed

- With a bank whose contents do not match the query at all, the hook emits
  **nothing** — not an empty section, not a best-of-bad-set.
- With a bank that does match, the previously-returned content is still returned.
- The score demonstrably survives `recall_at` — assert on the parsed value, not on
  the rendered string.
- Cost: report injected characters per turn before and after, on a real
  conversation, not a benchmark. The whole point is a user-visible behaviour.

## Status update (2026-10-08): mitigated by mechanism, not by threshold

Verification work on opencode/OMP ("Continue"-style prompts returned irrelevant
recall) shipped three changes. None adds a score cut — the conclusion reached
above (a threshold is a fitted constant) still stands.

- **Session-aware recall query** (extension v2 + opencode plugin): prompts
  under 160 chars borrow the previous turn's answer into the recall query, so
  what ranks is the work being continued instead of the literal word
  "Continue". Lives in `plugin/integrations/extension/memory-wire.ts`
  (`before_agent_start`) and `plugin/integrations/opencode-plugin/memory-wire.ts`
  (`experimental.chat.system.transform`). Mechanism, not a filter: nothing is
  dropped on a score.
- **Provenance label**: the injected block now opens with
  `(recalled by relevance from bank \`<id>\`, unverified)`, so a
  best-of-irrelevant set can no longer read as a verified one.
- **Retention discipline**: turns with no answer or a bare-acknowledgement
  answer are no longer retained, and `document_id = "turn-" + hash(prompt)`
  makes a repeated ask replace its row rather than pile up identical question
  texts. Verified live: bank `omp` gained `turn-*`-keyed rows carrying real
  answers, including an `asked: continue` row with substance.

**Residual, said out loud:** the borrowed-answer signal is in-process state; a
fresh harness process (headless `opencode run`, restarted TUI) recalls with the
prompt alone on its first turn. And the scoring gating question stays OPEN by
decision, not neglect.

# SECTION G-MEASURE

# FILE: research/audits/context-pack-2026-11-18-round2/measure_surface.sh (lines 1,65)

#!/usr/bin/env bash
# The authored-surface count this round's pack cites.
#
# Why this script exists: the protocol's rule 7 says count the AUTHORED surface
# only, and rule 8 says ship the measurement with the round. A line count that
# includes generated residue (target/, eval artifacts, vendored corpora) measures
# what the system produced by running, not what it is. This script excludes those
# and names the exclusion beside every number.
#
# Run from the repo root:  bash measure_surface.sh
set -euo pipefail

echo "# Authored surface — memory-wire at $(git rev-parse --short HEAD)"
echo
echo "## Excluded, by name"
echo "  target/            build output"
echo "  research/          audit rounds (this pack's own directory tree)"
echo "  _audit/, _audit_scratch/   read-only competitor checkouts, not our code"
echo "  eval/              datasets and generated result artifacts"
echo "  genome/, scripts/genome/    vendored third-party CLI"
echo "  tests/fixtures/    deliberate secret fixtures"
echo

echo "## src/ — lines of Rust per module (the binary)"
wc -l src/*.rs | sort -rn

echo
echo "## src/ — total"
find src -name '*.rs' | xargs cat | wc -l

echo
echo "## tests/ + examples/ — the harness surface"
wc -l tests/*.rs 2>/dev/null | tail -1
wc -l examples/*.rs 2>/dev/null | tail -1

echo
echo "## Dependency count (the default build must not gain one)"
grep -c '^\[dependencies\]' Cargo.toml >/dev/null
sed -n '/^\[dependencies\]/,/^\[/p' Cargo.toml | grep -cE '^[a-z0-9_-]+ *=' || true
echo "--- feature-gated (embed, OFF by default) ---"
sed -n '/^\[features\]/,/^\[/p' Cargo.toml | head -20

echo
echo "## Shared libraries the release binary links"
if [ -f target/release/memory-wire ]; then
  readelf -d target/release/memory-wire | grep NEEDED || echo "(static)"
else
  echo "no release build; run cargo build --release first"
fi

echo
echo "## Binary size"
ls -l target/release/memory-wire 2>/dev/null | awk '{print $5" bytes"}' || echo "no release build"

echo
echo "## Public MCP tool surface (the count the docs must not misstate)"
grep -oE 'pub const [A-Z_]+: &str = "memory_[a-z_]+"' src/mcp.rs | sed 's/.*= //'

echo
echo "## Public HTTP routes (the count the docs must not misstate)"
grep -oE '"/banks/[^"]*"' src/api.rs src/main.rs | sed 's/.*://' | sort -u

echo
echo "## Authored doc surface"
wc -l AGENTS.md README.md docs/*.md 2>/dev/null | tail -1

# FILE: research/audits/context-pack-2026-11-18-round2/SURFACE.md (lines 1,142)

# Authored surface — memory-wire at 555d1aa

## Excluded, by name
  target/            build output
  research/          audit rounds (this pack's own directory tree)
  _audit/, _audit_scratch/   read-only competitor checkouts, not our code
  eval/              datasets and generated result artifacts
  genome/, scripts/genome/    vendored third-party CLI
  tests/fixtures/    deliberate secret fixtures

## src/ — lines of Rust per module (the binary)
  34891 total
   5127 src/store.rs
   4106 src/api.rs
   2867 src/main.rs
   2841 src/connect.rs
   2429 src/recall.rs
   1873 src/mcp.rs
   1664 src/hooks.rs
   1132 src/daemon.rs
   1123 src/doctor.rs
   1103 src/connect_plugin.rs
    965 src/vector.rs
    948 src/cli_home.rs
    857 src/connect_codex.rs
    830 src/connect_ext.rs
    810 src/paths.rs
    655 src/render.rs
    649 src/seed.rs
    645 src/hygiene.rs
    613 src/update.rs
    484 src/mcp_http.rs
    355 src/capture.rs
    349 src/grounding.rs
    348 src/connect_opencode.rs
    324 src/connect_claude.rs
    306 src/connect_copilot.rs
    280 src/http.rs
    278 src/skills.rs
    276 src/sweep.rs
    251 src/guidelines.rs
    177 src/embed.rs
    175 src/memory.rs
     51 src/lib.rs

## src/ — total
34891

## tests/ + examples/ — the harness surface
 1307 total
 11648 total

## Dependency count (the default build must not gain one)
17
--- feature-gated (embed, OFF by default) ---
[features]
default = []
# Vendored, offline, dense-vector retrieval. Adds ~23 MB of model weights to the
# binary and the ONNX runtime that runs them; turns on nothing at runtime by
# itself, because `FusionWeights::vector` still ships at 0.0.
#
# OPT-IN AND NOT PART OF THE DEPLOYMENT. The shipped artifact is the default
# build; `install/get-memory-wire.sh` fetches a release tarball and the release
# is cut without this feature, so nothing that ships contains ONNX Runtime or the
# vendored weights. Build it with `cargo build --release --features embed`; it
# needs no network at build time beyond ort-sys's one binary download, and none
# at run time. See `docs/CONSISTENCY.md` §16 for the measurements.
embed = ["dep:fastembed"]

[dev-dependencies]

## Shared libraries the release binary links
 0x0000000000000001 (NEEDED)             Shared library: [libgcc_s.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libm.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]

## Binary size
10937128 bytes

## Public MCP tool surface (the count the docs must not misstate)
"memory_retain"
"memory_recall"
"memory_reflect"
"memory_bank_config_get"
"memory_supersede"
"memory_historian"

## Public HTTP routes (the count the docs must not misstate)
"/banks/{b}/config"
"/banks/b/config"
"/banks/{b}/memories"
"/banks/{b}/memories/nope"
"/banks/{b}/recall"
"/banks/b/recall"
"/banks/{b}/reflect"
"/banks/{b}/retain"
"/banks/b/retain"
"/banks/{b}/stats"
"/banks/c/memories"
"/banks/c/recall"
"/banks/c/retain"
"/banks/demo/config"
"/banks/demo/retain"
"/banks/d/recall"
"/banks/d/retain"
"/banks/{exists}/memories"
"/banks/{exists}/memories/{temp}"
"/banks/{exists}/retain"
"/banks/f2/recall"
"/banks/f2/retain"
"/banks/f/recall"
"/banks/f/retain"
"/banks/ghost/config"
"/banks/l/memories"
"/banks/l/memories/does-not-exist"
"/banks/l/memories/{id}"
"/banks/l/memories?limit=1"
"/banks/l/memories?limit=1&offset=1"
"/banks/l/memories?limit=99999"
"/banks/l/retain"
"/banks/l/stats"
"/banks/pb/memories"
"/banks/pb/memories?limit=10000"
"/banks/r/reflect"
"/banks/r/retain"
"/banks/s/recall"
"/banks/s/retain"
"/banks/t/recall"
"/banks/t/retain"
"/banks/u/memories"
"/banks/u/retain"
id/config"
id/hygiene"
id/memories"
id/recall"
id/reflect"
id/retain"
id/stats"
mid"

## Authored doc surface
 12021 total
