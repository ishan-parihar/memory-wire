# Integration audit — our footprint in five agent harnesses

Written 2026-09-30. Companion to [`INTEGRATION_GAPS.md`](INTEGRATION_GAPS.md), which is
the **competitor-side** register (G1–G7, what we lack against agentmemory and Hindsight).
This document is the **our-side** audit: what we actually install, what each harness
therefore gains, and where that is quietly broken.

Every claim carries a `file:line` from this tree or a command run on this machine. Where
I could not verify something, it says so rather than inferring it.

## 0. Method, and what "verified" means here

Three levels are used and are not interchangeable:

- **[V]** verified by a command on this machine, output quoted.
- **[C]** verified by reading this tree at the stated `file:line`.
- **[U]** unverified — the schema or docs imply it, but I could not observe it.

I installed and ran things rather than reasoning from documentation, which is how two of
the findings below were caught: one of them contradicts a claim I had previously written
down, and one of them is a bug I had introduced and shipped.

## 1. What we install, per harness

Seven hosts. The split is deliberate: hosts that support lifecycle hooks get hooks,
hosts that speak MCP get an MCP entry, and `hermes` is excluded from the implicit set.

| harness | mechanism | config file | what it gains | live state |
|---|---|---|---|---|
| claude-code | hooks ×5 | `~/.claude/settings.json` | context injected at 5 lifecycle points | wired |
| codex | hooks ×5 | `~/.codex/hooks.json` | context injected — **but no tools** | wired |
| copilot-cli | hooks ×5 | `~/.copilot/settings.json` | context injected | wired |
| cursor | MCP | `~/.cursor/mcp.json` | 4 tools | wired |
| opencode | MCP | `~/.config/opencode/opencode.json` | 4 tools | wired |
| **omp** | MCP | `~/.omp/agent/mcp.json` | 4 tools, namespaced `memory_wire_*` | **wired 2026-09-30, [V] launching** |
| hermes | plugin | `~/.hermes/config.yaml` + `plugins/` | 5 of 6 provider events | not wired; slot held by `agentgateway` |

`Style::Mcp` vs `Style::Claude` is decided per host at `src/connect.rs:237` **[C]**.

### 1.1 omp, verified end to end **[V]**

```
$ memory-wire connect omp
omp          wired         mcpServers  (backup: …/backups/omp-20260930-030236)
```

The entry written, and everything it must not touch:

```
$schema unchanged:        True
enabledServers unchanged: True  ['sourcehound', 'cloakctl']
foreign servers unchanged: True   (browseros-neo, sourcehound, cloakctl, supabase)
mcpServers now: ['browseros-neo','cloakctl','memory-wire','sourcehound','supabase']

{"type":"stdio","command":"/home/ishanp/.local/bin/memory-wire","args":["mcp"]}
```

A second run reports `omp already-wired` **[V]**.

I did not stop at "the config looks right". OMP was run with a trivial prompt and asked to
enumerate its tools **[V]**:

```
memory_wire_memory_bank_config_get
memory_wire_memory_recall
memory_wire_memory_reflect
memory_wire_memory_retain
```

So the whole chain is proven: OMP reads `mcp.json`, spawns the absolute path, and receives
4 tools. **Note the `memory_wire_` prefix** — OMP namespaces MCP tools by server name, so
an agent must ask for `memory_wire_memory_recall`, not `memory_recall`. That is a real
usability wrinkle and no document mentions it.

Two incidental findings from that run **[V]**: OMP's *only* working MCP servers are ours
(`browseros-neo` refused to connect, `supabase` returned `401 JWT could not be decoded`),
and the failures are reported as warnings while the session continues.

### 1.2 Why `enabledServers` was left alone

OMP's `mcp.json` carries a third key, `enabledServers: ["sourcehound","cloakctl"]`, which
reads like a launch allowlist — and if it were one, writing only `mcpServers` would have
produced a server that is defined and never started.

