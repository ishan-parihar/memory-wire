# CONTEXT PACK — context-pack-2026-11-20-round5

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

# FILE: AGENTS.md (lines 26,67)

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

- **The default build must not gain a dependency or grow materially.** HEAD is
  **10.63 MiB** (11,141,240 B, `target/release/memory-wire`, measured 2026-11-19)
  with `DT_NEEDED` = 3. `v0.7.0` was 10.25 MiB
  (`eval/baseline/2026-10-13-v0.7.0/BENCH_FOOTPRINT.md`); the 9.7 MiB this line
  used to state was earlier still, and 10.25 was itself a year-old figure this
  line carried until a reviewer checked it against the binary. **Do not trust a
  size quoted here without re-running `stat`** — quote the command, not the
  number. Verify `ldd` after touching anything that links.
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


# FILE: AGENTS.md (lines 103,148)

## 3. Where things live

`src/`, 33 modules, 35,674 lines. Counts are `wc -l` against this tree;
`scripts/module_table_check.sh` fails when the table and the tree disagree, so a
stale count is a red gate rather than a claim nobody checks.

| module | lines | role |
|---|---|---|
| `store.rs` | 5127 | SQLite: schema, migrations, put/recall/delete, FTS5, redaction, versions |
| `api.rs` | 4127 | HTTP routes, the frozen error contract, the historian prompt |
| `main.rs` | 2973 | clap surface, routing, wiring |
| `connect.rs` | 2844 | agent-host detection and config writing |
| `recall.rs` | 2429 | RRF fusion, candidate pool, budget trim, scoring |
| `hooks.rs` | 1890 | the five lifecycle events; the never-fail hook contract |
| `mcp.rs` | 2050 | MCP server: 6 tools + resources + prompts + completion |
| `daemon.rs` | 1132 | `daemon start/stop/status`, pidfile, start-time identity |
| `doctor.rs` | 1217 | the health report (`doctor`, `doctor --all-hosts`) |
| `connect_plugin.rs` | 1092 | the hermes `MemoryProvider` plugin |
| `vector.rs` | 965 | dense arm (`embed` feature, OFF by default) |
| `cli_home.rs` | 948 | the content-first bare-invocation view |
| `connect_codex.rs` | 821 | codex TOML config writing |
| `connect_ext.rs` | 795 | omp and pi: the two hosts installed as a **TypeScript file**, not a config entry |
| `paths.rs` | 810 | bank identity, endpoint, data paths |
| `render.rs` | 655 | human-readable report rendering |
| `update.rs` | 654 | `update`: the five-step self-update swap |
| `seed.rs` | 649 | transcript reading for hook capture |
| `hygiene.rs` | 646 | W4: deterministic hygiene scan (merge/contradiction/date/bloat) |
| `mcp_http.rs` | 484 | stateless streamable-HTTP MCP mount |
| `connect_opencode.rs` | 384 | opencode's plugin dir (v2 layout) |
| `capture.rs` | 355 | hook-based auto-capture: dedup, privacy filter, raw observation |
| `grounding.rs` | 350 | W5: bounded startup grounding capsule |
| `connect_claude.rs` | 312 | Claude Code's MCP entry |
| `connect_copilot.rs` | 293 | Copilot CLI's MCP entry |
| `http.rs` | 280 | dependency-free HTTP/1.1 client for loopback |
| `skills.rs` | 279 | `connect --with-skills`: deploy the agent skill per host |
| `sweep.rs` | 276 | `sweep`: forget what a bank said it would stop remembering |
| `guidelines.rs` | 251 | native agent-rules writer (marker-delimited block) |
| `memory.rs` | 181 | canonical memory types shared by all layers |
| `connect_common.rs` | 177 | helpers the per-host connect modules share |
| `embed.rs` | 177 | ONNX session, vendored MiniLM |
| `lib.rs` | 51 | crate root: re-exports and the module map |

`tests/` — `backup.rs`, `daemon.rs`, `e2e.rs`, `recall_budget.rs`, `scale.rs`.
`examples/` — 15 benchmark and evaluation harnesses.
`eval/amb/` — our provider for vectorize-io's AMB harness, and its committed runs.


# SECTION B-DELTA

# FILE: research/audits/context-pack-2026-11-20-round5/B-DELTA.md (lines 1,148)

# B-DELTA — what landed after round 4 was sent

Round 4 was sent **2026-11-19 23:52** (`SEND.log`, page 263). Everything below
is after that. Round 4's two live claims were checked against the tree and both
are already closed; the delta records that, because a reviewer told "still
broken" when it is fixed will spend the round on a stale defect.

## D0. Round 4's own claims, re-checked — both closed

| round 4 claimed | tree now | evidence |
|---|---|---|
| "`extension/memory-wire.ts` lines 224–240 still filter `untrusted_external` **after** slicing the first `RECALL_ENTRIES` — an active N−1 bug today" | **closed** — `ed8eea4` (2026-11-20 00:20), 28 minutes after the send. The loop is now `for (var i = 0; i < items.length; i++)` with `if (isUntrusted(item)) continue;` and `if (lines.length >= RECALL_ENTRIES) break;` — filter first, take on *output* | `plugin/integrations/extension/memory-wire.ts` `formatRecall`, read 2026-11-20 |
| "TODO.md row 18 still reads `named-not-scheduled` despite completing 2026-10-17" | **closed** — `e0f1bb4` | `TODO.md` row 18 is `done` |

So round 4's *conclusion* (the N−1 shape recurs) is not contradicted, but its
**instances** are gone. Do not re-litigate either.

## D1. The opencode door had never loaded on its host

`3376b5c`. Found by probing the question round 4 could not answer (does either
TS host resolve imports?), not by reading.

- The shipped plugin exported a **v1 bare function**. opencode 2.x requires
  `{id, setup}` → `PluginModule.LoadError` on 2.3.2.
- It was installed as a **flat file**. opencode 2.x discovers plugins as
  *directories* and reads `<dir>/index.ts`; a flat file is skipped with **no
  error and no log line at all**.
- Fixed both; version marker 2→3; legacy flat file removed on install and
  uninstall. Verified live on 2.3.2, plus a stub-ctx drive proving injection and
  the retain round-trip.
- `session.idle` does **not** fire under `opencode run`; the capture hook is
  `session.execution.succeeded`.

**Why this is in the delta and not a footnote:** the round-4 review looked at
this door and called it healthy. It was dead on the host it named. That is the
same "checked in N−1 places" shape one layer out — the check covered what
reached a model and could not see that *nothing* did.

## D2. `connect_common` — six byte-identical helpers extracted

`2bfe837`. Measured first: grepping every `fn` signature in `src/connect*.rs`
and grouping by name found 11 repeats, of which **six were byte-identical
bodies** (diffed per pair, not inferred):

| helper | copies | what it encodes |
|---|---|---|
| `mcp_url` | claude, copilot, **codex** (3) | the bank is a `/mcp/<bank>` path segment, because the stateless mount pins the subtree |
| `baked` | ext, opencode | render the template with the bank baked into `DEFAULT_BANK` |
| `carries_marker` | ext, opencode | ownership = both markers, not the name |
| `is_ours` | ext, opencode | an unreadable file is not ours |
| `preserve_comment` | codex, hermes | keep the user's trailing `# comment` on a rewritten line |

Net **−138 lines** in the six modules, +177 for `connect_common.rs` (incl. its
tests). Callers import directly; no same-named wrappers.

**Two name collisions were deliberately NOT merged**, and the reason matters
for anyone tempted to "finish the job":

- `connect::is_ours(&Value)` tests a JSON entry; `connect_ext::is_ours(&Path)`
  reads a file. Merging means serializing a value to a string it already holds.
- `version` and `prune_empty_dirs` differ between ext and opencode by the
  directory they stop at (`extensions` vs `plugins`) and by how they treat a
  non-empty directory.

## D3. `memory_redact` on MCP — the sixth tool

`b3ecc3b`. TODO #34, the door-parity gap that was a **lockout**: an agent that
retained a secret through MCP could not retract it through MCP.

`MemoryService::redact_memory` and the `readOnly` guard already existed, so this
was a registration, a dispatch arm, and tests.

**The test caught a defect in itself, twice:**

1. It first used `sk-live-deadbeef` as the secret and got `redacted_rows: 0`.
   `capture::redact_pii` runs at **retain** time and its OpenAI rule
   (`sk-[A-Za-z0-9_-]{8,}`) eats that string before it reaches the store. A test
   built on it would have passed a tool that never had to work. It now seeds
   prose that survives the retain-time redactor, proves it is there by reading
   it back, then redacts. `memory_redact` exists for the *second* pass.
2. The tool's `pattern` is a **regex**, not a literal (the store compiles it and
   refuses an invalid one). The schema said "literal text" — wrong about the
   interface a client builds against.

Nine places stated the tool count and all said five or four; all corrected.

## D4. `doctor` names every bank

