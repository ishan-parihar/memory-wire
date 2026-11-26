# Plan: emit the SEP-2549 cache hints on `tools/list`

Status: PLAN-ACTIVE · Owner: this file · Not for: retrieval quality, bank semantics, prompt wording.

## Executed — 2026-11-27 (v0.11.3, then v0.11.4)

Shipped and verified on this host. What the record below got wrong, corrected by
probing the *installed* binary through the real client rather than trusting the
unit suite:

- **v0.11.3** set the hints on the four list results this server overrides. It
  was necessary but not sufficient: OpenCode then reported
  `failed to list MCP resource templates — Invalid result for
  resources/templates/list`, because a client asks for that method on every
  server and rmcp answered it from its own `Default`, which carries neither
  hint. **v0.11.4** added an explicit handler. A suite that only covered the
  methods with overrides would have shipped v0.11.3 as "fixed".
- The OpenCode MCP row needed **no** workaround and no shim: it already pointed
  at `/home/ishanp/.local/bin/memory-wire mcp --bank omp`, so the fix was
  picked up by reconnecting the row after the swap.
- The DSH row now spawns the binary directly
  (`@local/dsh-memory-wire/cordis.patch.yml`, `command:
  /home/ishanp/.local/bin/memory-wire`, `args: [mcp]`, no `--bank` — the default
  bank stays git-derived, which `dsh-self-improve` callers rely on). The
  `memory-wire-mcp-shim.mjs` file is still in the bundle, inert and referenced
  by nothing; deleting it is cleanup, not a fix, and it was left rather than
  committed into a workspace tree that already carries unrelated edits.

Remaining after execution: nothing that affects either client. `memory-wire
connect` still refuses `~/.omp/agent/mcp.json`, which holds a hand-written
`http` entry at `http://127.0.0.1:8888/mcp/omp` — that is a working arrangement
of its own, not a gap this plan owns.

---

The plan as written, for the record:

## The defect, in the client's words

OpenCode 2.4.1 refuses the whole tool list:

```
Invalid result for tools/list:
  path ["ttlMs"]        expected number, received undefined
  path ["cacheScope"]   expected one of "public"|"private"
```

`opencode mcp list` reports `✗ memory-wire failed`, and every MCP tool
(`memory_retain`, `memory_recall`, `memory_reflect`, `memory_bank_config_get`,
`memory_supersede`, `memory_redact`) is unavailable in that host. The same client
family already rejected the DSH harness's own connection, which is why
`~/.dsh/profiles/web/node_modules/@local/dsh-memory-wire/memory-wire-mcp-shim.mjs`
exists and injects the two fields by hand.

## Root cause

`rmcp` 3.5.0 generates `ListToolsResult` from its `paginated_result!` macro
(`src/model.rs:1730-1772` in the crate). Both cache fields exist, are typed, and
have builder methods — but they are `Option` and `skip_serializing_if =
"Option::is_none"`:

```rust
pub ttl_ms: Option<u64>,          // -> "ttlMs"
pub cache_scope: Option<CacheScope>, // -> "cacheScope" ("public" | "private")
```

`Server::list_tools` (`src/mcp.rs:787-790` on `origin/main`) builds the result with
`..Default::default()`, which leaves both `None`. rmcp therefore writes neither
field, and a client that validates against the 2026-07-28 schema — where SEP-2549
makes them required — rejects the entire result, not just the missing keys.

There is no transport split to fix: `src/mcp_http.rs` serves `/mcp` through a
`StreamableHttpService` whose factory returns the same `Server`, so one handler fix
covers stdio (`memory-wire mcp`) and HTTP (`memory-wire serve`).

`list_prompts` (`src/mcp.rs:876-884`) has the identical `..Default::default()`
shape and should be fixed in the same change. `list_resources`
(`src/mcp.rs:846+`) needs checking while implementing — it carries extra cursor
logic, so confirm which constructor it uses before touching it.

## Prerequisite

The local checkout at `~/Documents/github/_catalog-measure/memory-wire` is on
`main` at `a16f374` (Cargo.toml `0.10.0`) and is **118 commits behind
`origin/main`, which is the deployed v0.11.2**. The defect must be patched on
`origin/main`; do not write it against this tree.

```bash
git fetch origin && git checkout main && git merge --ff-only origin/main   # -> 4b706dc
cargo test            # baseline green before the change
```

## The fix

One handler, plus the same two fields on the other list handlers.