I checked the schema OMP itself points at
(`https://raw.githubusercontent.com/can1357/oh-my-pi/main/.../mcp-schema.json`) **[V]**:

> `disabledServers`: User-level **denylist** … highest precedence.
> `enabledServers`: User-level allowlist that **overrides a discovered server's
> `enabled: false` flag** … The denylist still wins.

So it is an override for servers something *else* marked disabled, not a launch
allowlist; the real denylist is the separate `disabledServers`. An entry with no
`enabled: false` is on by default, and the tool listing above is the empirical proof. My
original instruction to leave the key byte-identical was right.

## 2. Findings, ranked

### F1 — The hook endpoint is a silent-misroute hazard **[V]**

`paths::endpoint()` (`src/paths.rs:46-52`) resolves to `$MEMORY_WIRE_URL` or
`http://127.0.0.1:8888` **[C]**, and `DEFAULT_ADDR` is `127.0.0.1:8888` (`src/paths.rs:19`).
The hooks call that endpoint over HTTP (`src/hooks.rs:92`).

On this machine **8888 is not memory-wire**:

```
$ curl -s http://127.0.0.1:8888/openapi.json | jq .info
{ "title": "Hindsight HTTP API", "version": "0.10.2" }     # uvicorn
GET /banks/x/stats -> 404
```

A hook invoked with no `MEMORY_WIRE_URL` therefore POSTs to Hindsight, which 404s
`/banks/{bank}/recall`, and the never-fail contract (`src/hooks.rs:3-7`) swallows it. The
hook returns **the preamble and no memories, and no error** **[V]**.

This is the worst failure shape available: not an error, a degraded result. It is also
machine-specific today and a *class* of bug tomorrow — any user who already runs something
on 8888 gets it.

`memory-wire daemon` handles the same collision correctly and is worth reading as the
model for the fix **[V]**:

```
memory-wire daemon: cannot bind 127.0.0.1:8888: Address already in use (os error 98)
  and it is not a memory-wire server: unexpected health response
```

It distinguishes "busy, and it is us" from "busy, and it is a stranger", and refuses
rather than clobbering.

### F2 — The daemon and the hooks do not agree on where the server is **[V]**

| | knows the endpoint? |
|---|---|
| `daemon start --addr 127.0.0.1:8899` | yes — prints `http://127.0.0.1:8899` |
| `daemon status` | yes — reads it from the state file |
| **the hooks** | **no** — `MEMORY_WIRE_URL` or `DEFAULT_ADDR`, never the state file |

Demonstrated: daemon healthy on 8899, `hook session-start` with no env var → preamble
only, no recall **[V]**. So a user who follows the daemon's own `--addr` advice ends up
with a server their hooks cannot reach. Three components hold the same piece of
information in two representations, and the two disagree.

### F3 — Codex is the only harness that gets context but no capability **[V]**

`~/.codex/config.toml` configures MCP by **url**, not by command **[V]**:

```toml
[mcp_servers.browseros-neo]
url = "http://127.0.0.1:9010/mcp"
```

`connect` writes only `~/.codex/hooks.json` **[V]**, and `Host::Codex` is
`Style::Claude` (`src/connect.rs:237`) **[C]**. So a Codex user has memory injected into
their context and **no way to call a single memory tool**.

This is now closeable. We have stateless HTTP MCP mounted at `/mcp` — exactly the `url`
shape Codex wants. It was recorded as "not recommended" in `INTEGRATION_GAPS.md` before
HTTP MCP existed; that judgement is stale and should be revisited.

The obstacle is real though, and it is F4.

### F4 — HTTP MCP needs a stable URL, and our port is a flag, not a constant **[V]**

An MCP-over-HTTP entry hardcodes a port. Ours is `daemon start --addr <addr>` with a
default that is already taken on this machine (F1). So an installer that writes
`url = "http://127.0.0.1:8888/mcp"` writes a **known-wrong URL here**, and there is no
single place to fix it.

