# Install memory-wire (agent runbook)

Self-contained HTTP API plus an MCP stdio server, no daemon to supervise. Nine
curl steps, each with an assertion you can check. Everything below was executed
against a local `memory-wire serve` on a scratch `--db` on 2026-09-27; the
`Expect:` lines are observed output, not aspirations, and every claim here is
listed in [`docs/CONSISTENCY.md`](docs/CONSISTENCY.md) with how it was checked.

```bash
export MW_REPO='ishan-parihar/memory-wire'                  # this repo
export MW_RAW="https://raw.githubusercontent.com/$MW_REPO/main"
export MW_BIN="$HOME/.local/bin/memory-wire"
export MW_DB="$HOME/.local/share/memory-wire/agents.db"     # your call
export MW="http://127.0.0.1:8888"
```

## Start the server

```bash
nohup memory-wire serve --addr 127.0.0.1:8888 --db "$MW_DB" >/tmp/memory-wire.log 2>&1 &
```

Omit `--db` and it defaults to `$XDG_DATA_HOME/memory-wire/memory.db`
(falling back to `~/.local/share/...`). Parent directories are created for you.
The server seeds a bank named `default` on boot; any other bank id in the URL is
created implicitly by its first `retain`.

**If 8888 is already taken**, `serve` fails to bind and exits — it does not pick
another port silently. Observed: `Error: Address already in use (os error 98)` on
stderr and exit 1. Start it on a free one and point `$MW` at that port:

```bash
export MW="http://127.0.0.1:18899"                       # any free port
nohup memory-wire serve --addr 127.0.0.1:18899 --db "$MW_DB" >/tmp/memory-wire.log 2>&1 &
```

`--addr` is the only thing that changes; every path below is port-agnostic
because it goes through `$MW`. The health body must be exactly `ok` — plain text,
not JSON, not `"ok"`, not a 200 with an HTML error page. A 404 or connection
refused there means you are pointed at the wrong port, not at memory-wire.

---

### 1. Health check

```bash
curl -sS -w '\n%{http_code}\n' "$MW/health"
```

`Expect:` body is exactly `ok` (not JSON) and the status is `200`.

### 2. Retain

```bash
MEM_ID=$(curl -sS -X POST "$MW/banks/demo/retain" \
  -H 'Content-Type: application/json' \
  -d '{"content":"auth uses jose middleware; key sk-abcDEF123456; mail me at ishan@example.com"}' \
  | sed 's/.*"id":"\([^"]*\)".*/\1/')
echo "stored $MEM_ID"
```

`Expect:` a `200` whose body is `{"id":"<uuid>"}`, and `$MEM_ID` holds that uuid —
step 7 fetches and deletes by it, so keep the variable in the same shell. The
stored content comes back redacted-on-write — it is `auth uses jose middleware;
key [REDACTED:api_key]; mail me at [REDACTED:email]`. Redaction happens in the
service before the write, so the secret is never on disk. `context` is redacted
too. Covers PEM keys, JWTs, `Bearer`, Slack tokens, Google API keys, emails, and
phone numbers, plus `<private>` blocks.

### 3. Recall

```bash
curl -sS -X POST "$MW/banks/demo/recall" \
  -H 'Content-Type: application/json' \
  -d '{"query":"how does auth work","budget":2000}'
```

`Expect:` a JSON **array of content strings** (not objects, no scores) and `200`:

```json
["auth uses jose middleware; key [REDACTED:api_key]; mail me at [REDACTED:email]"]
```

`budget` is optional and defaults to `2000`. Recall is bank-isolated: the same
query against `/banks/other/recall` returns `[]` until that bank has content.
Retrieval is SQLite FTS5 BM25 fused with token-overlap rank via RRF (k=60).

### 4. Budget and result caps

```bash
# budget is a hard cap, not a hint: 0 tokens -> empty
curl -sS -X POST "$MW/banks/demo/recall" \
  -H 'Content-Type: application/json' -d '{"query":"auth","budget":0}'

# seed one memory longer than the budget you are about to ask for
curl -sS -X POST "$MW/banks/demo/retain" -H 'Content-Type: application/json' \
  -d '{"content":"jose jose jose jose jose jose jose jose jose jose jose jose jose jose jose"}'

# over-budget top hit is truncated in place, never dropped
curl -sS -X POST "$MW/banks/demo/recall" \
  -H 'Content-Type: application/json' -d '{"query":"jose","budget":5}'
```

