# Integration gap analysis — v0.4.0

Written 2026-09-28, after `v0.4.0` shipped. Read-only audit of the three
codebases plus the locally installed harnesses; no code changed.

Sources: `_audit/agentmemory/` (rohitg00/agentmemory), `_audit/hindsight/`
(vectorize-io/hindsight), and this tree at `eec026f`. Every claim below carries
a `file:line` citation from those trees or a command run on this machine.

---

## 0. The shape of the field

| | agentmemory | hindsight | memory-wire |
|---|---|---|---|
| Harness integrations | 21 adapters, 1 registry (`src/cli/connect/index.ts:28-50`) | 18 in one unified package (`coding-agents/src/harness/registry.ts:41-73`) + ~50 legacy dirs | **5** (`src/connect.rs:49-55`) |
| MCP tools | **54** (`src/mcp/tools-registry.ts:957-968`) | 39 multi-bank / 36 single-bank (`hindsight_api/mcp_tools.py:36-78`) / 9 claude-plugin (`claude-code/scripts/mcp_server.py:87-196`) / 8 plugin (`coding-agents/src/core/knowledge-tools.ts:153-350`) | **4** (`src/mcp.rs:105-176`) |
| Hook event vocabularies | **5** distinct sets | 3 shapes across 18 hosts | **1** (3 events) |
| Long-lived process | detached engine + pidfiles | 4 daemonization paths | **none** |
| `stop` / `status` | `stop` `status` `remove` (`src/cli.ts:4001-4013`) | `embed daemon {start,stop,status,logs}` (`hindsight_embed/cli.py:1549-1557`), `fs {start,stop,restart,status}` (`hindsight-cli/src/commands/fs/mod.rs:67-87`) | `doctor` only, no stop |
| Harness uninstall | **none** — `remove-plan.ts` reads a `connect-manifest.json` nothing writes | full, all 18 (`installer.ts:2008-2021`) | marker-filter `--uninstall` |
| MCP annotations | **zero** on 54 tools | all 75 registrations (`mcp_tools.py:562-569`) | all 4 (`src/mcp.rs:80-87`) |

Two facts reframe everything below:

**agentmemory's 95.2% ships behind a daemon.** `npx @agentmemory/agentmemory`
spawns the engine `detached: true` then `unref()`s it
(`src/cli.ts:1520-1542`) and the CLI exits. Lifetime is pidfile-based:
`iii.pid` (`src/cli.ts:652-659`), `worker.pid` (`src/index.ts:111-118`),
`engine-state.json` (`src/cli.ts:637-649`).

**hindsight has four separate daemonization mechanisms** — Python
`--daemon` re-exec with `start_new_session=True` (`daemon.py:94-126`), a TS
plugin detached spawn (`coding-agents/src/core/daemon.ts:200-220`), a Rust
`setsid()` (`hindsight-cli/src/commands/fs/daemon.rs:90-104`), and Claude-plugin
`Popen(start_new_session=True)` (`claude-code/scripts/lib/daemon.py:305-313`).
Neither ships a launchd plist or systemd unit; both delegate to Docker or to
detachment.

**We ship no daemon at all.** No `fork`, no `setsid`, no unit files, no
container. The only documented way to get background survival is the user typing
`nohup memory-wire serve … &` (`INSTALL_FOR_AGENTS.md:20,34`).

---

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

## G2 — We capture nothing at the two moments worth capturing

**Status: resolved.** `PreCompact` and `SessionEnd` ship as `hook pre-compact` and
`hook session-end` (`src/hooks.rs`, wired in the per-host list at
`src/connect.rs:65-75`); `connect` now writes five events per host. Both read
the transcript at `transcript_path` and retain the conversation's own prose,
capped at 2,000 characters, because the transcript is a file this process can
read — so the moment does not have to be reduced to a pointer. Everything below
is the audit that motivated it and stays as the record.

Our hook surface was `SessionStart`, `UserPromptSubmit`, `Stop`
(`src/hooks.rs:47-55`). Both competitors reached moments we did not.

| Moment | agentmemory | hindsight | us |
|---|---|---|---|
| `PreCompact` | **yes** — in the 12-event Claude Code set (`plugin/hooks/hooks.json`) | no | **yes** — `hook pre-compact` |
| `SessionEnd` | yes (camelCase `sessionEnd`) | **yes** — the only one of its 18 harnesses that has it (`claude-code/hooks/hooks.json:1-49`) | **yes** — `hook session-end` |
| `PostToolUse` / `PreToolUse` | yes | yes (Cursor CLI, Copilot CLI) | no |
| `on_pre_compress` (plugin-native) | hermes plugin (`integrations/hermes/plugin.yaml:6-12`) | — | no |
| `system_prompt_block` | hermes plugin | — | no |

