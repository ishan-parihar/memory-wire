# AGENTS.md — orientation and rules for agents working in this repository

Read this before changing anything. Sections 1–4 are orientation: what this is, how
to build it, where things live, and which document answers your question. Sections
5 onward are rules — the ones that exist because they were learned the hard way.

---

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
  currently 9.7 MiB with `DT_NEEDED` = 3. Verify `ldd` after touching anything
  that links.
- **Retrieval reports are the record.** The competitor whose retrieval number we
  can actually compare against is agentmemory, on LongMemEval-S, and we lead it
  on the honest (unfitted) number. Do not publish a number you cannot trace to
  a committed artifact.
- **Retrieval is a reordering problem, not a coverage problem.** The candidate
  pool contains the gold memory in 500 of 500 LongMemEval questions. Work on
  ordering; do not spend effort on recall/coverage mechanisms.

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

All four are the gate. The default port is `8888` and there is exactly one
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

`src/`, 24.8k lines across 22 modules:

| module | lines | role |
|---|---|---|
| `store.rs` | 4353 | SQLite: schema, migrations, put/recall/delete, FTS5 |
| `api.rs` | 3080 | HTTP routes, the frozen error contract, the historian prompt |
| `recall.rs` | 2428 | RRF fusion, candidate pool, budget trim, scoring |
| `main.rs` | 1926 | clap surface, routing, wiring |
| `mcp.rs` | 1577 | MCP server: 4 tools + resources + prompts + completion |
| `connect.rs` | 1263 | agent-host detection and config writing |
| `daemon.rs` | 1132 | `daemon start/stop/status`, pidfile, start-time identity |
| `connect_plugin.rs` | 1100 | the hermes `MemoryProvider` plugin |
| `vector.rs` | 960 | dense arm (`embed` feature, OFF by default) |
| `cli_home.rs` | 948 | the content-first bare-invocation view |
| `hooks.rs` | 925 | the five lifecycle events |
| `connect_codex.rs` | 730 | codex TOML config writing |
| `render.rs` | 655 | human-readable report rendering |
| `seed.rs` | 649 | transcript reading for hook capture |
| `paths.rs` | 640 | bank identity, endpoint, data paths |
| `doctor.rs` | 590 | the health report |
| `mcp_http.rs` | 478 | stateless streamable-HTTP MCP mount |
| `embed.rs` | 177 | ONNX session, vendored MiniLM |

`tests/` — `backup.rs`, `daemon.rs`, `e2e.rs`, `recall_budget.rs`, `scale.rs`.
`examples/` — 14 benchmark and evaluation harnesses.

## 4. The surface, and which document answers what

Before you change behaviour, find the document that owns your question. Reading
the wrong one is how claims drift.

| if you are asking about… | read |
|---|---|
| can I tune this against the benchmark? | **this file**, §6, then `docs/EVALUATION_HYGIENE.md` |
| what was measured, on what, at what load, and what was rejected | `docs/CONSISTENCY.md` — **append-only, never rewrite** |
| how memory-wire compares to agentmemory and Hindsight | `README.md`, then `docs/EXCEED_PLAN.md` |
| what a latency number means and how to take one | `docs/BENCHMARK.md`, `docs/BENCHMARK_SCALE.md` |
| what is still open, and what is deliberately not being done | `docs/INTEGRATION_GAPS.md` — written at `eec026f`, partly stale; verify each row against the binary |
| what is left to implement, as tracked work items | **`TODO.md`** at the repo root — the implementation register; indexes the domain lists in this table rather than duplicating them |
| what we install into each agent harness, and what is quietly broken | `docs/INTEGRATION_AUDIT.md` — findings ranked F1–F7, each verified or explicitly marked unverified |
| how to make the harness integration correct rather than merely present | `docs/INTEGRATION_PLAN.md` — two open decisions block Phase 1 |
| why a hook injected something irrelevant, and why adding a score threshold is the wrong fix | `docs/OPEN_HOOK_RECALL_RELEVANCE.md` — diagnosed, deliberately unfixed |
| which retrieval lever was tried and killed, and by what measurement | `docs/RERANKING_PLAN.md` §3, `docs/PERFORMANCE_PLAN.md` |
| where the numbers came from, honestly | `eval/README.md`, and the artifact it names |
| dependency versions and why they are pinned | `docs/VERSIONS.md` |
| how a bank id is derived, what can collide, and why a hook recalls nothing | `docs/BANK_IDENTITY.md`; `memory-wire doctor` names the bank that holds the memories when the resolved one holds none |
| the next iteration's plan and its gates | `docs/NEXT_ITERATION.md`, `docs/EXCEED_PLAN.md` |
| the original audit this all came from | `docs/AUDIT.md` |

**The MCP surface.** Four tools — `memory_retain`, `memory_recall`,
`memory_reflect`, `memory_bank_config_get` — plus resources
(`memory://{bank}/{id}`), one prompt (the historian preamble), and argument
completion. That is 8 of the surface methods rmcp offers. Deliberately **not**
advertised: `subscribe`, the three `listChanged` flags, and `logging`, because
this server never sends those notifications. Advertising a capability that never
arrives is worse than advertising nothing, and a test pins the exact set.

The transport is **stateless** (rmcp 3.5, MCP spec `2026-07-28`): no
`initialize` handshake, no `Mcp-Session-Id`. A legacy client that still
handshakes at `2025-06-18` is answered and negotiates that version, so cursor,
opencode and the hermes plugin are unaffected. Note that `GET /mcp` answers
`405` — the SSE stream it used to open never carried a notification.

**Agent hosts.** Six: `claude-code`, `codex`, `copilot-cli`, `cursor`,
`opencode`, `hermes`. Five are additive (a hook entry or an MCP key in a JSON
file). **hermes is not** — it activates exactly one global memory provider, so a
bare `connect` would displace `agentmemory` on a machine that has both. It is
reachable by name only. That asymmetry is deliberate; do not "fix" it by adding
hermes to the implicit set.

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
- `install/get-memory-wire.sh` wires hosts with no `--bank`. On a machine
  pinned with `connect --bank <id>`, run it with `--no-connect` and re-run
  `memory-wire connect --bank <id> <hosts>` after, or the surfaces re-default
  to bank `memory-wire`.
- macOS assets cannot be cross-built from Linux — `libsqlite3-sys` links
  CoreFoundation. The script skips them with that reason.
- Delegate only with the `space-bunny` model.

## 12. Honesty rules for reports

- Report what a measurement shows, not what you hoped it would show.
- If an agent before you made a claim wrong, correct it by name.
- If you cannot determine which of two conflicting values is right, leave both
  and say so. Do not invent a reconciliation.
- A null result is a result. Record it.
- Verify before reporting. Run the gate yourself; do not cite a subagent's
  summary as if it were a measurement.