`Expect:` first call returns `[]`. The second call is the `retain` that seeds a
memory longer than the budget you are about to ask for, so the **third** call
returns the **top hit for your query** with its tail replaced by the literal
marker `…[truncated]` — observed against a bank that also holds the step-2
memory:

```json
["auth use…[truncated]"]
```

The result measures exactly the cap (4 chars per token), so `budget: 5` yields a
20-character string. One caveat worth knowing: the marker is 12 characters and is
appended only when it fits inside the remaining room, so the smallest budgets
return the top hit **cut to the cap and unlabelled** rather than dropped —
observed, same bank: `budget: 1` → `["auth"]`, `2` → `["auth use"]`,
`3` → `["auth uses jo"]`, `4` → `["auth…[truncated]"]`. Read a short answer as
"raise `budget`", not "there was nothing to say". Separately, one recall never
returns more than **100** memories (`MAX_RESULTS`): observed, a 150-memory bank
queried with `budget: 1000000` returns exactly 100.

### 5. Reflect

```bash
curl -sS -X POST "$MW/banks/demo/reflect" \
  -H 'Content-Type: application/json' -d '{"query":"what decided auth"}'
```

`Expect:` a bare JSON string, `200` — the id is the one step 2 stored:

```json
"based on [3b3a923f-b3ce-48ed-8829-cf071d6accb3]: auth uses jose middleware; key [REDACTED:api_key]; mail me at [REDACTED:email]"
```

Reflect is currently top-hit citation, not synthesis — there is no LLM in the
loop. The prompt an LLM step must follow (`REFLECT_SYSTEM_PROMPT` in
`src/api.rs`) frames a bank as a historian's record: declarative past-tense
prose, literal numbers verbatim, never an imperative.

### 6. Bank config

```bash
curl -sS "$MW/banks/demo/config"                                            # GET
curl -sS -X PUT "$MW/banks/demo/config" \
  -H 'Content-Type: application/json' \
  -d '{"recallMaxTokens":128,"retainTags":["ops"],"retain_mission":"own the release"}'
curl -sS "$MW/banks/demo/config"                                            # GET again
```

`Expect:` `200` and `{}` on the first GET (the bank exists from step 2, but has
no config yet), the PUT echoes the object it stored, and the second GET serves
it back byte-for-byte — including `retain_mission`, a key this build stores and
serves but does not act on. A bank that was never created is `404 unknown bank`,
not `{}`, so a typo cannot read back as "this bank has no settings". A `PUT` on a
bank that was never created is refused the same way and does **not** create it.

Two keys do change behavior:

- `recallMaxTokens` — the default recall budget for that bank. A recall that
  omits `budget` uses it; an explicit `budget` still wins. `budget` is a hard
  cap in tokens either way, so `{"recallMaxTokens":5}` truncates a long top hit
  in place (5 × 4 chars) rather than dropping it.
- `retainTags` — added to the tag set of every later retain in that bank.
  Request tags come first, so a bank-wide default never displaces a tag you
  named explicitly.

```bash
# bank-wide tag: a later retain inherits it, no `tags` in the body
curl -sS -X POST "$MW/banks/demo/retain" -H 'Content-Type: application/json' \
  -d '{"content":"jose inherits the bank default tag"}'
curl -sS -X POST "$MW/banks/demo/recall" -H 'Content-Type: application/json' \
  -d '{"query":"inherits","tags":["ops"]}'
```

`Expect:` `["jose inherits the bank default tag"]`.

Tags also work without any config: `POST .../retain` accepts
`{"content":..., "tags":[...]}`, and `POST .../recall` accepts
`{"query":..., "tags":[...]}` to restrict the search to memories carrying **any**
of them. An unfiltered recall is unchanged by any of this.

### 7. Memory lifecycle

```bash
# page a bank: limit defaults to 50 and is capped at 500, offset to 0
curl -sS "$MW/banks/demo/memories?limit=5&offset=0"

# one memory by id — the id you kept from step 2
curl -sS "$MW/banks/demo/memories/$MEM_ID"

# what is in the bank at all
curl -sS "$MW/banks/demo/stats"
```