`PreCompact` is the highest-value moment that exists in Claude Code: it fires
immediately before context compaction discards the conversation. Memory written
there survives; memory written afterwards does not. My own audit recorded this
as item G11 — "best hook bytes available" — and it was never built.

`SessionEnd` is the clean flush point. `Stop` fires per turn; `SessionEnd` fires
once, with a known-conversation state, and is registered on this machine
(`~/.claude/settings.json` has `SessionStart`, `UserPromptSubmit`, `SessionEnd`,
`PreToolUse`, `PostToolUse` — we use three).

**Cost:** two enum variants, two match arms, two entries in the per-host event
list. agentmemory spends 12; hindsight spends 4 on Claude Code. We spent 3, and
now spend 5.

**Still open, and it is the more interesting half of this gap.** `stop` retains
a transcript *path* and its size on disk, not the conversation
(`src/hooks.rs`, `stop_at`). Nothing in this crate can read that row back into
content: `recall` returns the string "session ended; transcript /home/…/abc.jsonl
(1.2 MB)" and no way to reach what the session said. The two new events do not
copy that shape — they read the file, because the file is right there and
readable — but `stop` itself was left byte-for-byte as it was, since changing it
was out of scope. It is now the weakest of the five and the obvious next cut.

---

## G3 — No hermes-agent integration, and the competitor is installed there

> **Resolved 2026-09-29.** `memory-wire connect hermes` installs a `MemoryProvider`
> plugin at `~/.hermes/plugins/memory-wire/` and sets `memory.provider` in
> `config.yaml`. Five of the six events ship; `on_memory_write` is refused with a
> visible one-line warning rather than faked. Verified through Hermes' own
> `load_memory_provider()`, including a byte-identical uninstall against a
> pristine 31 KB `config.yaml`. The audit below is kept because the loader findings
> are the interesting part — four of them are places the *installed competitor*
> violates Hermes' own contract. Full detail in `docs/CONSISTENCY.md` §20.3.

`~/.hermes/plugins/agentmemory/` **exists on this machine** (`plugin.yaml`,
`__init__.py`, `README.md`). It declares six hook events — `prefetch`,
`sync_turn`, `on_session_end`, `on_pre_compress`, `on_memory_write`,
`system_prompt_block` — and implements a formal `MemoryProvider` interface:
`is_available()`, `initialize(session_id)`, `get_tool_schemas()`,
`handle_tool_call(name, args)`, `get_config_schema()`, `save_config()`.

hindsight ships a hermes integration too
(`hindsight-integrations/hermes/`, 6 events). agentmemory's own
`connect hermes` adapter is a stub — it returns
`{kind:"stub", reason:"yaml-merge-not-implemented"}`
(`src/cli/connect/hermes.ts:44-45`) — and routes users to the plugin folder
instead.

**We have neither.** And hermes is not a JSON-config host: it has
`~/.hermes/plugins/` (20+ installed), `~/.hermes/agent-hooks/` with a
`pre_tool_call` protocol that returns `{"action":"block","message":…}` on
stdin, `~/.hermes/skills/`, and HTTP MCP via `[mcp_servers.*] url = …` in
`config.yaml`. Our `connect` model — write JSON into a host's config file —
does not reach it.

---

## G4 — Bank isolation is name-based, and two different repos can collide

**Status: resolved.** The hook bank is derived from the `origin` remote in
`.git/config`, parsed as text — `git` is never spawned, which also removed the
pre-existing `git rev-parse --show-toplevel` subprocess that the note below
warns about. The order is `--bank` → `MEMORY_WIRE_BANK` → remote `owner/repo` →
worktree basename → `memory-wire`, documented in `docs/BANK_IDENTITY.md`, with
`doctor` warning when a legacy-named bank still holds memories. The collision
test `same_basename_under_different_parents_should_not_share_a_bank` is the bug
in one line and fails against the old derivation.

**One caveat that does not go away:** sanitisation is lossy, so `acme/api` and
`acme.api` both become `acme-api`. A large improvement on basename, not a
uniqueness proof. The explicit and env steps are the escape hatches when an id
must never collide.

The audit below is the record of why it was done.

This one is a correctness issue, not a feature gap.

Our hook path derives the bank from the **git worktree top-level basename**
(`src/paths.rs:80-94`, `src/paths.rs:61-78`). So `/home/x/api` and
`/home/y/api` are both bank `api`. Two unrelated projects, one namespace.
`README.md:406-407` documents that `MEMORY_WIRE_BANK` does *not* override the
hook's bank, so there is no escape hatch on the hook path — only on
`serve`/`mcp`.

Both competitors solved this deliberately:

