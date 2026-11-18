# CONTEXT PACK — context-pack-2026-11-18-round1

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

# FILE: docs/handoff/STATE.md (lines 13,108)

## 1. What is in the working tree (post-release residue)

  **2026-11-18 — read this first if you are picking the upgrade up.** HEAD is
  `36bb947`, **v0.10.0**, not the `c919f13`/v0.8.2 this page was written at;
  the tree has moved a long way since. Three external upgrade proposals arrived
  (Claude hierarchical memory + Dreaming, a red-team of it, and an `operant` ×
  `memory-wire` blueprint) and **all three describe a tree that does not exist**:
  they claim 54 MCP tools where there are **4**, they ask for `WAL` /
  `busy_timeout` / loopback bind / `spawn_blocking` which **shipped in v0.7.0**,
  and they promise a `single-session-preference` R@1 lift from a hierarchy that
  **structurally cannot produce one** (LongMemEval-S is per-question fresh index,
  session-as-document — there is no HotIndex and no global tier to score).

  The grounded replacement is **`docs/audit/2026-11-18-surface-audit.md`**
  (findings F1–F10, every one marked verified or asserted) and
  **`docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md`** (W0–W6).
  TODO rows 21–28 are the register. **Nothing is implemented yet.**

  Three facts from that audit outrank everything else on this page:

  - **The four "built but inert" ranking mechanisms are not inert-pending, they
    are measured and closed** (`eval/LOCOMO_LEVERS.md`, `CLOSED.md`): H1a
    `keyword_scope` R@1 +3.0pp but gate failed; H1b `overlap_scope` multi-hop
    R@1 **−11.2pp**; H1c `bm25_magnitude` a wash; H3 recency temporal R@1
    **−33.4pp**. Do not re-propose them (AGENTS §8).
  - **`cargo test --locked` is currently RED on this machine**, not from a
    regression: `connect::tests::golden_install_shapes` uses the real `PATH`
    probe and `copilot` is not installed. Verified pre-existing by stashing and
    re-running on the pristine tree. TODO #21 fixes it; until then "the gate is
    green" is false here.
  - **No item in the new plan changes `FusionWeights::SHIPPED`**, and no item is
    selected on LongMemEval. The plan ships zero ranking changes.

  The §45 records pass and the navigation pass are **committed** (`3ac422c`,
  `c919f13`); the tree was clean at `c919f13`. It now holds the **§6 cycle
  pass, uncommitted by instruction**: the outline (`RND_PLAN.md` §6, H12
  pre-registered) plus its execution — W1 (BEAM pair-unit labels + harness:
  `eval/beam_label.py`, `eval/beam_labels_100k.json`, `examples/beam.rs`,
  artifacts `eval/BEAM.md` + `eval/results_beam.json`), W2 (frozen battery:
  `eval/relevance/freeze_battery.py`, grown `cases.jsonl`, artifact
  `BATTERY-2026-10-16-frozen.txt`), `docs/CONSISTENCY.md` §47–§48,
  `docs/handoff/README.md`, `docs/handoff/STATE.md`, `TODO.md` (rows 6, 9, 20).
  Untracked data under `eval/data/` is gitignored by design. W3's adapter fix
  lives in the pinned clone, not here.

  Cycle state (§6, **closed 2026-10-17 — closing record `docs/handoff/CLOSING-2026-10-17.md`**): **W1 done** — the BEAM pool-scale
  overlap re-test answered (`eval/BEAM.md`, §47): the stream is **not
  deleteable** (0.25 > 0.00 on every ordering metric; 0.00 additionally loses
  R@pool 100→95.5% — the overlap stream carries coverage below BM25's 50-row
  cut at 94–196-doc banks), **but the unfitted 1.00 leads 0.25 on the same
  corpus** (R@5 67.1 vs 63.6, opposite sign to LoCoMo's +1.4pp) — the weight
  stays user-gated. **W2 done** — the battery is frozen (sha256-pinned
  snapshot → deterministic 1,413-row bank) and grown 10→21 (gold 16/17,
  junk 0/9, D-case passes, H8 re-confirmed); precision levers registrable
  from here — none registered. **W3 done — H12** (`eval/omnimevaleval/
  memory-wire-mw_h12_dated_medium/`, §46): the dated instrument reads
  Dynamic Update 0.2056→**0.4056** (+20.0pp), overall 0.6129→**0.7433**,
  Memory Boundary control holding at −2.5pp and context only +12% — the
  H10 headline measured a reader blind to time; both numbers stand as two
  instrument versions. **The §6 cycle is complete**: every workstream
  terminal, records current, 0 test consultations added all cycle. Blocked:
  Track 2/agent-memory-bench (agent-model budget). No ranking lever is open;
  the fitted `overlap 0.25` is now *replicated on LoCoMo, inverted on BEAM* —
  an honest split, the user's call.

That release batch (feature `04123f7`, bump `a272196`) carried everything the old
list below named — the full R&D record set, the product code, and every new
harness and artifact — into the v0.8.0 tag. The pre-release residue list is
retired here rather than kept current:

  Modified (all now **in `v0.8.0`**): `AGENTS.md INSTALL_FOR_AGENTS.md README.md TODO.md
  docs/CONSISTENCY.md docs/INTEGRATION_GAPS.md docs/INTEGRATION_PLAN.md eval/LOCOMO.md
  eval/README.md eval/RESULTS.md eval/download.sh eval/locomo_dev.rs eval/results_locomo.json
  examples/answer_quality.rs examples/bench_footprint.rs examples/locomo.rs
  examples/longmemeval.rs plugin/skills/memory-wire/SKILL.md src/api.rs src/connect.rs
  src/connect_ext.rs src/connect_opencode.rs src/doctor.rs src/hooks.rs src/main.rs
  src/mcp.rs src/paths.rs src/store.rs src/sweep.rs`.
 Untracked (all now **in `v0.8.0`**): `docs/AROUTE_BUGS.md docs/BENCHMARK_LANDSCAPE.md
  docs/handoff/ eval/amb/ eval/baseline/ eval/llm_shim.py eval/LOCOMO_TURNS.md
  eval/results_locomo_turns.json examples/locomo_ranks.rs eval/LOCOMO_RANKS.md
  eval/results_locomo_ranks.json examples/locomo_levers.rs eval/LOCOMO_LEVERS.md
  eval/results_locomo_levers.json eval/LOCOMO_WINDOWS.md eval/results_locomo_windows.json
  eval/h4_subset.json eval/amb/h4_build_subset.py src/connect_claude.rs src/connect_copilot.rs
  eval/SELECTION_VECTOR_AXIS_ALL10.md eval/results_selection_vector_all10.json src/update.rs`.

`eval/amb/` now also carries the run artifacts `PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz`
and `h4_locomo_rag_topk{3,5,10}.json.gz`; `eval/baseline/` carries
`2026-10-14-v0.7.0-dated-answer-quality/` (H5) and `2026-10-14-bench-concurrency/` (H7).

Check with `git status --short`; `git diff --stat` shows size. The gate passed on this tree
on **2026-10-15, after the H1d measurement** (release build + all examples, **589** tests
across 9 suites, clippy `-D warnings --all-features` clean — the embed code path included,
0 doc warnings). The embed build ran in an isolated `CARGO_TARGET_DIR`; the default
binary is untouched (`DT_NEEDED` 3). **Re-run the whole gate (AGENTS §2) before
reporting anything as verified** — the tree keeps moving.


# FILE: docs/handoff/STATE.md (lines 138,196)

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


# SECTION B-LAW

# FILE: AGENTS.md (lines 203,263)

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


# FILE: AGENTS.md (lines 282,312)

## 8. Rejected work stays rejected

Levers already measured and removed, with the measurement that killed each, are
listed in `docs/RERANKING_PLAN.md` §3 and `docs/PERFORMANCE_PLAN.md`. Do not
re-propose one without new evidence. If you find yourself re-litigating a
rejected lever, the answer is in those documents, not in your intuition.

Currently closed, each with its measurement recorded: vectors as a default,
a cross-encoder, a graph stream, consolidation and extraction, Porter stemming,
IDF in the overlap stream, a distinct-term-coverage stream, page-cache and
mmap tuning, and the token-overlap stream's fitted weight.

## 9. Interface rules

- **The hook contract is never-fail.** A hook that cannot reach the server must
  exit 0 silently: a memory server that is down must not put an error in front
  of a model. See the module doc at the top of `src/hooks.rs`. Silent swallowing
  is *required* there and is a finding everywhere else.
- **The HTTP error contract is frozen.** Unknown enum value → `400 invalid
  content`; wrong type → `422` from the deserializer; duplicate `document_id`
  under `append` → `409`; a pinned-bank mismatch → refusal naming the pin.
  Clients depend on these. Changing one is an API break.
- **Write config defensively.** A malformed host config is refused, not
  overwritten. A moved binary's hook is replaced, not duplicated. Every write
  is atomic with re-read verification.
- **Do not hand-edit an installed agent host's files** to make something work.
  `connect` and `connect --uninstall` own those entries.
- Default JSON responses are part of the contract. Pagination totals ride in the
  `X-Total-Count` header, and `?truncate=` / `?format=toon` are opt-in, because
  changing the body shape breaks every existing client.


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

# FILE: eval/LOCOMO_LEVERS.md (lines 1,76)

# LoCoMo lever A/B — H1a, H1b, H1c, H3 (dev set, single pre-registered values)

## Provenance

| | |
|---|---|
| Commit under test | `84580d577cb3d0299da37ca9636081b1f3d30a2d` |
| Command | `cargo run --release --example locomo_levers -- --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json --out-md eval/LOCOMO_LEVERS.md --out-json eval/results_locomo_levers.json` |
| Run | 2026-10-14, from `release`, on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| Load at start / at end | `37.62 31.92 28.57 16/6112 1428829` / `35.49 31.87 28.64 24/6099 1431741` |
| Dataset | LoCoMo (Snap Research) via the canonical distribution `vectorize-io/agent-memory-benchmark`, `data/locomo/locomo10/`, pinned at upstream commit `decbb07f4f9899deac28a76293564cf263872652` and sha256-verified by `eval/download.sh`; corpus is date-aware since H0 (each memory carries its own `timestamp`) |
| **LLM involvement** | **none.** Retrieval metrics only, no network call. |
| Queries | 1531 evaluated (9 dropped, empty gold set) |

The shipped arm must equal `eval/LOCOMO.md`'s `0.25` row; the harness is deterministic and there is no latency column, so the load row is for completeness only.

## Aggregate, every lever vs shipped

| lever | R@1 | R@5 | R@10 | R@20 | recall_all@10 | NDCG@10 | MRR | R@pool | Δall10 | all10 up/down | lists unchanged |
|---|---|---|---|---|---|---|---|---|---|---|---|
| shipped | 60.4% | 86.0% | 94.2% | 98.0% | 83.0% | 73.6% | 71.8% | 100.0% | +0.0pp | 0/0 | 0/1531 |
| h1a keyword_scope ExactAndPrefix | 63.4% | 87.7% | 93.9% | 98.6% | 83.5% | 75.3% | 74.0% | 100.0% | +0.5pp | 34/26 | 42/1531 |
| h1b overlap_scope BestSentence | 57.0% | 86.8% | 93.6% | 98.1% | 82.6% | 72.5% | 69.9% | 100.0% | -0.3pp | 14/19 | 0/1531 |
| h1c bm25_magnitude 1.0 | 60.4% | 86.2% | 94.1% | 98.0% | 82.8% | 73.6% | 71.7% | 100.0% | -0.2pp | 4/7 | 249/1531 |
| h3 recency 1.0 TemporalQueriesOnly | 48.9% | 78.5% | 92.2% | 98.0% | 80.4% | 65.6% | 62.2% | 100.0% | -2.5pp | 8/47 | 1025/1531 |

## recall_all@10 per category

| lever | multi-hop | open-domain | single-hop | temporal | overall (n 1531) |
|---|---|---|
| shipped | 56.2% | 97.0% | 43.1% | 88.4% | 83.0% |
| h1a keyword_scope ExactAndPrefix | 53.9% (-2.2) | 97.0% (+0.0) | 46.6% (+3.6) | 88.4% (+0.0) | 83.5% |
| h1b overlap_scope BestSentence | 56.2% (+0.0) | 96.6% (-0.5) | 43.8% (+0.7) | 87.5% (-0.9) | 82.6% |
| h1c bm25_magnitude 1.0 | 57.3% (+1.1) | 96.9% (-0.1) | 42.3% (-0.7) | 88.1% (-0.3) | 82.8% |
| h3 recency 1.0 TemporalQueriesOnly | 56.2% (+0.0) | 95.8% (-1.2) | 41.6% (-1.4) | 80.6% (-7.8) | 80.4% |

## R@1 per category

| lever | multi-hop | open-domain | single-hop | temporal | overall (n 1531) |
|---|---|---|
| shipped | 37.1% | 67.8% | 49.5% | 57.2% | 60.4% |
| h1a keyword_scope ExactAndPrefix | 38.2% (+1.1) | 71.0% (+3.2) | 50.2% (+0.7) | 61.9% (+4.7) | 63.4% |
| h1b overlap_scope BestSentence | 25.8% (-11.2) | 66.5% (-1.3) | 39.1% (-10.3) | 56.2% (-0.9) | 57.0% |
| h1c bm25_magnitude 1.0 | 33.7% (-3.4) | 68.5% (+0.7) | 47.0% (-2.5) | 58.4% (+1.2) | 60.4% |
| h3 recency 1.0 TemporalQueriesOnly | 34.8% (-2.2) | 60.5% (-7.3) | 47.0% (-2.5) | 23.8% (-33.4) | 48.9% |

## R@5 per category

| lever | multi-hop | open-domain | single-hop | temporal | overall (n 1531) |
|---|---|---|
| shipped | 67.4% | 91.8% | 79.4% | 81.6% | 86.0% |
| h1a keyword_scope ExactAndPrefix | 69.7% (+2.2) | 93.7% (+1.9) | 79.0% (-0.4) | 84.4% (+2.8) | 87.7% |
| h1b overlap_scope BestSentence | 66.3% (-1.1) | 92.7% (+1.0) | 80.4% (+1.1) | 82.5% (+0.9) | 86.8% |
| h1c bm25_magnitude 1.0 | 66.3% (-1.1) | 91.7% (-0.1) | 79.7% (+0.4) | 82.8% (+1.2) | 86.2% |
| h3 recency 1.0 TemporalQueriesOnly | 66.3% (-1.1) | 87.9% (-3.9) | 76.2% (-3.2) | 59.4% (-22.2) | 78.5% |

## NDCG@10 per category

| lever | multi-hop | open-domain | single-hop | temporal | overall (n 1531) |
|---|---|---|
| shipped | 50.0% | 82.6% | 54.9% | 72.8% | 73.6% |
| h1a keyword_scope ExactAndPrefix | 50.0% (+0.0) | 84.5% (+1.9) | 55.4% (+0.4) | 75.6% (+2.7) | 75.3% |
| h1b overlap_scope BestSentence | 45.4% (-4.5) | 82.1% (-0.5) | 52.5% (-2.4) | 72.1% (-0.7) | 72.5% |
| h1c bm25_magnitude 1.0 | 49.1% (-0.8) | 82.8% (+0.1) | 54.5% (-0.5) | 73.3% (+0.4) | 73.6% |
| h3 recency 1.0 TemporalQueriesOnly | 49.0% (-1.0) | 77.8% (-4.8) | 53.1% (-1.9) | 49.2% (-23.7) | 65.6% |

## Gates (pre-registered, derived from the tables above)

- **h1a keyword_scope ExactAndPrefix**: **FAIL** — `recall_all@10` +0.5pp (up 34/down 26), R@20 +0.6pp, category regressions: single-hop R@5 79.4->79.0
- **h1b overlap_scope BestSentence**: **FAIL** — `recall_all@10` -0.3pp (up 14/down 19), R@20 +0.1pp, category regressions: multi-hop R@1 37.1->25.8; multi-hop R@5 67.4->66.3; multi-hop NDCG@10 50.0->45.4; open-domain R@1 67.8->66.5; open-domain NDCG@10 82.6->82.1; single-hop R@1 49.5->39.1; single-hop NDCG@10 54.9->52.5; temporal R@1 57.2->56.2; temporal NDCG@10 72.8->72.1
- **h1c bm25_magnitude 1.0**: **FAIL** — `recall_all@10` -0.2pp (up 4/down 7), R@20 +0.0pp, category regressions: multi-hop R@1 37.1->33.7; multi-hop R@5 67.4->66.3; multi-hop NDCG@10 50.0->49.1; open-domain R@5 91.8->91.7; single-hop R@1 49.5->47.0; single-hop NDCG@10 54.9->54.5
- **h3 recency 1.0 TemporalQueriesOnly**: **FAIL** — temporal `recall_all@10` -7.8pp, temporal R@1 -33.4pp; detector fired on 506 queries (category temporal: 287, spillover: multi-hop 9, open-domain 179, single-hop 31); detector-negative lists identical 1025/1025 (asserted at run time)

A LoCoMo null closes the lever on the dev set (`docs/handoff/CLOSED.md`); a pass earns a single LongMemEval measurement, counted in `docs/CONSISTENCY.md`.

Per-question data (shipped rows + retrieved lists): `eval/results_locomo_levers.json`.

# FILE: README.md (lines 29,123)

## Benchmarks

Official LongMemEval-S, retrieval-only (no LLM judge), same methodology as the
competitors' harnesses — per-question fresh index, session-as-document, question-text
query. Regenerate: `cargo run --example longmemeval` → `eval/RESULTS.md`.

| System | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR |
|---|---|---|---|---|---|---|
| **memory-wire — shipped config** | **83.8%** | **97.2%** | **98.6%** | **99.6%** | **88.2%** | **89.2%** |
| memory-wire — unfitted (equal weight) | not measured | 93.0% | 97.4% | 99.6% | 83.5% | 83.9% |
| agentmemory BM25-only | not reported | 86.2% | 94.6% | 98.6% | 73.0% | 71.5% |
| agentmemory BM25+Vector | not reported | 95.2% | 98.6% | 99.4% | 87.9% | 88.2% |

**The first row is measured at a weight that was fitted to this set; the second is
the one to trust.** `overlap: 0.25` was chosen by sweeping 46 configurations
against these same 500 questions. Re-measured on 1,531 LoCoMo queries that never
chose it, the weight is worth **+1.4pp R@5, not +4.2pp**
([`eval/LOCOMO.md`](eval/LOCOMO.md)) — the direction replicated, the magnitude did
not, which is what the maximum of 46 noisy draws looks like. A third re-measure
on 313 pool-scale BEAM queries ([`eval/BEAM.md`](eval/BEAM.md), turns-as-documents,
`docs/CONSISTENCY.md` §47) moved the other way — equal weight leads the shipped
value by +3.5pp R@5 there — so the honest out-of-sample estimate of this line is
"the direction holds on one dev corpus and inverts on another, neither dominant".
Adding LoCoMo's measured delta to the clean baseline would put the estimate at
**≈94.4% R@5: behind agentmemory's 95.2% hybrid, not 2.0pp ahead of it** — but
BEAM says that adjustment is corpus-dependent, so the weight stays the shipped
default and changing `FusionWeights::SHIPPED` is a user decision, not ours.

So the honest position is **decisively ahead on footprint, roughly level to
slightly behind on retrieval.** Both rows ship because publishing only the first
would mean publishing only the fitted number. `eval/LOCOMO.md` and
`docs/CONSISTENCY.md` §14 have the working.

**R@1 is the column that decides whether the answer is right, so it is the one to
read first.** An agent that reads the first memory it gets back is correct 83.8%
of the time; one that reads five is correct 97.2% of the time. The two figures are
the same 500 questions, the same build and the same harness method
(`eval/ORACLE_RERANK.md`, which re-derives the pool `recall` builds and verifies it
row-for-row against what `recall` serves). R@1 is **not** a coverage claim: it is
the shipped ranking's own first-hit rate, and the 100.0% quoted beside it in that
artifact is an *oracle* — a perfect reranker over the pool this build already
assembles, computed with the gold labels. That oracle is 500/500, so the entire
+16.2pp at R@1 is ordering, not matching, and it is the largest single number in
the system. Neither competitor publishes an R@1, so the column carries one number
rather than a comparison — that is the honest shape of it.

**R@1 is also the clearest statement of where the remaining work is.** Overall
83.8%, but `single-session-preference` ranks the right session first **43.3%** of
the time while its R@20 is **100.0%** — an ordering collapse, invisible in every
K≥5 column. Four mechanisms aimed at it (a BM25-magnitude tie-breaker the fusion
was discarding, a recency stream, sentence-level overlap, and a temporal-query
classifier) are built, unit-tested and **deliberately inert**: every weight sits at
a default that leaves the shipped ranking byte-identical. They are not a feature
until a weight is chosen on the dev set — see `docs/PERFORMANCE_PLAN.md`.

**The two streams are not equally weighted, and that is a measured result.** At
equal weight the fusion scored 93.0 / 97.4 / 99.6 / 83.5 / 83.9 — behind
agentmemory's hybrid on R@5 and 4.4pp behind on NDCG@10. A 500-question sweep of
the weights (`eval/SWEEP_FUSION.md`, `docs/NEXT_ITERATION.md` Phase E1) found the
token-overlap stream was a *net negative* as a co-equal voter, and moved its
weight to 0.25: **+4.2pp R@5, +4.8pp NDCG@10, R@20 unchanged.** The gain is
concentrated exactly where the deficit was — `single-session-preference` R@5
56.7% → 86.7% and `single-session-assistant` 87.5% → 100.0%, with no category
regressing. The cost is stated in full below rather than buried. **The +4.2pp is a
fitted number and did not hold its size off this dataset** — on LoCoMo it is
+1.4pp. The table above reports both the clean and the fitted configuration
rather than only the flattering one.

**The weight has not been shown to earn its place, and the comparison that matters
was not run until now.** `eval/SWEEP_FUSION.md` section D measures each stream
alone: **BM25 on its own scores 97.0 / 99.0 / 99.6 / 89.9 / 91.4** at
R@5 / R@10 / R@20 / NDCG@10 / MRR. Against the shipped fusion that is
**R@10 +0.4, NDCG@10 +1.7, MRR +2.2, and R@5 −0.2** — three metrics better and
one worse. So the token-overlap stream buys 0.2pp of R@5 and costs NDCG@10 and
MRR against deleting it, and the independent LoCoMo set separately found
`overlap: 0.00` ties `0.25` on R@1 and NDCG@10. Two datasets point the same way.
The counter-argument is real and is why the stream is still here: LoCoMo banks
hold 19–32 documents against LongMemEval's ~48 and `tests/scale.rs`'s 5,000, so
the dev set under-measures the pool size at which a second lexical voter pays
(`docs/CONSISTENCY.md` 14.2). **This is a live, unmade decision, not a settled
result** — and the honest reading is that the burden of proof has moved.

**Two later attempts to make that weight unnecessary both failed, and one of them
failed for an instructive reason** (`docs/NEXT_ITERATION.md` Phases E3–E4).
Weighting the overlap stream by IDF did exactly what E1's diagnosis predicted —
at equal weight it scores 96.4 / 88.6 where the raw stream scores 93.0 / 83.5 —
but at the shipped 0.25 it costs 0.4pp R@5 for +1.7pp NDCG@10, so it is not
shiped. A third distinct-term-coverage stream is *algebraically identical* to
raising the overlap weight: for any query with no repeated token the two rank
identically, and RRF is linear in the weights, so `coverage: w` ≡
`overlap: 0.25 + w`. Both levers were measured, rejected, and then **removed from
the tree** — they were dormant config nobody would ever turn on. The findings live
in `docs/NEXT_ITERATION.md`; the sweep artifact that produced them is
`eval/SWEEP_FUSION.md`.


# FILE: README.md (lines 124,188)

## What an answer actually costs to get right

`recall_any@K` is a proxy. The number a user feels is whether the agent answers
correctly, so that is measured directly (`eval/ANSWER_QUALITY.md`): the same 25
LongMemEval questions, answered by an LLM from the retrieved context and graded
by an LLM, against a **closed-book control** that gets the question and no memory
at all.

| arm | accuracy | scored | prompt tokens |
|---|---|---|---|
| retrieval-conditioned | **60.0%** | 25 | 315,343 |
| closed-book control | **8.3%** | 24 | 150,372 |
| **delta** | **+51.7pp** | | |

Memory is doing real work, and the control is what makes that claim falsifiable —
if the conditioned arm were at parity with the closed-book one, every ranking
number above would be measuring nothing.

**The single largest win in this project was a budget, not a ranking change.**
`DEFAULT_RECALL_BUDGET` was 2,000 tokens against a **2,626-token median
LongMemEval session** — so the budget could not return a median memory. The trim
truncates the top hit and skips everything below it. Raising it to 8,000
(3× the median, so three sessions fit whole) moved answer accuracy
**40.0% → 60.0%**.

The mechanism is visible in the artifact, and it is the reason this is a
mechanism and not a coincidence:

| on this slice | before (2,000) | after (8,000) |
|---|---|---|
| gold session present in the served context | 92.0% | **92.0%** — unchanged |
| … answer judged **wrong** despite gold served | **52.0%** | **32.0%** |
| … answer judged correct | 40.0% | 60.0% |

Retrieval did not improve; the answerer stopped being starved. Note that
`eval/RESULTS.md` **cannot see this defect at all** — its harness passes an
explicit 100,000-token budget, so the default is structurally invisible to it.
That is why weeks of retrieval work left a 2× gap between what recall found and
what the consumer received. `tests/recall_budget.rs` now asserts a gold row
survives the trim.

Caveats, stated rather than buried: n=25 with 2 row errors, so the closed-book
cell is provisional under the harness's own rule; the judge is a local free-tier
model, not LongMemEval's official grader; and this is a 25-question sample, not
a benchmark. It sizes an effect. It does not establish a rate.

Competitor rows: agentmemory's `benchmark/LONGMEMEVAL.md` (same metric, same 500
questions, verified in-audit). Hindsight has **no comparable retrieval number**:
its one published LoCoMo result, 92.0%, is LLM-judged *answer* accuracy from a
`rag` run with two Gemini calls in the loop, and `git grep -iE "ndcg|recall@|
pool_size"` finds nothing in the benchmark repo — so the per-pool-size recall
table in their reranker blog is not reproducible from either repo. Their retrieval
stack is 4-stream TEMPR + cross-encoder rerank behind a 0.8–1.0 GB idle RSS
(see below).