`Expect:` `200` and an array of objects whose keys serialise in the order below,
with a `"context"` key only on memories that actually have capture context:

```json
[{"content":"auth uses jose middleware; key [REDACTED:api_key]; mail me at [REDACTED:email]","created_at":"2026-09-27T11:23:30.993Z","id":"3b3a923f-b3ce-48ed-8829-cf071d6accb3"}]
```

`created_at` is RFC 3339 UTC with milliseconds, stamped at insert. `stats`
returns `{"memories":N,"tags":N,"oldest":…|null,"newest":…|null}` — observed
`{"memories":3,"tags":1,"oldest":"2026-09-27T11:23:30.993Z","newest":"2026-09-27T11:23:31.163Z"}`.
An unknown memory id is `404 unknown memory`. Paging caps are real, not
advisory: on a 150-memory bank, no `?limit=` returns 50, `?limit=99999` returns
all 150 (cap 500), and `?limit=10&offset=145` returns 5.

**The unknown-bank rule** (steps 2–7 all name a bank). An operation that *names
one resource* answers `404` when it is absent, so a typo cannot read back as a
plausible empty answer: `GET`/`PUT .../config` on a bank that was never created
is `404 unknown bank`, and `GET .../memories/:mid` is `404 unknown memory`. An
operation that reads a *collection or an aggregate* answers `200` with the empty
answer, because a bank is created implicitly by its first `retain` and a
pre-retain recall is normal, not exceptional: `memories` → `[]`, `stats` →
all-zero/nulls, `recall` → `[]`, `reflect` → `"no relevant memories"`. So the
difference from step 6 is not an accident — it is the rule, and it holds
identically whether the bank has never been created or merely holds nothing.
Corollary: **`PUT .../config` requires an existing bank** and will not create
one; configure a bank after its first retain.

```bash
# delete is idempotent: 200 both times, the flag says whether a row went away
curl -sS -X DELETE "$MW/banks/demo/memories/$MEM_ID"
curl -sS -X DELETE "$MW/banks/demo/memories/$MEM_ID"
```

`Expect:` `{"deleted":true}` then `{"deleted":false}` — not a `404` on the repeat.

### 8. `document_id` upsert and `format: "full"`

```bash
# same document_id twice: the second write supersedes the first
curl -sS -X POST "$MW/banks/demo/retain" -H 'Content-Type: application/json' \
  -d '{"content":"spec v1","document_id":"spec"}'
curl -sS -X POST "$MW/banks/demo/retain" -H 'Content-Type: application/json' \
  -d '{"content":"spec v2","document_id":"spec"}'
curl -sS "$MW/banks/demo/memories?limit=500"   # one spec row, holding "spec v2"

# recall in full form, for a caller that must cite what it got
curl -sS -X POST "$MW/banks/demo/recall" -H 'Content-Type: application/json' \
  -d '{"query":"spec","format":"full"}'

# append: the first one under a fresh document_id lands...
curl -sS -w ' [%{http_code}]\n' -X POST "$MW/banks/demo/retain" \
  -H 'Content-Type: application/json' \
  -d '{"content":"plan v1","document_id":"plan","update_mode":"append"}'

# ...and a second one under the same document_id is refused
curl -sS -w ' [%{http_code}]\n' -X POST "$MW/banks/demo/retain" \
  -H 'Content-Type: application/json' \
  -d '{"content":"plan v2","document_id":"plan","update_mode":"append"}'
```

`Expect:` one row per `document_id`, holding the last content — the upsert
deletes the prior revision and inserts the new one in a single transaction.
Observed for `spec`: `[{"content": "spec v2", "created_at": "2026-09-27T11:23:31.265Z", "id": "272de770-…"}]`,
one row, `spec v2`. `format:"full"` returns `{"content":…,"id":…,"score":…}`
objects — observed `[{"content":"spec v2","id":"272de770-…","score":0.032786885245901638}]`;
`score` is the fused RRF value the ranking used (`Σ 1/(60 + rank)`), not a token
count, so it is a small positive float and only comparable against other scores
from the same recall. The
default (no `format`, or any value other than `full`) stays a bare array of
strings.