Worse, `connect` writes `std::env::current_exe()` into stdio entries **[V]**, so testing
an install with a debug build wires the debug path. Harmless in production, but it means
"what does the config point at" and "which binary is installed" can silently disagree.

### F5 — The published artifact is not reproducible from the tree **[V]**

| build | size | `DT_NEEDED` |
|---|---|---|
| published `v0.5.0` asset | 9,023,808 B | 3 (`libgcc_s`, `libm`, `libc`) |
| `cargo build --release --locked --target x86_64-unknown-linux-gnu` (the documented command) | 10,604,776 B | 3 |
| `cargo zigbuild --release --target …` | 8,609,792 B | **4** (`libpthread`, `libdl`) |

The only source change since the tag is `src/connect.rs` **[V]** — one enum variant and
three tests, which cannot account for 1.5 MB. The release run reported
`Finished in 0.34s` **[V]**, i.e. it reused a cached artifact built under conditions I
could not reconstruct.

Consequence: **the README's "8.6 MiB binary" is a true statement about a specific
downloaded artifact and a false statement about what our documented build command
produces.** Given this project's documented history of a wrong size in the headline, that
distinction has to be stated rather than averaged away.

### F6 — hermes' slot is held under a different name than its directory **[V]**

```
~/.hermes/plugins/agentmemory/     exists
config.yaml:  memory.provider: agentgateway
```

A memory-provider directory's name must equal `memory.provider`. These do not. Either a
second provider resolves `agentgateway`, or the slot is held by something not visible in
`plugins/`. Until that is explained, `connect hermes` risks displacing a provider whose
identity we cannot state — which is the exact condition that made us exclude hermes from
the implicit set in the first place.

### F7 — hermes' plugin re-implements the four tools in Python **[C]**

`connect hermes` writes a `MemoryProvider` that speaks our REST API and reimplements
retain/recall/reflect/config in Python, rather than delegating over MCP. It also refuses
`on_memory_write` rather than faking it.

Delegating over MCP would be a **regression in complexity** — it would require writing an
MCP client (session handshake, SSE parsing, JSON-RPC framing) in Python to remove a small
amount of duplication against an API that already works. This was mislabelled a
simplification and is recorded here as a **rejected** item, not a backlog item.

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

## 5. Where we are better than the competitors

Verified in our tree, absent from both audited codebases — not padding:

- **MCP annotations on all 4 tools.** agentmemory has **zero** across 54, so no MCP client
  can distinguish a read from a write in their system.
- **A malformed host config is refused, not overwritten** — with a test asserting the file
  stays byte-identical *and* that no backup directory is created.