| Suite | memory-wire | Competitors |
|---|---|---|
| coding-life, 15 labeled queries (`eval/CODING_LIFE.md`) | hit-rate **100%**, R@5 96.7%, p50 **260–620 µs** across release runs of this binary | grep baseline 96.7% (same harness); agentmemory publishes no score |
| Scale sweep 240→10k (`eval/SCALE_SWEEP.md`) | search p50 0.99→6.03 ms, token savings ≥95.7% | agentmemory: 0.1→22.8ms BM25, heap 6→316MB, savings to 100% |

Both latency ranges move 2–3x with the machine's load. `eval/CODING_LIFE.md` and
`eval/SCALE_SWEEP.md` each say so in their own text, and each artifact's
provenance line carries the load-relevant date, profile and machine. Quote them
as ranges, never as a single run.


# SECTION D-ASK

# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,163)

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

`src/hooks.rs:333-362`, the live path.

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

# SECTION E-ANALYSIS

# FILE: docs/audit/2026-11-18-surface-audit.md (lines 19,153)

## Scope

| | |
|---|---|
| Repo | `agentic-utility/memory/memory-wire`, branch `main` |
| Commit | **`36bb9474f7733227676b1e79a307b9cf195014a3`** (v0.10.0) — *not* the `b478d15` the proposals pin |
| Tree | clean except `M scripts/genome` (a vendored CLI refresh, pre-existing) |
| Gate state | `cargo test --locked`: **345 passed, 1 failed** in the bin suite (`connect::tests::golden_install_shapes`, environmental — see F9); all 4 integration suites pass (18 tests). **This failure predates the audit and is not a regression** — verified by stashing the working tree and re-running on the pristine tree. |
| Machine | AMD Ryzen 9 5900X, 24 logical CPUs, 47 GiB RAM, load 4.33 at test time |

