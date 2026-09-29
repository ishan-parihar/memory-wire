# memory-wire for Hermes Agent

A Hermes **memory provider plugin**: a directory under `~/.hermes/plugins/` that
`memory-wire connect hermes` copies into place. It talks to a running
`memory-wire` server over HTTP.

```bash
memory-wire connect hermes              # copy the plugin, set memory.provider
memory-wire daemon start                # the server it talks to
memory-wire connect hermes --uninstall
```

Hermes loads user-installed memory providers from `$HERMES_HOME/plugins/<name>/`
and activates exactly one, named by `memory.provider` in `~/.hermes/config.yaml`.
The directory name is the provider name, so the installed directory is
`~/.hermes/plugins/memory-wire/` and the config key is `memory: provider: memory-wire`.

## What it talks to

Four HTTP routes, the same ones `memory-wire mcp` and `memory-wire serve` already
expose. No SDK, no sidecar process, no language runtime on the client side.

| Route | Used by |
|---|---|
| `POST /banks/{bank}/retain` | `sync_turn`, `on_session_end`, `on_pre_compress`, `memory_retain` |
| `POST /banks/{bank}/recall` | `prefetch`, `memory_recall` |
| `POST /banks/{bank}/reflect` | `memory_reflect` |
| `GET /banks/{bank}/config` | `memory_bank_config_get` |

## Configuration

Resolution order, highest first:

1. `MEMORY_WIRE_URL` / `MEMORY_WIRE_BANK` — the same variables `memory-wire hook`
   and every other client reads, so one setting moves all of them together.
2. `$HERMES_HOME/memory-wire.json` — `{"url": ..., "bank": ...}`, written by
   `hermes memory setup` through this plugin's `save_config`.
3. `http://127.0.0.1:8888`, bank `memory-wire` — the same literals as
   `paths::DEFAULT_ENDPOINT` and `paths::DEFAULT_BANK`.

There is no auth. The server is unauthenticated by design and defaults to
loopback; if you bind it to anything else, anything that can reach the port can
read and delete every bank.

`serve`, `daemon start` and every client default to port **8888**, from one
constant (`paths::DEFAULT_ADDR`). An earlier build defaulted the two servers to
8899 while clients looked at 8888, so a server started with no arguments was
unreachable; that is fixed. If you are running against a non-default port, set
`MEMORY_WIRE_URL`.

## Bank naming

This plugin resolves the bank from the environment or the config file. It does
**not** reimplement the remote-derived bank naming that the CLI and hooks use
(`owner/repo` from `origin`, falling back to the work-tree name) — that rule lives
in Rust, in `src/paths.rs`, and a second copy in Python could drift from it
silently. To share a bank with a Claude Code or cursor session, set
`MEMORY_WIRE_BANK` to the same id those resolve to, or pass `bank` per tool call.

## Lifecycle events

Five of the competitor's six, mapped to what this build actually has.

| Event | What it does |
|---|---|
| `system_prompt_block` | The historian preamble plus the bound bank. Static, no network — it is called inline while the system prompt is assembled. |
| `prefetch` | `recall` on the turn's query, rendered as a bulleted block, capped at 1200 tokens and 8 lines (the same `BUDGET`/`MAX_LINES` the CLI hooks use). |
| `sync_turn` | Retains the turn: the user's message in full, the answer truncated, headed by session and identity, capped at 2000 characters. |
| `on_session_end` | Retains the conversation's own prose — not a pointer to a file. |
| `on_pre_compress` | Retains the prose, and returns a short marker for the compression summary. |

`on_memory_write` is not implemented. See below.

### Why this is better than a transcript file

`on_pre_compress` and `on_session_end` are handed the message list **in-process**,
so the conversation is already in memory and the words never have to survive a
round trip through a file on disk. The file-based hooks (`memory-wire hook
pre-compact`) can only retain a pointer to a transcript, because that is all a
host payload gives them. Here the 128 KiB tail window has no equivalent, because
there is no file to window.

### The sixth event

`on_memory_write` fires when Hermes' **built-in** memory tool writes `MEMORY.md`
or `USER.md`. Mirroring it is not something this build can do honestly:

- `action='remove'` has no counterpart. memory-wire deletes by memory id, and a
  retain of identical content is a content-hash no-op rather than a handle we were
  handed — so a remove either cannot be expressed or would delete the wrong row.
- `target` is `'memory'` or `'user'` — two files in Hermes' store. memory-wire has
  banks, not two files, and inventing a `MEMORY.md`→bank mapping is a namespace
  decision this crate has not made.
- Retaining on `add` alone would be half the contract under a name that promises
  all of it.

So the hook does not write, and prints **one line to stderr the first time it
fires** so the gap is visible in a log rather than inferred from a silence. It is
not listed in `plugin.yaml`'s `hooks:`, because that list is documentation of what
is implemented and it is not implemented.

## Tools

`get_tool_schemas` exposes the same four tools `memory-wire mcp` serves over stdio
— `memory_retain`, `memory_recall`, `memory_reflect`, `memory_bank_config_get` —
with names, descriptions, parameter objects and annotations copied from
`Server::tools()` in `src/mcp.rs`. A Hermes agent and an MCP client see one
surface. Hermes wraps each in `{"type":"function","function":…}`, so the
`annotations` block rides along and is honest about what each tool does.

## Failure behaviour

Every call returns `""` or `None` on any failure. A down server costs one 2-second
timeout (the same value as `paths::IO_TIMEOUT`) and nothing else: no exception,
no message to the model, no log line per turn. That is the contract the CLI hooks
have always had, and it has a known consequence — a stopped server is
indistinguishable from a working one that had nothing to say. `memory-wire daemon
status` is what tells the two apart.

Writes are skipped for non-primary agent contexts (`agent_context` of `subagent`,
`cron`, or `flush`), because a cron employee replaying a system prompt would
otherwise write that system prompt into a user representation.

## Uninstall

`memory-wire connect hermes --uninstall` removes the plugin directory and
restores whatever `memory.provider` was before — including removing the key
entirely if there was none. If `memory.provider` names some third provider that
this install never wrote, it is left alone and reported.

`hermes` is the one host that a bare `memory-wire connect` does **not** wire.
Every other host is additive — a hook entry or an MCP key added to a JSON file,
leaving anything already there working. Hermes activates exactly one memory
provider, so wiring it flips a single global slot away from whatever held it,
and a bare `connect` should not do that unasked. Name it explicitly.