**One trap, verified against this build:** `"update_mode":"append"` only ever
*adds* a memory — it never extends a row — and a *second* append under a
`document_id` that already holds a row comes back `409` with the body
`document already exists; use update_mode=replace`. Observed: the first `plan`
append returned `200 {"id":"efc33dc0-…"}`, the second returned
`409 document already exists; use update_mode=replace`, and the `plan` row still
held `plan v1`. Use the default `replace` for repeated writes.

### 9. Hook wiring, doctor, and MCP

```bash
memory-wire --help
memory-wire connect claude-code
memory-wire hook --help
memory-wire doctor
```

`Expect:` the binary ships **eight** subcommands (`help` is clap's own and is not
one of them), and all four commands above work. Verbatim `--help`:

```
$ memory-wire --help
memory-wire: Rust agent-memory infrastructure (Hindsight x agentmemory)

Usage: memory-wire [COMMAND]

Commands:
  info     Print audit + plan pointers (default)
  serve    Start the HTTP server (retain/recall/reflect + health)
  connect  Wire memory-wire into agent hosts (default: every detected host)
  hook     Lifecycle hook (hosts call this; reads hook JSON on stdin)
  doctor   Report endpoint, bank, store, and server health
  mcp      Serve MCP over stdio: memory_retain / recall / reflect / bank_config_get
  sweep    Delete memories older than their bank's `ttl_days` (banks with none are skipped)
  seed     Seed a bank once from this repo's git history (and, opt-in, transcripts)
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

`connect claude-code` on a machine with no `~/.claude/settings.json` prints
`claude-code  wired         SessionStart, UserPromptSubmit, Stop` and exits 0;
run it again and it prints `claude-code  already-wired -`; `--uninstall` prunes
exactly those entries (`claude-code  unwired       SessionStart (pruned),
UserPromptSubmit (pruned), Stop (pruned)  (backup: …)`). It never touches a hook
it did not write: where the host already has a `SessionStart` value that is not a
hook list, it wires the other two and reports
`| left alone: SessionStart (existing value is not a hook list)`; a
`settings.json` that is not valid JSON is refused untouched with
`claude-code  FAILED        <path>: malformed JSON (…); left untouched` and
exit 1. `connect` with no host argument wires all five it detects
(claude-code, codex, copilot-cli, cursor, opencode) — cursor and opencode get an
MCP entry, the other three the three lifecycle hooks. `connect --uninstall
--guidelines` is refused (`memory-wire: cannot combine --uninstall with
--guidelines`, exit 1) and an unknown host is a clap error, exit 2.

`hook --help` lists `session-start`, `prompt`, `stop`. `doctor` prints endpoint,
bank, store size and health, row counts, and server state.

`doctor` takes `--db` so it inspects the same file your server opened — pass the
same value you gave `serve --db`, or the report describes a store the server
never touches. It never creates the store it reports on: pointing it at a
nonexistent path reports `missing` without creating the file, and `--strict`
exits 1. It does run `PRAGMA integrity_check` on an **existing** store and exits
1 under `--strict` when a page does not check out, and it folds that store's WAL
into the main file on the way past (best effort, `PASSIVE`, never blocking) so a
`cp` taken right after a `doctor` run is not missing pages. Verified output:

```
$ memory-wire doctor --db "$MW_DB"
memory-wire doctor
  endpoint   http://127.0.0.1:8888
  bank       memory-wire
  store      /tmp/canon.db  (450.4 KB, ok)
  banks      2
  memories   5
  server     up