Nothing was tuned, no benchmark was run, no ranking weight was touched, and
**no LongMemEval consultation was consumed** by this audit.

---

## Part I — The surface, as it actually is

### 1. It is much bigger than the proposals describe, and much smaller

| axis | proposals claimed | verified |
|---|---|---|
| MCP tools | **54** | **4** — `memory_retain`, `memory_recall`, `memory_reflect`, `memory_bank_config_get` (`src/mcp.rs`, `RETAIN`/`RECALL`/`REFLECT`/`CONFIG_GET` consts) |
| Rust LOC | ~3k core | **31,480** across 28 modules |
| tests | unmeasured | **602** `#[test]` functions |
| version | `b478d15` | **v0.10.0**, commit `36bb947` |

The "54 MCP tools" figure appears in all three proposals and in *none* of the
repo. `README.md:656` says "the same four tools are served two ways";
`README.md:1024` says "the MCP server (`rmcp`, 4 tools)". The proposals
apparently counted the portfolio's tool surface, not this project's.

**Why this matters and is not pedantry:** two proposals built a whole pillar
("audit & prune the 54 tools", "do not inflate past 54") on a number that is
off by 13×. The real constraint is the opposite one: **4 tools is already
lean, and the risk is not bloat but that the four cannot express the new
model.** A plan that starts by "auditing 54 tools" spends its first week on a
non-problem.