`6a2a125`. C2's deliverable. `collect` already computed per-bank counts and
threw them away, keeping only the single largest when the resolved bank is
empty. The screen now lists every bank with its count, largest first, and lists
the **resolved** bank even when it holds nothing (`<- resolved here, holds
nothing`) — a list built from `memories` rows cannot contain a zero-row bank.

Nothing else: no migration, no auto-move, no recommendation. Suppressed below
two banks.

## D5. Three gates that were wired to nothing

- `131cffb` — `plugin/integrations/trust_gate_test.sh` wired into
  `.githooks/pre-commit` (behind `command -v node`) and `ci.yml`. It had been
  running nowhere; both bugs it catches were found by hand.
- `4646e7a` — an **export-shape** check per door in the same script (opencode
  must be an object with `id`/`setup`; omp/pi must be a function). This is the
  check whose absence let D1 ship.
- `9e95d9b` — `scripts/module_table_check.sh`, which compares AGENTS.md §3's
  module set and per-module counts to the tree. **It caught its first real
  drift on the very next commit** (`2bfe837` added a module and shifted seven
  counts) and again on `b3ecc3b`.

## D6. Round 4's falsifiers, now measurable

Round 4 said it could not tell whether `connect_*` is mostly inline test
fixtures or duplicated wiring, and named the falsifier: ">50% of the 6,609 lines
are per-host inline `#[test]` fixtures." That split is now measured
(`measure_surface.sh`, §code-vs-test):

```
connect.rs          total=2844 code=1328 test=1516
connect_plugin.rs   total=1092 code=555  test=537
connect_codex.rs    total=821  code=387  test=434
connect_ext.rs      total=795  code=256  test=539
connect_opencode.rs total=384  code=258  test=126
connect_claude.rs   total=312  code=160  test=152
connect_copilot.rs  total=293  code=177  test=116
connect_common.rs   total=177  code=108  test=69
```

**7 modules, 6,718 lines: 3,229 code / 3,489 test — 52% test.** Round 4's
falsifier is therefore *satisfied on its own terms*, which is the evidence it
asked for and one of the two readings it could not choose between. The round
should treat that as an input, not a verdict: 52% is a test-to-code ratio, and
the question it bears on is whether a shared abstraction would save real
production lines or only fixture lines.

## D7. What did NOT change (so the reviewer does not re-derive it)

- No new dependency. Still 17 in `[dependencies]`, `DT_NEEDED`=3.
- `src/vector.rs` (965) + `src/embed.rs` (177) still compile to **zero bytes**
  in a default build (`#[cfg(feature = "embed")]`, `default = []`).
- The recall pipeline is untouched: FTS5 BM25 + overlap + RRF, budget trim.
  No ranking change, no weight change, no new signal.
- The trust gate's read-time filter is unchanged since `1f5ee17` except that it
  now exists in all four auto-injection doors.
- `docs/OPEN_HOOK_RECALL_RELEVANCE.md` is still deliberately unfixed: no score
  threshold, ever (CONSISTENCY §32/§48).
- Footprint: **11,166,048 B = 10.64 MiB**, `DT_NEEDED`=3, at `6a2a125`.
  Round 4 recorded 10.63 MiB; the delta is D2–D4 and is +5 KB.

# SECTION C-ASK

# FILE: research/audits/context-pack-2026-11-20-round5/C-ASK.md (lines 1,87)

# C-ASK — what this round exists to decide

## The question, stated plainly

**Where is memory-wire's architecture actually bottlenecked, redundant,
fragmented, or gapped on the path to an optimal agentic experience — and which
one of those is the one to fix next?**

Not "is it well built". Four rounds have established that it is. This round asks
for the *ranked* answer, with the ranking earned from the pack rather than
asserted.

## What the last four rounds established (do not re-derive)

| round | question | verdict, in one line |
|---|---|---|
| 1 (11-18) | is the W0–W6 plan the right shape? | yes on substance, no on process (the pack omitted the plan body — my defect) |
| 2 (11-18) | where is the shipped architecture? | per-turn hook injection is un-gated on relevance and trust → **adopted as the next move**; four bookkeeping defects; two refutations that must not be acted on |
| 3 (11-19) | should the next cycle keep optimising retrieval ordering? | no — it named the door-enforcement asymmetry instead |
| 4 (11-19) | is the `connect_*` family (i) the right shape or (ii) the N−1 risk? | (ii) holds for cross-surface capabilities; the smallest change is an *enumerated parity check*, not a shared abstraction. **Never verified** — the round was sent at 23:52 and no `VERIFICATION.md` was written |

Round 4's two *instances* were closed by D0. Its *conclusion* stands and is the
starting point here.

## The three things that changed the picture since round 4

1. **A door was dead and nobody knew** (D1). The opencode plugin did not load on
   opencode 2.x at all — wrong export shape, wrong install layout, and a flat
   file is skipped with no error. The round-4 review had called that door
   healthy. That is the N−1 shape one layer out: the check covered what reaches
   a model and could not see that *nothing* did.
2. **Round 4's falsifier is now measurable** (D6). It could not tell whether
   `connect_*` is mostly inline test fixtures or duplicated wiring. Answer:
   **52% test, 48% production** (3,489 / 3,229 of 6,718). That is evidence for
   one of its two readings, and this round should say which and why.
3. **Three gates now exist that previously ran nowhere** (D5). One of them
   (`module_table_check.sh`) caught real drift on its first outing.

## The tension to weigh

The stated competitive position is **the cheap option** (`AGENTS.md` §1): one
static binary, 10.64 MiB, 3 shared libraries, 17 dependencies, no daemon
required. Every structural change is paid against that.

Two readings of the same facts, and the pack deliberately does not pick one:

- **(A) The architecture's real bottleneck is the per-turn injection path.**
  Round 2 adopted this and it has not been re-measured since. The hook path
  injects up to the recall budget on every turn, and
  `docs/OPEN_HOOK_RECALL_RELEVANCE.md` is still deliberately unfixed (no score
  threshold, ever). The new D1 shows the injection doors can be *silently
  dead* — which is a worse failure than a noisy injection.
- **(B) The architecture's real bottleneck is now verification, not code.**
  Four of the last five defects were found by an external reviewer, not by the
  system's own checks (round 4's framing). Since then three gates were wired.
  Is that enough, or is the pattern that gates get written for what was just
  found and not for the next shape?

Both are consistent with the pack. **Decide, and say what would change your
mind.**

## What is NOT being asked

- Whether the recall pipeline should change. Ranking is frozen: every lever in
  `docs/handoff/CLOSED.md` was closed on measurement, and AGENTS §8 says
  rejected stays rejected.
- Whether to add a score threshold anywhere. Prohibited (§32/§48).
- Whether to cut a release. That is the operator's call (AGENTS §11) and is
  measured in CONSISTENCY §68.
- Whether `vector.rs`/`embed.rs` should be deleted. Closed by measurement
  (H1d) and they compile to zero bytes.

## The output I need

A ranked answer to the question, where each item carries:

1. **which of the four axes** it is (bottleneck / redundancy / fragmentation /
   integration gap),
2. **the artefact and the line** that evidences it — not a restatement of the
   pack's framing,
3. **its cost against the "cheap option" position** (§1), stated in the unit
   that position is measured in,
4. **what would have to be true for the opposite choice to be right** — the
   falsifier, not a caveat.

And then one thing the pack does not contain that you would want before
committing to the ranking. Name it; do not invent it.

# SECTION D-LAW

# FILE: AGENTS.md (lines 230,290)

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


# FILE: AGENTS.md (lines 309,339)

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


# SECTION E-SURFACE

# FILE: research/audits/context-pack-2026-11-20-round5/measure_surface.sh (lines 1,101)

#!/bin/sh
# Round-5 surface measurement. Every number packed in round 5 comes from this
# script, so the reviewer re-derives a figure instead of trusting a claim.
#
# This is round 4's script with its measured defects fixed:
#   - the host regex listed 6 of the 9 hosts connect.rs actually declares
#     (round 4 §2: "wouldn't even catch a 7th host or pi/omp")
#   - the AGENTS.md claim grep targeted the pre-2026-11-20 wording
#   - the code-vs-test split was never measured, which is what the round-4
#     falsifier (">50% of connect_* is inline #[test] fixtures") needed
#
# Run from the repo root: sh research/audits/context-pack-2026-11-20-round5/measure_surface.sh
set -eu
cd "$(dirname "$0")/../../.."