- **agentmemory**: `AGENT_ID` env plus `AGENTMEMORY_AGENT_SCOPE=isolated`
  (`src/config.ts:320-348`), and an explicit `agentId` parameter on save,
  recall and search whose description warns that omitting it means shared memory
  (`src/mcp/tools-registry.ts:81-86`).
- **hindsight**: a template `coding-agent::{gitProject}`
  (`coding-agents/src/core/bank.ts:53`) resolved against the git filesystem
  layout — read **without spawning `git`**, because the old
  `git rev-parse` path mapped every failure, including `EAGAIN`, to `null`
  (`coding-agents/src/core/git-layout.ts:1-13`). Opt-in **fails closed**:
  `isOptedIn` is off by default and a bare `bankId` does not approve
  (`bank.ts:252-269`). There is deliberately no per-repo config file, because
  "a cloned repository must not be able to turn memory on."

**We have no opt-in at all and no collision resistance.** A repo you clone can
read and write your memory by having the same directory name.

---

## G5 — 4 MCP tools against 8, 39 and 54, and stdio only

Our surface: `memory_retain`, `memory_recall`, `memory_reflect`,
`memory_bank_config_get` (`src/mcp.rs:105-176`). Transport is stdio only
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

## G6 — Smaller items, each cheap

| # | Gap | Evidence | Note |
|---|---|---|---|
| G6.1 | **No `stop`/`restart` command.** Signals only (`src/main.rs:418-440`). | `agentmemory stop` `src/cli.ts:3445-3585`; `hindsight fs restart` `fs/mod.rs:150-153` | `doctor` covers *status*, not termination |
| G6.2 | **`/health` verifies nothing** — `async fn health() -> &'static str { "ok" }` (`src/main.rs:500-503`), no store access. | hindsight splits liveness/readiness by design (`worker/main.py:107-123`) | `doctor` compensates (`src/doctor.rs:134-139`) and this is a **strength**, not a gap |
| G6.3 | **Guidelines have no uninstall.** `--uninstall --guidelines` is refused (`src/main.rs:236-238`). | both competitors fully reverse every write | the block is marker-delimited so removal is easy to add |
| G6.4 | **No `--endpoint` flag on the hook path.** Only `MEMORY_WIRE_URL` (`src/paths.rs:38-45`). | agentmemory threads `--api-url` through re-resolution with a `tokenProvider` (`host-client.ts:68-72`) | minor |
| G6.5 | **Port default mismatch: `serve` binds 8899, every client defaults to 8888.** **RESOLVED** — `paths::DEFAULT_ADDR` is now the only port literal in the tree; both clap defaults (`serve`, `daemon start`) read from it; a test reads the default back out of the built clap `Command` and has teeth. Binary byte-identical. See `docs/CONSISTENCY.md` §20.2. | `src/main.rs:57`, `src/daemon.rs:175` vs `src/paths.rs` | **our bug**, fixed |
| G6.6 | **SKILL.md is not referenced from the README.** A search for `plugin`/`SKILL`/`marketplace` in `README.md` returns nothing. | hindsight publishes 2 plugins to a marketplace (`.claude-plugin/marketplace.json:1-21`); agentmemory 5 marketplaces | our agent skill is undiscoverable |
| G6.7 | **SKILL.md is internally inconsistent**: front matter says 8.5 MiB, body says 8.4 MiB. Fixed to 8.5 MiB; the byte count is now 8,964,464 B (8.55 MiB), not the 8,872,000 B (8.46 MiB) this row originally quoted — that was the pre-daemon build. | `plugin/skills/memory-wire/SKILL.md:3` vs `:8-9` | ships to agents; corrected |
| G6.8 | **No `system_prompt_block` injection.** We inject at `UserPromptSubmit`; hermes wants a block in the system prompt. | `~/.hermes/plugins/agentmemory/plugin.yaml:6-12` | different mechanism, not strictly missing |
| G6.9 | **No health check in `doctor` that the *installed hook* still points at a live binary.** `doctor` checks the store and the server, not whether `~/.claude/settings.json` references an existing path. | hindsight's installer refuses to write when a foreign `hindsight` server exists (`installer.ts:1766-1775`) | would catch a moved or deleted binary |

---

## What we already do better

Not padding. These are mechanisms neither competitor has, verified in our tree.

1. **Atomic write with re-read verification.** `paths::write_atomic` is
   temp-file + rename + read-back (`src/paths.rs:124-158`). agentmemory's
   equivalent exists (`util.ts:96-101`) and hindsight's does not appear to.
2. **Malformed host config is refused, not guessed.** Parse failure aborts that
   host, leaves the file **byte-identical**, and the test asserts both the
   byte-identity *and* that no backup directory was created
   (`src/connect.rs:691-712`). Both competitors overwrite.
