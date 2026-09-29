# Integration plan — from "wired" to "correct"

Written 2026-09-30. Executes [`INTEGRATION_AUDIT.md`](INTEGRATION_AUDIT.md); finding
references (`F1`…`F7`) point into it. Ordered cheapest-first, and within each item the
correctness gain per byte edited.

Two rules govern everything below, both learned the hard way in this repo:

- **A hook that cannot reach the server must not degrade silently.** The never-fail
  contract is correct for a *down* server and catastrophic for a *wrong* one. The two
  need different handling, and conflating them is how F1 became invisible.
- **Never verify an install against a fixture.** 477 tests exercise temp directories.
  Exactly one assertion in this project's history ran a real harness binary, and it is the
  one that proved the `omp` install works. Everything else is our own model of the host.

## Phase 0 — correctness, ~30 lines total

### P0.1 Hooks resolve the endpoint from the daemon state file → fixes F2

**Why first.** Ten lines, and it removes a silent-memory-loss path. Today
`daemon start --addr X` records X, `daemon status` reports X, and the hooks ignore both
and use `$MEMORY_WIRE_URL || 127.0.0.1:8888`. A user who follows the daemon's own advice
gets a server their hooks cannot reach.

**Change.** In `paths.rs`, make `endpoint()` fall back to the daemon state file's `addr`
when the env var is unset. Precedence: `$MEMORY_WIRE_URL` → state file → `DEFAULT_ADDR`.
`daemon status` already resolves this way, so this makes the two agree by construction
rather than by convention.

**Gate.** A test that starts a daemon on a free port, clears the env var, and asserts a
hook's recall reaches *that* daemon. Plus the existing never-fail tests must still pass
unchanged — a hook pointed at nothing must still exit 0.

**Rejected alternative.** Writing the endpoint into each hook shim at install time. It
reintroduces the problem the state file solves: the value goes stale the moment the daemon
moves, and there is no single place to fix it.

### P0.2 One port, decided once → fixes F1

**This needs your decision, and I am not going to pick it silently.**

`DEFAULT_ADDR` is `127.0.0.1:8888`. On this machine that is the Hindsight HTTP API, so:
`daemon start` refuses, and any hook without `MEMORY_WIRE_URL` talks to Hindsight and
returns nothing without erroring.

Three options, with what each costs:

| option | cost |
|---|---|
| **(a) keep 8888, document it, validate at install** | Correct everywhere except where something else holds 8888. Install must then *refuse* or *adapt*, not silently produce a dead integration. |
| **(b) move the default to a free port (e.g. 8899)** | A one-constant change, a breaking change for anyone already using 8888, and it only moves the problem to the next popular port. |
| **(c) keep 8888, make `connect` detect the collision and write a working endpoint** | No breaking change. Costs an install-time probe, and the probe is the thing that must not be wrong. |

I recommend **(a) with the install-time probe**, i.e. option (c) as a safety net on top of
(a) — because the probe is strictly better than either alternative at the moment it runs,
and it is the same check `daemon start` already performs successfully. The one thing it
cannot do is *choose* for the user; it reports and refuses.

**Gate.** A test that occupies 8888 with a foreign listener, runs the install, and asserts
it refuses with the occupant's identity rather than writing a config that points nowhere.
`tests/daemon.rs` already has the machinery for a port fixture.

**Standing constraint.** This box was at loadavg 51–213 for most of the session. No phase
below may report a latency or RSS figure measured here.

## Phase 1 — capability, the "no harness gets both" gap

### P1.1 Codex gets HTTP MCP → fixes F3

Codex is the clearest defect in the matrix: context is injected, and the agent has no way
to call a tool. It configures MCP by `url` (`[mcp_servers.x] url = "http://…"`), and we
now serve exactly that, statelessly, at `/mcp`.

**Depends on P0.2.** An HTTP entry hardcodes a port; if the port is unresolved, we write a
known-wrong URL. This is the reason the item sat in "not recommended" before HTTP MCP
existed, and the reason it cannot be built before the port is decided.

