# CONTEXT PACK — context-pack-2026-11-21-rulings

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

# FILE: AGENTS.md (lines 1,462)

# AGENTS.md — orientation and rules for agents working in this repository

Read this before changing anything. Sections 1–4 are orientation: what this is, how
to build it, where things live, and which document answers your question. Sections
5 onward are rules — the ones that exist because they were learned the hard way.

---

## Contents

- [1. What this project is](#1-what-this-project-is)
- [2. Build, test, run](#2-build-test-run)
- [3. Where things live](#3-where-things-live)
- [4. The surface, and which document answers what](#4-the-surface-and-which-document-answers-what)
- [5. Rust discipline](#5-rust-discipline)
- [6. The evaluation set is not a training set](#6-the-evaluation-set-is-not-a-training-set)
- [7. Artifacts and measurement](#7-artifacts-and-measurement)
- [8. Rejected work stays rejected](#8-rejected-work-stays-rejected)
- [9. Interface rules](#9-interface-rules)
- [10. Output conventions for agents](#10-output-conventions-for-agents)
- [11. Git and releases](#11-git-and-releases)
- [12. Honesty rules for reports](#12-honesty-rules-for-reports)
- [13. R&D in flight — read this if you were handed the work](#13-rd-in-flight--read-this-if-you-were-handed-the-work)
- [14. Secrets and live data](#14-secrets-and-live-data)

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
  **10.65 MiB** (11,166,048 B, `target/release/memory-wire`, measured 2026-11-20
  at `6a2a125`) with `DT_NEEDED` = 3. `v0.7.0` was 10.25 MiB
  (`eval/baseline/2026-10-13-v0.7.0/BENCH_FOOTPRINT.md`); the 9.7 MiB this line
  used to state was earlier still, and 10.25 was itself a year-old figure this
  line carried until a reviewer checked it against the binary. It happened again on
  2026-11-20: the figure was 24,808 B stale after four commits, and the round-5
  reviewer caught it by recomputing the delta rather than reading it. **Do not
  trust a size quoted here without re-running `stat`** — quote the command, not
  the number. Verify `ldd` after touching anything that links.
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

## 2. Build, test, run

```bash
cargo build --release --locked
cargo test --locked                                     # the full suite
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo doc --no-deps --all-features --locked             # must be 0 warnings
```

All four are the gate.

First clone, once: `git config core.hooksPath .githooks` — turns on the secret
gates for every commit and commit message (`scripts/secret-scan.sh`). Without it
nothing checks what you commit. Layers and their limits: `docs/SECRETS.md` §1.

The default port is `8888` and there is exactly one
literal for it (`paths::DEFAULT_ADDR`); every clap default reads from that
constant.

```bash
memory-wire                       # live state: server, bank, store, next steps
memory-wire daemon start          # run in the background
memory-wire doctor                # endpoint, store, server health
memory-wire connect --list        # which agent hosts exist here
memory-wire hook session-start    # what a host runs
```

**Benchmarks and latency artifacts must be built `--release`.** Debug p50 is not
a usable gate. And latency is only meaningful on a quiet box — check `loadavg`
before and after and record it (see §7).

**On this workstation `~/.local/bin/ar` shadows GNU `ar`** (an unrelated tool,
v0.1.4). Prefix `PATH=/usr/bin:$PATH` for every cargo command, or the
`libsqlite3-sys` build fails with `ar: error: unrecognized subcommand 'cq'`.

## 3. Where things live

`src/`, 33 modules, 39,976 lines. Counts are `wc -l` against this tree;
`scripts/module_table_check.sh` fails when the table and the tree disagree, so a
stale count is a red gate rather than a claim nobody checks.

| module | lines | role |
|---|---|---|
| `store.rs` | 5874 | SQLite: schema, migrations, put/recall/delete, FTS5, redaction, versions |
| `api.rs` | 4724 | HTTP routes, the frozen error contract, the historian prompt |
| `main.rs` | 3379 | clap surface, routing, wiring |
| `connect.rs` | 3515 | agent-host detection and config writing |
| `recall.rs` | 2707 | RRF fusion, candidate pool, budget trim, scoring |
| `hooks.rs` | 2210 | the five lifecycle events; the never-fail hook contract |
| `mcp.rs` | 2299 | MCP server: 6 tools + resources + prompts + completion |
| `daemon.rs` | 1208 | `daemon start/stop/status`, pidfile, start-time identity |
| `doctor.rs` | 1272 | the health report (`doctor`, `doctor --all-hosts`) |
| `connect_plugin.rs` | 1172 | the hermes `MemoryProvider` plugin |
| `vector.rs` | 1058 | dense arm (`embed` feature, OFF by default) |
| `cli_home.rs` | 948 | the content-first bare-invocation view |
| `connect_codex.rs` | 1033 | codex TOML config writing |
| `connect_ext.rs` | 886 | omp and pi: the two hosts installed as a **TypeScript file**, not a config entry |
| `paths.rs` | 826 | bank identity, endpoint, data paths |
| `render.rs` | 683 | human-readable report rendering |
| `update.rs` | 686 | `update`: the five-step self-update swap |
| `seed.rs` | 693 | transcript reading for hook capture |
| `hygiene.rs` | 658 | W4: deterministic hygiene scan (merge/contradiction/date/bloat) |
| `mcp_http.rs` | 542 | stateless streamable-HTTP MCP mount |
| `connect_opencode.rs` | 443 | opencode's plugin dir (v2 layout) |
| `capture.rs` | 363 | hook-based auto-capture: dedup, privacy filter, raw observation |
| `grounding.rs` | 350 | W5: bounded startup grounding capsule |
| `connect_claude.rs` | 362 | Claude Code's MCP entry |
| `connect_copilot.rs` | 330 | Copilot CLI's MCP entry |
| `http.rs` | 296 | dependency-free HTTP/1.1 client for loopback |
| `skills.rs` | 282 | `connect --with-skills`: deploy the agent skill per host |
| `sweep.rs` | 313 | `sweep`: forget what a bank said it would stop remembering |
| `guidelines.rs` | 275 | native agent-rules writer (marker-delimited block) |
| `memory.rs` | 179 | canonical memory types shared by all layers |
| `connect_common.rs` | 174 | helpers the per-host connect modules share |
| `embed.rs` | 185 | ONNX session, vendored MiniLM |
| `lib.rs` | 51 | crate root: re-exports and the module map |

`tests/` — `backup.rs`, `daemon.rs`, `e2e.rs`, `recall_budget.rs`, `scale.rs`.
`examples/` — 15 benchmark and evaluation harnesses.
`eval/amb/` — our provider for vectorize-io's AMB harness, and its committed runs.

## 4. The surface, and which document answers what

Before you change behaviour, find the document that owns your question. Reading
the wrong one is how claims drift.

| if you are asking about… | read |
|---|---|
| **I am continuing the memory-performance R&D — what is the state, the hypotheses, the commands?** | **`docs/handoff/README.md`** first, then its STATE, RND_PLAN, RUNBOOK, CLOSED |
| can I tune this against the benchmark? | **this file**, §6, then `docs/EVALUATION_HYGIENE.md` |
| which external benchmarks exist (BEAM, PrecisionMemBench, AMB, OAMB, MemBase, memorybench, agent-memory-bench…), what each costs, how we plug in, what role it may play, and what was measured | **`docs/BENCHMARK_LANDSCAPE.md`** — start here before running or citing anything external |
| which competitor architectures (MemOS, Honcho, OpenViking…) hold candidate mechanisms, which are vetoed by our own measurements, and how a candidate becomes a pre-registered H-item | **`docs/COMPETITOR_GAPS_PLAN.md`** — the audit plan, the screens and the veto chain; `docs/handoff/RND_PLAN.md` remains where surviving candidates get registered |
| what the current performance is, everywhere at once, and how to re-measure after a change | **`eval/baseline/README.md`** and `eval/baseline/run_baseline.sh` — one dated directory per run; compare directory to directory |
| why an LLM-judged run through aroute needs `eval/llm_shim.py`, and what aroute must fix before it can go | `docs/AROUTE_BUGS.md` — a handoff for the aroute repo; memory-wire agents do not edit aroute |
| what was measured, on what, at what load, and what was rejected | `docs/CONSISTENCY.md` — **append-only, never rewrite** |
| how memory-wire compares to agentmemory and Hindsight | `README.md`, then `docs/EXCEED_PLAN.md` |
| what a latency number means and how to take one | `docs/BENCHMARK.md`, `docs/BENCHMARK_SCALE.md` |
| what is still open, and what is deliberately not being done | `docs/INTEGRATION_GAPS.md` — written at `eec026f`, partly stale; verify each row against the binary |
| what is left to implement, as tracked work items | **`TODO.md`** at the repo root — the implementation register; indexes the domain lists in this table rather than duplicating them |
| what we install into each agent harness, and what is quietly broken | `docs/INTEGRATION_AUDIT.md` — findings ranked F1–F7, each verified or explicitly marked unverified |
| how to make the harness integration correct rather than merely present | `docs/INTEGRATION_PLAN.md` — two open decisions block Phase 1 |
| why a hook injected something irrelevant, and why adding a score threshold is the wrong fix | `docs/OPEN_HOOK_RECALL_RELEVANCE.md` — diagnosed, deliberately unfixed |
| which retrieval lever was tried and killed, and by what measurement | **`docs/handoff/CLOSED.md`** (the complete current list), then `docs/handoff/RND_PLAN.md` §4 (the results log, one row per verdict); `docs/RERANKING_PLAN.md` §3 and `docs/PERFORMANCE_PLAN.md` hold the older closure *records* |
| where the numbers came from, honestly | `eval/README.md`, and the artifact it names |
| dependency versions and why they are pinned | `docs/VERSIONS.md` |
| how a bank id is derived, what can collide, and why a hook recalls nothing | `docs/BANK_IDENTITY.md`; `memory-wire doctor` names the bank that holds the memories when the resolved one holds none |
| the next iteration's plan and its gates | **`docs/handoff/RND_PLAN.md`** (the live register) and `docs/handoff/README.md` (fresh-agent entry point); `docs/NEXT_ITERATION.md` and `docs/EXCEED_PLAN.md` are the earlier dated records |
| the original audit this all came from | `docs/AUDIT.md` |
| a credential reached a commit, a tag, or a message; or history must be rewritten | **`docs/SECRETS.md`** — the layers, the response order, and the incident log with its commit map |

**The MCP surface.** Six tools — `memory_retain`, `memory_recall`,
`memory_reflect`, `memory_bank_config_get`, `memory_supersede` (W7a), and
`memory_redact` (2026-11-20, TODO #34: an agent that retained a secret through
MCP can retract it through MCP rather than dropping to the CLI).
`src/mcp.rs` pins the list in `tools_list_should_show_six_annotated_tools`.
— plus resources (`memory://{bank}/{id}`), one prompt (the historian preamble,
served through the prompt surface, not as a tool), and argument
completion. Deliberately **not** advertised: `subscribe`, the three
`listChanged` flags, and `logging`, because this server never sends those
notifications. Advertising a capability that never arrives is worse than
advertising nothing, and a test pins the exact set. This line said "four tools"
until 2026-11-19 and "five" until 2026-11-20; it lagged W7a by a release and
`memory_redact` by one.

The transport is **stateless** (rmcp 3.5, MCP spec `2026-07-28`): no
`initialize` handshake, no `Mcp-Session-Id`. A legacy client that still
handshakes at `2025-06-18` is answered and negotiates that version, so cursor,
opencode and the hermes plugin are unaffected. Note that `GET /mcp` answers
`405` — the SSE stream it used to open never carried a notification.

**Agent hosts.** Eight, and `Host::ALL` is the authority: `claude-code`,
`codex`, `copilot-cli`, `cursor`, `opencode`, `omp`, `pi`, `hermes`. **Seven are
additive** (a hook entry, an MCP key or an extension file) — the implicit set a
bare `connect` wires. **hermes is not** — it activates exactly one global memory
provider, so a bare `connect` would displace `agentmemory` on a machine that
has both. It is reachable by name only. That asymmetry is deliberate; do not
"fix" it by adding hermes to the implicit set.

---

# Rules

## 5. Rust discipline

The `rust-best-practices` skill (`~/.agents/skills/rust-best-practices`, based on
Apollo's handbook) is the house style. **Read the relevant chapter before
reviewing or writing Rust** — the chapters are in `references/` and are worth
more than the quick-reference summary at the bottom of the skill file.

The rules this repo actually enforces beyond the compiler:

- `deny(missing_docs)` is on. Public items need `///` docs.
- No `unwrap()` in non-test code. `expect()` needs a reason naming the
  invariant. `unwrap()` under `#[cfg(test)]` is fine.
- No new dependency without first saying what the standard library or an
  already-present dependency cannot do. See §8.
- Blocking SQLite work must not run on a tokio worker. `src/store.rs` uses
  `spawn_blocking` for this; do not add a blocking call to an async path.
- Comments explain **why**; `///` docs explain **what**. A comment restating the
  code is a finding.
- A `ponytail:` comment marks a deliberate simplification with a named ceiling
  and an upgrade path. That is an accepted pattern, not debt to remove.

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

## 11. Git and releases

- **Do not tag, publish, or cut a release unless asked.** Cutting a release is
  the user's call.
- Do not force CI to run. `ci.yml` triggers on `pull_request`,
  `workflow_dispatch` and `push: tags: ['v*']` only — never on branch pushes —
  because **GitHub Actions quota is a metered exhaustible resource and is
  currently exhausted**. Publishing a release triggers CI once, via the tag.
- Releases are cut locally with `scripts/build-release.sh`, which publishes
  through the `gh` REST API and therefore bills no Actions minutes. It refuses a
  dirty tree, a tag that does not name `HEAD`, an incomplete target set, and
  clobbering a release that already has assets.
- Disable CI around a tag push — **both `ci.yml` and `release.yml`** trigger on
  `push: tags: ['v*']` (`gh workflow disable ci.yml release.yml` … re-enable) so
  the cut costs nothing.
- The complete Linux set from this host is
  `--only "linux-x86_64 linux-x86_64-musl linux-aarch64"` — a quoted word-list;
  the script splits `--only` on spaces. No macOS asset has ever shipped, so the
  three-Linux set is not a partial release. `linux-x86_64` is a glibc build and
  carries the build host's floor (needs ≥ 2.39, built on 2.44);
  `linux-x86_64-musl` is plain-cargo static-pie, `DT_NEEDED=0`, no floor —
  live-verified on the Debian 12 / glibc 2.36 racknerd VPS, where the
  installer's probe rejects the gnu asset and completes on musl;
  `linux-aarch64` is a `cargo-zigbuild` cross.
- Version bump first: edit `Cargo.toml`, run one unlocked `cargo check` to
  refresh the own-version line in `Cargo.lock`, then the `--locked` gates pass.
  Two commits — feature, then bump — and the tag names the bump commit.
- `install/get-memory-wire.sh --bank <id>` forwards the pin to every `connect` it
  runs, including the `--uninstall` that unwires hosts before the binary is removed.
  `connect` keeps a pin already baked into a host when `--bank` is absent (TODO #1),
  so a plain re-run does not drop it.
- macOS assets cannot be cross-built from Linux — `libsqlite3-sys` links
  CoreFoundation. The script skips them with that reason.
- Delegate only with the `space-bunny` model.
- `scripts/build-release.sh` runs `scripts/secret-scan.sh --all` before it builds
  anything, `--check` included, and refuses a tracked secret. A tag publishes the
  tree GitHub serves, so this gate is the last one before a credential goes public.
- A tag moves only in a history rewrite, and only with the owner's go-ahead. The
  procedure, with its lease-checked push and the SHA map, is `docs/SECRETS.md` §2.

## 12. Honesty rules for reports

- Report what a measurement shows, not what you hoped it would show.
- If an agent before you made a claim wrong, correct it by name.
- If you cannot determine which of two conflicting values is right, leave both
  and say so. Do not invent a reconciliation.
- A null result is a result. Record it.
- Verify before reporting. Run the gate yourself; do not cite a subagent's
  summary as if it were a measurement.

## 13. R&D in flight — read this if you were handed the work

State as of 2026-10-14; the authoritative, kept-current record is
`docs/handoff/` (index: `docs/handoff/README.md`).

- **v0.8.2 released 2026-10-16** (user-ratified chain; v0.8.1 the same day — §45 records the
  `update` hop bug found between them, fixed in v0.8.2 with the earlier live-verify claim
  corrected). The R&D cycle's records and
  the product batch — baseline artifacts, occurrence-time capability, date-aware
  `answer_quality`, `recall_all@10`, `update`, doctor work, the new host entries, all harnesses —
  are **committed and shipped** (feature `04123f7`, bump `a272196`, tag `v0.8.0`, three-Linux
  assets). The R&D queue is empty; `docs/handoff/` is the current record.
- **The open work is a hypothesis list with pre-registered tests** —
  `docs/handoff/RND_PLAN.md` (H1.0 first: where do the missing gold sessions sit?).
  `docs/handoff/CLOSED.md` lists what is already rejected; do not re-propose.
- **Inner loop = LoCoMo retrieval harness (free, ~25 s).** The LLM instrument
  (aroute `small-stack` through `eval/llm_shim.py`) is slow, free-tier and has known
  defects (`docs/AROUTE_BUGS.md`); use it for confirmation, never for iteration. Never
  edit the aroute repo or its service, never read its credentials, never put a real API
  key in a harness (`env -u GOOGLE_API_KEY`).
- **Every LongMemEval run is counted** in `docs/CONSISTENCY.md` (§34: 2 consultations for
  the v0.7.0 baseline; §35: 0). The dated `answer_quality` re-measure (H5) will be the
  next, and is measurement only.
- **Append to `docs/CONSISTENCY.md`, never rewrite.** The next free section is the one its last paragraph names (§50 as of 2026-10-18; this line used to say §36 and was stale).
- When you finish a unit of work, update `docs/handoff/STATE.md` and the status line of
  the hypothesis in `RND_PLAN.md`; a handoff that is not kept current is the next
  agent's phantom.

## 14. Secrets and live data

A credential that reaches a commit is public the moment it is pushed, and
deleting the commit does not revoke it. The repository is public. Rotate first,
then clean up.

- **Never write a credential into a file, log, artifact, or memory row.** A
  pasted token is captured by the hooks into the bank, so it reaches every
  generator that reads the bank.
- **Generators that render memory text redact it.** `eval/relevance/run_battery.py`
  does this with `SECRET`; its pattern list mirrors `scripts/secret-scan.sh` and
  the two change together.
- **The gate is `scripts/secret-scan.sh`**, run from `.githooks/pre-commit`. It
  reports `file:line` and never the value. A line carrying `secret-scan:allow` is
  skipped; a fixture file with deliberate fake secrets goes in `EXCLUDE` instead.
- **Live-bank run output is not committed.** Add the directory to `.gitignore`
  before the run that reads the bank. The pmv2 run directory already is.
- **Fix the generator, then regenerate.** A committed artifact that contains a
  leak is regenerated from its frozen input and diffed, not hand-edited.
- **Rotate before you clean.** Removing a credential from history does not revoke
  it; every copy made before the rewrite still works until the issuer revokes it.
  Tell the owner to revoke first. `docs/SECRETS.md` §2 has the order.
- **Never rewrite history in a working tree that has uncommitted work.**
  `git filter-repo` resets the tree it runs in. Use a mirror clone, then sync the
  working copy with `git reset --mixed`, which keeps the files.
- **A message is history too.** The commit-msg gate scans it; do not paste a
  credential into a commit message to explain where it came from.

# SECTION B-ASK

# FILE: docs/plans/2026-11-21-remaining-gaps-plan.md (lines 1,282)

# Outline plan — remaining gaps, redundancy, and upgrades (2026-11-21)

Written at `1485add`. The previous plan
(`2026-11-20-remaining-gaps-plan.md`) was written the same way this one starts —
from the tree — and a rulings review then found four errors in it, including one
structural misframing. **This plan is built to be reviewed the same way, and it
names the questions the last one failed to ask.**

The headline is different from last time. The register is nearly empty (7 open
rows, 4 of them blocked on you or the world). What is actually left is not
cleanup — it is **one unmeasured gap that matters more than everything on the
register**, and two structural questions nobody has asked.

---

## 0. What this plan is not

- **Not a re-litigation of `docs/handoff/CLOSED.md`.** Nothing here proposes
  vectors, a reranker, consolidation, or a score threshold (AGENTS §8; §32/§48
  on the threshold).
- **Not a release plan.** The operator has ruled: **release after development
  is final**. R3a was measured (10.6515 MiB, `DT_NEEDED=3`, +3.92% over
  v0.7.0, 68 commits past `v0.10.0`) and **not cut**. That stands.
- **Not another register sweep.** The last review's sharpest structural point
  was that I answered "gaps, redundancies or upgrades" with "remaining register
  rows". This plan leads with the questions the register cannot answer.

---

## 1. What I verified before writing this

| check | result |
|---|---|
| `git status` | clean, HEAD `1485add` |
| TODO rows | 7 open, 31 done |
| open rows | 10, 11, 12, 13, 31, 36, 38 |
| installed vs repo | **0.9.0** vs **0.10.0**, 68 commits behind |
| release | measured, **not cut** (CONSISTENCY §71) |
| `v0.10.0` exists | yes; `v0.9.0..HEAD` = 96, `v0.10.0..HEAD` = 68 |
| integration test files | 6 files, 22 tests, **none per-host round-trip** |
| server-side injection format | none — `format=summary`/`toon` exist for recall bodies, not for an injection block |
| policy duplication | 4,037 lines across the four auto-injection doors |

---

## 2. The gap that matters most, and it is not on the register

**Nothing in this repository verifies that a memory written in session 1
reaches a model's prompt in session 2, on any real host.**

Verified: `grep` across `tests/` for `before_agent_start`, `ev.system`,
`inject` returns nothing. The six integration test files cover backup, daemon,
e2e retain/recall/reflect, installer, recall budget and scale — **none drives a
host and reads back what the host injected.**

This is the exact failure that produced the worst defect on record. TODO 35: the
opencode door **shipped dead on the host it named**, and it was found by
accident, while probing for something else. Every Rust test passed. The door
exported the v1 shape against a 2.x loader, `connect` reported success, and the
host silently had no memory.

Why this is the top item and not a footnote:

- Every other defect in this repository's record is a *policy* defect in a door
  that was at least running. This one is "was it running at all".
- The last review named it independently and called it cheaper than the parity
  matrix it had been asked to rank. **A write-then-read smoke test is one
  script, not a four-driver suite.**
- It is the only check that catches the class `connect` cannot: a host that
  changed its loader between versions.

**What "done" means:** a script that, for each detected host, starts a real
memory-wire, wires the host, drives the host's own lifecycle (or the closest
thing available without an API key), and asserts the injected block contains a
marker string that was retained through the normal path. Where a host needs a
live agent runtime and none is installed, the check prints **SKIPPED** loudly —
never silent, because a silent skip is the opencode door again.

---

## 3. Tier 1 — build now, no decision needed

### 3.1 The per-host write-then-read smoke test (§2)

New `plugin/integrations/door_roundtrip_test.sh`, wired into the trust gate.
Sourced from `Host::ALL` for the same reason as `5aea83a`: a literal list is how
round 4 found a script covering 6 of 8 hosts.

Cost: the bulk is a stub server (memory-wire already has one pattern for this in
`trust_gate_test.py`) plus a driver per host shape — `Style::Mcp`,
`Style::Plugin`, `Style::Extension`, `Style::Copilot`. Four drivers, not eight
hosts.

### 3.2 TODO 38's remaining half, rescoped per R2

The review's R2 finding stands and I am adopting it: the original "one fixture
set, four doors, one oracle" spec was wrong because the doors **legitimately
render differently** (`###` vs `##`, bullets vs numbers, footer vs none, chars
vs code points vs UTF-16 units). A single oracle would be red on day one for
reasons that are design.

So the rescope, which is the review's own "cheapest version that still catches
the class":

- **The `Host::ALL` half is done** (`5aea83a`): two tests, enumerate `ALL`,
  wire and unwire in a pristine home.
- **New:** a cross-door *row-inclusion* invariant, which is the thing that
  actually broke four times (trust before take, blank before take, footer
  count). Not rendered bytes — which rows were included.
- **New:** a `cargo test` that fails when a host in `Host::ALL` has no
  door-table entry.
- **Dropped:** the four-door rendered-output oracle. The trust gate's new
  behavioural comparison (7 shared fixtures, both TS doors, identical output)
  covers the TS pair where a shared renderer is meaningful.

### 3.3 TODO 31 — door parity residue: document the asymmetry, don't keep adding doors

Verified state: `redact` is on MCP; `hygiene` has no MCP/CLI door; `supersede`
has no HTTP route; `grounding` has neither. The review's audit of the last round
confirmed this is absence, not lockout.

Recommendation: **rule it and record it.** Every added door is another surface
to keep in parity, and the audit's own finding is that asymmetry is what happens
when each capability is added through the door its item needed. Add a short
section to `docs/INTEGRATION_GAPS.md` stating the doors are deliberately
asymmetric, with the reason. This is a one-page doc change that closes a
recurring review finding.

### 3.4 The count-claim hygiene the review exposed

The review found **ten** sites needing a tool-count correction, nine of which
TODO 34 listed. The tenth was hermes. That is not a one-off — it is a class:
**a number in prose that has no test behind it.**

Cheap fix: extend the module-table gate's shape to cover the counts that have
actually drifted — tool count, host count, module count — each sourced from the
enum or the code, never a literal. `Host::ALL.len()` is 8; `src/mcp.rs`'s tool
list is 6. A test asserting "the number in `AGENTS.md` §4 equals the number in
the code" is ten lines and would have caught all ten sites.

**And the very act of writing this plan found an eleventh, today.** `AGENTS.md`
line 200 opens **"**Agent hosts.** Six: `claude-code`, `codex`, `copilot-cli`,
`cursor`, `opencode`, `hermes`"** while `Host::ALL` holds **eight** (adding
`omp` and `pi`) and `IMPLICIT` holds seven. The section then spends three
sentences on hermes's asymmetry without ever naming the two hosts the list
omits. This is the same defect as the ten the review found, caught by the same
method: read the prose, then read the code it describes.

---

## 4. Tier 2 — the two structural questions nobody has asked

These are the last review's sharpest contribution, and they are questions, not
conclusions. Both are recorded here so they are not lost.

### 4.1 Why does rendering policy live in the doors at all?

Every defect on record is the same shape: rendering policy (trust gate, blank
rows, footer, cap) re-implemented client-side in Rust, Python and TypeScript.
4,037 lines across four doors, and a check in N−1 of them — which is TODO 33's
finding, restated.

The server already serves `format=summary` and an opt-in `toon`. **A
server-rendered injection format is the untested alternative**: one
implementation, parity nearly trivial, doors thin.

What the pack could not settle, and what would:

- Does it conflict with the frozen HTTP error contract (AGENTS §9)? A new
  response shape is an API change; a new *content type* on an existing route may
  not be.
- Do the doors' query and cache logic survive it? The extension derives its
  query from `lastPrompt` and clears it after; hermes passes `_strip_reminders`
  of the last user turn. Both are client-side by necessity — the server never
  sees the prompt.
- What does it cost on the wire? The block is capped at `INJECT_CAP_CHARS` in
  every door; server-side rendering means the server picks the cap.

**This is a design question and needs a decision, not an afternoon.** I am
recording it, not proposing to build it.

### 4.2 Does the opencode door actually load on the installed host, today?

TODO 35 fixed the export shape and the install layout. The fix was verified on
opencode 2.3.2 live. **But `plan §7` of the last plan admits no live 2.x load
has been checked since the port**, and the trust gate is explicitly a shape
check, not a load test.

One command, if opencode is installed here: load the plugin, inject a prompt,
confirm the block arrives. If it is not installed, the answer is "not verifiable
here" and §3.1's SKIPPED path is the honest result.

---

## 5. Tier 3 — blocked on you, or on the world

Unchanged from the last plan, with the counts corrected:

| # | item | blocker |
|---|---|---|
| 3.1 | release (measured, not cut) | **you** — 68 commits past `v0.10.0`, +3.92% over v0.7.0 |
| 3.2 | install 0.10.0 over the 0.9.0 binary | **you** — a write to your live environment |
| 10 | agent-memory-bench Claude Code arm | **external** — needs the benchmark's harness + your agent budget |
| 11 | LLM-judged runs (AMB/OAMB/memorybench/MemBase) | **you** — free-tier quota |
| 12 | AMB provider at turn granularity | decided **against** on the dev set; measurement only |
| 13 | aroute defects B1–B4 | **external** — a different repo, memory-wire agents don't edit it |
| 31 | door parity residue | becomes a doc change under §3.3, not a blocker |
| 36 | TS door duplication | **R1 ruled: fence, don't extract** (`333185b`). What remains is whether the fence should also cover the glue. Low value. |

**Note on 3.1:** the operator's ruling is explicit — release after development
is final. So this is not blocked on a decision, it is blocked on *this plan
being done*. That is the correct reading and I am not going to re-ask.

---

## 6. What I am *not* proposing, and why

- **No TS door extraction.** The rulings review's measurement (`git log -L` over
  the 8 shared functions) showed divergent history in two, both of them commits
  from the same session, so extraction buys nothing *observed*. Fenced instead,
  behaviourally, in `333185b`.
- **No shared abstraction over `connect_*`.** B1 extracted the byte-identical
  helpers; the remainder is golden-expectation tests that would be obscured by
  one.
- **No new dependency.** AGENTS §1: the default build must not gain one.
- **No re-proposal of any closed lever** (`CLOSED.md`).
- **No change to the four `ponytail:` simplifications.** Each names a ceiling
  and an upgrade path.
- **No touch to any count inside a historical-record fence**
  (`docs/BENCHMARK.md`'s "4 tools" is a dated record).

---

## 7. Order, and what each tier costs

| # | item | tier | ~size | blocked on |
|---|---|---|---|---|
| 3.4 | count-claim gate (tool/host/module) | 1 | ½ day | no |
| 3.1 | per-host write-then-read smoke test | 1 | 1–2 days | no |
| 3.2 | TODO 38 rescoped: row-inclusion invariant | 1 | ½ day | no |
| 3.3 | door-parity asymmetry ruling, documented | 1 | an hour | no |
| 4.2 | opencode live load check | 2 | minutes | opencode installed |
| 4.1 | server-rendered injection: decide, don't build | 2 | a ruling | **you** |
| 5 | release, install, benchmarks, aroute | 3 | — | you, or external |

**Recommended order:** 3.4 → 3.1 → 3.3 → 3.2 → then stop and ask about 4.1.

That front-loads the cheapest check with the widest reach (3.4 catches a class
that produced ten defects), then the one unmeasured gap that matters (§2), then
the two doc-level items that close recurring review findings.

---

## 8. What I could not determine

Per AGENTS §12, stated rather than guessed:

- **Whether opencode is installed on this machine, and whether the current
  plugin loads.** I have not checked; §4.2 is the check.
- **Whether a server-rendered injection format is compatible with the frozen
  contract.** I can read the contract; I cannot settle whether a new content
  type on an existing route counts as a break without a ruling.
- **How many users the "other people's machines" cost in R1(d) actually
  represents.** The last review flagged this and it is still unevidenced: one
  box and one VPS.
- **Whether 3.92% growth is "material".** AGENTS §1 names no number. That line
  is the operator's to draw.

---

## 9. Definition of done

Tier 1 closed with a gate green and a `CONSISTENCY.md` section. Tier 2 answered
with a ruling recorded either way — **a question answered "not now, and here is
why" is done**, and a question silently dropped is the phantom that
`docs/handoff/` exists to prevent. Tier 3 items done on your word or recorded as
blocked with the blocker named.

And: this plan gets reviewed before it is executed, the same way the last one
did. That is not ceremony — the last review found four errors in a plan I had
checked against the tree, and one of them was a structural misframing that
would have sent three days of work at the wrong problem.

# SECTION C-LAW

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

# SECTION D-EVIDENCE

# FILE: TODO.md (lines 1,90)

# TODO — what is left to implement

Started 2026-10-08, at `v0.7.0`. One row per item; when an item closes,
strike it and name the commit — the `docs/INTEGRATION_GAPS.md` convention.
This file is the *implementation* register. The domain docs keep their own
lists and this file points at them rather than duplicating them:

- `docs/INTEGRATION_GAPS.md` "What is left": **G4 only** (bank opt-in
  policy — a decision, not a defect). G6.3 is a recorded decision, and
  G6.4, G6.8 and G6.9 all closed 2026-10-15/2026-11-19 (see that doc's
  closed tables); this list used to carry all five and was corrected on
  2026-11-19 when G6.8 was found already implemented.
- `docs/INTEGRATION_PLAN.md`: **Phases 0–1 complete**; P2.1, P2.2, P4.1 and
  P4.2 closed 2026-10-15. P3.1's recorder and single build path are done —
  only the v0.5.0 published-asset byte-compare (archaeology) remains.
- `docs/RETRIEVAL_EFFICACY_PLAN.md` §7: operant-side tickets — a different
  repo (`agentic-harness/operant`), indexed here so nobody hunts for them
  in this one

## Open

R&D rows (6, 9, 10, 11, 12, 20) are sequenced in `docs/handoff/RND_PLAN.md` —
§5 the closed survey cycle, **§6 the upcoming cycle's outline**; start there.

| # | status | item | why | where | acceptance |
|---|---|---|---|---|---|
| 1 | done | ~~`connect` preserves a surface's baked bank when `--bank` is absent~~ — **done 2026-10-15** (shipped v0.8.0): `apply` reads the pin from the host's own config before any edit — hooks from the command strings, MCP from the argv, extensions/plugins from the installed file's `DEFAULT_BANK` — and rewrites with it; only a config with no pin anywhere stays runtime-resolved (`src/connect.rs`, `bank_from_extension`; tests: bare-rewire-preserves-pin on hooks + MCP + no-pin cases) → [2026-10-15] `src/connect.rs` | kills the installer's `--no-connect` dance on pinned machines (README §Installation documents it); today a bare re-connect silently re-defaults to `memory-wire` | `src/connect.rs` + test | ~~against a temp `HOME` pinned to a second bank, a bare `connect <host>` re-wires with the same bank, unchanged~~ — asserted |
| 2 | done | ~~`doctor` reports a serve running an older binary than the installed one~~ — **done 2026-10-15** (shipped v0.8.0): `Report.stale_serve` — reads the daemon state file's pid, `/proc/<pid>/exe` (the `(deleted)` marker stripped), and **byte**-compares against the running binary; a copy at a different path is not stale. Renders the finding with the restart command; quiet when down, absent state, or identical bytes (`src/doctor.rs`; 3 tests incl. the byte-identical-copy case) → [2026-10-15] `src/doctor.rs` | nothing today detects the "serve keeps old behavior until restarted" window — the exact gotcha from the v0.7.0 deploy on both machines; the running binary outlives the swap by an unbounded time | `src/doctor.rs`, tail of `install/get-memory-wire.sh` | ~~a stale serve produces a finding line naming the restart command; a current serve produces none~~ — asserted |
| 3 | done | ~~`memory-wire update` subcommand~~ — **done 2026-10-15** (shipped v0.8.0): `update` drives `gh` (the binary stays dependency-free plain-HTTP; a missing `gh` refuses with the install hint) — newest-tag compare, `gh release download`, **sha2 in-process verification before anything is touched**, extract, stop-if-live, atomic swap keeping `<exe>.old` for a one-command rollback, start-on-the-new-binary, bare `connect` (pins preserved via #1). Order asserted over a recording fake; live network never in tests (`src/update.rs`; 7 tests). MW_REPO/MW_ASSET overrides → [2026-10-15] `src/update.rs` | one command for check-latest → sha256-verify → swap → re-connect installed hosts (banks preserved via #1) → serve restart (via #2); today all five update steps are manual and one of them (hermes) is invisible to the installer entirely | `src/main.rs`, composing `connect.rs`/`doctor.rs`/`daemon.rs` | ~~end-to-end test against a fake releases API served from a temp dir; the live network check stays behind a flag and never runs in tests~~ — asserted over the effects seam; live mode is `gh`-gated and smoke-verified against the real repo |
| 4 | done | ~~INTEGRATION_PLAN Phase 0, starting with P0.1~~ — **re-verified 2026-10-15 against the binary**; the register is now corrected in place. P0.1 **done** (`paths::endpoint()` reads `serve.json` behind `MEMORY_WIRE_URL`, `src/paths.rs`, 5 regression tests). P0.2 **resolved as keep-8888 + probe** (`daemon start` binds-or-refuses; `connect` takes no probe by decision). P1.1 **done** (`src/connect_codex.rs`). P1.2 **done 2026-10-15, both halves** (`src/connect_claude.rs` for `~/.claude.json`; `src/connect_copilot.rs` for `~/.copilot/mcp-config.json` — a user definitions file whose schema was verified from GitHub's own Copilot CLI reference and the github-mcp-server install guide, so the earlier "JSONC, unverified" blocker is discharged). P1.3 **done 2026-10-15** (`edit_mcp` refuses a foreign server under our name in both directions; claude URL-identity ownership). P2.1 **done 2026-10-15** (`connect::tests::golden_install_shapes`, `src/connect.rs` — a pristine `HOME` plus one `connect` must equal the golden bytes of every file the host parses, the eight hosts in `Host::ALL`; shape drift fails at the diff). P2.2 **done 2026-10-15** (`doctor --all-hosts` + G6.9 liveness). P3.1 **recorder done 2026-10-15** (DT_NEEDED row in `bench_footprint`, `readelf -d`); the v0.5.0 byte-compare archaeology stays open. P4.1 **done 2026-10-15** — the slot binds by *directory name* per hermes' own `find_provider_dir`, so a directory stripped to residue is not `wired` even though it exists (`src/connect.rs:535-541`; the audit-era state was a dead slot, the live one empty, and the false-positive `wired` check is fixed). P4.2 **done** (this correction) — → [2026-11-19] re-verified against the binary: every item above is closed by the `file:line` in this cell, and the only residue is the v0.5.0 published-asset byte-compare, which is archaeology rather than work (batch 1, `e0f1bb4`; CONSISTENCY §54). The evidence cell used to hold a to-do for a future agent, which is what E003 was reporting. | re-verify each item against the `v0.7.0` tree first — the plan was written 2026-09-30 and its blocking decisions may have been answered by the retrieval-efficacy waves | `docs/INTEGRATION_PLAN.md` | ~~the plan's own gates~~ |
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
| 31 | open | door parity: hygiene has no MCP/CLI door, supersede has no HTTP route, grounding has neither. **Narrowed 2026-11-20**: the widest gap (redact had no MCP tool) is **closed** by row 34 — `memory_redact` is the 6th tool. **The round-5 review corrected this row's own text**: it said redact had "no MCP tool or HTTP route" and there has never been an HTTP redact route either; the row conflated `MemoryService` with a route. What remains is absence, not lockout. Round 4 also confirmed the same review's other structural readings and **refuted one**: hermes `is_available()` returning True on a down server is documented and deliberate (a probe would put a 2 s timeout in front of every Hermes init, `plugin/integrations/hermes/memory-wire/__init__.py:447-458`), not a silent fail-open | round-2 review's sharpest structural finding (its §3a, verified): "one service, three doors" is true of `MemoryService` and false of parity — every door exposes a different subset, so a capability can be unreachable from the surface an agent is actually on. Recorded with the verified table in `docs/INTEGRATION_GAPS.md` (2026-11-19 addendum); not silently broken, just absent | `docs/INTEGRATION_GAPS.md` addendum; `src/mcp.rs`, `src/api.rs`, `src/main.rs` | a surface decision, not a bug fix: either a documented ruling that the doors stay asymmetric, or redact on MCP — the widest gap, since an agent that retained a secret through MCP cannot retract it through MCP |
| 32 | done | ~~recall failures are indistinguishable from an empty bank~~ — **done 2026-11-19**: `recall_at_fmt` returned `None` for three different reasons — connection refused, non-2xx, and a 200 body this build could not parse — and all three produced byte-identical silence (exit 0, empty stdout). Each now names itself on stderr via `tracing::warn!`, so a hook that recalled nothing says which of the three happened. `RUST_LOG`-gated and stderr-only, so §9's never-fail contract is intact and `RUST_LOG=off` is total silence. The empty-query path stays silent because it is not a failure → [2026-11-19] `src/hooks.rs`; `CONSISTENCY.md` §52 | round-2 review had this as "empty framing violates AGENTS §10"; its verification pass refuted that and named the real defect — the agent and the user cannot tell "no memories matched" from "the server is down". §9 (never-fail silence) and §10 (an empty result says so) genuinely conflict here; §10 governs output that claims to be an answer, and a hook's empty stdout claims nothing | `src/hooks.rs`, `recall_at_fmt` | ~~each failure mode produces a distinct stderr line, none reaches stdout, exit stays 0, `off` is silent~~ — observed live against three real servers, plus a test pinning the modes differ and stdout stays empty |
| 33 | done | ~~the remaining-work plan's four batches~~ — **all done 2026-11-19** (register corrections `e0f1bb4`; hermes trust-gate proof `db57850`; dates on three doors `0b518de`; capsule knob on the plugin surfaces `1f93bf0`). Batch 1 corrected four stale open rows (P2.1, P4.1, P3.1, G6.8) whose work had already landed. Batch 2 gave `Trust::UntrustedExternal`'s fourth door an executable check. Batch 3 served `created_at` in hermes and both TypeScript plugins, which requested `format: "full"` and dropped the field. Batch 4 decided the capsule knob by cost: the plugin surfaces read `MEMORY_WIRE_FORMAT`, not the bank config key, because the env costs no request and the config would add one per turn → [2026-11-19] `plugin/integrations/`, `docs/`, `CONSISTENCY.md` §54–§57 | the round-3 context-pack review and the follow-up register audit kept finding one shape — a policy enforced in N places with a check in N-1, and a list that stopped being maintained. These four batches close that shape for the four doors and leave a runner that enumerates them | `docs/handoff/REMAINING_WORK_PLAN.md` | ~~every door honours the same serving shape and is covered by the same runner; every claim in the register matches the tree~~ — asserted, with per-door mutation checks and live runs |
| 34 | done | ~~door parity, widest gap only: **redact is reachable from the CLI but not from MCP**~~ — **done 2026-11-20** (`memory_redact` is the 6th tool): an agent that retained a secret through MCP can retract it through MCP. `MemoryService::redact_memory` already existed and the `read_only` guard already covered it, so the work was a tool registration, a dispatch arm and three tests. **Corrected by the round-5 review**: this row first said "CLI and HTTP" and there is no HTTP redact route -- the row conflated `MemoryService` with a route. The test deliberately uses content that *survives* the retain-side PII redactor — `sk-live-deadbeef` is caught at retain time by the OpenAI rule, so a test built on it would pass a tool that never had to work. Mutation-checked: a fake implementation returning `rows = 1` without writing fails the test. Tool counts corrected in `AGENTS.md` §4 + §3, `README.md` ×3, `src/main.rs`, `src/mcp.rs` (HISTORIAN doc, two `four tools above`), `src/mcp_http.rs`, `docs/INTEGRATION_GAPS.md` ×2 → [2026-11-20] `src/mcp.rs`, `src/mcp_http.rs`, docs; `CONSISTENCY.md` §65 | the one door parity hole where the *retraction* of a credential was impossible from the surface an agent is on; the other doors' asymmetries (hygiene has no CLI, supersede has no HTTP) are absence, not lockout | `src/mcp.rs` (`McpServer` tool list), `src/api.rs` `redact_memory`, `src/store.rs:2242` | ~~an MCP client can redact a row it can read, and the tool count test expects six~~ — done, with the live surface test and the readOnly refusal both passing |
| 35 | done | ~~the shipped opencode plugin does not load on opencode 2.x~~ — **done 2026-11-20** (`3376b5c`): found live while probing B2. The plugin shipped a v1 bare-function export (`PluginModule.LoadError` on 2.3.2, which requires `{id, setup}`) and was installed as a flat file, which opencode 2.x skips (it needs a subdirectory with `index.ts`). Both fixed, the version marker moved 2→3, legacy flat files are removed on install and uninstall, and a stub-ctx drive proved injection and the retain round-trip. `session.idle` does **not** fire under `opencode run`, so the capture hook is `session.execution.succeeded`. This was the same door the round-4 review looked at and called healthy — it was dead on the host it named | a door that has never loaded on its named host is worse than a missing door: `connect` reports success and the host silently has no memory. The review's trust-gate work correctly covered what reaches a model and could not see that nothing did | `plugin/integrations/opencode-plugin/memory-wire.ts`, `src/connect_plugin.rs`, `src/connect.rs`, `plugin/integrations/trust_gate_test.sh` | ~~opencode 2.x loads the plugin, the prompt hook injects, the retain hook writes~~ — live on 2.3.2, plus an export-shape check in the trust gate |
| 36 | open | the TypeScript doors (extension/omp, opencode) still carry a hand-copied ~100-line domain core, and the opencode port changed the file enough that a straight extract is now the wrong first move. Probing found both hosts resolve relative imports *if the door is a directory* — omp loads every flat `.ts` as an extension and errors on a helper (`does not export a valid factory function`), while `index.ts` in a subdirectory loads cleanly. So extraction is feasible but it means restructuring both installs, and the hermes door is a Python package with its own packaging. Option 1: concatenate the shared core via `include_str!` (no host resolves it, no restructure). Option 2: subdirectory + relative import. Option 3: a golden-file drift test that fails when the cores diverge, costing nothing and proving nothing about correctness | measured, not assumed: 101 lines of genuine duplication (markers, cap derivation, tier handling), and the drift is a real maintenance cost — a change to the cap shape must be made in two places or the doors diverge silently | `plugin/integrations/hermes/memory-wire/__init__.py`, `plugin/integrations/extension/memory-wire.ts` | a mutation in one door's core is caught in the other, or the core exists in exactly one place |
| 37 | done | ~~**cross-door blank-content handling differs**~~: the TS doors skip blank/whitespace rows before the take (`extension/memory-wire.ts:241`, `opencode-plugin:215`), while `src/hooks.rs` and `hermes/__init__.py` count them against the cap and render `- ` for them. **Verified as latent, not live**: `src/api.rs:760` rejects blank content on every retain path, so this build cannot write one — but the schema is `content TEXT NOT NULL` with no `CHECK`, so an older build or a raw `sqlite3` insert can. Fix is to skip blank rows in Rust and Python too, matching TS. Not done yet because a change made on a review's mention, with no reproducing case, is a change made on assertion (AGENTS §8); a `CHECK` constraint or a test that inserts a blank row through SQL would make it reproducible, and then it is one line | round-5 review (2026-11-20) found the divergence by reading all four auto-injection doors side by side; narrowed by me with the write-path check. Four doors implementing one policy in three languages is the shape where this recurs | `src/hooks.rs` (`recall_section`), `plugin/integrations/hermes/memory-wire/__init__.py` (`prefetch`), `src/api.rs:760`, `src/store.rs` schema | ~~all four doors agree on blank rows~~ — Rust + Python skip blank content before the take, and a test inserts a blank row through SQL to prove the path is reachable |

| 38 | open | **the enumerated cross-door parity check both review rounds asked for** — rounds 4 and 5 independently reached the same instrument from different evidence: a check *sourced from `Host::ALL`* in `src/connect.rs` that asserts every host wires the same hooks/MCP/env contract, and a cross-door suite that drives `hooks.rs`, hermes, extension and opencode against one fixture set (failed-then-recovered `/recall`, repeated prompts, blank content, section-cap overflow, `format=summary`). Round 5's argument for this over a shared abstraction: 52% of `connect_*` is inline tests that cannot be deduplicated without obscuring golden expectations, only 1,793 production lines sit in the six per-host adapters, and a Rust trait cannot see host-side load failures like the opencode one. Round 5's falsifier: if all four doors and all declared host layouts pass such a suite with zero silent-dead-door failures across a release cycle, the verification bottleneck is closed | two independent reviews converging on the same instrument is the strongest signal in `research/audits/` — stronger than either round alone. The `trust_gate_test.sh` extensions in this session are a partial down-payment: they check export shape and the opencode cache rule, not the full matrix | `scripts/` (new, CI-only, 0 binary bytes), `src/connect.rs` (`Host::ALL`), `plugin/integrations/trust_gate_test.sh` | ~~a change to any one door fails a check unless every door got it~~ — a parity suite that enumerates doors from `Host::ALL` and drives each against one fixture set |


## Deliberately not doing

- **Score thresholds on auto-injection** without the dev set above — the
  lever list is composition, caps, labels and tags; measured in
  CONSISTENCY §32.
- **Parent parity, tool-count matching, container images** —
  `docs/INTEGRATION_GAPS.md` "Explicitly not recommended"; the reasons are
  measured, not stylistic.
- **Anything LongMemEval was consulted to choose** — AGENTS §6, always.
| 18 | done | ~~OmniMemEval provider + first measured run (HaluMem)~~ — **done 2026-10-17** (stale-row flip; acceptance met by H10, then H12 re-measured the instrument): provider shipped + first HaluMem run committed (`eval/omnimevaleval/memory-wire-mw_h10_medium/`, §44), then H12's dated re-run (`eval/omnimevaleval/memory-wire-mw_h12_dated_medium/`, §46 — Dynamic Update 0.2056→0.4056, overall 0.6129→0.7433, different-instrument pair, 0 test consultations). **PersonaMem v2 remains named-not-scheduled** (RND_PLAN §6.5 second-order) → [2026-10-17] `eval/omnimevaleval/memory-wire-mw_h12_dated_medium/` | the battery's only two brand-new corpora are behind it: **HaluMem** (hallucination resistance — nothing in our battery measures that) and PersonaMem v2, both measure-only, classified before any run (`docs/BENCHMARK_LANDSCAPE.md` §4.6). Their LongMemEval arm is a **test consultation**; their LoCoMo is a dev-judged cross-check, never a ranking-selection signal | `eval/amb/memory_wire.py` (uuid→document map is the template), pinned clone of MemTensor/OmniMemEval (Apache-2.0; dataset licences stay upstream — never vendor their data), `docs/COMPETITOR_GAPS_PLAN.md` §3 | ~~one HaluMem run, committed as their own output artifact next to the prose that quotes it, whatever it shows~~ — H10 + H12 landed |
| 19 | done | ~~competitor architecture audit — MemOS, Honcho, OpenViking (+ fresh-question re-reads of agentmemory, Hindsight)~~ — **done 2026-10-15**: all six complete against pinned commits (`COMPETITOR_GAPS_PLAN.md` §6). Net: **H8 unblocked** (OpenViking's capsule is structural — `tiers.py:107` first-line/section/AST, no LLM), **H10 unblocked** (OmniMemEval LLM clients are OpenAI-compatible — `ANSWER_BASE_URL`/`EVAL_BASE_URL`; HaluMem 3,467 QA, **CC BY-NC-ND 4.0 — run, never vendor**), **one new H-item registered (H11** — Honcho's `times_derived` → a `times_retained` dedup counter as a fusion tiebreak, battery-gated), **one candidate killed honestly** (agentmemory's session diversification — a no-op at session-as-memory granularity), and **one comparability fact strengthened** (their 95.2% ran with graph weight 0.0; the reranker is a bi-encoder). **The ranking core receives no incoming mechanism from the field** — every scoring-relevant idea across all six repos is closed, dependency-gated, or a no-op here → [2026-10-15] `docs/COMPETITOR_GAPS_PLAN.md` | the in-house R&D queue was empty; the field's architectures were the only evidence-backed source of new mechanisms. Veto chain enforced: audit note → closed-lever veto → constraint gate → pre-registered H-item | pinned scratch clones in `../_audit_scratch/` (memos `a7367d0`, honcho `ed17042`, openviking `621b1dd`, omnimevaleval `a5c8b4b`); parents in `../_audit/` | ~~audit notes with pinned commits; anything that survives becomes an H-item~~ — H8 confirmed, H10 unblocked, H11 registered, everything else closed or recorded |
| 20 | done | ~~**H12: dated HaluMem re-measure**~~ — **done 2026-10-17** (CONSISTENCY §46; artifacts `eval/omnimevaleval/memory-wire-mw_h12_dated_medium/`, their own output unedited; pipeline rc=0 after a one-judgment metric resume): Dynamic Update 0.2056→**0.4056** (+20.0pp), Memory Conflict +19.7pp, Basic Fact Recall +22.8pp, overall 0.6129→**0.7433**; the Memory Boundary control held approximately (0.9372→0.9118) and context grew only +12% — the gain is dates, not tokens. Measure-only, 0 test consultations; the pair is two instrument versions, nothing selected; 0.6129 stands as the undated instrument, 0.7433 the honest headline for any §6.4 head-to-head → [2026-10-17] `eval/omnimevaleval/memory-wire-mw_h12_dated_medium/` | (mechanism and pre-registration as originally filed) H10's Dynamic Update **0.2056** was measured with the reader blind to occurrence time: the adapter stores `created_at` on retain but `search()` discarded it — unanswerable-by-construction for newest-value questions, the same defect TODO #16 fixed for LongMemEval (17.0→18.6%, H5) | `../_audit_scratch/omnimevaleval/scripts/client_factory/memory_wire_client.py` (env-flagged `MEMORY_WIRE_SERVE_DATES=1`, landed at pinned `a5c8b4b`), their HaluMem pipeline through `eval/llm_shim.py` | ~~artifacts + §46, whatever they show~~ — landed |

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
| 21 | done | ~~**W0.1 — make `golden_install_shapes` hermetic.**~~ — **done 2026-11-18** (`a45ea07`): the golden test drives `apply(..., no_path, ...)` like its 9 siblings, so detection is a function of the fixture home, not of what is installed; the environment-dependent failure is gone → [2026-11-18] `a45ea07` | `cargo test --locked` is red on any host without `copilot` installed, so every later gate was unreadable | `src/connect.rs` golden test | `PATH=/usr/bin:$PATH cargo test --locked` green on a host with no agent harnesses installed |
| 22 | done | ~~**W0.2 — pin the redactor against code identifiers.**~~ — **done 2026-11-18** (`a45ea07`): `redact_should_leave_identifiers_alone` pins UUIDs (plain and namespaced), git SHAs (short/abbreviated/full), file paths (ordinary and dotted), semver (with and without pre-release/build metadata), credential-free URLs, and bare `user@host`; a companion test pins the documented `user@dotted-host` boundary so a future rule cannot silently widen the email match → [2026-11-18] `a45ea07` | redaction runs on every write; a false positive silently mangles a code snippet an agent stored | `src/capture.rs` tests | one case per shape, asserting what it must and must not become |
| 23 | done | ~~**W1 — dimensional metadata.**~~ — **done 2026-11-18** (`e10ba0b`): `scope` / `aspect` / `lifecycle` / `trust` / `parent_id` / `superseded_by` / `scope_globs` added to `memories` via the `has_column` migration path, threaded through `MemoryMeta`; `recall` filters narrow in SQL before ranking; `the_w1_columns_should_be_able_to_hold_a_non_uniform_corpus` asserts the triple is not constant; migration from a pre-column DB verified and no-filter recall is byte-identical → [2026-11-18] `e10ba0b` | without these there is no way to separate a preference from an episodic turn, so nothing downstream can prioritize one | `src/store.rs` (`migrate`), `src/memory.rs`, `src/api.rs`, `src/recall.rs` | migration reads a pre-column DB; recall with no filters byte-identical to today; non-uniformity assertion passes |
| 24 | done | ~~**W2 — mount profiles.**~~ — **done 2026-11-18** (`8eaeac3`): `read_only` and `maxTrust` on `BankConfig`; writes to a `read_only` bank refused on HTTP, MCP and hook; a write above the ceiling is downgraded rather than refused; a bank with no config is unchanged. Batch-1 (`af3a0a5`) completed the surface afterwards: every write path — including the custody writes `delete_memory` / `supersede` / `redact` — is now gated on `read_only` → [2026-11-18] `8eaeac3` + `af3a0a5` | an agent that ingested web content must not write into the same bank it later reads as trusted rules | `src/api.rs` (`BankConfig`, `retain_doc_at`), `src/mcp.rs`, `src/hooks.rs` | `read_only` refuses on HTTP + MCP + hook; `untrusted_external` cannot land as `scope=policy`; a bank with no config is unchanged |
| 25 | done | ~~**W3 — retroactive redaction.**~~ — **done 2026-11-18** (`052f3a6`): `memory_versions` (append-only) + `redact(memory_id, pattern)` rewriting current and historical content; the W3 CLI surface (`memory-wire redact`) landed with `37c39b5`; a redacted secret is absent from both tables, the version skeleton survives, a second redact is a no-op → [2026-11-18] `052f3a6` + `37c39b5` | a secret discovered after ingest cannot be scrubbed without destroying the row | `src/store.rs`, `src/api.rs`, `src/capture.rs` | a redacted secret absent from both tables; version rows and timestamps survive; a second redact is a no-op |
| 26 | done | ~~**W4 — deterministic hygiene scan.**~~ — **done 2026-11-18** (`ad8472a`): `src/hygiene.rs` — delta cursor, near-duplicate, contradiction, relative-date, promotion and bloat candidates; read-only `hygiene_scan` + explicit `hygiene_apply`. **Surface deviation, recorded:** the plan's "5th MCP tool = `memory_hygiene`" did not ship — hygiene is on the HTTP door (`GET /banks/:id/hygiene`) and the fifth MCP tool is `memory_supersede` (W7a); the deviation is documented in the plan §2/W4 rather than retro-fitted → [2026-11-18] `ad8472a` | no relative-date or contradiction detection existed anywhere; the "Dreaming" substrate with no LLM and no process | `src/hygiene.rs` (new), `src/mcp.rs`, `src/store.rs` | planted duplicate/contradiction/relative-date/oversized row each named and nothing else; scan deterministic across runs; ranking path untouched |
| 27 | done | ~~**W5 — startup grounding capsule.**~~ — **done 2026-11-18** (`4540b19`): `src/grounding.rs` — per-bank, opt-in, bounded index of active preference/invariant/trap/map rows at policy/global/domain scope; 40 lines / 2048 bytes default; `scope_globs` path conditioning; the CLI surface (`memory-wire grounding`) landed with `37c39b5`; the capsule is subtracted from the recall budget when both are served → [2026-11-18] `4540b19` + `37c39b5` | the measured wins were serving shape and budget; the capsule must cost ~500 tokens, not 25 KiB | `src/grounding.rs`, `src/api.rs`, `src/hooks.rs`, `src/store.rs` | ≤40 lines / ≤2048 bytes at defaults; capsule + recall ≤ `DEFAULT_RECALL_BUDGET`; glob-scoped row appears only with a matching path; flag-off path byte-identical |
| 28 | done | ~~**W6 — `connect --with-skills`.**~~ — **done 2026-11-18** (`4793326`): `connect --with-skills` deploys `plugin/skills/memory-wire/SKILL.md` idempotently, refuses a modified copy by name, atomic with re-read verification; a bare `connect` installs it too and `--uninstall` composes with it (Batch 3, `3805085`); `SKILL.md` updated for the W1–W5 model → [2026-11-18] `4793326` + `3805085` | a skill no host receives is documentation, not integration | `src/connect.rs`, `src/skills.rs`, `plugin/skills/memory-wire/SKILL.md` | a fresh `HOME` gets the file once; a second run is a no-op; a locally modified copy is refused with a named reason |
| 29 | done | ~~**W7 — the per-turn injection path, and a supersede verb.**~~ — **done 2026-11-18** (W7a `d34a0a5`: `memory_supersede` MCP tool — the 5th tool; default recall excludes superseded rows, `include_history` returns them with lineage; W7b `eb95a0a`: **decision, not code — no raw score gate on `recall_section`**; a raw cut is mathematically meaningless (per-query min-max normalisation), a closed lever, and a runtime policy, not a store concern; any future gate is a frozen-battery hypothesis, never a threshold). Batch 1 (`af3a0a5`) finished the readOnly story: the supersede annotation is written `read_only_hint=false` because it is a write → [2026-11-18] `d34a0a5` + `eb95a0a` + `af3a0a5` | the one place the review found the plan optimising a subsystem that is already fine while a diagnosed per-turn defect sat outside it | `src/hooks.rs` (`recall_section`), `src/mcp.rs` (5th tool), `src/api.rs` | supersede round-trips; 4-tool count becomes 5 with a documented reason; any score gate is a frozen-battery hypothesis, never a threshold |