echo "## src/ — authored Rust, per module (wc -l; includes inline #[test] blocks)"
wc -l src/*.rs | sort -rn

echo
echo "## src/ — totals"
printf 'files: %s\n' "$(ls src/*.rs | wc -l | tr -d ' ')"
printf 'lines: %s\n' "$(cat src/*.rs | wc -l | tr -d ' ')"

echo
echo "## src/ — code vs inline tests, per module (the split round 4 flagged as unevidenced)"
# Everything above the module's `#[cfg(test)]` is production code; everything
# below is test. A per-file awk because the split is not derivable from wc.
for f in src/*.rs; do
  at=$(grep -n '^#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1)
  total=$(wc -l < "$f" | tr -d ' ')
  if [ -n "$at" ]; then code=$((at - 1)); else code=$total; fi
  printf '%s total=%s code=%s test=%s\n' "$(basename "$f")" "$total" "$code" "$((total - code))"
done | sort -t= -k3 -rn

echo
echo "## connect_* — the per-host adapters"
wc -l src/connect*.rs | sort -rn

echo
echo "## Doors (retrieval / injection surfaces)"
printf 'HTTP route registrations: %s\n' "$(grep -rhoE '\.route\("[^"]+"' src/*.rs | wc -l | tr -d ' ')"
grep -rhoE '\.route\("[^"]+"' src/*.rs | sed 's/.*route("//;s/"//' | sort -u
printf 'MCP tools registered in tools(): %s\n' "$(sed -n '/pub fn tools()/,/^    }/p' src/mcp.rs | grep -cE 'Tool::new\(')"
printf 'MCP prompts: %s\n' "$(sed -n '/fn prompts()/,/^    }/p' src/mcp.rs | grep -cE 'Prompt::new\(')"
printf 'CLI subcommands: %s\n' "$(sed -n '/^enum Cmd {/,/^}/p' src/main.rs | grep -cE '^    [A-Z]')"
sed -n '/^enum Cmd {/,/^}/p' src/main.rs | grep -E '^    [A-Z]' | sed 's/^ *//;s/[{,(].*//' | tr '\n' ' '
echo

echo
echo "## MCP tools and prompts"
grep -ohE 'pub const [A-Z_]+: &str = "memory_[a-z_]+"' src/mcp.rs \
  | sed 's/.*= "//;s/"//' | sort -u
echo "  (memory_historian is a PROMPT constant, not a tool — see tools() above)"

echo
echo "## Agent hosts — sourced from the enum connect.rs declares, not a literal list"
# Round 4's regex hardcoded six names and missed pi, omp and the plugin hosts.
# The Host enum is the authority; its variant arms are at one indent level.
sed -n '/^pub enum Host {/,/^}/p' src/connect.rs | grep -E '^    [A-Z]' | sed 's/^ *//;s/[{,(].*//' | tr '\n' ' '
echo
printf 'ALL_HOSTS / IMPLICIT count: %s / %s\n' \
  "$(sed -n '/pub const ALL: &\[Host\]/,/;/p' src/connect.rs | grep -c 'Host::')" \
  "$(sed -n '/pub const IMPLICIT: &\[Host\]/,/;/p' src/connect.rs | grep -c 'Host::')"

echo
echo "## plugin/ — the injection path (runs inside a model's context)"
find plugin -name '*.ts' -o -name '*.py' | grep -v node_modules | xargs wc -l 2>/dev/null | sort -rn

echo
echo "## TS plugin duplication — lines present verbatim in BOTH files"
A=plugin/integrations/extension/memory-wire.ts
B=plugin/integrations/opencode-plugin/memory-wire.ts
shared=$(comm -12 <(sort "$A") <(sort "$B") | grep -vE '^[[:space:]]*$|^[[:space:]]*(//|\*)' | wc -l | tr -d ' ')
a_code=$(grep -vE '^[[:space:]]*$|^[[:space:]]*(//|\*)' "$A" | wc -l | tr -d ' ')
b_code=$(grep -vE '^[[:space:]]*$|^[[:space:]]*(//|\*)' "$B" | wc -l | tr -d ' ')
printf 'shared verbatim: %s of %s (%.0f%%)\n' "$shared" "$b_code" \
  "$(python3 -c "print(100*$shared/$b_code)")"
printf 'extension non-comment: %s\n  opencode non-comment: %s\n' "$a_code" "$b_code"
echo "  (a shared line is shared syntax by construction: comm overcounts '}')"

echo
echo "## Authored docs (target/, research/ excluded)"
printf 'docs/*.md: %s\n' "$(ls docs/*.md | wc -l | tr -d ' ')"
printf 'docs/**/*.md: %s\n' "$(find docs -name '*.md' | wc -l | tr -d ' ')"

echo
echo "## AGENTS.md module table vs tree (gate: scripts/module_table_check.sh)"
sh scripts/module_table_check.sh || echo "  DRIFT"

echo
echo "## Dependency count (the default build must not gain one — AGENTS 1)"
awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f && /^[a-zA-Z0-9_-]+ *=/' Cargo.toml \
  | sed 's/[[:space:]]*=.*//' | tr '\n' ' '
echo
printf 'count: %s\n' "$(awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f && /^[a-zA-Z0-9_-]+ *=/' Cargo.toml | wc -l | tr -d ' ')"

echo
echo "## Footprint (AGENTS 1: one static binary, 3 shared libs)"
ls -l target/release/memory-wire | awk '{printf "binary bytes: %s\n", $5}'
readelf -d target/release/memory-wire 2>/dev/null | grep -c NEEDED || echo 0
echo "  ^ DT_NEEDED count"

# SECTION F-CLOSED

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

# SECTION G-RELEVANCE

# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 6,85)

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


# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 103,111)

## What is NOT wrong, and must not be "fixed"

- **Do not add a score threshold to `recall`.** `AGENTS.md` §1 forbids acquiring
  one without a dev set, and the reasoning at `recall.rs:330-334` is correct. A
  hard-coded cut would be a fitted constant with no evaluation behind it — the
  exact failure mode this project has already paid for once in the fusion weights.
- **Do not treat "it returned something" as a bug in FTS5 or the BM25 arm.** Both
  are measured to work; the corpus is simply not about the query.


# SECTION H-GAPS

# FILE: docs/INTEGRATION_GAPS.md (lines 77,157)

## G1 — No persistent service, and our hooks fail silently without one

**Status: resolved.** `memory-wire daemon {start,stop,status}` ships
(`src/daemon.rs`, wired in `src/main.rs`). `start` re-execs the binary with
`serve --addr … [--db …]` — the same flags `serve` already took, passed straight
through — detached into its own session with `libc::setsid()` in
`Command::pre_exec`, and returns only after `/health` has answered `ok`, so a
start that printed success is a start that worked. State lives in one file,
`$XDG_DATA_HOME/memory-wire/serve.json` (the directory is the parent of
`default_db_path`, not a second reading of `XDG_DATA_HOME`), beside the store and
the `serve.log` the child's stdio is redirected to. `stop` sends SIGTERM — the
signal `serve` already drains on — waits for exit, and removes the state file.
`serve` itself, the store schema, the HTTP routes and MCP are untouched; the
dependency delta is `libc` promoted from transitive to direct, which
`cargo tree --edges normal` already showed in the tree, so the default build's
`DT_NEEDED` count is still three.

Everything below is the audit that motivated it and stays as the record. Three
decisions inside it are worth naming, because each is a place the obvious
implementation is wrong:

**The port is the lock, and the check is in the parent.** `start` attempts the
bind itself before spawning anything, so a second `start` refuses with the reason
on the terminal and a nonzero exit instead of leaving a child that dies on bind.
No lockfile, so there is nothing to go stale — hindsight's arrangement, quoted
below, and the reason it is better than the alternative.

**A pid is not an identity, so the state file carries a start time.** The audit
did not ask for this and the design is better for it. The brief's staleness rule
was "pid alive but the endpoint does not answer → report both facts, do not kill
it", which is safe but leaves one case unresolved: a pid that was reused by an
unrelated process *while a memory-wire server is also answering on the port*
would pass an endpoint-only check, and `stop` would signal a stranger. The state
file therefore records `/proc/<pid>/stat` field 22 next to the pid, and
`daemon status` reports a mismatch as a recycled pid — `not running`, with the
reason attached. Off Linux there is no such field, the check degrades to
endpoint-only, and `status` says that in the line next to the pid rather than
assuming it away. This closes G6.1 (`stop`/`restart`) at the same time;
`restart` is still `stop` then `start`.

**`setsid` does not return 0 on success here, and the difference is a silent
no-op daemon.** The first implementation tested `setsid() == 0`, which is what
POSIX documents. On this platform (glibc, Linux 6.x, rustc 1.98) it returns the
new session id instead, so every `daemon start` reported
`could not spawn the detached server: No such file or directory (os error 2)` and
exited 1 — with the detach never happening. `-1` is the documented failure *and*
is what the standard library itself compares against, so the test is now on `-1`.
This is recorded here rather than only in the code comment because it is the kind
of thing that silently reintroduces itself the next time someone tidies the
comparison.

**The gap.** Every host we integrate with needs a running server. We never
start one, never keep one alive, and never report one.

**Why it is worse for us than for them.** Our hook contract is
deliberately never-fail (`src/hooks.rs:3-7`): every failure path is silent, and
`run` returns 0 unconditionally (`src/hooks.rs:58,76`). That is correct in
isolation — a memory server that is down must not put an error in front of a
model. But it converts "server not running" from a visible error into **silent
memory loss with zero signal.** agentmemory's hooks can fail loudly; ours
cannot. So they need a daemon more than we do, and we have less.

**What exists instead.** `doctor` (`src/doctor.rs:73-92`) reports endpoint, bank,
store path/size/integrity, and server state — and it is the *only* thing standing
between a user and a silent no-op.

**What the competitors do.** `agentmemory status` hits
`health`/`sessions`/`graph/stats`/`config/flags` (`src/cli.ts:2112-2205`);
`agentmemory stop` retires worker-then-engine in that order for a documented
reason (`src/cli.ts:3538-3543`); `remove` is a double-confirmed destruction plan
(`src/cli/remove-plan.ts:1-13`). hindsight exposes
`hindsight-embed daemon {start,stop,status,logs}` — the fullest lifecycle
surface — and `hindsight fs {start,stop,restart,status}`.

**The honest size of this.** A `memory-wire daemon {start,stop,status}` that
writes a pidfile and detaches is roughly the work agentmemory's
`spawnEngineBackground` does. It is not exotic. It is also the single change
that would most improve the experience of every existing user.

---


# FILE: docs/INTEGRATION_GAPS.md (lines 285,323)

## G5 — 4 MCP tools against 8, 39 and 54, and stdio only

> **Transport half RESOLVED 2026-09-29. Tool count deliberately unchanged.**
> `memory-wire serve` now answers MCP over HTTP at `/mcp` and `/mcp/{bank}` — the
> second pins the bank, and a call naming a different one is rejected rather than
> silently honoured. `connect codex` writes a `[mcp_servers.memory-wire]` `url`
> entry, so Codex can finally call the tools; previously it got hooks and no tool
> access at all. Zero new dependencies. Cost: **+1,084,720 B (+12.0%)**, 8.6 → 9.7
> MiB, download 3.5 → 3.9 MiB, `DT_NEEDED` still 3. Full detail, including why
> **2026-11-20: redact landed on MCP** (`memory_redact`, CONSISTENCY §65), so
> this table is now six tools against their 8/39/54 — still below three of the
> four, and the row is kept as the record of what the gap looked like.
>
> Still open: six tools against their 8/39/54, and rmcp's default loopback-only
> `Host` allowlist, which is a good default but is not authentication.

Our surface: `memory_retain`, `memory_recall`, `memory_reflect`,
`memory_bank_config_get`, `memory_supersede`, `memory_redact` (`src/mcp.rs`). Transport is stdio only
(`src/mcp.rs:297`); `INSTALL_FOR_AGENTS.md:448` states plainly that
`/mcp/:bank` URL scoping "is not built."

Consequences a user can observe:

- **No HTTP MCP.** A host that can only reach an HTTP MCP server cannot use us.
  hindsight is streamable-HTTP-first (`api/__init__.py:88-100`); hermes
  configures MCP by `url`; agentmemory ships REST-emulated MCP endpoints
  (`src/mcp/server.ts:72,1295,1348`).
- **No per-bank URL scoping.** hindsight's `/mcp/{bank_id}` path segment swaps
  the entire tool set and every schema (`api/mcp.py:499-504,518`). Our
  resolution is a per-call argument with a server default
  (`src/mcp.rs:184-189`) — which hindsight's comment says is the *right*
  internal shape, so ours is compatible with that mode when it lands.
- **No lifecycle tools over MCP.** `SKILL.md:49-50` admits list/get/delete/stats
  are HTTP-only.
- **4 tools is a thin surface.** hindsight's plugin ships 8 for the same job;
  agentmemory ships 54.

---


# FILE: docs/INTEGRATION_GAPS.md (lines 460,530)

## Addendum 2026-11-19 — the doors are asymmetric, and that is the honest state

Recorded because round-2 review read "one service, three doors" as a promise of
parity. It is not. It is a claim about `MemoryService`: there is **one**
implementation, reached through doors that each expose a **different subset**.
Verified per capability against this tree:

| capability | HTTP | MCP | CLI | hook |
|---|---|---|---|---|
| redact (W3) | no route | no tool | **yes** (`Cmd::Redact`) | no |
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

`src/mcp.rs` has seven `pub const …: &str = "memory_…"` constants, but only
**six** are tools: `HISTORIAN` (`memory_historian`) is a **prompt**, advertised
through `prompts/list` and served by `prompts/get`, and `tools()` does not
register it. The server exposes six tools and one prompt. Round-2 review read
six constants as six tools and concluded the README overcounted; the README's
prose ("five tools") was right at the time, and its table had been the thing
missing `memory_supersede`. `memory_redact` arrived 2026-11-20 (CONSISTENCY
§65), so the constants/tools offset is unchanged at one: the next count will
find seven constants and six tools, which is correct. The `HISTORIAN` doc
comment says this in place, so the next count does not re-add it.

### A fourth door asymmetry, found the same round: the trust gate was Rust-and-hermes only

`Trust::UntrustedExternal` was added in `1f5ee17` and both `src/hooks.rs` and the
hermes plugin gate on it. The **two TypeScript plugins did not**: both called
`formatRecall` on everything a recall returned, so an untrusted mount's web
content was injected verbatim into a model's system prompt. The weakest door
decided what the bank's untrusted rows could reach, and it was the two files with
no Rust test suite to catch it.

Fixed by carrying the same gate into both plugins, with the same evidence-on-
absence rule (a row with no `trust` field is kept, because a server old enough
not to send one is not evidence). Proof: `plugin/integrations/trust_gate_test.sh`
executes each file's real `formatRecall` in `node` and asserts that
`untrusted_external` is withheld while an absent field is not; removing either
gate makes it fail with `untrusted_external reached the block`.

Why this is recorded here rather than closed silently: three doors implementing
the same gate in three languages is already a maintenance surface, and
`measure_surface.sh` did not count `plugin/` at all — the count this repo uses to
describe its own surface stopped at `src/`, which is precisely how 1,622 lines
of injection-path code stayed unexamined. `measure_surface.sh` now reports
`plugin/` and `install/` **separately** rather than folding them into a total,
because a TypeScript plugin is not a Rust module and one misleading sum would
hide exactly that.

# SECTION I-DOORS

# FILE: src/hooks.rs (lines 737,789)

/// **The trust gate lives here.** Rows whose tier is `untrusted_external` are
/// dropped before rendering, because this function's output is the only path by
/// which a recalled memory reaches a model without a reader asking for it.
///
/// The ordering matters: filtering happens before the [`MAX_LINES`] take, so an
/// untrusted row does not consume a slot that a trusted row would have filled. A
/// gate applied after the take would silently *reduce* recall to fewer than
/// `MAX_LINES` trusted rows whenever the ranking put untrusted rows on top —
/// which is exactly the adversarial case the gate exists for, where fetched text
/// is shaped to rank first.
fn recall_section(bank: &str, hits: &[ScoredHit]) -> String {
    let hits: Vec<&ScoredHit> = hits
        .iter()
        .filter(|h| !is_untrusted(h.trust.as_deref()))
        .collect();
    if hits.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "\n### memory-wire recall — `{bank}`\n(recalled by relevance, unverified)\n\n"
    );
    let mut left = SECTION_CAP_CHARS;
    for hit in hits.iter().take(MAX_LINES) {
        // The date prefix is the serving-shape change H12 measured: a reader
        // resolving "what did we decide last week" is blind without it, and the
        // wire carried the occurrence time all along. The prefix renders only
        // when the wire did — an old server's bare strings degrade to the exact
        // pre-2026-10-17 rendering, byte for byte.
        let date_prefix = hit
            .created_at
            .as_deref()
            .filter(|t| t.len() >= 10)
            .map(|t| format!("[{}] ", &t[..10]))
            .unwrap_or_default();
        let entry = format!("- {date_prefix}{}\n", clamp(&hit.content, ENTRY_CAP_CHARS));
        if entry.chars().count() > left {
            break;
        }
        out.push_str(&entry);
        left -= entry.chars().count();
    }
    if hits.len() > MAX_LINES {
        out.push_str(&format!("- …and {} more\n", hits.len() - MAX_LINES));
    }
    out
}

/// Whether a tier string names the one tier the injection block refuses.
///
/// Only a positive match drops a row. `None` (the bare wire shape, or a server
/// too old to send the field) keeps it, and so does any tier this build does not
/// recognise — including one a *newer* server invents. Both are the same
/// judgement: a row is withheld on evidence, never on the absence of evidence.

# FILE: plugin/integrations/hermes/memory-wire/__init__.py (lines 508,573)

    def prefetch(self, query: str, *, session_id: str = "") -> str:
        """Recall for the upcoming turn, as injected context.

        The signature is keyword-only on ``session_id`` because that is what
        ``MemoryManager._prefetch_provider`` calls
        (``provider.prefetch(query, session_id=session_id)``). A provider that
        took it positionally would be called correctly and then be wrong about
        which argument it received.

        Returns "" for a blank query, an unreachable server, an empty bank, and a
        server that errored. Hermes wraps this in its own thread with an 8s
        deadline and logs a warning if we exceed it, so one TIMEOUT is the whole
        cost of a down server.

        The top ``MAX_LINES`` are injected. Each carries its fused RRF ``score``
        — see :func:`_recall` for what it is and why nothing branches on it —
        and the ranking is *relative*, so this emits the best of whatever set
        the bank offered, relevant or not. That is the defect
        ``docs/OPEN_HOOK_RECALL_RELEVANCE.md`` records. The score reaches this
        function, but the policy half stays closed on determinism grounds
        (CONSISTENCY §32/§48) rather than for want of data: AGENTS §6 designates
        ``eval/data/locomo/`` as the dev set, ~1,530 queries.

        One thing *is* filtered, and it is not relevance: rows marked
        :data:`UNTRUSTED_TIER` are dropped, because this function's output is
        how a recalled memory reaches a model without anyone asking for it. An
        explicit ``memory_recall`` still returns those rows. See
        :data:`UNTRUSTED_TIER` for why this is a boundary and not a ranking rule.
        """
        if not _valid_url(self._base):
            return ""
        hits = _recall(self._base, self._bank, _strip_reminders(query), EXCLUDE_TAGS)
        if not hits:
            return ""
        # Before the take, not after: a poisoned row at rank 0 must not consume
        # a slot a trusted row would have filled. Ranking untrusted text first
        # is exactly the adversarial case this exists for.
        hits = [h for h in hits if h.get("trust") != UNTRUSTED_TIER]
        if not hits:
            return ""
        # The shape every auto-injection surface emits (audit GAP-1): the bank
        # header, the provenance label — a best-of-irrelevant set must not read
        # as a verified one — and at most MAX_LINES entries, each clamped, the
        # whole block under SECTION_CAP_CHARS.
        out = (
            f"### memory-wire recall — `{self._bank}`\n"
            "(recalled by relevance, unverified)\n\n"
        )
        left = SECTION_CAP_CHARS
        for hit in hits[:MAX_LINES]:
            # The date prefix renders only when the wire carried one, so a
            # server older than the field degrades to the exact pre-2026-11-19
            # rendering, byte for byte. Same shape as `src/hooks.rs`:
            # `[<YYYY-MM-DD>] ` before the clamped content, day precision
            # because that is what an occurrence time is for.
            stamp = hit.get("created_at") or ""
            prefix = f"[{stamp[:10]}] " if len(stamp) >= 10 else ""
            entry = f"- {prefix}{_clamp(hit['content'], ENTRY_CAP_CHARS)}\n"
            if len(entry) > left:
                break
            out += entry
            left -= len(entry)
        if len(hits) > MAX_LINES:
            out += f"- …and {len(hits) - MAX_LINES} more\n"
        return out


# FILE: plugin/integrations/extension/memory-wire.ts (lines 205,356)

var UNTRUSTED_TIER = "untrusted_external";

/** A row a mount may not read back as instructions. Absent means keep. */
function isUntrusted(item) {
  if (!item || typeof item !== "object") return false;
  return item.trust === UNTRUSTED_TIER;
}

/** The block appended to the system prompt, or `""` when there is nothing worth
 *  saying. Empty is the signal the caller uses to inject nothing at all.
 *
 *  `items` are the server's `{id, score, content}` entries, so each carries the
 *  fused RRF score the ranking used. It is *relative* — the top hit of an
 *  irrelevant set scores well by construction, because BM25 is min-max
 *  normalised per query — and nothing branches on it here, and nothing will: a
 *  score threshold on the injection path was measured and rejected twice
 *  (CONSISTENCY §32/§48) on determinism grounds. The reason is not a shortage
 *  of labelled data — AGENTS §6 designates `eval/data/locomo/` as the dev set,
 *  ~1,530 queries. A cut would be fitted on one corpus and would move when the
 *  ranking moved. The top `RECALL_ENTRIES` are injected unconditionally; see
 *  `docs/OPEN_HOOK_RECALL_RELEVANCE.md`. */
function formatRecall(items) {
  var lines = [];
  for (var i = 0; i < items.length; i++) {
    var item = items[i];
    // Filtered **before** the take, and the take is `lines.length` below —
    // not `i`. Looping on `i < RECALL_ENTRIES` would bound the *input* scan,
    // so five untrusted rows ranked above one trusted row would consume the
    // whole budget and the trusted row would never be reached. That is a
    // starvation bug, not a cap: `src/hooks.rs` and the hermes plugin both
    // filter first and take second, and this file has to agree with them.
    if (isUntrusted(item)) continue;
    // The default recall response is a bare array of content strings; a
    // `format: "full"` server answers with `{id, score, content}` instead. Both
    // are accepted so this file does not depend on which one it is talking to.
    var content = typeof item === "string" ? item : item && item.content;
    if (typeof content !== "string" || !content.trim()) continue;
    if (lines.length >= RECALL_ENTRIES) break;
    // The occurrence time, when the wire carried one. `src/hooks.rs` prefixes
    // it for the same reason H12 measured: a reader resolving "what did we
    // decide last week" is blind without it. It renders only when the field is
    // there, so a server older than it degrades to the exact pre-2026-11-19
    // rendering, byte for byte. Day precision -- that is what an occurrence
    // time is for.
    var stamp = (item && item.created_at) || "";
    var datePrefix = stamp.length >= 10 ? "[" + stamp.slice(0, 10) + "] " : "";
    lines.push(lines.length + 1 + ". " + datePrefix + clamp(content, ENTRY_CAP_CHARS));
  }
  if (!lines.length) return "";
  var block =
    "## memory-wire long-term memory\n" +
    "(recalled by relevance from bank `" + bank() + "`, unverified)\n" +
    lines.join("\n");
  return block.length <= INJECT_CAP_CHARS ? block : block.slice(0, INJECT_CAP_CHARS - 1) + "…";
}

/** memory-wire's memory extension: recall injected before each turn, the turn
 *  retained after it.
 *  @param {{ on: (event: string, handler: Function) => void }} pi */
export default function memoryWireExtension(pi) {
  var lastPrompt = "";
  var lastAnswer = "";
  var lastRetained = "";

  pi.on("before_agent_start", async function (event) {
    var prompt = event && typeof event.prompt === "string" ? stripReminders(event.prompt) : "";
    if (!prompt) return undefined;
    // Recorded before the await, so a recall that fails still leaves the prompt
    // available to `agent_end`.
    lastPrompt = prompt;

    // A prompt this short cannot rank anything on its own: "Continue" matches
    // every memory equally and the top of that tie is noise. The query borrows
    // the previous turn's answer, so what ranks is the work being continued,
    // with the prompt still steering inside it. A mechanism, not a filter --
    // nothing is cut on a score (docs/OPEN_HOOK_RECALL_RELEVANCE.md).
    var query = prompt;
    if (prompt.length <= SHORT_PROMPT_CHARS && lastAnswer) {
      query = clamp(lastAnswer, QUERY_CONTEXT_CHARS) + "\n" + prompt;
    }

    var items = await post(
      "/banks/" + encodeURIComponent(bank()) + "/recall",
      // `format: "full"` is what puts `score` on the wire; the default shape is a
      // bare array of content strings. `formatRecall` reads both, so this stays
      // compatible with a server that does not know the field.
      // `MEMORY_WIRE_FORMAT=summary` asks for H8 capsules instead of whole
      // memories. It comes from the environment rather than the bank's
      // `injectionFormat` config key because this file already reads
      // `MEMORY_WIRE_URL` and `MEMORY_WIRE_BANK` from there, so it costs no
      // extra request; reading the config key would add one per turn, which
      // `src/hooks.rs` documents as already costing one. The Rust hook reads
      // both and the bank wins there. Anything but "summary" reads as default.
      {
        query: query,
        budget: RECALL_BUDGET,
        format:
          (process.env.MEMORY_WIRE_FORMAT || "").trim().toLowerCase() === "summary"
            ? "summary"
            : "full",
        exclude_tags: EXCLUDE_TAGS,
      },
      RECALL_TIMEOUT_MS
    );
    if (!Array.isArray(items)) return undefined;
    var block = formatRecall(items);
    if (!block) return undefined;

    // Returning `systemPrompt` REPLACES it, so the harness's own prompt has to be
    // carried through or its instructions are silently erased. A host may hand
    // it over as a string or as a list of segments; both are joined rather than
    // stringified, which would collapse a list into one comma-riddled line.
    var own = event.systemPrompt;
    if (Array.isArray(own)) own = own.join("\n\n");
    return { systemPrompt: [own, block].filter(Boolean).join("\n\n") };
  });

  pi.on("agent_end", async function (event) {
    var prompt = lastPrompt;
    // Cleared before the await: a turn that starts before this one finishes must
    // not have its prompt consumed by the earlier retain.
    lastPrompt = "";
    var answer = lastAssistantText(event && event.messages);
    // The next turn's short-prompt query composition borrows this answer.
    lastAnswer = answer;
    // An unanswered or bare-acknowledgement turn is a question with no knowledge
    // in it: retaining prompt-only rows is how the bank filled with duplicate
    // question texts ("asked: continue", asked three times over) that recall
    // then injected as if they were memory. The pair is the memory.
    if (!prompt || !answer || answer.length < MIN_ANSWER_CHARS) return undefined;

    var content = [
      "asked: " + clamp(prompt, RETAIN_PART_CAP_CHARS),
      "answered: " + clamp(answer, RETAIN_PART_CAP_CHARS),
    ].join("\n");
    // An auto-retry re-submits the same turn, and the server dedupes identical
    // content anyway -- but not before paying for the write, so it is checked here.
    if (!content || content === lastRetained) return undefined;

    // `document_id` keyed on the prompt makes a repeated ask replace its row
    // (the server's default update mode) rather than add a twin.
    var stored = await post(
      "/banks/" + encodeURIComponent(bank()) + "/retain",
      { content: content, document_id: "turn-" + hash(prompt) },
      RETAIN_TIMEOUT_MS
    );
    // Only a confirmed store updates the marker, so a retain that failed is
    // retried on the next turn rather than dropped for the rest of the session.
    if (stored) lastRetained = content;
    return undefined;
  });
}

# FILE: plugin/integrations/opencode-plugin/memory-wire.ts (lines 176,462)

var UNTRUSTED_TIER = "untrusted_external";

/** A row a mount may not read back as instructions. Absent means keep. */
function isUntrusted(item) {
  if (!item || typeof item !== "object") return false;
  return item.trust === UNTRUSTED_TIER;
}

/** The block appended to the system prompt, or `""` when there is nothing
 *  worth saying. Empty is the signal the caller uses to inject nothing at all.
 *
 *  `items` are the server's `{id, score, content}` entries, so each carries
 *  the fused RRF score the ranking used. It is *relative* -- the top hit of an
 *  irrelevant set scores well by construction, because BM25 is min-max
 *  normalised per query -- and nothing branches on it here, and nothing will: a
 *  score threshold on the injection path was measured and rejected twice
 *  (CONSISTENCY §32/§48) on determinism grounds. The reason is not a shortage
 *  of labelled data -- AGENTS §6 designates `eval/data/locomo/` as the dev set,
 *  ~1,530 queries. A cut would be fitted on one corpus and would move when the
 *  ranking moved. The label line exists instead: it says what these rows are,
 *  so an irrelevant set cannot read as a verified one. The top
 *  `RECALL_ENTRIES` are injected unconditionally either way; see
 *  docs/OPEN_HOOK_RECALL_RELEVANCE.md. */
function formatRecall(items) {
  var lines = [];
  for (var i = 0; i < items.length; i++) {
    var item = items[i];
    // Filtered **before** the take, and the take is `lines.length` below —
    // not `i`. Looping on `i < RECALL_ENTRIES` would bound the *input* scan,
    // so five untrusted rows ranked above one trusted row would consume the
    // whole budget and the trusted row would never be reached. That is a
    // starvation bug, not a cap: `src/hooks.rs` and the hermes plugin both
    // filter first and take second, and this file has to agree with them.
    if (isUntrusted(item)) continue;
    // The default recall response is a bare array of content strings; a
    // `format: "full"` server answers with `{id, score, content}` instead.
    // Both are accepted so this file does not depend on which one it is
    // talking to.
    var content = typeof item === "string" ? item : item && item.content;
    if (typeof content !== "string" || !content.trim()) continue;
    if (lines.length >= RECALL_ENTRIES) break;
    // The occurrence time, when the wire carried one. `src/hooks.rs` prefixes
    // it for the same reason H12 measured: a reader resolving "what did we
    // decide last week" is blind without it. It renders only when the field is
    // there, so a server older than it degrades to the exact pre-2026-11-19
    // rendering, byte for byte. Day precision -- that is what an occurrence
    // time is for.
    var stamp = (item && item.created_at) || "";
    var datePrefix = stamp.length >= 10 ? "[" + stamp.slice(0, 10) + "] " : "";
    lines.push(lines.length + 1 + ". " + datePrefix + clamp(content, ENTRY_CAP_CHARS));
  }
  if (!lines.length) return "";
  var block =
    "## memory-wire long-term memory\n" +
    "(recalled by relevance from bank `" + bank() + "`, unverified)\n" +
    lines.join("\n");
  return block.length <= INJECT_CAP_CHARS ? block : block.slice(0, INJECT_CAP_CHARS - 1) + "…";
}

/** The newest assistant text of the session, via `ctx.session.context`.
 *
 *  In v2 the context *is* the client (`docs/build/plugins/server`), so there
 *  is no separate `client.session.messages` to reach for -- the v1 code called
 *  that and it no longer exists. `context()` takes `{ sessionID }` and returns
 *  the session's messages, verified live against 2.3.2 (it schema-validates
 *  the id before doing any work).
 *
 *  `""` whenever any of it is missing, refused, or slow: a retain that cannot
 *  read the answer is skipped, and the turn is simply not remembered. Nothing
 *  here may throw into the agent.
 *
 *  @param {{ context?: Function }} session the v2 session domain */
async function lastAssistantTextFor(session, sessionID) {
  if (!session || typeof session.context !== "function" || !sessionID) {
    return "";
  }
  var list;
  try {
    var bounded = Promise.withResolvers();
    var timer = setTimeout(bounded.resolve, RETAIN_TIMEOUT_MS);
    try {
      var res = await Promise.race([session.context({ sessionID: sessionID }), bounded.promise]);
      // The v2 client answers `{ data }`; a bare array is accepted too, so a
      // shape change costs nothing.
      list = Array.isArray(res) ? res : res && typeof res === "object" ? res.data : undefined;
    } finally {
      clearTimeout(timer);
    }
  } catch {
    return "";
  }
  if (!Array.isArray(list)) return "";
  for (var i = list.length - 1; i >= 0; i--) {
    var entry = list[i];
    var info = entry && typeof entry === "object" ? entry.info : undefined;
    if (!info || info.role !== "assistant") continue;
    var text = textOf(entry.parts);
    if (text) return text;
  }
  return "";
}

/** memory-wire's memory plugin: recall injected before each turn, the turn
 *  retained after it.
 *
 *  **OpenCode v2, not v1.** Until 2026-11-20 this file exported the v1 shape --
 *  `export default async (input) => ({...hooks})` -- and opencode 2.x refuses it
 *  at load with `PluginModule.LoadError: Plugin must export a default
 *  definition with an id and an effect or setup function`. The door was
 *  therefore *silently dead* on every 2.x install: the file is copied to disk,
 *  `connect` reports `wired`, and nothing runs it. The plugin is not listed by
 *  `opencode plugin list`, and its memory injection never happens.
 *
 *  The v2 contract (`opencode.ai/docs/build/plugins/migrate-v1`): a default
 *  object with `id` and `setup(ctx)`, hooks registered through `ctx` rather
 *  than returned as a string map. Local plugins must live in a **directory**,
 *  so the install path is `plugins/memory-wire/index.ts` -- a bare file now
 *  only loads if configured, and `connect` writes no such entry (see
 *  `src/connect_opencode.rs`).
 *
 *  The three hook ports that matter:
 *    `chat.message`                                    -> `session.hook("prompt")`
 *    `experimental.chat.system.transform`              -> `session.hook("context")`
 *    `event` + `session.idle`                          -> `ctx.event.subscribe()`
 *  In v2 the callbacks take **one mutable event**, not `(input, output)`, and
 *  `ctx.session` replaces the old `client.session`. Everything below the host
 *  boundary -- the caps, `post`, `formatRecall`, the trust gate -- is the same
 *  code as the extension door, unchanged.
 *
 *  The `id` is not cosmetic: plugin storage is scoped by it and diagnostics
 *  name it, so it has to stay stable across versions or a user's stored
 *  plugin state stops being reached.
 *  @param {{
 *    session: { hook: Function, messages: Function },
 *    event: { subscribe: Function },
 *    location: object,
 *    options: object,
 *    storage: object
 *  }} ctx the v2 plugin context */
export default { id: "memory-wire", setup: setup };

function setup(ctx) {
  var session;
  var event;
  var subscribe;
  // v2 groups these on the context; v1 put them on a `client` argument. Guarded
  // rather than destructured, because a plugin that throws inside `setup`
  // fails to load rather than failing to recall -- the door would be dead
  // again, which is the bug this port exists to undo.
  session = ctx && typeof ctx === "object" ? ctx.session : undefined;
  event = ctx && typeof ctx === "object" ? ctx.event : undefined;
  subscribe = event && typeof event.subscribe === "function" ? event.subscribe : undefined;
  var lastPrompt = "";
  var lastAnswer = "";
  var lastRetained = "";
  // A multi-step turn calls the hook once per step; one recall per distinct
  // query keeps step N from paying (or duplicating) step 1's recall.
  var injectedQuery = "";
  var injectedBlock = "";

  if (session && typeof session.hook === "function") {
    // The user message text for the turn in flight. The hook hands over the
    // event; reading its parts is the whole capture -- no fetch needed for
    // what arrives in the event.
    session.hook("prompt", function (ev) {
      var parts =
        ev && typeof ev === "object" && Array.isArray(ev.parts) ? ev.parts : [];
      var text = stripReminders(textOf(parts));
      if (text) lastPrompt = text;
    });

    // The injection point. Pushing onto `ev.system` reaches the model as
    // system content.
    session.hook("context", async function (ev) {
      if (!(ev && typeof ev === "object" && Array.isArray(ev.system))) return;
      var prompt = lastPrompt;
      if (!prompt) return;

      // A prompt this short cannot rank anything on its own: "Continue"
      // matches every memory equally and the top of that tie is noise. The
      // query borrows the previous turn's answer, so what ranks is the work
      // being continued, with the prompt still steering inside it. A
      // mechanism, not a filter -- nothing is cut on a score
      // (docs/OPEN_HOOK_RECALL_RELEVANCE.md).
      var query = prompt;
      if (prompt.length <= SHORT_PROMPT_CHARS && lastAnswer) {
        query = clamp(lastAnswer, QUERY_CONTEXT_CHARS) + "\n" + prompt;
      }

      if (query === injectedQuery) {
        if (injectedBlock) ev.system.push(injectedBlock);
        return;
      }
      var items = await post(
        "/banks/" + encodeURIComponent(bank()) + "/recall",
        // `format: "full"` is what puts `score` on the wire; the default shape
        // is a bare array of content strings. `formatRecall` reads both, so
        // this stays compatible with a server that does not know the field.
        // `MEMORY_WIRE_FORMAT=summary` asks for H8 capsules instead of whole
        // memories. It comes from the environment rather than the bank's
        // `injectionFormat` config key because this file already reads
        // `MEMORY_WIRE_URL` and `MEMORY_WIRE_BANK` from there, so it costs no
        // extra request; reading the config key would add one per turn, which
        // `src/hooks.rs` documents as already costing one. The Rust hook reads
        // both and the bank wins there. Anything but "summary" reads as default.
        {
          query: query,
          budget: RECALL_BUDGET,
          format:
            (process.env.MEMORY_WIRE_FORMAT || "").trim().toLowerCase() === "summary"
              ? "summary"
              : "full",
          exclude_tags: EXCLUDE_TAGS,
        },
        RECALL_TIMEOUT_MS
      );
      var block = Array.isArray(items) ? formatRecall(items) : "";
      injectedQuery = query;
      injectedBlock = block;
      if (block) ev.system.push(block);
    });
  }

  if (subscribe) {
    var controller = new AbortController();
    void (async () => {
      try {
        for await (var ev of subscribe({ signal: controller.signal })) {
          // Turn end. `session.idle` still exists in 2.3.2 and still carries
          // `{sessionID}`, but it does NOT fire under `opencode run` --
          // measured, over a real turn, 31 other event types delivered and
          // idle not among them. `session.execution.succeeded` does, and it
          // is the narrower claim anyway: the execution completed rather
          // than the UI going quiet. Verified live as
          // `{"type":"session.execution.succeeded", ...}` with the
          // `sessionID` on the event, not in `properties`.
          if (!(ev && typeof ev === "object" && ev.type === "session.execution.succeeded")) {
            continue;
          }
          var sessionID = typeof ev.sessionID === "string" ? ev.sessionID : "";
          var prompt = lastPrompt;
          // Cleared before the await: a turn that starts before this one
          // finishes must not have its prompt consumed by the earlier
          // retain.
          lastPrompt = "";
          var answer = await lastAssistantTextFor(session, sessionID);
          // The next turn's short-prompt query composition borrows this
          // answer.
          lastAnswer = answer;
          // An unanswered or bare-acknowledgement turn is a question with no
          // knowledge in it: retaining prompt-only rows is how the bank fills
          // with duplicate question texts that recall then injects as if they
          // were memory. The pair is the memory.
          if (!prompt || !answer || answer.length < MIN_ANSWER_CHARS) continue;

          var content = [
            "asked: " + clamp(prompt, RETAIN_PART_CAP_CHARS),
            "answered: " + clamp(answer, RETAIN_PART_CAP_CHARS),
          ].join("\n");
          // An auto-retry re-submits the same turn; the server dedupes
          // identical content anyway -- but not before paying for the write.
          if (!content || content === lastRetained) continue;

          // `document_id` keyed on the prompt makes a repeated ask replace
          // its row (the server's default update mode) rather than add a
          // twin.
          var stored = await post(
            "/banks/" + encodeURIComponent(bank()) + "/retain",
            { content: content, document_id: "turn-" + hash(prompt) },
            RETAIN_TIMEOUT_MS
          );
          if (stored !== null) lastRetained = content;
        }
      } catch (_e) {
        // The connection closing is the harness going away, not a memory
        // fault. Never break the agent: nothing here propagates.
      }
    })();
  }

  // v2 replaces the v1 `dispose()` hook with whatever `setup` returns. Only
  // what this plugin owns -- the event subscription -- needs undoing; the two
  // session hooks are scoped and unloaded with the plugin.
  return function cleanup() {
    if (controller) controller.abort();
  };
}

# SECTION J-REGISTER

# FILE: TODO.md (lines 20,52)

## Open

R&D rows (6, 9, 10, 11, 12, 20) are sequenced in `docs/handoff/RND_PLAN.md` —
§5 the closed survey cycle, **§6 the upcoming cycle's outline**; start there.

| # | status | item | why | where | acceptance |
|---|---|---|---|---|---|
| 1 | done | ~~`connect` preserves a surface's baked bank when `--bank` is absent~~ — **done 2026-10-15** (shipped v0.8.0): `apply` reads the pin from the host's own config before any edit — hooks from the command strings, MCP from the argv, extensions/plugins from the installed file's `DEFAULT_BANK` — and rewrites with it; only a config with no pin anywhere stays runtime-resolved (`src/connect.rs`, `bank_from_extension`; tests: bare-rewire-preserves-pin on hooks + MCP + no-pin cases) → [2026-10-15] `src/connect.rs` | kills the installer's `--no-connect` dance on pinned machines (README §Installation documents it); today a bare re-connect silently re-defaults to `memory-wire` | `src/connect.rs` + test | ~~against a temp `HOME` pinned to a second bank, a bare `connect <host>` re-wires with the same bank, unchanged~~ — asserted |
| 2 | done | ~~`doctor` reports a serve running an older binary than the installed one~~ — **done 2026-10-15** (shipped v0.8.0): `Report.stale_serve` — reads the daemon state file's pid, `/proc/<pid>/exe` (the `(deleted)` marker stripped), and **byte**-compares against the running binary; a copy at a different path is not stale. Renders the finding with the restart command; quiet when down, absent state, or identical bytes (`src/doctor.rs`; 3 tests incl. the byte-identical-copy case) → [2026-10-15] `src/doctor.rs` | nothing today detects the "serve keeps old behavior until restarted" window — the exact gotcha from the v0.7.0 deploy on both machines; the running binary outlives the swap by an unbounded time | `src/doctor.rs`, tail of `install/get-memory-wire.sh` | ~~a stale serve produces a finding line naming the restart command; a current serve produces none~~ — asserted |
| 3 | done | ~~`memory-wire update` subcommand~~ — **done 2026-10-15** (shipped v0.8.0): `update` drives `gh` (the binary stays dependency-free plain-HTTP; a missing `gh` refuses with the install hint) — newest-tag compare, `gh release download`, **sha2 in-process verification before anything is touched**, extract, stop-if-live, atomic swap keeping `<exe>.old` for a one-command rollback, start-on-the-new-binary, bare `connect` (pins preserved via #1). Order asserted over a recording fake; live network never in tests (`src/update.rs`; 7 tests). MW_REPO/MW_ASSET overrides → [2026-10-15] `src/update.rs` | one command for check-latest → sha256-verify → swap → re-connect installed hosts (banks preserved via #1) → serve restart (via #2); today all five update steps are manual and one of them (hermes) is invisible to the installer entirely | `src/main.rs`, composing `connect.rs`/`doctor.rs`/`daemon.rs` | ~~end-to-end test against a fake releases API served from a temp dir; the live network check stays behind a flag and never runs in tests~~ — asserted over the effects seam; live mode is `gh`-gated and smoke-verified against the real repo |
| 4 | done | ~~INTEGRATION_PLAN Phase 0, starting with P0.1~~ — **re-verified 2026-10-15 against the binary**; the register is now corrected in place. P0.1 **done** (`paths::endpoint()` reads `serve.json` behind `MEMORY_WIRE_URL`, `src/paths.rs`, 5 regression tests). P0.2 **resolved as keep-8888 + probe** (`daemon start` binds-or-refuses; `connect` takes no probe by decision). P1.1 **done** (`src/connect_codex.rs`). P1.2 **done 2026-10-15, both halves** (`src/connect_claude.rs` for `~/.claude.json`; `src/connect_copilot.rs` for `~/.copilot/mcp-config.json` — a user definitions file whose schema was verified from GitHub's own Copilot CLI reference and the github-mcp-server install guide, so the earlier "JSONC, unverified" blocker is discharged). P1.3 **done 2026-10-15** (`edit_mcp` refuses a foreign server under our name in both directions; claude URL-identity ownership). P2.1 **done 2026-10-15** (`connect::tests::golden_install_shapes`, `src/connect.rs` — a pristine `HOME` plus one `connect` must equal the golden bytes of every file the host parses, seven hosts; shape drift fails at the diff). P2.2 **done 2026-10-15** (`doctor --all-hosts` + G6.9 liveness). P3.1 **recorder done 2026-10-15** (DT_NEEDED row in `bench_footprint`, `readelf -d`); the v0.5.0 byte-compare archaeology stays open. P4.1 **done 2026-10-15** — the slot binds by *directory name* per hermes' own `find_provider_dir`, so a directory stripped to residue is not `wired` even though it exists (`src/connect.rs:535-541`; the audit-era state was a dead slot, the live one empty, and the false-positive `wired` check is fixed). P4.2 **done** (this correction) — → [2026-11-19] re-verified against the binary: every item above is closed by the `file:line` in this cell, and the only residue is the v0.5.0 published-asset byte-compare, which is archaeology rather than work (batch 1, `e0f1bb4`; CONSISTENCY §54). The evidence cell used to hold a to-do for a future agent, which is what E003 was reporting. | re-verify each item against the `v0.7.0` tree first — the plan was written 2026-09-30 and its blocking decisions may have been answered by the retrieval-efficacy waves | `docs/INTEGRATION_PLAN.md` | ~~the plan's own gates~~ |
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
| 31 | open | door parity: redact has no MCP tool or HTTP route, hygiene has no MCP/CLI door, supersede has no HTTP route — **round 4 (2026-11-19) re-verified and still open; the widest gap is unchanged and its shape is a ruling, not a bug**: `svc.redact_memory` is already one method behind the CLI, so adding the MCP tool is ~30 lines and a `read_only` guard that already exists; nothing else about the doors would move. Round 4 also confirmed the same review's other structural readings and **refuted one**: hermes `is_available()` returning True on a down server is documented and deliberate (a probe would put a 2 s timeout in front of every Hermes init, `plugin/integrations/hermes/memory-wire/__init__.py:447-458`), not a silent fail-open | round-2 review's sharpest structural finding (its §3a, verified): "one service, three doors" is true of `MemoryService` and false of parity — every door exposes a different subset, so a capability can be unreachable from the surface an agent is actually on. Recorded with the verified table in `docs/INTEGRATION_GAPS.md` (2026-11-19 addendum); not silently broken, just absent | `docs/INTEGRATION_GAPS.md` addendum; `src/mcp.rs`, `src/api.rs`, `src/main.rs` | a surface decision, not a bug fix: either a documented ruling that the doors stay asymmetric, or redact on MCP — the widest gap, since an agent that retained a secret through MCP cannot retract it through MCP |
| 32 | done | ~~recall failures are indistinguishable from an empty bank~~ — **done 2026-11-19**: `recall_at_fmt` returned `None` for three different reasons — connection refused, non-2xx, and a 200 body this build could not parse — and all three produced byte-identical silence (exit 0, empty stdout). Each now names itself on stderr via `tracing::warn!`, so a hook that recalled nothing says which of the three happened. `RUST_LOG`-gated and stderr-only, so §9's never-fail contract is intact and `RUST_LOG=off` is total silence. The empty-query path stays silent because it is not a failure → [2026-11-19] `src/hooks.rs`; `CONSISTENCY.md` §52 | round-2 review had this as "empty framing violates AGENTS §10"; its verification pass refuted that and named the real defect — the agent and the user cannot tell "no memories matched" from "the server is down". §9 (never-fail silence) and §10 (an empty result says so) genuinely conflict here; §10 governs output that claims to be an answer, and a hook's empty stdout claims nothing | `src/hooks.rs`, `recall_at_fmt` | ~~each failure mode produces a distinct stderr line, none reaches stdout, exit stays 0, `off` is silent~~ — observed live against three real servers, plus a test pinning the modes differ and stdout stays empty |
| 33 | done | ~~the remaining-work plan's four batches~~ — **all done 2026-11-19** (register corrections `e0f1bb4`; hermes trust-gate proof `db57850`; dates on three doors `0b518de`; capsule knob on the plugin surfaces `1f93bf0`). Batch 1 corrected four stale open rows (P2.1, P4.1, P3.1, G6.8) whose work had already landed. Batch 2 gave `Trust::UntrustedExternal`'s fourth door an executable check. Batch 3 served `created_at` in hermes and both TypeScript plugins, which requested `format: "full"` and dropped the field. Batch 4 decided the capsule knob by cost: the plugin surfaces read `MEMORY_WIRE_FORMAT`, not the bank config key, because the env costs no request and the config would add one per turn → [2026-11-19] `plugin/integrations/`, `docs/`, `CONSISTENCY.md` §54–§57 | the round-3 context-pack review and the follow-up register audit kept finding one shape — a policy enforced in N places with a check in N-1, and a list that stopped being maintained. These four batches close that shape for the four doors and leave a runner that enumerates them | `docs/handoff/REMAINING_WORK_PLAN.md` | ~~every door honours the same serving shape and is covered by the same runner; every claim in the register matches the tree~~ — asserted, with per-door mutation checks and live runs |
| 34 | done | ~~door parity, widest gap only: **redact is reachable from the CLI and HTTP but not from MCP**~~ — **done 2026-11-20** (`memory_redact` is the 6th tool): an agent that retained a secret through MCP can retract it through MCP. `MemoryService::redact_memory` already existed and the `read_only` guard already covered it, so the work was a tool registration, a dispatch arm and three tests. The test deliberately uses content that *survives* the retain-side PII redactor — `sk-live-deadbeef` is caught at retain time by the OpenAI rule, so a test built on it would pass a tool that never had to work. Mutation-checked: a fake implementation returning `rows = 1` without writing fails the test. Tool counts corrected in `AGENTS.md` §4 + §3, `README.md` ×3, `src/main.rs`, `src/mcp.rs` (HISTORIAN doc, two `four tools above`), `src/mcp_http.rs`, `docs/INTEGRATION_GAPS.md` ×2 → [2026-11-20] `src/mcp.rs`, `src/mcp_http.rs`, docs; `CONSISTENCY.md` §65 | the one door parity hole where the *retraction* of a credential was impossible from the surface an agent is on; the other doors' asymmetries (hygiene has no CLI, supersede has no HTTP) are absence, not lockout | `src/mcp.rs` (`McpServer` tool list), `src/api.rs` `redact_memory`, `src/store.rs:2242` | ~~an MCP client can redact a row it can read, and the tool count test expects six~~ — done, with the live surface test and the readOnly refusal both passing |
| 35 | done | ~~the shipped opencode plugin does not load on opencode 2.x~~ — **done 2026-11-20** (`3376b5c`): found live while probing B2. The plugin shipped a v1 bare-function export (`PluginModule.LoadError` on 2.3.2, which requires `{id, setup}`) and was installed as a flat file, which opencode 2.x skips (it needs a subdirectory with `index.ts`). Both fixed, the version marker moved 2→3, legacy flat files are removed on install and uninstall, and a stub-ctx drive proved injection and the retain round-trip. `session.idle` does **not** fire under `opencode run`, so the capture hook is `session.execution.succeeded`. This was the same door the round-4 review looked at and called healthy — it was dead on the host it named | a door that has never loaded on its named host is worse than a missing door: `connect` reports success and the host silently has no memory. The review's trust-gate work correctly covered what reaches a model and could not see that nothing did | `plugin/integrations/opencode-plugin/memory-wire.ts`, `src/connect_plugin.rs`, `src/connect.rs`, `plugin/integrations/trust_gate_test.sh` | ~~opencode 2.x loads the plugin, the prompt hook injects, the retain hook writes~~ — live on 2.3.2, plus an export-shape check in the trust gate |
| 36 | open | the two TypeScript doors (hermes, omp) still carry a hand-copied 101-line domain core, and the opencode port changed the file enough that a straight extract is now the wrong first move. Probing found both hosts resolve relative imports *if the door is a directory* — omp loads every flat `.ts` as an extension and errors on a helper (`does not export a valid factory function`), while `index.ts` in a subdirectory loads cleanly. So extraction is feasible but it means restructuring both installs, and the hermes door is a Python package with its own packaging. Option 1: concatenate the shared core via `include_str!` (no host resolves it, no restructure). Option 2: subdirectory + relative import. Option 3: a golden-file drift test that fails when the cores diverge, costing nothing and proving nothing about correctness | measured, not assumed: 101 lines of genuine duplication (markers, cap derivation, tier handling), and the drift is a real maintenance cost — a change to the cap shape must be made in two places or the doors diverge silently | `plugin/integrations/hermes/memory-wire/__init__.py`, `plugin/integrations/extension/memory-wire.ts` | a mutation in one door's core is caught in the other, or the core exists in exactly one place |