### 2. Most of what Pillar 1–2 of the proposals asks for is already built

| proposal asks for | verified state |
|---|---|
| `WAL` + `busy_timeout` + `BEGIN IMMEDIATE` | **done** — `src/store.rs:1091-1094`: `PRAGMA journal_mode=WAL; foreign_keys=ON; busy_timeout=5000; synchronous=NORMAL`. `synchronous=NORMAL` is a *documented, measured* durability trade (`src/store.rs:1043-1055`). |
| serialize multi-process writes | **done, structurally** — one `Mutex<Connection>` writer + a 4-connection WAL **read pool** (`src/store.rs:784-860`), not one mutex for everything. 16-client soak: 0 `SQLITE_BUSY`, 0 panics (`eval/SOAK.md`). |
| keep blocking SQLite off the tokio workers | **done** — `src/api.rs:1357` `spawn_blocking`; every route and every MCP `call_tool` goes through `api::blocking`. |
| bind loopback by default | **done** — `paths::DEFAULT_ADDR = "127.0.0.1:8888"`, the single literal; both clap defaults read it; a test asserts it. A non-loopback bind emits a **documented, once-per-process warning** naming that there is no auth (`src/main.rs:645-672`). |
| zero daemon / low RSS | **done and measured** — 8.8 MiB binary, 10.7 MiB idle RSS, `DT_NEEDED`=3, 0 background processes until `daemon start`. |
| per-bank `summary` capsules | **done** — `format: "summary"` (`api.rs:205-216`, `summary_content`), per-bank adoption knob plus `MEMORY_WIRE_FORMAT` (`src/hooks.rs:533-574`), frozen-battery measured at −41–43% bytes at byte-identical ranking. |
| occurrence dates in-band | **done** — `[YYYY-MM-DD]` prefixing, HaluMem Dynamic Update 0.2056→0.4056 (`CONSISTENCY.md` §46). |
| `redact` retroactively across history | **absent** — redaction is write-time only (`capture::redact_pii`); there is **no** version table to scrub. |
| `scope`/`domain`/`aspect`/`lifecycle`/`trust_level` | **absent** — no such column exists anywhere in `src/`. |
| `HotIndex` / startup capsule ≤200 lines | **absent**. |
| read-only vs read-write mounts | **absent** — bank isolation is by *name* only. |