**Change.** `connect codex` additionally performs a surgical insert into
`~/.codex/config.toml` of a `[mcp_servers.memory-wire]` table with
`url = "<resolved endpoint>/mcp"`. `codex` becomes `Style::Both`.

**Constraints, all from precedent:**
- **No TOML crate.** The precedent set by `connect_plugin.rs` for YAML is *refuse rather
  than corrupt*: parse enough to find the table, refuse a shape we do not understand
  untouched, and never reserialise a file we did not author. Codex's config is
  hand-maintained TOML; rewriting it wholesale would destroy comments and ordering.
- **Hooks must be preserved.** We already own `~/.codex/hooks.json`; this is a second
  file, not a change of style.
- **Uninstall removes exactly the table we added** and nothing else in the file.

**Gate.** Install → assert the table exists with the resolved URL; uninstall → assert the
file is byte-identical to the pre-install snapshot; malformed TOML → refused untouched
with no backup dir. Same three assertions as every other host, because that is the
project's established bar.

### P1.2 The same treatment for claude-code and copilot-cli → closes the matrix

Both are hook-only today and both support MCP. Same `url`-based entry, same surgical
editor, same gates. Lower value than Codex (those harnesses have a richer hook
vocabulary) but it removes the structural split rather than leaving three exceptions.

**Deliberately not:** cursor and opencode are MCP-only and their hook vocabularies are
unknown to us. Writing a hook entry into a file whose schema we have not read is how we
would create a *silent* integration instead of a missing one. Leave them, and record why.

### P1.3 Namespace collisions are impossible by construction

OMP prefixes our tools `memory_wire_*` **[V]**. So does anything else that namespaces by
server. Three consequences, all cheap:

1. Document the prefixed name wherever a tool name appears — `SKILL.md`, `README.md`,
   `INSTALL_FOR_AGENTS.md`, the historian prompt. Today no document mentions it.
2. The install-time probe from P0.2 also enumerates the servers already registered in the
   target file and refuses, or renames, on collision.
3. The `memory_wire_` prefix appears in the *preamble* the hook emits, so an agent reading
   its own instructions sees the name it must actually call.

**Gate.** A test that writes a fixture with a colliding server name and asserts install
refuses rather than producing two servers an agent cannot tell apart.

## Phase 2 — the meta-property

### P2.1 Assert against real harnesses, not fixtures

Every finding in the audit except F1 and F5 came from running something. None came from a
test. The plan is to make that the default for the integration surface specifically.

**Two tiers, because a test that needs `omp` on `$PATH` cannot run in CI:**

- **Tier 1 — always on, no harness required.** A test that, for each of the 7 hosts,
  writes into a temp `HOME`, invokes `connect`, and asserts the *written file* matches a
  checked-in golden fixture. This catches shape drift; it does not catch "we wrote a key
  the host ignores" — which is precisely how the `enabledServers` question was only
  settled by running OMP.
- **Tier 2 — gated on the binary being present.** The `connect --list`-style check against
  each installed harness, skipped with a printed reason when absent. The OMP tool-listing
  probe becomes a script, not a one-off command I ran by hand.

**Gate.** Tier 2 must fail, not skip, when a harness *is* present and does not accept the
config. A skip that hides a broken integration is worse than no test.

### P2.2 One command that answers "is this install actually working?"

Today answering that takes 6 commands and a model call. It should be one, and it should
distinguish the four states that matter: not installed, installed but the harness ignores
it, installed and the server is unreachable, installed and working.

**Change.** `memory-wire doctor --all-hosts`, which for each wired host reports: entry
present, entry well-formed, the command it points at exists and is the installed version,
and — for MCP hosts — a real handshake against it. `--deep` additionally proves recall
end-to-end per bank.

This is the single highest-leverage addition for support, and it is what would have made
F1 and F2 obvious to a user without a maintainer.

**Gate.** A fixture host with a dangling command path, and one with a valid path and a dead
server, must produce the two different diagnoses.

## Phase 3 — honesty about the build

### P3.1 Make the build reproducible, or stop claiming a size

F5: the published 9,023,808 B artifact is not reproducible from the tree by the documented
command, which yields 10,604,776 B, and only `src/connect.rs` differs since the tag.

