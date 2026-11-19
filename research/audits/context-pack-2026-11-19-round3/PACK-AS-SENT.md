# CONTEXT PACK — context-pack-2026-11-19-round3

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
| idle footprint | **~10.7 MiB RSS · 10.6 MiB binary · 3 libs** | ~6 MB heap @1k → **316 MB @50k** | **0.8–1.0 GB** | container stack (not measured by us) | server process (not measured by us) |
| LongMemEval-S retrieval, same protocol (measured by us) | R@5 **93.0% clean unfitted** · 97.2% fitted (marked) · ≈94.4% out-of-sample est · R@1 **83.8%** | hybrid 95.2% · BM25-only 86.2% (their harness, tuning undisclosed) | **no comparable number exists** — their published figures are LLM-judged answers | — | — |
| LoCoMo judged accuracy (AMB `rag`) | **84.1%** (n 1540) — free-tier small reader, 15.2k ctx, **26.5 ms** retrieve | — | 92.0% — Gemini-class answerer, 36k ctx, 964 ms retrieve. **Different answerer ⇒ not comparable** to ours | — | self-reported 89.9% |
| PrecisionMemBench (LLM-free, AMB retrieval) | **19/77, precision 0.24**, recall 0.95 — no identity machinery | — | **66/77, precision 0.85**, recall 1.00 — *with* scope filters + identity-extraction LLM at ingest | — | — |
| self-update | `memory-wire update` — sha256-verified, rollback kept, live-verified on a second box | manual | manual | container redeploy | server redeploy |
| license | MIT | MIT | MIT | Apache-2.0 | **AGPL-3.0** — embedding cost for proprietary stacks |
| eval evidence discipline | 5 test-set consultations, all measurement-only; every number traceable to a committed artifact (`docs/CONSISTENCY.md` §46) | none disclosed | — | — | — |

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


# FILE: docs/ARCHITECTURE.md (lines 1,100)

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

A Rust library with nine modules behind one service, plus a binary that
exposes the same service four ways. They are separate because they fail
differently and are measured separately.

| Module | What it is | Why it is its own module |
|---|---|---|
| `api` | `MemoryService` — retain, recall, reflect. The only thing a caller touches. | The interface is the contract; changing it is a breaking change, so it sits apart from every implementation. |
| `store` | SQLite persistence, and an in-memory variant for tests and zero-setup use. | Storage is swappable by design, and the in-memory store exists so a test never touches a file on disk. |
| `recall` | Bank-isolated retrieval: FTS5 BM25 plus reciprocal-rank fusion. | Ranking is the part that is benchmarked, so it is the part that must be measured in isolation. |
| `capture` | What gets kept automatically while a coding agent works. | Capture and recall meet at `api` and nowhere else; coupling them would make retention policy a side effect of search. |
| `embed` | The cargo feature that gates the optional dense-vector arm. | A feature that doubles the binary has to be optional at compile time, not just at runtime. |
| `vector` | The dense-vector arm itself: 23 MB of int8 weights, behind `embed` and absent from a default build. | Kept apart from `embed` so the feature gate is one line and the arm is honestly attributable. |
| `memory` | Bank model, isolation rules, and PII redaction on write. | The privacy guarantee is a boundary, and a boundary belongs where it can be audited in one place. |
| `hygiene` | The deterministic scan that finds near-duplicates and merge candidates without destroying anything. | Non-destructive by construction, so a bug in it cannot lose a memory. |
| `grounding` | The bounded startup capsule — what a session is told before its first turn. | A fixed character budget with a different failure mode from recall: too little is useless, too much is a wall. |

The binary's own modules (`connect*`, `daemon`, `doctor`, `mcp`, `mcp_http`,
`cli_home`, `hooks`, `paths`, `update`, …) are not in this table because they are
not library modules: they are the doors and the plumbing, and they are described
under "The boundaries" below.

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

- **One service, four doors.** HTTP, an MCP server over stdio, a **stateless
  MCP mount on the HTTP server** (`/mcp`, `/mcp/<bank>`), and the CLI all reach
  the same `MemoryService`. There is no second implementation of recall.

  The doors are **not** at parity, and that is deliberate rather than accidental:
  each capability is exposed on the door that needed it, so `redact` is CLI-only,
  `hygiene` is HTTP-only, and `memory_supersede` is MCP-only. The verified
  per-capability table is in `docs/INTEGRATION_GAPS.md` (2026-11-19 addendum);
  `TODO.md` row 31 tracks closing the widest gap.
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

# SECTION B-DELTA

# FILE: research/audits/context-pack-2026-11-19-round3/B-DELTA.md (lines 1,83)

# Delta since round 2 (2026-11-18 → 2026-11-19)

Round 2 audited the shipped W0–W7 tree at `b4b4698`. Three commits have landed
since. This section is what changed, with the evidence class marked inline, so
the reviewer audits the *delta* against the record rather than re-deriving it.

## D1 — `1f5ee17`: the trust tier had no bottom rung

`[LIVE]` `src/api.rs`, `src/hooks.rs`, `plugin/integrations/hermes/memory-wire/__init__.py`

Round 2's finding 4a ("no read-time trust gate: W2 only downgrades on write")
was verified and adopted. Implementing it exposed a deeper defect the finding
did not name: `Trust::tier` accepted **three** tiers (`agent_inferred`,
`human_stated`, `verified`) and mapped every other value to `agent_inferred` —
**including `untrusted_external`**, the one tier the gate refuses. The
plan's own schema table and `src/memory.rs` both documented that value as
storable. So the gate, built as specified, would have been **dead code that
every test still passed**: no row could carry the marker it matched on.

What shipped: `UntrustedExternal` added below `AgentInferred` (the ceiling's
`downgrade` now bottoms out there); `trust` carried on the `full`/`summary`
recall wire; the tier dropped from `hooks::recall_section` **before** the
`MAX_LINES` take; the same hole closed in the hermes plugin's `prefetch`.

Explicitly **not** a relevance threshold: a deterministic allow/deny on a
stored tier with no fitted constant. CONSISTENCY §32/§48 (no score threshold,
ever) is untouched.

## D2 — `81a2212`: three recall failures were byte-identical silence

`[LIVE]` `src/hooks.rs`, `docs/CONSISTENCY.md` §52