So: **the security/concurrency half of the proposals (their Batch 1) is
already shipped**, and the dimensional-schema half (Batch 2) is genuinely
missing. A plan that "starts with WAL and auth" would be re-doing v0.7.0.

### 3. The ranking half: the proposals' central idea was already tried and killed

This is the most important finding in the audit.

Two proposals independently argued that `single-session-preference` sits at
**43.3% R@1 vs 100.0% R@20** because a 2-sentence preference is diluted inside
a 2,626-token session, and that `memory-wire`'s **four "built but inert"
mechanisms** (sentence-level overlap, BM25-magnitude tiebreak, recency stream,
temporal-query classifier) are the exact tools for it.

**[verified]** Every one of those four was pre-registered as a hypothesis,
measured on the LoCoMo dev set (n=1,531), and **closed**:

| mechanism | measurement | verdict |
|---|---|---|
| `keyword_scope: ExactAndPrefix` (H1a) | R@1 60.4→**63.4%**, MRR +2.2pp, NDCG +1.7pp | **gate failed** — `recall_all@10` +0.5pp < the +1pp gate; single-hop R@5 −0.4 |
| `overlap_scope: BestSentence` (H1b) | R@1 60.4→**57.0%**; multi-hop R@1 **−11.2pp**; single-hop **−10.3pp** | **gate failed** |
| `bm25_magnitude: 1.0` (H1c) | R@1 60.4→**60.4%**; `recall_all@10` **−0.2pp** (4 up / 7 down); multi-hop R@1 −3.4pp | **gate failed**, re-closed 2026-10-14 after a re-open |
| `recency 1.0 TemporalQueriesOnly` (H3) | temporal `recall_all@10` **−7.8pp**, temporal R@1 **−33.4pp**; detector-negative lists bit-identical 1025/1025 | **falsified** |