**Step 1 — diagnose, do not guess.** Three builds produced three sizes. The prime
suspects are the cached-artifact reuse (`Finished in 0.34s`) and a build.rs path that
differs between the host-triple and zigbuild routes. Concretely: build the `v0.5.0` tag in
a clean worktree with the documented command and compare to the published asset byte for
byte. If they differ, the README's size claim is currently unfounded and gets corrected
like every other stale figure in `CONSISTENCY.md`.

**Step 2 — single build path.** One documented command, used by the release script, by the
docs, and by anyone reproducing a figure. If `cargo build --target <host-triple>` and
`cargo zigbuild` genuinely differ, the release script's choice becomes the only one we
document.

**Step 3 — assert it.** A test that builds nothing but *records* the binary size and
`DT_NEEDED` into an artifact, so a size change is a visible diff rather than a silent
drift. `DT_NEEDED` growing is the one that breaks installs on slim targets, and it is
worth failing a build over.

## Phase 4 — research before touching

### P4.1 Explain the hermes slot before displacing it

F6: `config.yaml` says `provider: agentgateway`, the plugin directory is named
`agentmemory`, and a provider directory's name must equal `memory.provider`. Until that
is explained, `connect hermes` risks displacing a provider whose identity we cannot state.
This is read-only research; **no config change until it is understood.**

### P4.2 Re-examine `INTEGRATION_GAPS.md` against today's tree

That register was written at `eec026f` and predates the daemon, hermes support, HTTP MCP,
the AXI output work, and the `omp` host. Several "not recommended" verdicts were correct
when written and are now stale — F3 is the clearest. It should be re-audited rather than
incrementally patched, and its summary line should stop asserting closure it has not got.

## Ordering, and what is deliberately absent

```
P0.1  hooks read the state file        ~10 lines   removes silent memory loss
P0.2  one port (DECISION NEEDED)       ~40 lines   removes the misroute class
P1.1  codex + HTTP MCP                 ~120 lines  removes the capability gap
P1.2  claude-code + copilot MCP        ~120 lines  closes the matrix
P1.3  namespace safety                 ~40 lines   removes an agent-visible ambiguity
P2.1  real-harness assertions          ~200 lines  stops this audit recurring
P2.2  doctor --all-hosts               ~150 lines  makes the state legible
P3.1  reproducible build               ~1 day      makes the size claim true
P4.1  hermes slot research             read-only   prevents a bad displacement
P4.2  re-audit INTEGRATION_GAPS.md     read-only   retires stale verdicts
```

**Not doing, and why:**

- **A native OMP memory backend.** Closed to third parties — verified: the backends are
  bundled modules, there is no `registerMemoryBackend`, and an extension is
  `export default function (pi) {…}`. MCP works and is verified working.
- **Writing OMP's `memory.backend`.** It reads `hindsight`. Displacing a competitor
  unasked is exactly why hermes is excluded from the implicit set; the same rule applies
  here, and the user should make that call explicitly.
- **Matching agentmemory's 54 tools.** 54 exists because it carries governance, audit,
  export and entity tooling. That is surface without semantics.
- **Delegating the hermes plugin over MCP.** Rejected, not deferred: it needs an MCP
  *client* in Python — handshake, SSE, JSON-RPC framing — to remove a little duplication
  from an API that already works.
- **A supervisor for the daemon.** Our `daemon` is a pidfile plus a `/proc` start-time
  identity check. agentmemory's is a detached Node engine with pidfiles and state. Ours is
  deliberately lighter and that is defensible; a supervisor is a separate proposal and
  should not be smuggled in here.

## Two decisions I need from you

1. **The canonical port** (P0.2). My recommendation is option (a)+(c): keep 8888, and have
   `connect` probe it and refuse rather than write a dead integration. This is the only
   thing blocking P1.1, which is the highest-value item in Phase 1.
2. **Whether hermes should be wired at all**, given P4.1 shows the slot's identity is not
   what we assumed. It is excluded from the implicit set today, so nothing changes either
   way — but "excluded because it displaces a competitor" and "excluded because we don't
   understand the slot" are different reasons, and the second is the honest one.