`recall_at_fmt` returned `None` for three different reasons — connection
refused, a non-2xx answer, and a 200 body this build could not parse — and all
three produced the same observable: exit 0, empty stdout, nothing anywhere. An
agent and a user could not tell "no memories matched" from "the server is down"
from "the hook and the server disagree about the wire".

Each path now emits a distinct `tracing::warn!` on stderr. Exit stays 0, stdout
stays empty, `RUST_LOG=off` is total silence. Observed live against three real
servers (127.0.0.1:1; a 404 responder; a 200 with `{"not":"an array"}`).

## D3 — the architecture doc is now demonstrably stale

`[LIVE]` `docs/ARCHITECTURE.md` vs `src/lib.rs`, measured this round

Not a code defect but a **documentation** one, and it is the kind a reviewer
should weigh: the doc that exists to tell a newcomer what this system *is*.

- It says "a Rust library with **five modules** behind one service", then lists
  **six** rows in its own table. `src/lib.rs` declares **nine** `pub mod`s:
  eight unconditional (`api`, `capture`, `embed`, `hygiene`, `grounding`,
  `memory`, `recall`, `store`) plus `vector` behind the `embed` feature.
- Its module table omits `hygiene`, `grounding`, `embed`, `vector`,
  `connect*`, `daemon`, `doctor`, `mcp`, `cli_home` and the rest of the binary's
  own modules entirely.
- "One service, **three doors**" lists HTTP, MCP-stdio and CLI. There are four:
  `mcp_http` mounts MCP on the serve router (`/mcp`, `/mcp/<bank>`), which is a
  fourth door on the same service. `docs/INTEGRATION_GAPS.md` (added this
  round) records the asymmetry per capability.

The doc was written at v0.4.0-scale and never re-derived after W0–W7.

## D4 — the round-2 verification's own open item

`[LIVE]` `TODO.md` row 31

Round 2's sharpest structural finding (its §3a) was verified and adopted as
"door parity": one `MemoryService`, four doors, each exposing a **different**
subset. Recorded in `docs/INTEGRATION_GAPS.md` with the verified table; left
open because closing it is a surface decision (document the asymmetry as
intended, or ship `redact` on MCP/HTTP) rather than a bug fix.

The widest gap in that table: **`redact` has no MCP tool and no HTTP route.**
An agent that retained a secret through MCP cannot retract it through MCP.

## What has NOT changed since round 2

- No ranking, scoring or fusion parameter moved. `[LIVE]` `docs/handoff/CLOSED.md`
  is unchanged; the fitted `overlap 0.25` is still fitted, not earned.
- The gate at HEAD: 659 tests, clippy clean, 0 doc warnings. `[LIVE]`
- No new dependency. `DT_NEEDED` is still 3, binary 11,141,240 B. `[LIVE]`
- LongMemEval consultation count unchanged at 5. `[LIVE]` `CONSISTENCY.md`

# SECTION C-LAW

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


# FILE: AGENTS.md (lines 294,327)

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


# SECTION D-EVIDENCE

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

# FILE: docs/handoff/STATE.md (lines 218,276)

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


# SECTION E-ASK

# FILE: research/audits/context-pack-2026-11-19-round3/C-ASK.md (lines 1,70)

# The ask — what this round exists to decide

Round 1 asked "is the plan the right shape?" and answered yes on substance, no
on process. Round 2 asked "where is the shipped architecture, and what should
the next iteration be?" and produced four verified structural findings, two of
which are now closed.

This round asks the question neither of those reached, because both of them
could only be answered from inside the system's own framing:

> **The W0–W7 cycle optimised a subsystem that was already fine.**
>
> Round 1's reviewer said this about the *plan*, and the plan's own author
> agreed. It is now true of the shipped tree: three full cycles (retrieval
> efficacy, BEAM/battery, W0–W7) have gone into **ordering**, and the measured
> position is that ordering is largely solved — the candidate pool contains the
> gold memory in **500 of 500** LongMemEval questions (`AGENTS.md` §1).
> LoCoMo, the **dev** set, is less clean and the pack states it rather than
> quoting only the flattering half: `recall_any@10` is **94.2%** but
> `recall_all@10` is **83.0%**, i.e. questions with several gold sessions still
> miss at k = 10 (`AGENTS.md` §1, `RND_PLAN.md` §1). That gap is an *ordering*
> problem inside the pool, which is the same subsystem again.
>
> Meanwhile the **product** surface — what an agent can actually *do* with the
> memory once it is recalled — has not been re-derived since v0.4.0.

So the decision this round tests is **where the next cycle's effort goes**, and
specifically whether it should keep going where the last three went.

## What the reviewer is asked to weigh

1. **Is the "ordering is solved" reading right, or is it an artefact of the
   instrument?** The 500/500 pool figure and the 94.2% `recall_any@10` are
   measured on LongMemEval-S and LoCoMo. `docs/CONSISTENCY.md` records that
   every LongMemEval number is a **test** consultation, never a selection
   target (§6). If the reviewer thinks the headroom claim is overstated, that is
   the finding.

2. **Given that reading, what is the highest-value next cycle?** Candidates on
   the table, none of them chosen here:
   - the **door-parity** gap (`TODO.md` row 31) — `redact` is reachable only
     from the CLI, so an MCP client that stored a secret cannot retract it
   - the **architecture doc**, which no longer describes the system (D3)
   - the **remaining R&D queue** (`RND_PLAN.md` §2, H-items)
   - something the reviewer can see that this pack cannot

3. **Is there a structural problem none of the four lenses names?** The pack
   carries four lenses (below) because those are the ones the record already
   argues about. A reviewer who sees a fifth is answering the question the round
   was opened for.

## What is NOT being asked

- Whether to add a score threshold or relevance cut to injection. **Rejected
  twice by measurement** (`CLOSED.md`; `CONSISTENCY.md` §32/§48). Do not
  re-propose it; the pack carries the rejection so the reviewer does not spend
  its turn rediscovering it.
- Whether to re-run any benchmark. Every number in this pack is `[LIVE]` and
  traceable to an artefact.