Source: `eval/LOCOMO_LEVERS.md` (generator `examples/locomo_levers.rs`,
artifact `eval/results_locomo_levers.json`), registered in
`docs/handoff/CLOSED.md` as closed.

**And the proposals' proposed fix has a second, worse problem.** Both sketch
`RRF_lexical + w_tier · TierPrior + …`. RRF terms are `1/(60+rank)` ≈ 0.005–0.016.
An additive structural constant of even 1.0 therefore swamps the entire lexical
signal on non-preference queries — which is exactly what `docs/handoff/RND_PLAN.md`
and the third proposal's own red-team caught. A structural prior must enter
**as a rank voter inside the RRF**, or not at all.

**Also [verified]:** the third proposal's core claim about the benchmark is
right, and it is the one thing that would have wasted the most time.
LongMemEval-S runs *per-question fresh index, session-as-document,
question-text query*. There is no pre-existing `HotIndex` and no cross-session
`global` tier during scoring. Therefore:

> A `tier` column and a `HotIndex` table move LongMemEval-S
> `single-session-preference` R@1 by **0.0pp**. They cannot move it: the
> benchmark has no such rows.

Only changes to **what `retain` writes** and **what `recall` ranks on the
session-as-document corpus** can move that number. Everything else in the
hierarchy is real product value with **zero** effect on the one metric the
proposals promise to lift.