```

```bash
memory-wire doctor --db "$MW_DB"
memory-wire doctor --db "$MW_DB" --strict   # exit 0 only if server+store are usable
MEMORY_WIRE_URL="$MW" memory-wire doctor --db "$MW_DB" --strict   # point at a non-default port
```

`endpoint` comes from `$MEMORY_WIRE_URL`, so against the `$MW` you have been
using all along the last line exits 0 and reports `server up`; without it
`doctor` probes `127.0.0.1:8888` whatever port you served on. `server up` means
`/health` answered 2xx **and** the body was exactly `ok` — a foreign process on
the port that answers 200 is reported `server down: unexpected health response`,
which is what happened on this machine's 8888 during the sweep.

The hooks read the bank's framing from the `GET /banks/:id/config` route, for
**the bank the hook resolves from its own working directory** — the git
worktree's top-level directory name, else `memory-wire`. That is usually not the
`demo` bank of step 6, and `MEMORY_WIRE_BANK` does not change it. Put
`{"background": "..."}` in *that* bank's config and `hook session-start` prints
it instead of the built-in historian framing; with no config, or a 404, it falls
back to the framing `reflect` synthesises with. Observed both ways: with the key
set, the preamble body is `Project P: keep the rail thin.` in place of the
historian text.

#### MCP (stdio)

```bash
memory-wire mcp --help
memory-wire seed --help
memory-wire sweep --help
```

`Expect:` `--bank <BANK>` and `--db <DB>` for `mcp`; `--commits <N>` (default 100,
capped at 500), `--transcripts`, `--bank <BANK>`, `--db <DB>` for `seed`;
`--dry-run` and `--db <DB>` for `sweep`. The server speaks JSON-RPC 2.0 over
newline-delimited stdin/stdout — nothing else is written to stdout, logs go to
stderr. `--bank` sets the default bank for every call; `MEMORY_WIRE_BANK` is the
fallback, and `memory-wire` is the fallback for that. A per-call `bank` argument
overrides both. `seed` reads this repository's git history (and, with
`--transcripts`, the 50 newest `~/.claude/projects` transcripts) into one bank;
outside a git work tree it says so and exits 0 rather than failing.

| Tool | Writes? | Arguments |
|---|---|---|
| `memory_retain` | yes | `content` (required), `bank?`, `context?`, `tags?` |
| `memory_recall` | no | `query` (required), `bank?`, `budget?`, `tags?` |
| `memory_reflect` | no | `query` (required), `bank?` |
| `memory_bank_config_get` | no | `bank?` |

Every tool declares `destructiveHint: false`; `memory_retain` is the only one
without `readOnlyHint: true`. An unknown tool name is a JSON-RPC
`MethodNotFound` (`-32601`); a tool that runs and fails returns
`isError: true` with the same opaque message the HTTP surface returns, never a
crash and never driver text.

Point an MCP client at it, e.g.:

```json
{ "mcpServers": { "memory-wire": {
  "command": "memory-wire", "args": ["mcp", "--bank", "my-project"] } } }
```

`sweep` is the one destructive command, and it is opt-in per bank: it deletes what
`ttl_days` in a bank's config says is too old, and lists every bank without one as
`skipped` without touching it. Nothing runs it for you — there is no scheduler and
no background process — and `--dry-run` reports the same counts while deleting
nothing. Both exit 0, and both name the cutoff they used:

```
$ memory-wire sweep --dry-run --db "$MW_DB"
memory-wire sweep
  demo        ttl 7d  cutoff 2026-09-20T13:52:46.190Z  would delete 0
  default     skipped (no ttl_days)
total 0 would be deleted
```

---

## Not in this build

Verified absent, so you do not curl a route that is not there and report a bug:

| Looking for | Status |
|---|---|
| delete a whole bank | one memory at a time only — no `DELETE /banks/:id` |
| `tags` in the lifecycle responses | they serve `{content, created_at, id[, context]}`; tag filtering stays on `recall` |
| `document_id` in any response | a write-side handle only — store it yourself |
| repeat `update_mode: "append"` | `409` with `document already exists; use update_mode=replace` — the mode that works for repeated writes is the default `replace` |
| a `backup` subcommand | not a command, deliberately — see "Back up and restore" below |
| MCP over HTTP | stdio only — `/mcp/:bank` bank-in-path scoping is not built |

## Final verify

One command that fails loudly if any of the above regressed:

```bash
set -e
curl -fsS "$MW/health" | grep -qx ok
ID=$(curl -fsS -X POST "$MW/banks/verify/retain" -H 'Content-Type: application/json' \
      -d '{"content":"memory-wire install verification token"}' | sed 's/.*"id":"\([^"]*\)".*/\1/')
curl -fsS -X POST "$MW/banks/verify/recall" -H 'Content-Type: application/json' \
  -d '{"query":"install verification token"}' | grep -q 'install verification token'
