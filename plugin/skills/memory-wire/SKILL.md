---
name: memory-wire
description: Persist and retrieve agent memory across sessions with memory-wire — one 8.5 MiB binary, three shared libraries — exposing an HTTP retain/recall/reflect API over bank-isolated SQLite + FTS5 with PII redaction on write. Use when you need durable memory across sessions, want to record a decision or fact you will need later, or need to recall what was previously decided. Invoke with curl against a running `memory-wire serve`, or over MCP stdio with `memory-wire mcp`.
---

# memory-wire

Agent memory over HTTP or MCP stdio. One 8.4 MiB binary, three shared libraries, no language
runtime, no daemon. **retain** stores a
fact/decision (redacted before it touches disk), **recall** is ranked bank-isolated retrieval under a
token budget, **reflect** is top-hit citation prefixed with the memory id (not LLM synthesis). Banks are
isolated namespaces — `demo` and `other` cannot see each other — so use one bank per project or client
and stick to the name.

**Which bank the hooks pick, and how to change it.** A hook run with no arguments resolves its bank in
this order, first hit wins: `--bank <id>`, then `$MEMORY_WIRE_BANK`, then `owner/repo` from the
repository's `origin` remote (read from `.git/config`, no `git` process spawned — a checkout of
`https://github.com/acme/api.git` uses bank `acme-api`, whether or not the directory is called `api`),
then the git worktree's top-level directory name, then the literal `memory-wire`. A repo with no remote,
or a remote naming no `owner/repo`, falls back to the directory name.

**If you are answering "where did my memories go":** a remote-derived bank id differs from the old
directory-name one, so memories written before the switch are still under the old name and nothing was
moved. `memory-wire doctor` prints a `warning` line naming both when it sees this. To read the old
ones, either pass `--bank <old-name>` to the hook or set `MEMORY_WIRE_BANK=<old-name>`, or just address
that bank directly over HTTP (`GET /banks/<old-name>/memories`). No automatic migration exists and
none is planned — a wrong guess moves memories between namespaces silently. See `docs/BANK_IDENTITY.md`.

## Quick start

```bash
MW=http://127.0.0.1:8888
DB="$HOME/.local/share/memory-wire/agents.db"    # or pass --db /tmp/scratch.db
# nohup memory-wire serve --addr 127.0.0.1:8888 --db "$DB" >/tmp/memory-wire.log 2>&1 &
curl -sS -X POST "$MW/banks/demo/retain" -H 'Content-Type: application/json' -d '{"content":"auth uses jose middleware; key sk-abcDEF123456"}'   # -> {"id":"<uuid>"}
curl -sS -X POST "$MW/banks/demo/recall" -H 'Content-Type: application/json' -d '{"query":"how does auth work","budget":2000}'   # -> ["auth uses jose … [REDACTED:api_key]"]
```

Eight routes: `GET /health` (body exactly `ok`, plain text), `POST /banks/:id/{retain,recall,reflect}`,
`GET`/`PUT /banks/:id/config`, and the lifecycle half — `GET /banks/:id/memories?limit=&offset=`,
`GET`/`DELETE /banks/:id/memories/:mid`, `GET /banks/:id/stats`. Lifecycle responses serve
`{id, content, created_at}` (plus `context` when the memory has one), so every served memory carries an
RFC 3339 UTC timestamp. Omit `--db` for the `$XDG_DATA_HOME/memory-wire/memory.db` default; if 8888 is
taken, serve on a free `--addr 127.0.0.1:<port>` and point `$MW` at it.

## MCP

`memory-wire mcp` serves those same four operations as MCP tools over stdio (JSON-RPC 2.0,
newline-delimited; stdout is protocol-only, logs to stderr), run as `memory-wire mcp --bank
my-project` (`$MEMORY_WIRE_BANK`, else `memory-wire`; a per-call `bank` overrides both):

| Tool | Writes? | Arguments |
|---|---|---|
| `memory_retain` | yes | `content` (required), `bank?`, `context?`, `tags?` |
| `memory_recall` | no | `query` (required), `bank?`, `budget?`, `tags?` |
| `memory_reflect` | no | `query` (required), `bank?` |
| `memory_bank_config_get` | no | `bank?` |

Every tool declares `destructiveHint: false` and `idempotentHint: false`; `memory_retain` is the only
one without `readOnlyHint: true`. **Use curl** when the server is already running, you are scripting
a check, or you want the raw status/body and the `400`/`404`/`500` contract below; **use MCP** when
your host speaks MCP. Both route through the same service methods, so answers are identical and retrieval,
redaction, and budgeting are never reimplemented for one surface. The lifecycle routes, `format`, and
`document_id` are HTTP-only so far — no MCP tool exposes them yet.

## Config and tags

`GET`/`PUT /banks/:id/config` is a whole-object store-and-serve: unknown keys (`retain_mission`, anything a
future version adds) come back byte-for-byte, and a malformed value in a key this build *acts on* falls
back to its default instead of taking recall down.