### 4. What the proposals get right, and it is a lot

- **The 43.3% / +16.2pp ordering gap is the largest single number in the
  system** and is correctly identified as the target (`README.md:62-76`).
- **Uniform-default trap** is real: adding `kind`/`aspect` columns that
  `retain` never populates is dormant config, which this repo already has a
  policy against (`RND_PLAN.md` H11 was closed *because* the counter would
  read 1 everywhere).
- **`operant` ↔ `memory-wire` boundary** (third proposal) is correctly drawn:
  Dreaming orchestration belongs to the caller; the substrate exposes
  deterministic primitives. This repo has no LLM in the default build and
  must keep it that way.
- **`connect` does not deploy `SKILL.md`** — **[verified]**, `grep -c SKILL`
  returns 0 for `src/connect.rs`, `src/main.rs`, `src/paths.rs`, and no
  `skills/` path appears anywhere in `src/`. This is a real, small, honest gap.
- **Relative-date and contradiction hygiene has no home**: **[verified]**,
  `src/sweep.rs` is TTL-deletion only; no relative-time detection exists.
- **No retroactive redaction**: **[verified]** above.

### 5. Where the proposals contradict this repo's own rules

| proposal says | repo rule |
|---|---|
| "measure `single-session-preference` R@1 target ≥80%" | `AGENTS.md` §6: LongMemEval is **test**, never selection. "Sweep a parameter against the LongMemEval numbers and keep the winner" is forbidden. |
| "hot index ≤200 lines / 25 KiB" | No source. This repo's own measured cost is `DEFAULT_RECALL_BUDGET = 8000` tokens = 3× the 2,626-token median session (`tests/recall_budget.rs`); a 25 KiB index is ~2–3× that budget *before* any recall. |
| "activate the 4 dormant mechanisms" | `CLOSED.md` forbids re-proposing a closed lever without new evidence. |
| "`content_sha256` + `epoch` CAS" | Not present, not measured, and the writes it would protect are **already serialized** by one writer mutex + WAL. |
| "retire redundant tools among the 54" | There are 4. |

---


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


# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 11,57)

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


# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 58,81)

## 1. The shape of the work

Six capability items (W1–W6), ordered by risk and dependency, plus two
correctness fixes that must land first because they make every later gate
meaningful.

```
W0  correctness fixes (test hermeticity, redactor test gap)   [hours, unblocks gates]
W1  dimensional metadata (scope/aspect/lifecycle/trust)       [schema + write + read]
W2  mount profiles (read_only, trust ceiling)                 [config + 2 guards]
W3  retroactive redaction (id/version + pattern)              [version table]
W4  deterministic hygiene scan + apply                        [new module + verb]
W5  startup grounding capsule (bounded, opt-in)               [new read path]
W6  connect --with-skills                                     [one flag]
```

W1 is the foundation: W2, W4 and W5 all read its columns. W3 is independent
and can run beside it. W6 is independent of everything.

**No item changes `FusionWeights::SHIPPED`.** If a later cycle wants a ranking
lever, it is a new hypothesis with its own pre-registration, not a line here.

---


# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 332,358)

## 4. Gates / success criteria (fixed before the run)

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

The same four tools are served two ways. `memory-wire mcp` is stdio —
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