```rust
// src/mcp.rs — imports (alongside the existing rmcp::model use)
use rmcp::model::CacheScope;

async fn list_tools(
    &self,
    _request: Option<PaginatedRequestParams>,
    _context: RequestContext<RoleServer>,
) -> Result<ListToolsResult, McpError> {
    Ok(ListToolsResult {
        tools: Server::<SqliteStore>::tools(),
        // SEP-2549: a 2026-07-28 client REQUIRES both fields. `ttl_ms: 0` is the
        // spec's "immediately stale" value, so no client cache may serve a stale
        // tool list; `Private` is the conservative scope, so an intermediary is
        // never allowed to share the listing across principals.
        ..ListToolsResult::default()
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private)
    })
}
```

Apply the same two setters to `list_prompts`' `ListPromptsResult` (and to
`list_resources` if its constructor is the same shape).

### Why unconditional, and not gated on the negotiated version

rmcp does not strip these fields on the server path, and the spec's rule for a
field a client does not know is to ignore it, so emitting always is safe for
2025-06-18 and older peers. The alternative — emit only when the peer negotiated
2026-07-28, read from the `RequestContext`'s peer info — is stricter but buys
nothing measurable and adds a branch that has to be tested twice. The DSH shim
proved the unconditional shape works against both client families live; match it.

## Tests

1. **stdio, extends the existing assertion** —
   `tools_list_should_show_six_annotated_tools` (`src/mcp.rs:1180`):

   ```rust
   // the two SEP-2549 hints a 2026-07-28 client requires (this is the
   // OpenCode 2.4.1 / DSH rejection, pinned so it cannot come back)
   assert_eq!(reply["result"]["ttlMs"], json!(0), "{reply}");
   assert_eq!(reply["result"]["cacheScope"], json!("private"), "{reply}");
   ```

2. **HTTP parity** — `a_client_can_reach_the_four_tools_over_http_with_the_bank_in_the_path`
   (`src/mcp_http.rs:260+`). Its parity assertion compares `tools` only; extend it
   to compare the cache hints too, so the second transport cannot drift from the
   first.

3. **The client that actually failed** (host-side, after install):

   ```bash
   opencode mcp list    # must print: ✓ memory-wire  connected
   ```

4. **No regression on the other consumers** — `memory-wire doctor`, the DSH
   harness's memory tools, and a raw `tools/list` over `memory-wire serve`:

   ```bash
   printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"p","version":"0"}}}' \
     '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
     '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
     | memory-wire mcp --bank omp | tail -1 | python3 -c 'import sys,json;r=json.loads(sys.stdin.read())["result"];print(len(r["tools"]), r["ttlMs"], r["cacheScope"])'
   # expected: 6 0 private
   ```

## Docs and housekeeping

- This file, plus one row in `docs/INDEX.md` (status PLAN-ACTIVE: *Plan: emit
  SEP-2549 cache hints so 2026-07-28 clients accept `tools/list`*). Both belong in
  the same commit as the code change — **after** the fast-forward above. This file
  is written on the stale tree on purpose (it is a new path, so a later
  `merge --ff-only` cannot conflict over it); do not commit it against
  `a16f374`, that would fork `main`.
- No README change: the tool table describes tools, not the wire envelope.
- `src/connect_opencode.rs` (the plugin surface) needs no change; the MCP entry
  `connect` writes is unaffected by the hints.

## Release and deploy

- Version bumps: `0.11.2` → `0.11.3` (the four overridden list results), then
  `0.11.4` (the explicit `resources/templates/list` handler the client turned out
  to require). Each was a `fix(mcp):` commit plus an annotated tag, built and
  published with `scripts/build-release.sh --only linux-x86_64 --upload <tag>`
  (a deliberate single-platform subset; `install/get-memory-wire.sh` is what
  needs the rest).
- Install on this host: `memory-wire update`, then
  `systemctl --user restart memory-wire.service` — the HTTP/MCP-http server is a
  systemd unit here, not `memory-wire daemon`, so the swap alone leaves the old
  binary serving on :8888.
- Verify in order: the raw probe above, `opencode mcp list`, the DSH memory
  tools, `~/.local/bin/memory-wire doctor`.

## Retiring the workarounds (only after the release is verified)

Both stopgaps become dead weight and each is a silent failure waiting for the
next client:

1. OpenCode's `mcp.memory-wire` row pointed straight at
   `/home/ishanp/.local/bin/memory-wire mcp --bank omp`, so there was nothing to
   restore. The shim never reached this host.
2. The DSH shim entry is replaced by the direct binary in
   `~/.dsh/profiles/web/node_modules/@local/dsh-memory-wire/cordis.patch.yml`,
   recomposed by touching the profile's own `cordis.patch.yml`. Its
   `memory-wire-mcp-shim.mjs` file stays in the bundle, inert.

## Rollback

The change is additive on the wire: `memory-wire update` keeps the previous
binary as `<exe>.old`, and a client that does not know `ttlMs`/`cacheScope`
ignores them. There is no data or schema migration in this change.