echo "ok — retained $ID and recalled it back"
```

## Error contract

| Condition | Status | Body |
|---|---|---|
| bank id is empty or whitespace | `400` | `invalid bank id` |
| `content` is empty or whitespace | `400` | `invalid content` |
| `GET`/`PUT /banks/:id/config` on a bank that was never created | `404` | `unknown bank` |
| `GET`/`PUT /banks/:id/memories/:mid` on an unknown memory | `404` | `unknown memory` |
| `PUT /banks/:id/config` with a non-object body | `400` | `invalid bank config` |
| `?limit=` that is not a number | `400` | `Failed to deserialize query string: invalid digit found in string` (axum's own) |
| a repeat `update_mode: "append"` under one `document_id` | `409` | `document already exists; use update_mode=replace` |
| a retain that names a bank that was never created | `200` | `{"id":…}` — the bank is created by its first retain, as it already was over HTTP |
| a retain whose content is already in that bank | `200` | `{"id":…}` of the memory that already holds it — no second copy is stored |
| any other storage failure | `500` | `storage error` |
| unknown path | `404` | empty body (axum's default) |

The `500` body is deliberately opaque: raw driver text can echo SQL fragments and
on-disk paths, so every non-input failure collapses to `storage error`.

## Back up and restore

There is no `backup` subcommand, on purpose: the store is one SQLite file and
SQLite already ships a correct backup command. What you need is the `sqlite3`
CLI (`sqlite3 --version`; on Debian/Ubuntu `apt install sqlite3`, on macOS it is
in the base system).

**Online — the server may keep running.** This is the recipe, and it is the one
`tests/backup.rs` executes:

```bash
sqlite3 "$MW_DB" ".backup '$MW_DB.bak'"
```

`.backup` is SQLite's online backup API, not a file copy: it takes a read lock
per page, so the result is a consistent snapshot of a store that is being
written while you copy it, and it carries the FTS5 index across with the rows —
a restored store answers `recall` immediately, no reindex step.

**Offline — stop the server, then copy.** Once nothing is writing, a plain copy
is fine, and the sidecars must go with it:

```bash
kill -TERM %1                  # or the serve pid: it drains, then exits 0
cp "$MW_DB.bak" "$MW_DB"        # or any other file you want to restore
rm -f "$MW_DB-wal" "$MW_DB-shm"  # do this BEFORE opening the restored store
```

`serve` treats SIGINT and SIGTERM the same: it stops accepting and lets the
requests already in flight finish, so a `kill -TERM` is a clean stop and the
`-wal` is complete. Do not reach for `kill -9`, which cuts whatever request is
mid-write.

**Why not just `cp "$MW_DB" "$MW_DB.bak"`?** Because a live store is a WAL
database, and in WAL mode the schema and every recent row are still sitting in
`$MW_DB-wal`. Verified on a live server: after one `retain`, `agents.db` was
4,096 bytes and `agents.db-wal` 156,592, and the copy had **no `memories` table
at all** — not stale, empty. Two ways that bites:

- **A naive copy is not a backup.** You get a file you cannot restore from.
- **A naive restore is worse than none.** `cp "$MW_DB.bak" "$MW_DB"` with a
  stale `$MW_DB-wal` still next to it lets SQLite replay that WAL over the copy:
  observed, the "restored" store came back with the *live* rows, not the
  backup's. Removing the sidecars first is what makes the copy authoritative.

If you want the cheap route, fold the WAL in first and then copy — this is what
`doctor` does for you on an existing store:

```bash
memory-wire doctor --db "$MW_DB"     # PASSIVE wal_checkpoint, never blocks
cp "$MW_DB" "$MW_DB.bak"             # now the main file is complete
```

To check a backup instead of trusting it:

```bash
sqlite3 "$MW_DB.bak" "PRAGMA integrity_check;"   # -> ok
sqlite3 "$MW_DB.bak" "SELECT COUNT(*) FROM memories;"
```

`memory-wire doctor --db "$MW_DB.bak" --strict` is the same check from the other
direction, and it is also the one that fails when a live store's page does not
check out.

## Uninstall

```bash
curl -fsSL "$MW_RAW/install/get-memory-wire.sh" | sh -s -- --uninstall
```

Removes the binary only. Observed output:

```
removed /home/you/.local/bin/memory-wire (database left in place at /home/you/.local/share/memory-wire — delete it by hand)
```

Your database is left in place — delete it by hand at `$MW_DB` (that message
names the *default* data directory, which is not where `--db` put it if you
passed one).