- Whether to match a competitor's feature count. Explicitly not recommended,
  with reasons (`AGENTS.md` §8, `docs/INTEGRATION_GAPS.md` "Explicitly not
  recommended").

## The shape of the answer wanted

A ranked list, each item carrying: **what** the defect or gap is, **where** it
lives (a file or a documented decision), **why it matters to an agent using the
product**, and **what would have to be true** for it to be worth a cycle. An
item the reviewer cannot point at an artefact for should be labelled "not in
evidence" rather than asserted — that label is useful, and it is not a failure.

# SECTION F-ANALYSIS

# FILE: docs/INTEGRATION_AUDIT.md (lines 231,274)

## 3. The capability matrix, honestly

| | claude-code | codex | copilot-cli | cursor | opencode | omp | hermes |
|---|---|---|---|---|---|---|---|
| context injected automatically | ✅ 5 events | ✅ 5 | ✅ 5 | ❌ | ❌ | ❌ | ✅ 5/6 |
| tools callable by the agent | ❌ | ❌ | ❌ | ✅ 4 | ✅ 4 | ✅ 4 | ✅ 4 |
| browse memories without a query | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ |
| namespaced tool names | — | — | — | — | — | ⚠️ `memory_wire_*` | — |

**No harness gets both.** That is the single largest structural gap and it is what
"perfect integration" has to mean. Three hosts are write-only (context in, no tools) and
three are read-only-on-demand (tools, but nothing is injected unless the model asks).

The reason is structural, not an oversight: hooks and MCP are configured through entirely
different files in different formats, and `Style` is a per-host decision made in one enum.
Both are now implemented for both transports — HTTP MCP exists, and the hook shims are a
shell script — so the split is a choice rather than a constraint. It was a defensible
choice when a host had only one of the two mechanisms.

## 4. What "perfect" would be

Seven properties, each currently false somewhere. Ordered by how much user-visible
correctness they buy.

1. **One port, one source of truth.** A documented canonical port, validated at install
   time, with `daemon` refusing to start elsewhere. Fixes F1 and F4's root cause.
2. **Hooks resolve the endpoint the same way the daemon does** — from the state file when
   the env var is unset. Fixes F2. Roughly ten lines.
3. **Every host gets both mechanisms, or the split is justified per host in writing.**
   Fixes the §3 gap; the first instance is Codex + HTTP MCP (F3).
4. **Install writes the running endpoint, not a default.** The MCP entry is generated
   from what `daemon status` actually reports.
5. **Namespace collisions are impossible by construction.** `memory_wire_*` must be
   documented everywhere a tool name appears, and a two-server collision should be
   detected at install rather than discovered by a confused agent.
6. **Every host's capability set is asserted by a test against a real harness binary**,
   not only against a fixture directory. Today 477 tests exercise fixtures; exactly one
   assertion in this document's history ran a real `omp`.
7. **The build is reproducible**, and the published size is a claim about a command
   anyone can run. Fixes F5.

Properties 1–3 are correctness. 4–5 are robustness. 6 is the meta-property that would have
caught most of this document's findings automatically. 7 is honesty.


# FILE: docs/INTEGRATION_GAPS.md (lines 446,489)

## Addendum 2026-11-19 — the doors are asymmetric, and that is the honest state

Recorded because round-2 review read "one service, three doors" as a promise of
parity. It is not. It is a claim about `MemoryService`: there is **one**
implementation, reached through doors that each expose a **different subset**.
Verified per capability against this tree:

| capability | HTTP | MCP | CLI | hook |
|---|---|---|---|---|
| redact (W3) | no route | no tool | **yes** (`Cmd::Redact`) | write-time only |
| grounding (W5) | no route | no tool | **yes** (`Cmd::Grounding`) | SessionStart, default off |
| hygiene (W4) | **yes** (`GET /banks/:id/hygiene`) | no tool | no | no |
| recall | yes | `memory_recall` | no | yes |
| supersede (W7a) | no route | **yes** (5th tool) | no | no |

This is not a defect in any one door; it is what happens when each capability
was added through the door its own item needed. It is recorded here rather than
fixed because closing it is a surface decision, not a bug fix:

- **redact on MCP and HTTP is the widest gap.** Redacting a secret is a custody
  operation, and today the only way to reach it is the CLI on the machine that
  holds the store. An agent that retained a credential through MCP cannot
  retract it through MCP.
- **hygiene has no MCP or CLI door at all**, which the W4 record already notes
  as a deviation from the plan (`memory_hygiene` was the planned 5th tool;
  `memory_supersede` shipped instead).
- **supersede has no HTTP route**, so a non-MCP, non-local caller cannot retract
  a row.

None of these is silently broken. Each door that does not implement a capability
is simply absent, and the CLI's `--help` lists what the CLI can do. The risk is
a reader assuming a capability is reachable everywhere because the service is
shared.

### A related count, corrected

`src/mcp.rs` has six `pub const …: &str = "memory_…"` constants, but only
**five** are tools: `HISTORIAN` (`memory_historian`) is a **prompt**, advertised
through `prompts/list` and served by `prompts/get`, and `tools()` does not
register it. The server exposes five tools and one prompt. Round-2 review read
six constants as six tools and concluded the README overcounted; the README's
prose ("five tools") is right, and its table had been the thing missing
`memory_supersede`. The `HISTORIAN` doc comment now says this in place, so the
next count does not re-add it.

# FILE: TODO.md (lines 18,46)

## Open

R&D rows (6, 9, 10, 11, 12, 20) are sequenced in `docs/handoff/RND_PLAN.md` —
§5 the closed survey cycle, **§6 the upcoming cycle's outline**; start there.

| # | status | item | why | where | acceptance |
|---|---|---|---|---|---|
| 1 | done | ~~`connect` preserves a surface's baked bank when `--bank` is absent~~ — **done 2026-10-15** (shipped v0.8.0): `apply` reads the pin from the host's own config before any edit — hooks from the command strings, MCP from the argv, extensions/plugins from the installed file's `DEFAULT_BANK` — and rewrites with it; only a config with no pin anywhere stays runtime-resolved (`src/connect.rs`, `bank_from_extension`; tests: bare-rewire-preserves-pin on hooks + MCP + no-pin cases) → [2026-10-15] `src/connect.rs` | kills the installer's `--no-connect` dance on pinned machines (README §Installation documents it); today a bare re-connect silently re-defaults to `memory-wire` | `src/connect.rs` + test | ~~against a temp `HOME` pinned to a second bank, a bare `connect <host>` re-wires with the same bank, unchanged~~ — asserted |
| 2 | done | ~~`doctor` reports a serve running an older binary than the installed one~~ — **done 2026-10-15** (shipped v0.8.0): `Report.stale_serve` — reads the daemon state file's pid, `/proc/<pid>/exe` (the `(deleted)` marker stripped), and **byte**-compares against the running binary; a copy at a different path is not stale. Renders the finding with the restart command; quiet when down, absent state, or identical bytes (`src/doctor.rs`; 3 tests incl. the byte-identical-copy case) → [2026-10-15] `src/doctor.rs` | nothing today detects the "serve keeps old behavior until restarted" window — the exact gotcha from the v0.7.0 deploy on both machines; the running binary outlives the swap by an unbounded time | `src/doctor.rs`, tail of `install/get-memory-wire.sh` | ~~a stale serve produces a finding line naming the restart command; a current serve produces none~~ — asserted |
| 3 | done | ~~`memory-wire update` subcommand~~ — **done 2026-10-15** (shipped v0.8.0): `update` drives `gh` (the binary stays dependency-free plain-HTTP; a missing `gh` refuses with the install hint) — newest-tag compare, `gh release download`, **sha2 in-process verification before anything is touched**, extract, stop-if-live, atomic swap keeping `<exe>.old` for a one-command rollback, start-on-the-new-binary, bare `connect` (pins preserved via #1). Order asserted over a recording fake; live network never in tests (`src/update.rs`; 7 tests). MW_REPO/MW_ASSET overrides → [2026-10-15] `src/update.rs` | one command for check-latest → sha256-verify → swap → re-connect installed hosts (banks preserved via #1) → serve restart (via #2); today all five update steps are manual and one of them (hermes) is invisible to the installer entirely | `src/main.rs`, composing `connect.rs`/`doctor.rs`/`daemon.rs` | ~~end-to-end test against a fake releases API served from a temp dir; the live network check stays behind a flag and never runs in tests~~ — asserted over the effects seam; live mode is `gh`-gated and smoke-verified against the real repo |
| 4 | in-progress | ~~INTEGRATION_PLAN Phase 0, starting with P0.1~~ — **re-verified 2026-10-15 against the binary**; the register is now corrected in place. P0.1 **done** (`paths::endpoint()` reads `serve.json` behind `MEMORY_WIRE_URL`, `src/paths.rs`, 5 regression tests). P0.2 **resolved as keep-8888 + probe** (`daemon start` binds-or-refuses; `connect` takes no probe by decision). P1.1 **done** (`src/connect_codex.rs`). P1.2 **done 2026-10-15, both halves** (`src/connect_claude.rs` for `~/.claude.json`; `src/connect_copilot.rs` for `~/.copilot/mcp-config.json` — a user definitions file whose schema was verified from GitHub's own Copilot CLI reference and the github-mcp-server install guide, so the earlier "JSONC, unverified" blocker is discharged). P1.3 **done 2026-10-15** (`edit_mcp` refuses a foreign server under our name in both directions; claude URL-identity ownership). P2.1 **open** (golden-fixture tier). P2.2 **done 2026-10-15** (`doctor --all-hosts` + G6.9 liveness). P3.1 **recorder done 2026-10-15** (DT_NEEDED row in `bench_footprint`, `readelf -d`); the v0.5.0 byte-compare archaeology stays open. P4.1 hermes-slot research **open**; P4.2 **done** (this correction) | re-verify each item against the `v0.7.0` tree first — the plan was written 2026-09-30 and its blocking decisions may have been answered by the retrieval-efficacy waves | `docs/INTEGRATION_PLAN.md` | ~~the plan's own gates~~ |
| 5 | done | ~~GAP-8: name the `hook:stop` marker producer~~ — **closed 2026-10-15 as stop-looking, by evidence, not fatigue**: the producer was characterized 10/10 probe runs (execs the installed binary's `hook stop` by absolute path, once per opencode session end, ~15 ms after the plugin's idle retain) and is absent from every visible binary, config and package on the box; further identification means syscall-tracing live sessions, out of all proportion to a residual risk that is already neutralized (its markers are auto-tagged and excluded from auto-injection since v0.7.0). The 2026-10-08 addendum in `docs/INTEGRATION_AUDIT.md` is the record; a name arriving by accident is the only thing that would reopen it → [2026-10-15] `docs/INTEGRATION_AUDIT.md` | effect already closed — it execs the installed binary, so its markers are tagged and auto-excluded since `v0.7.0`; only the identity is unknown (absolute-path invocation, absent from every visible binary, config and package on the box) | start at the 2026-10-08 addendum in `docs/INTEGRATION_AUDIT.md` | ~~a named producer with evidence, or a recorded decision to stop looking~~ — the recorded decision is this row |
| 6 | done | ~~dev-set growth before any score gating~~ — **done 2026-10-16**: the battery is frozen (`eval/relevance/freeze_battery.py`; 1,429-row snapshot sha256 `c780939b…` gitignored, seed → deterministic 1,413-row bank) and grown 10 → 21 (11 verified turn-row harvests); frozen marks gold 16/17 auto, junk 0/9, search-reachable 17/17; D-case re-measured — passes under the shipped config; H8 gate re-confirmed. Precision levers now registrable (one at a time, junk@5 down / gold@5 flat, never a threshold — CONSISTENCY §32/§48) → [2026-10-16] `eval/relevance/freeze_battery.py` | the standing decision is tag hygiene, no threshold (CONSISTENCY §32); gating may only be revisited after the labelled set grows well past today's 10 cases — and the D-case judgment (cross-project turn rows ranking in a single-user bank) is the first thing to re-measure | `eval/relevance/`, `docs/EVALUATION_HYGIENE.md` | a gated change carries its dev-set numbers and states its fitted-ness, per AGENTS §6 |
| 7 | done | ~~PrecisionMemBench precision: 0.17 at recall 0.95 (14/77)~~ — done 2026-10-14 (shipped v0.8.0): the adapter ingests dataset tags and honours the filter's `any` scope groups via the store's include-any recall; **19/77, precision 0.24, recall unchanged 0.95** (`eval/amb/PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz`, `CONSISTENCY.md` §37). The number moved because of the provider capability change, not ranking (filters are pre-fusion SQL narrowing). `narrow_any` identity clauses remain unhonoured (Hindsight-native fuzzy tag resolution), so misspelled-name cases still over-return — residue named, no threshold picked off this 77-case table → [2026-10-14] `eval/amb/PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz` | first independent public measurement of the over-return `docs/OPEN_HOOK_RECALL_RELEVANCE.md` diagnosed; 563 of 567 failures are an unexpected belief. Candidate levers must be argued by mechanism (e.g. the provider honouring the dataset's caller-supplied `scope:` tags, which memory-wire can already filter on) and decided on the dev set — never a threshold picked off this 77-case table | `eval/amb/memory_wire.py`, `src/recall.rs`; `docs/BENCHMARK_LANDSCAPE.md` §5.1 | ~~a change decided on LoCoMo/relevance, then PMB re-run once and committed as a new `eval/amb/PRECISIONMEMBENCH-*.json.gz`, whatever it shows~~ |
| 8 | done | ~~LongMemEval: emit the 470-question (abstention-excluded) slice~~ — done 2026-10-13: `examples/longmemeval.rs` prints the row; first reading 84.0% R@1 / 97.2% R@5, n 470 (`eval/baseline/2026-10-13-v0.7.0/LONGMEMEVAL.md`) → [2026-10-13] `eval/baseline/2026-10-13-v0.7.0/LONGMEMEVAL.md` | upstream's retrieval protocol skips the 30 `_abs` questions; ours and agentmemory's both score 500 (agentmemory's filter tests `question_type` and never matches), so neither number sits beside the paper's tables | `examples/longmemeval.rs` (the generator, not a hand computation) | `eval/RESULTS.md` carries both the 500 and the 470 row, from one run, counted as one consultation |
| 9 | done | ~~turn-level gold for BEAM (and/or PersonaMem) → a second dev set~~ — **done 2026-10-16**: pair-level labels for BEAM-100k (`eval/beam_label.py` + digest `eval/beam_labels_100k.json`; 313/400 resolved, 13/15 review agreement) and the pool-scale re-test (`eval/BEAM.md`, CONSISTENCY §47) — **stream not deleteable** (0.25 > 0.00 everywhere; 0.00 loses R@pool 100→95.5%), **unfitted 1.00 leads 0.25** (R@5 +3.5pp, opposite sign to LoCoMo's +1.4pp) — weight question stays open and user-gated → [2026-10-16] `eval/BEAM.md` | AMB's copies cannot be scored retrieval-only: BEAM gold ids never resolve (0 of 1,800 at 100k/500k/1m), PersonaMem's gold is the whole bank. A labelled BEAM would add 2,000 questions over 27–50-doc banks — the pool size LoCoMo under-measures (CONSISTENCY §14.2) — and feeds #6 | `eval/download.sh`, a new example; upstream `mohammadtavakoli78/BEAM` | labels derived from upstream source-turn data, not hand-picked; resolve rate reported | and gold/bank share reported as in `docs/BENCHMARK_LANDSCAPE.md` §3 |
| 10 | open | agent-memory-bench (GiulioDER) Claude Code arm | the only external benchmark that grades the product's actual job — task success with hooks+MCP wired — and it has a harm suite (superseded/contradictory/adjacent) | an `adapters/memory_wire/` in their repo, wiring `memory-wire connect claude-code` | needs the user's agent-model budget; result reported as measure-only beside their `claude_md` baseline |
| 11 | open | LLM-judged runs: AMB `rag` (BEAM/PersonaMem/LifeBench), OAMB LME-60, memorybench ConvoMem, MemBase RealMem/MobileMem | answer-level evidence on corpora we have nothing on; OAMB's answer-visible-token axis is where a budget-trimmed recall should show | `docs/BENCHMARK_LANDSCAPE.md` §4, §7 | user-approved budget first; OAMB and every LME run counted as a test consultation |
| 12 | open | AMB provider writes at the product's granularity (turn pairs, as the hooks do), not 25k-token sessions | with whole-session documents, AMB's top-10 is the entire bank on BEAM 100k / PersonaMem 32k and ~250k tokens on BEAM 500k/1m — those corpora measure the reader's context window, not memory-wire (`eval/baseline/README.md`). Decided by mechanism (mirror the real write path), not by score | `eval/amb/memory_wire.py` | BEAM / PersonaMem / LifeBench added to `run_baseline.sh llm`; top-10 tokens per question reported, as in the README table. **Measured cost, 2026-10-14:** on LoCoMo (dev) turn-level writes pooled to sessions (`locomo --turns`, `eval/LOCOMO_TURNS.md`) lose to whole sessions on every aggregate — `recall_all@10` 83.0% → 80.7%, R@1 60.4% → 54.6%, n 1531 — and windows of W=2 turns lose too (`eval/LOCOMO_WINDOWS.md`: R@1 59.3%, `recall_all@10` 82.6%, R@pool 98.7% — windowing passes the candidate-pool ceiling). Every granularity finer than the session measured so far is worse; H2 is closed. This row still buys measurability of the long corpora at a ranking cost, and is not a ranking lever |
| 13 | open | aroute: 200-with-`{"error"}` logged `outcome=ok` with no failover (B1); cline replies forwarded inside a `{"success","data"}` envelope (B2); cline token/cost headers always 0, so spend limits miss cline (B3); a 403 provider gets `cooldown_ms=0` and is retried on every request (B4) | B1/B2 break every OpenAI client; `eval/llm_shim.py` papers over them and should be deleted once aroute is fixed | `agentic-utility/proxy/artificial-route` (a different repo) — handoff in **`docs/AROUTE_BUGS.md`** | in `llm-calls.jsonl`: no `enveloped: true`, no retries, non-zero `tokens_in` for cline |
| 14 | done | ~~re-measure `bench_concurrency` on a quiet box~~ — **done 2026-10-14, PASS** (shipped v0.8.0): load 0.98→4.33, **1.85×** at 64 clients vs the 1.50× floor, RSS flat 12.9 MB, 0 panics (`eval/baseline/2026-10-14-bench-concurrency/BENCH_CONCURRENCY.md`, `CONSISTENCY.md` §40). The 2026-10-13 failure (1.30×) was load noise at 17→22 — the baseline artifact stays as the dated record of that regime → [2026-10-14] `eval/baseline/2026-10-14-bench-concurrency/BENCH_CONCURRENCY.md` | the 2026-10-13 baseline failed the 2-client scaling gate (1.30× vs 1.50×) at load 17→22 while absolute ops/s rose against the committed artifact — unreadable as a regression or a pass | `examples/bench_concurrency.rs` | ~~one run with `loadavg` < 4, recorded either way~~ |
| 15 | done | ~~an occurrence time on HTTP `retain` (and in `format: full` recall)~~ — done 2026-10-14 (shipped v0.8.0): HTTP and MCP retain take `created_at` (RFC 3339 or `YYYY-MM-DD`, UTC-normalized, else the frozen 400), `full` recall serves it, and `eval/amb/memory_wire.py` round-trips `Document.timestamp` through it; tests in `src/main.rs` and `src/mcp.rs` → [2026-10-14] `src/main.rs` | a memory's *event* date cannot round-trip through the product: `retain` has no field for it and `full` recall returns no date, so LoCoMo/LongMemEval temporal questions reach a reader undated; `eval/amb/memory_wire.py` and `examples/answer_quality.rs` have to supply or omit it client-side (found 2026-10-13, `docs/CONSISTENCY.md` §34) | `src/api.rs` retain/recall bodies; `Memory.created_at` already exists in the store | an AMB document's `timestamp` round-trips through retain → recall with no client-side table |
| 16 | done | ~~`answer_quality` passes no session dates to its reader~~ — done 2026-10-14 (shipped v0.8.0): harness fixed (sessions at `haystack_dates`, served memories prefixed `[<date>]`, `question_date` in the prompt; ranking untouched) **and re-measured once**: dated instrument reads **18.6%** conditioned (93/499 scored, 1 unscored), closed-book 6.0%, delta +12.6pp — beside the undated 17.0% as a different instrument, no retrieval conclusion drawn (`CONSISTENCY.md` §39, `eval/baseline/2026-10-14-v0.7.0-dated-answer-quality/`; test consultation #3) → [2026-10-14] `eval/baseline/2026-10-14-v0.7.0-dated-answer-quality/` | LongMemEval ships `haystack_dates` / `question_date`; without them temporal-reasoning (5.3%) and knowledge-update (14.1%) are close to unanswerable by construction. Fix the harness, then re-measure once — a harness fix, not a retrieval change (§34) | `examples/answer_quality.rs` | ~~the reader's prompt carries each session's date and the question date~~ |
| 17 | done | ~~served-context precision for small readers~~ — **closed 2026-10-14 as not reproduced** (shipped v0.8.0): H4 built the dev set this row demanded — fixed 200-q LoCoMo subset, every gold inside the shipped top-3 (`eval/h4_subset.json`, harness `examples/locomo_h4_subset.rs`), served at k = 3/5/10 via the adapter's `MEMORY_WIRE_TOPK` cap — and the drowning failure did not reproduce: judge accuracy **92.0% / 94.0% / 94.0%** at median context **4.3k / 7.2k / 14.6k** tokens, zero refusal-styled wrong answers at any k (`eval/amb/h4_locomo_rag_topk{3,5,10}.json.gz`, `CONSISTENCY.md` §38). Serving 3.4× the context at fixed gold coverage moved nothing the judge can see, so the LME 10→20 dip attaches to that instrument's 0.5B reader, not to memory-wire's served set. No precision lever was ever built; per AGENTS §6 none was ever selected on the LME observation either → [2026-10-14] `eval/h4_subset.json` | 2026-10-13: on 10 LongMemEval refusals the free reader answered from the gold session alone 6/10 and from gold + 2 other sessions 1/10, with gold ranked first in 419 of 454 served contexts — ordering is solved, *what else is served* is what a small reader drowns in. Same failure shape as PMB's precision 0.17 (row 7) and the turn-granularity row 12. Select any lever on LoCoMo / the battery, never on this LME observation (§34) | recall budget/cap composition, turn-level writes | ~~a dev-set (LoCoMo AMB `rag`) accuracy gain at equal or better R@k~~ — the dev set was built and the failure is absent from it; reopens only with a *new* dev set that exhibits the failure |

| 30 | done | ~~read-time trust gate on auto-injection~~ — **done 2026-11-19, and it exposed a deeper defect**: `Trust::tier` accepted three tiers and mapped `untrusted_external` to `agent_inferred`, so the tier the gate refuses could never be *written* — a gate built on it would have been dead code with every test green. Fixed the ladder (`UntrustedExternal` added below `AgentInferred`, `downgrade` bottoms out there), put `trust` on the recall wire, and dropped that tier from `hooks::recall_section` **before** the `MAX_LINES` take so a poisoned rank-0 row cannot cost a trusted row its slot. An explicit `memory_recall` still returns those rows: a reader who asked consented. The same hole is closed in the hermes plugin's `prefetch` (`UNTRUSTED_TIER`). Load-bearing proof: removing the filter fails four tests, including a full-chain test through the real router → [2026-11-19] `src/api.rs`, `src/hooks.rs`, `plugin/integrations/hermes/memory-wire/__init__.py`; `CONSISTENCY.md` §51 | round-2 review's strongest finding (its §4a, verified): W2 only downgrades on write, so a row marked `untrusted_external` was injected into context verbatim. Not a relevance threshold — a deterministic allow/deny on a stored tier with no fitted constant, so CONSISTENCY §32/§48 (no score threshold, ever) does not apply | `src/api.rs`, `src/hooks.rs`, hermes plugin | ~~an untrusted row is absent from the injected block while an explicit recall still returns it; no score threshold introduced~~ — asserted, incl. a full-chain test and a negative check |
| 31 | open | door parity: redact has no MCP tool or HTTP route, hygiene has no MCP/CLI door, supersede has no HTTP route | round-2 review's sharpest structural finding (its §3a, verified): "one service, three doors" is true of `MemoryService` and false of parity — every door exposes a different subset, so a capability can be unreachable from the surface an agent is actually on. Recorded with the verified table in `docs/INTEGRATION_GAPS.md` (2026-11-19 addendum); not silently broken, just absent | `docs/INTEGRATION_GAPS.md` addendum; `src/mcp.rs`, `src/api.rs`, `src/main.rs` | a surface decision, not a bug fix: either a documented ruling that the doors stay asymmetric, or redact on MCP — the widest gap, since an agent that retained a secret through MCP cannot retract it through MCP |
| 32 | done | ~~recall failures are indistinguishable from an empty bank~~ — **done 2026-11-19**: `recall_at_fmt` returned `None` for three different reasons — connection refused, non-2xx, and a 200 body this build could not parse — and all three produced byte-identical silence (exit 0, empty stdout). Each now names itself on stderr via `tracing::warn!`, so a hook that recalled nothing says which of the three happened. `RUST_LOG`-gated and stderr-only, so §9's never-fail contract is intact and `RUST_LOG=off` is total silence. The empty-query path stays silent because it is not a failure → [2026-11-19] `src/hooks.rs`; `CONSISTENCY.md` §52 | round-2 review had this as "empty framing violates AGENTS §10"; its verification pass refuted that and named the real defect — the agent and the user cannot tell "no memories matched" from "the server is down". §9 (never-fail silence) and §10 (an empty result says so) genuinely conflict here; §10 governs output that claims to be an answer, and a hook's empty stdout claims nothing | `src/hooks.rs`, `recall_at_fmt` | ~~each failure mode produces a distinct stderr line, none reaches stdout, exit stays 0, `off` is silent~~ — observed live against three real servers, plus a test pinning the modes differ and stdout stays empty |


# FILE: docs/handoff/RND_PLAN.md (lines 45,64)

## 1. What the evidence says about where headroom is

1. **Ordering of single-gold questions is mostly solved** on LongMemEval: first gold at
   rank 1 in 419/500, rank 2–5 in 67 (EXCEED_PLAN §1). The remaining LongMemEval answer
   gap is the *reader*, not retrieval (STATE §2).
2. **On LoCoMo the headroom is coverage of multi-session ("list everything") questions.**
   Shipped config: `recall_all@10` 83.0% vs `recall_any@10` 94.2%. Misses by gold-set
   size (shipped config, n 1,531): multi-hop 1 gold 13/58, 2 gold 10/14, 3 gold 6/7,
   ≥4 gold 10/10; single-hop 2 gold 75/163, 3 gold 40/56, ≥4 gold 43/49; temporal and
   open-domain mostly single-gold with 29/292 and 25/840 misses.
3. **The gold session is always somewhere in the fused list** (`gold_in_pool` 100% on
   the session arm). So a miss is an *ordering* problem inside a 19–32 session bank — but
   for a plural question the several gold sessions each share only part of the query, so
   lexical rank cannot put all of them in the top 10. Whether the missing sessions sit at
   rank 11–15 (reachable by reordering) or at rank 20+ (semantic) is **unmeasured** and is
   the first thing to find out (H1.0).
4. A **small reader drowns in extra context** on LongMemEval (gold alone 6/10, gold + 2
   others 1/10). LoCoMo does not exhibit it. A precision lever therefore has no dev set
   yet (H4).


# FILE: docs/handoff/RND_PLAN.md (lines 134,209)

## H1 — Coverage levers for multi-session questions (target: `recall_all@10`) — **all levers measured, all gates failed**

- **Status (2026-10-15):** **H1d now measured too — gate FAILED, lever closed** (`CONSISTENCY.md`
  §41, artifacts `eval/SELECTION_VECTOR_AXIS_ALL10.md` +
  `eval/results_selection_vector_all10.json`). The vector axis re-scored with
  `recall_all@10` added to the metric vector (`Agg::all` → 7 metrics; the walk objective
  untouched): +0.4 / +0.9 / +1.3 / **+1.8** / +1.3 / −2.7pp at vec = 0.10/0.25/0.50/0.75/
  1.00/1.50, up/down 10/3 … 54/25 … 76/116. 0.10–0.25 sit below the +1pp gate; 0.50–1.00 cross
  it only by trading −7.0 to −12.3pp R@1 (deterministic harness — the regressions are real);
  1.50 collapses. **The deep-miss claim is falsified**: of the 99 worst-gold-at-21+ misses,
  dense fusion rescues 2/2/3/4 at 0.10/0.25/0.50/0.75 — the aggregate gain is shallow-end
  reordering, not semantic reach. No LongMemEval consultation (LoCoMo fail closes it).
  **The `--features embed` capability stays** as an opt-in, off-by-default arm; the evidence
  says do not promote it, and the user's shipping decision is now informed. H1a–c failed
  2026-10-14 (see below); H1e stays a user mechanism, not a benchmark lever.
- **Status (2026-10-14):** H1a/H1b/H1c measured once each on the dev set — **all three
  gates failed** (`CONSISTENCY.md` §37, artifacts `eval/LOCOMO_LEVERS.md`), CLOSED.md rows
  added. No LongMemEval consultation: a LoCoMo fail closes the lever, as pre-registered.
  H1a is the near-miss worth knowing about: R@1 +3.0pp, MRR +2.2pp, NDCG@10 +1.7pp, but
  `recall_all@10` +0.5pp < the +1pp gate and single-hop R@5 −0.4pp. H1d remains open and
  is the **user's** call (~3× artifact size); H1e stays a user mechanism, not a benchmark
  lever.
All are `FusionWeights` / query-side mechanisms that already exist and ship **inert**
(`src/recall.rs` module doc table). Turning one on is a one-line edit to
`FusionWeights::SHIPPED`; the work is the measurement. Each is judged on LoCoMo
`recall_all@10` **and** must not regress R@1/R@5/NDCG on any category by more than the
run-to-run spread (the harness is deterministic, so any regression is real — report it).

| id | lever | mechanism (argue it before measuring) | already measured? |
|---|---|---|---|
| H1a | `keyword_scope: ExactAndPrefix` (FTS5 `token*`) | BM25's exact-token match misses morphological variants (migrate/migration). Plural questions use a verb ("play", "visit") whose sessions use other forms | **No.** Not a walked axis in `select_fusion`. Read `KeywordScope`'s rustdoc first: it records measured `bm25()` values that decided what the prefix arm emits |
| H1b | `overlap_scope: Sentence` (per-segment overlap) | whole-document overlap is diluted by session length; a session where one sentence matches strongly is scored like one where many sentences match weakly. Plural questions need exactly the former | **No.** `eval/SELECTION.md` limitation 5 names it unswept |
| H1c | `bm25_magnitude > 0` | rank-only RRF discards the real BM25 score; a rank-7 row with a far better score is invisible to it | Partly: on LoCoMo R@5 86.0→86.3% at 4.00, no collapse (`eval/SELECTION.md` reading 2). **`recall_all@10` was not measured then** |
| H1d | dense stream (`--features embed`, `FusionWeights::vector`) | the one arm that reaches a document no lexical stream scores above zero | Axis exists (`select_fusion --axes vector`; see `eval/SELECTION_VECTOR_AXIS_FIXED.md`). Re-read it, then measure `recall_all@10` |
| H1e | per-bank `recallSynonyms` | user-supplied paraphrase; not a hardcoded table (EXCEED_PLAN Phase B) | a mechanism for *users*, not a benchmark lever: a table tuned on LoCoMo is fitted. Do not build one to score |

- **Test:** add `all10` to `select_fusion`'s metric vector (`METRIC_NAMES`) so existing
  axes report it, then run `cargo run --release --example select_fusion -- --axes <axes>`
  for the axes in question; for H1a/H1b add the field as an axis or run `locomo` with the
  field edited (`FusionWeights { keyword_scope: ..., ..SHIPPED }` is already how
  `locomo.rs` builds its grid — see `GRID`).
- **Pre-register values.** `ExactAndPrefix` and `Sentence` are binary, not a sweep. For
  `bm25_magnitude` use the one value the algebra suggests (1.0: one extra RRF-sized term,
  bounded by construction) rather than a grid.
- **Gate:** `recall_all@10` up by more than 1 pp **and** per-query up-count > down-count
  by a margin you can state, no category regression, R@20 not down. Then, and only then,
  LongMemEval retrieval **once** (test consultation, counted). A null on LoCoMo means do
  **not** run LongMemEval; record the null.
- **Record:** `docs/CONSISTENCY.md` §36+; if a default changes, mark it `provisional:` in
  code and docs if any LongMemEval number informed it.
- **Cost:** free; ~25 s per LoCoMo configuration.
- **Prior:** H1a/H1b low-to-moderate (cheap, mechanism is plausible); H1c low (already
  near-null); H1d the only one that can reach semantic misses, but it costs bytes — a
  user decision (README "Decisions that belong to the user").

## H2 — Intermediate granularity (one value, low prior)

- **Status:** **rejected 2026-10-14** — windows W=2 (stride 1, pre-registered from the
  production turn-pair unit) measured once: R@1 60.4→59.3%, `recall_all@10` 83.0→82.6%,
  and R@pool 100.0→98.7% — windowing pushes banks past the candidate-pool ceiling, so
  some gold becomes unreachable at any K. Gate failed; granularity is now closed
  (`Turns` §35 and `Windows(2)` both worse than sessions). No second W tried, as
  pre-registered (`CONSISTENCY.md` §37, `eval/LOCOMO_WINDOWS.md`).
- **Mechanism:** a window of consecutive turns keeps the local context a bare turn loses
  while still concentrating the match. Hooks write turn *pairs* in production, so a
  window is closer to the product's real unit than a 25k-token session (TODO #12).
- **Test:** `--turns`-style arm with windows of W consecutive turns (stride W/2), session
  scored at its best window (MaxP via `session_of`). **Pre-register a single W** from
  mechanism (e.g. the product's turn-pair ⇒ W = 2, or W ≈ 10 as "one topical exchange")
  and state why *before* running. Do not scan W.
- **Gate:** better than the session arm on `recall_all@10` and R@1, per-category. If not,
  record the null and stop; do not try a second W.
- **Record:** `eval/LOCOMO_WINDOWS.md` + CONSISTENCY. **Cost:** free. **Prior:** low.
- Note AGENTS/EXCEED_PLAN Phase E: granularity is a *product* capability and must not
  enter the LongMemEval comparison (it changes the corpus unit).


# SECTION G-LENS

# FILE: README.md (lines 514,579)

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


# FILE: README.md (lines 657,709)

## MCP

The same five tools are served two ways (the fifth, `memory_supersede`, is the W7a
supersede verb — a correction path `memory_retain` does not have). `memory-wire
mcp` is stdio —
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
| `memory_supersede` | yes | `memory_id` (required), `bank?`, `superseded_by?` |

Every tool declares `destructiveHint: false`; `memory_retain` and
`memory_supersede` are the two without `readOnlyHint: true` — a repeated retain
of the same content in the same bank writes nothing and returns the id that
already holds it, but a repeated retain of *different* content is a second
memory; a supersede marks a row superseded (the default recall excludes it, and
`include_history` returns it with its lineage) rather than deleting it, so the
audit trail survives. Each tool is a thin adapter onto
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

# SECTION H-MEASURE

# FILE: research/audits/context-pack-2026-11-19-round3/measure_surface.sh (lines 1,69)

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
echo "# Only ACTUAL .route() registrations. A grep for \"/banks/...\" also"
echo "# matches test fixtures and doc strings, which measure residue, not surface."
grep -hoE '\.route\("[^"]*"' src/*.rs | sed 's/.*route("//;s/"//' | sort -u
echo "# MCP mounts (a fourth door, not a separate service)"
grep -hoE 'nest_service\("[^"]*"' src/*.rs | sed 's/.*("//;s/"//' | sort -u

echo
echo "## Authored doc surface"
wc -l AGENTS.md README.md docs/*.md 2>/dev/null | tail -1

# FILE: research/audits/context-pack-2026-11-19-round3/SURFACE.md (lines 1,104)

# Authored surface — memory-wire at 81a2212

## Excluded, by name
  target/            build output
  research/          audit rounds (this pack's own directory tree)
  _audit/, _audit_scratch/   read-only competitor checkouts, not our code
  eval/              datasets and generated result artifacts
  genome/, scripts/genome/    vendored third-party CLI
  tests/fixtures/    deliberate secret fixtures

## src/ — lines of Rust per module (the binary)
  35295 total
   5127 src/store.rs
   4127 src/api.rs
   2968 src/main.rs
   2841 src/connect.rs
   2429 src/recall.rs
   1890 src/hooks.rs
   1882 src/mcp.rs
   1132 src/daemon.rs
   1123 src/doctor.rs
   1103 src/connect_plugin.rs
    965 src/vector.rs
    948 src/cli_home.rs
    857 src/connect_codex.rs
    830 src/connect_ext.rs
    810 src/paths.rs
    655 src/render.rs
    654 src/update.rs
    649 src/seed.rs
    645 src/hygiene.rs
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
    181 src/memory.rs
    177 src/embed.rs
     51 src/lib.rs

## src/ — total
35295

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
11141240 bytes

## Public MCP tool surface (the count the docs must not misstate)
"memory_retain"
"memory_recall"
"memory_reflect"
"memory_bank_config_get"
"memory_supersede"
"memory_historian"

## Public HTTP routes (the count the docs must not misstate)
# Only ACTUAL .route() registrations. A grep for "/banks/..." also
# matches test fixtures and doc strings, which measure residue, not surface.
/banks/:id/config
/banks/:id/hygiene
/banks/:id/memories
/banks/:id/memories/:mid
/banks/:id/recall
/banks/:id/reflect
/banks/:id/retain
/banks/:id/stats
/health
# MCP mounts (a fourth door, not a separate service)
/mcp

## Authored doc surface
 12213 total