```bash
curl -sS -X PUT "$MW/banks/demo/config" -H 'Content-Type: application/json' -d '{"recallMaxTokens":128,"retainTags":["ops"],"retain_mission":"own the release"}'
```

Two keys change behavior. `recallMaxTokens` is the bank's default recall budget — a recall that omits
`budget` uses it, an explicit `budget` still wins. `retainTags` is added to every later retain in that
bank, after the request's own tags, so a bank-wide default can never displace a tag you named. Tags also
work with no config at all: `retain` accepts `{"content","tags"}` and `recall` accepts `{"query","tags"}`
to restrict the search to memories carrying **any** of them (capped at 20, head kept).
`memory-wire connect` installs five lifecycle hooks (`SessionStart`, `UserPromptSubmit`, `Stop`, `PreCompact`,
`SessionEnd`) into
every host it detects — claude-code, codex, copilot-cli, cursor, opencode. Idempotent, and it never
rewrites a hook it did not write; `--uninstall` prunes only its own entries and is refused alongside
`--guidelines`. `memory-wire doctor` is a read-only health screen (`--strict` exits nonzero if the server
or store is unusable).

## Workflow

**Retain when** a decision is made and its *why* will matter later: an approach chosen and rejected, a
constraint discovered, a config value, a user preference. One fact per call — a paragraph of unrelated
claims retrieves as a paragraph and crowds out the useful memory under a small budget. Use `context` for
the surrounding condition (hook payload, the error you hit); it is redacted too.
**Recall when** about to act in a project you have touched before, before answering a question that may
already be recorded, or at the start of a session on known ground: query the *concept* ("how does auth
work"), not the string you happen to be holding — retrieval is BM25 + token overlap, so natural language
outperforms an exact paste of an unrelated identifier.
**Reflect when** you need the decision *and its rationale* in one line and a citation suffices; it does
not synthesize across memories, so recall and read the array for breadth. **Delete** with
`DELETE .../memories/:mid` when a stored fact turns out wrong or secret — that is what the lifecycle
routes are for, and the call is idempotent (`{"deleted":false}` the second time). **Never retain** secrets,
credentials, personal data, or anything derivable from the repository: redaction is a safety net (PEM,
JWT, `Bearer`, Slack, Google API keys, emails, phones, `<private>` blocks), not permission to store PII.

## Anti-patterns

- **`budget: 0`** returns `[]` — a hard cap, not "unlimited". Default 2000 tokens, and one recall never
  returns more than **100** memories.
- **Expecting an over-budget top hit whole.** It is cut to exactly the cap (4 chars per token) with a
  literal `…[truncated]` tail — but only when the 12-char marker still fits, so budgets of 1–3 tokens
  return the top hit cut to 4–12 chars with no marker, not `[]`. Raise `budget` for the rest.
- **Expecting `recall` rows to be objects.** The default is a bare array of content strings: no ids, no
  scores, no timestamps. Send `{"format":"full"}` when you need `{id, score, content}`.
- **Reaching for `recall` to enumerate a bank.** It is relevance-ranked, so a memory you stored but cannot
  match is invisible. `GET .../memories` is the custody route: `?limit=` (default 50, capped 500) and
  `?offset=`, plus `GET`/`DELETE .../memories/:mid` and `GET .../stats`.
- **Using `update_mode: "append"` for repeated writes.** The default `replace` is the working upsert: two
  retains under one `document_id` leave one row holding the last content. A *second* `append` under an
  existing `document_id` is refused with `409 document already exists; use update_mode=replace` and the
  first row survives, unchanged — `append` is single-use per document id. An unrecognised `update_mode`
  string comes back as `400 invalid content`; a wrong-typed one is `422` from the JSON deserializer.
- **Cross-bank recall.** A query against a bank you never wrote to returns `[]`; an empty result is not
  proof the memory was never stored. The rule is by shape, not by route: an operation that *names one
  resource* (`config`, `memories/:mid`) answers `404` when it is absent, while an operation that reads a
  *collection or aggregate* (`memories`, `stats`, `recall`, `reflect`) answers `200` with the empty
  answer, because banks are created implicitly by their first `retain`. Note `PUT .../config` requires an
  existing bank and will not create one — configure a bank after its first retain.
- **Blaming a 500.** `storage error` is deliberately opaque (raw driver text can leak SQL fragments and
  on-disk paths). The actionable errors are `400 invalid bank id` (empty/whitespace id), `400 invalid
  content`, `400 invalid bank config`, `404 unknown memory`, and `404 unknown bank`.

## Checklist

- [ ] Server up — `curl -s "$MW/health"` returns exactly `ok` (plain text), and the bank name is
      non-empty (blank/whitespace is a `400`)
- [ ] Query is a concept, not a pasted identifier, and `budget` is at the default unless you have a reason
- [ ] Retain returned `{"id":"<uuid>"}`, carried nothing sensitive (redaction is a net, not a licence),
      and stored one fact — not a paragraph
- [ ] Recall returned a JSON array of strings; any `…[truncated]` tail read as "raise `budget`", not "the
      rest is lost"; an empty array means a wrong bank, or a budget too small to hold the marker