- **Stale-path self-heal**: a moved binary's entry is replaced rather than duplicated.
- **The refuse-rather-than-clobber port guard** (F1's quote above) — the check that the
  occupant is *us* before proceeding.
- **A read-only discovery command**, `connect --list`, whose defining property is that it
  writes nothing; verified live against a fresh `HOME`.
- **Pinned-bank enforcement over HTTP MCP**: a call naming another bank is refused rather
  than silently preferring the URL, so a caller's `bank` argument can never become a no-op
  that looks like it worked.
- **A single-binary, 3-shared-library, offline install** — the reason the daemon/hook work
  is tractable at all. agentmemory ships behind a detached Node engine; Hindsight needs
  Postgres + pgvector.

## 6. Where they are better

- **Breadth.** **20 distinct harnesses across 28 connect modules**
  (`src/cli/connect/*.ts`, collapsing `-cli`/`-hooks` variants and excluding `index`,
  `types`, `util`, `guidelines`, `json-mcp-adapter`) against our 7. And **54 MCP tools**
  (`src/mcp/tools-registry.ts`) against our 4. Partly deliberate — a tool count we cannot
  justify with semantics is surface, not capability — and partly not.

  Correction: an earlier draft of this document said "21 adapters". That number was
  carried over from `INTEGRATION_GAPS.md` and is wrong; 20 is the verified count. The
  54-tool figure is confirmed.
- **A real plugin protocol on hermes**, which we cannot reach except by writing a
  `MemoryProvider` in Python.
- **Per-harness lifecycle coverage.** We write 5 hook events; agentmemory ships 12 for
  Claude Code, including the two we lack.
- **Mature daemon supervision** — pidfiles plus `engine-state.json`, with the engine
  detached and `unref()`ed. Our `daemon` is a pidfile, a start-time identity check, and no
  supervisor; deliberate, and the weakest part of the design.
- **An unbundled, 21-harness test matrix.** We test against fixtures.

## 7. What is explicitly not on the table

- **Matching their tool count.** 54 exists because agentmemory carries governance, audit,
  export and entity tooling. We would be adding surface without semantics.
- **Copying their daemon design.** Theirs is a TypeScript runtime with a WebSocket bus and
  a separate worker; we need a pidfile and a detach. Theirs is heavier because it does
  more.
- **A native OMP memory backend.** Verified closed to third parties: the backends
  (`hindsight`, `Mnemopi`, `Sharpshooter`, `local-backend`, `messages`, `off-backend`) are
  bundled modules inside `@oh-my-pi/pi-coding-agent`, there is no `registerMemoryBackend`
  symbol, and an OMP extension is `export default function (pi) {…}`. MCP is the only
  route, and it works.
- **Writing `memory.backend` in OMP's `config.yml`.** It currently reads `hindsight`.
  Displacing a competitor unasked is the hazard that keeps hermes out of the implicit set.

## 8. Two open questions about OMP, one of them about a competitor

### 8.1 agentmemory integrates with pi, and I cannot yet say whether it works **[U]**

They ship `src/cli/connect/pi.ts` and an `integrations/pi/` package. The installer targets
**`~/.pi/agent/extensions/agentmemory/`** (`pi.ts:18-19`), using native lifecycle hooks
against their REST API — recall on agent start, capture on agent end — with the comment
*"pi auto-discovers `~/.pi/agent/extensions/*/index.ts`"*.

We target `~/.omp/`, which is right for OMP 18.3.0. The interesting part is whether theirs
still lands. OMP's binary contains `const w = T.omp || T.pi || { version: T.version }` —
an explicit `.omp`-prefers-`.pi` fallback — so `.pi` is **legacy but referenced**, not dead.
On this machine `~/.omp` was last modified 2026-09-30 and carries `natives/18.3.1`, while
`~/.pi` was last modified 2026-08-24.

So `.pi` is the pre-migration location and `.omp` the current one. **Whether OMP still
loads extensions from `~/.pi/agent/extensions/` when `~/.omp/agent/extensions/` also
exists is untested**, and the binary string is about a config object rather than the
extensions directory, so it does not answer the question. I nearly published "their pi
integration is broken" and the evidence does not support it.

The test is cheap and worth doing: drop a marker extension into `~/.pi/agent/extensions/`,
ask omp to enumerate its extensions, and see whether it loads. If it does not, that is a
defect worth reporting to them and a reason to be sure our own path stays correct across
OMP renames.

### 8.2 Whether an OMP extension is worth building

`~/.omp/agent/extensions/` already holds a third-party TypeScript extension
(`herdr-omp-agent-state.ts`, 12,746 B) and OMP loads it — the `--extension` /
`--plugin-dir` flags and auto-discovery are real. That makes the P1.3 "OMP loses automatic
injection" concern actionable rather than theoretical: an extension is a supported surface.

It is not free. Our extension would have to either call the REST API (like the hermes
plugin) or embed an MCP client, and it would need to hook the same lifecycle moments. That
is the same trade already recorded as **rejected** for hermes in §2/F7 — an MCP client in
TypeScript is less bad than one in Python, but it is still a client.