3. **Stale-path self-heal.** Ownership is a substring match on the serialized
   entry (`src/connect.rs:206-209`), so re-installing from a moved binary
   *replaces* the stale path rather than stacking a duplicate
   (`src/connect.rs:518-525`).
4. **Guidelines refuse on unbalanced markers** rather than guessing where user
   content resumes (`src/guidelines.rs:130-139`).
5. **A published worst-case hook wall clock.** Three 2s caps, a derived ~6s
   `session-start` ceiling (`src/paths.rs:22-26`), and a host-side `timeout: 5`
   written into every entry (`src/connect.rs:28`).
6. **MCP annotations on every tool** (`src/mcp.rs:80-87`) — agentmemory has
   **zero** on 54, so no client can tell a read from a write there.
7. **Opaque client-facing errors.** Unknown tool → `MethodNotFound`; tool failure
   → `isError` with the same message HTTP returns, so no driver text or on-disk
   path reaches a client (`src/mcp.rs:9-15`).
8. **A doctor that refuses a foreign 200.** It requires 2xx *and* a body of
   exactly `ok` (`src/doctor.rs:134-139`), with a regression test named for the
   bug (`src/doctor.rs:284-286`).

---

## Recommended order

Cheapest and highest-value first. Each is independently shippable.

1. ~~**G4 — bank collision + a fail-closed opt-in.**~~ **Done** for the collision; the
   opt-in policy is not. Identity now resolves from the `origin` remote in
   `.git/config` — `git` is never spawned — so `/home/x/api` and `/home/y/api` no
   longer share a bank. Whether a repo must *opt in* to memory is a product policy
   question and was deliberately not bundled into a correctness fix. See §20.3 and
   `docs/BANK_IDENTITY.md`.
2. ~~**G2 — `PreCompact` and `SessionEnd` hooks.**~~ **Done.** Both ship, and
   they persist the conversation rather than a pointer to it. Carried forward:
   make `stop` read the transcript too, so no retained memory is a bare path.
3. ~~**G1 — `memory-wire daemon {start,stop,status}`.**~~ **Done.** A detached
   `serve`, a state file with a pid *and* a start time, and a `status` that exits
   nonzero unless a server is really answering. The largest single improvement to
   every existing user's experience, and the one that makes our never-fail hooks
   safe rather than merely quiet. Also closes G6.1's `stop`.
4. **G6.5, G6.6, G6.7 — the three small corrections.** Port default, README
   cross-reference to the skill, exact binary size.
5. ~~**G3 — hermes integration.**~~ **Done**, as a plugin rather than a config edit.
   A bare `connect` deliberately does **not** wire it: the other five hosts are
   additive, while Hermes activates exactly one memory provider, so wiring it moves a
   single global slot. `connect hermes` says what it does. See §20.3.
   rather than a config-file one.
6. **G5 — HTTP MCP transport**, which also unlocks `/mcp/{bank_id}` scoping.

## What is left

G1, G2, G3, G4 (collision), G6.1–G6.4 and G6.6 are **closed**. What remains:

| | gap | why it is still open |
|---|---|---|
| **G4** | bank opt-in | A policy decision, not a bug: should a repo have to opt in to memory? Both parents fail closed; we do not. Not bundled into the collision fix. |
| **G6.5** | ~~port default~~ | Closed — one literal, `8888`. |
| **G7** | `stop` retains a pointer | Highest-churn writer of a useless row. Reads the transcript the way the two new events do; out of scope when they landed. |
| **G8** | pre-compact and session-end store the same prose | Inherited from a shared `flush_at`. Collapsing via `document_id` is one line and would also collapse genuinely different tails. |
| **G5** | 4 MCP tools, stdio only | Not started. A new transport for no measured gain; three of four hosts already get tools over stdio. |

Two things recorded in `docs/CONSISTENCY.md` §20.6 rather than here, because they are
small and already written down: `$HOME/.hermes` is hardcoded so a `HERMES_HOME` profile
override is missed, and `on_memory_write` is refused rather than implemented.

## Explicitly not recommended

- **Matching their tool counts.** 54 tools exists because agentmemory carries
  governance, audit, export and entity tooling. We would be adding surface we
  do not have semantics for. Four tools that are correct beats thirty-nine that
  are approximate.
- **Copying the daemon design.** agentmemory's engine is a TypeScript runtime
  with a WebSocket bus and a separate worker process. We need a pidfile and a
  detach, not a runtime.
- **A container or Helm chart.** Both competitors ship them; both also ship a
  daemonless path. Ours is one binary and a SQLite file — adding an orchestrator
  would cost the property that makes us worth using.
