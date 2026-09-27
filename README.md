# memory-wire

**Agent memory that out-retrieves both incumbents at ~1% of their RAM.** One static
Rust binary: retain/recall/reflect with bank isolation, FTS5 BM25 + RRF fusion, PII
redaction on write — over HTTP, or as an MCP stdio server. No runtime, no daemon, no
database to install.

- **93.0% R@5** LongMemEval-S · **10.5 MB** idle RSS · **8.4 MB** binary · 100% hit-rate coding-life
- Docs: `docs/AUDIT.md` (competitor teardowns) · `PLAN.md` · `docs/VERSIONS.md` (pins) · [`INSTALL_FOR_AGENTS.md`](INSTALL_FOR_AGENTS.md) (curl runbook)
- Proof: `eval/RESULTS.md` · `eval/CODING_LIFE.md` · `eval/SCALE_SWEEP.md` · `docs/BENCHMARK.md`

**Contents** — [Benchmarks](#benchmarks) · [Footprint](#footprint-vs-competitors) · [Quick start](#quick-start) · [CLI](#cli) · [Install](#install) · [How it works](#how-it-works) · [Reproduce](#reproduce) · [Roadmap](#roadmap)

## Benchmarks

Official LongMemEval-S, retrieval-only (no LLM judge), same methodology as the
competitors' harnesses — per-question fresh index, session-as-document, question-text
query. Regenerate: `cargo run --example longmemeval` → `eval/RESULTS.md`.

| System | R@5 | R@10 | R@20 | NDCG@10 | MRR |
|---|---|---|---|---|---|
| **memory-wire (FTS5 BM25 + overlap RRF)** | **93.0%** | **97.4%** | **99.6%** | **83.5%** | **83.9%** |
| agentmemory BM25-only | 86.2% | 94.6% | 98.6% | 73.0% | 71.5% |
| agentmemory BM25+Vector | 95.2% | 98.6% | 99.4% | 87.9% | 88.2% |

Competitor rows: their `benchmark/LONGMEMEVAL.md` (same metric, verified in-audit).
Hindsight publishes QA-accuracy leaderboard scores (needs an LLM reader), not
retrieval recall — not comparable here; their retrieval stack is 4-stream TEMPR +
cross-encoder rerank behind a 0.8–1.0 GB idle RSS (see below).

| Suite | memory-wire | Competitors |
|---|---|---|
| coding-life, 15 labeled queries (`eval/CODING_LIFE.md`) | hit-rate **100%**, R@5 100%, p50 0.27–0.62 ms across 5 release runs | grep baseline 96.7% (same harness); agentmemory publishes no score |
| Scale sweep 240→10k (`eval/SCALE_SWEEP.md`) | p50 0.6→4.9ms, token savings ≥95.7% | agentmemory: 0.1→22.8ms BM25, heap 6→316MB, savings to 100% |

## Footprint vs competitors

| Dimension | memory-wire (measured) | Hindsight (their install docs) | agentmemory (their SCALE.md) |
|---|---|---|---|
| Ship artifact | **8.4 MB** binary — 8,816,120 B (LTO, incl. the MCP SDK) | Python API image + PG/pg0 | Node 20 + iii-engine binary |
| Idle RSS | **10.5 MB** (`ps -o rss`, serve + 1 retain, mean of 3) | **0.8–1.0 GB** full / ~100s MB slim | heap **6 MB** @1k obs |
| RSS under load | **12.7 MB** (5k retains + 200 recalls, recall p50 6ms / p95 10ms) | **1.2–1.5 GB** full (models + ONNX arenas) | heap **316 MB** @50k obs |
| Minimum box | any (binary + SQLite) | **1.5 GB** full / 512 MB slim + external providers + separate DB | Node + engine + 4 ports |
| 10k-memories storage | **3.0 MB** (SQLite+FTS5, 3,194,880 B settled — 7,442,440 B while the WAL is unflushed) | Postgres + pgvector (+ reranker) | **35.7 MB** (BM25+vector) |
| Background processes | **0** | API + worker + UI + DB | REST + streams + viewer + worker WS |

Hindsight rows: their `docs/developer/installation.md` (RAM table). agentmemory rows:
their `benchmark/SCALE.md` (§1 heap + storage tables). memory-wire rows: measured
2026-09-27 on this tree at `0.2.0` — `cargo build --release --locked`, then `ls -l
target/release/memory-wire`; RSS by `ps -o rss` against a `serve` on a scratch
`--db`, read after health check + 1 retain (idle; mean of 3 runs) and after 5,000
retains + 200 recalls over HTTP (under load; one process, sequential); storage is
the 10k row of `eval/SCALE_SWEEP.md`.

The deltas are the interesting part, and three of them moved in opposite
directions. **The binary grew, not shrank**: 8,816,120 B against the 8,593,368 B
measured at `0.1.0`, **+222,752 B (+2.6%)**. The `fastembed` optional dependency
and its ONNX runtime are gone — there is no `embed` feature and no `fastembed`,
`ort` or `tokenizers` entry in `Cargo.lock` — but the MCP SDK landed between those
two measurements, and `rmcp` + `schemars` cost more than `fastembed` ever did. The
binary was 6.4 MB before MCP; MCP is the reason the artifact is 8.4 MB and not
smaller, and it is code, not a resident dependency, so idle RSS does not move
with it.

**Idle RSS is flat**: 9,123 KB before the first retain and 10,704 KB (10.45 MiB)
after it, mean of 3 runs, against 10,396 KB for the same measurement in the
previous audit — **+308 KB (+3.0%)**, tracking the +2.6% binary and the extra
schema pages. The pre-retain/post-retain gap is the FTS5 index and the write
path's first pages, and it is the reason the two readings must not be quoted
interchangeably.

**The 10k store grew and is understated by its own harness**: 3,194,880 B
(3.05 MiB) settled, **+819,200 B (+34.5%)** over the previous 2,375,680 B. A
drop-index-and-`VACUUM` ladder on a real 10k store attributes it to the two
indexes that the `document_id` upsert and content-hash dedup added —
`idx_memories_bank_hash` 565,248 B (17.7%) and `idx_memories_bank_doc` 106,496 B
(3.3%) — plus the `content_hash` and `document_id` columns themselves. The
pre-existing `idx_memories_bank_time` (`src/store.rs:577`) is 344,064 B, 10.8%.
Separately, the store runs in **WAL mode**, and `eval/SCALE_SWEEP.md` reports the
*main file* only: while the process holds it open, 10,000 memories actually occupy
7,442,440 B (main 3,194,880 + `-wal` 4,214,792 + `-shm` 32,768), which collapses
back to 3,194,880 B at a clean `wal_checkpoint(TRUNCATE)`. The 3.0 MB figure is
the settled size; a server that has just been fed 10k memories and not yet
checkpointed is using more than twice that.

**Under load is where the bounded candidate pool pays**: 12,964 KB (12.66 MiB)
against 22,116 KB before it, **−9,152 KB (−41.4%)**, with recall p50/p95 at
6 ms/10 ms against 45 ms/132 ms. Recall cost no longer grows with the bank, so a
bank 50× the size costs less resident memory than it did at 1/50th the size.

## Quick start

```bash
cargo build --release
./target/release/memory-wire serve --addr 127.0.0.1:8888 --db ~/.local/share/memory-wire/agents.db
curl -sS -X POST localhost:8888/banks/demo/retain -H 'Content-Type: application/json' -d '{"content":"auth uses jose"}'
curl -sS -X POST localhost:8888/banks/demo/recall -H 'Content-Type: application/json' -d '{"query":"how does auth work","budget":2000}'

# or, for an MCP client, over stdio:
./target/release/memory-wire mcp --bank my-project
```

`--db` is optional: without it the store lands at `$XDG_DATA_HOME/memory-wire/memory.db`,
falling back to `~/.local/share/memory-wire/memory.db` when `XDG_DATA_HOME` is unset
or relative. Point it at a scratch file for throwaway work —
`--db /tmp/scratch.db` — and nothing touches your real bank.

`GET /health` → `ok` (plain text) · `POST /banks/:id/{retain,recall,reflect}` ·
`GET`/`PUT /banks/:id/config` · `GET /banks/:id/memories[?limit=&offset=]` ·
`GET`/`DELETE /banks/:id/memories/:mid` · `GET /banks/:id/stats`.
Banks are isolated namespaces; any bank id is created implicitly by its first
`retain`, and a `default` bank is seeded at boot. Recall returns a bare JSON array
of content strings by default; `{"format":"full"}` returns `{id, score, content}`
objects instead, where `score` is the fused RRF value the ranking actually used
(`Σ 1/(60 + rank)`, higher is better) rather than a token count — so it is a
fraction, and only comparable against other scores from the same recall.
`reflect` returns the top hit prefixed with its id — there is no
LLM in the loop yet.

**Bounds, and the error contract.** `budget` is a hard token cap, not a hint:
`budget: 0` returns `[]`, and a top hit longer than the budget is cut to exactly
the cap (4 chars per token) rather than dropped — with a literal `…[truncated]`
tail when the marker's own 12 characters fit inside the remaining room, and
unlabelled when they do not (budgets 1–3 return the top hit cut to 4–12 chars, not
`[]`). Lower hits that do not fit are skipped so smaller ones further down still
fill what is left. One recall never returns more than **100** memories. A blank
or whitespace bank id is a `400 invalid bank id`; a blank or whitespace
`content` is a `400 invalid content` (a memory of nothing matches no query and
cites nothing); an unknown memory id is `404 unknown memory`; and every storage
failure — including raw driver text that would echo SQL fragments and on-disk
paths — collapses to an opaque `500 storage error`.

Secrets are redacted before the write, on both `content` and `context`:
`<private>` blocks, PEM private keys, JWTs, `Bearer` headers, Slack tokens, Google
API keys, emails, and phone numbers.

## CLI

Eight subcommands, exactly as `--help` reports them (`help` is clap's own and is
not one of them):

| Command | What it does |
|---|---|
| `info` | Audit + plan pointers (the default when no command is given) |
| `serve` | Start the HTTP server — `--addr` (default `127.0.0.1:8899`), `--db`. Stops accepting and drains in-flight requests on SIGINT/SIGTERM |
| `connect` | Wire the hooks into agent hosts — optional `<agent>`, `--uninstall`, `--guidelines` |
| `hook` | Lifecycle hook the hosts invoke — `session-start`, `prompt`, `stop` |
| `doctor` | Endpoint, bank, store, and server health — `--db`, `--strict` |
| `mcp` | Serve MCP over stdio — `--bank`, `--db` |
| `sweep` | Delete memories past their bank's `ttl_days` — `--dry-run`, `--db` |
| `seed` | One-shot bank seeding from this repo's git history — `--commits <N>` (default 100, capped at 500), `--transcripts`, `--bank`, `--db` |

`--addr` defaults to loopback because there is no authentication: anything that
can reach the port can read and delete every bank. A non-loopback address still
binds, and says so once on stderr before it does.

If 8899 is already taken, `serve --addr 127.0.0.1:<free port>` and point your
client at that port; `GET /health` on it must return exactly `ok`. There is no
`backup` subcommand — a SQLite store is backed up with `sqlite3 "$DB" ".backup
'$DB.bak'"`, recipe and caveats in `INSTALL_FOR_AGENTS.md`.

## Connect, hooks, doctor

`memory-wire connect` installs the three lifecycle hooks into every agent host it
detects (claude-code, codex, copilot-cli) and the MCP server entry into the two
that speak it (cursor, opencode); pass a host to wire just that one, `--uninstall`
to prune, `--guidelines` to write the rules block into this project's agent files
instead. It is idempotent and never touches a hook it did not write — malformed
config is refused untouched rather than rewritten. (The file it writes is
re-serialised with sorted keys and two-space indent; foreign entries survive
intact, and whenever there was a file to copy, a timestamped backup of it is kept
under `$XDG_DATA_HOME/memory-wire/backups/` and its path is printed.)

```bash
memory-wire connect claude-code     # -> claude-code  wired         SessionStart, UserPromptSubmit, Stop
memory-wire doctor --db /path/to/agents.db
```

`hook session-start|prompt|stop` is what those host entries invoke: a bank preamble
plus a recall, the recall lines for a submitted prompt, and one compacted retained
line at session end. All three are best-effort by contract — a down server prints
the local framing and exits 0, never an error in front of the model. The preamble
reads the bank's `background`/`preamble`/`system_prompt` from
`GET /banks/:id/config` when that key exists — for the bank the hook resolves
from its own working directory (the git worktree's top-level name, else
`memory-wire`), which `MEMORY_WIRE_BANK` does not override.

`doctor` is one screen: endpoint, bank, store size and health, bank/memory row
counts, server state. `server up` means `/health` answered 2xx **and** the body
was exactly `ok`, so a foreign process on the port cannot pass for the server.
`--strict` exits nonzero when the server or store is unusable, and a store whose
pages fail `PRAGMA integrity_check` is reported `unreadable` rather than counted.
It never creates the store it reports on — pointing `--db` at a nonexistent path
reports `missing` without creating the file — and on an *existing* store it folds
the WAL into the main file on the way past (best effort, `PASSIVE`, never
blocking), which is what makes a `cp` taken straight afterwards a usable backup.
Point it at the same file you gave `serve --db`, or the report describes a store
the server never touches.

## Bank config

`GET`/`PUT /banks/:id/config` is a whole-object store-and-serve: unknown keys
(`retain_mission`, `recallPromptPreamble`, anything a future version adds) come
back byte-for-byte, and a malformed value in a key this build *does* act on falls
back to the default rather than taking recall down.

```bash
curl -sS -X PUT localhost:8888/banks/demo/config -H 'Content-Type: application/json' \
  -d '{"recallMaxTokens":128,"retainTags":["ops"],"retain_mission":"own the release"}'
```

Two keys change behavior. `recallMaxTokens` is the bank's default recall budget —
a recall that omits `budget` uses it, an explicit `budget` still wins.
`retainTags` is added to every later retain in that bank, after the request's own
tags, so a bank-wide default can never displace a tag you named.

Tags work without any config: `retain` accepts `{"content", "tags"}` and `recall`
accepts `{"query", "tags"}` to restrict the search to memories carrying **any** of
them (normalized, deduped, capped at 20; the cap keeps the head, preserving your
ordering).

## Forgetting (opt-in TTL)

Memory-wire has no background process, so nothing is ever deleted on a timer.
A bank that wants to forget says so in its own config, and
`memory-wire sweep` is what carries that out:

```bash
curl -sS -X PUT localhost:8899/banks/scratch/config -H 'Content-Type: application/json' \
  -d '{"ttl_days":30}'

memory-wire sweep --dry-run   # what would go, and how much
memory-wire sweep             # actually forget it
```

`ttl_days` is the retention window in days; **absent means nothing ever
expires**, so forgetting is off until a bank asks for it and every other bank is
listed as `skipped` and never touched. There is no way to switch forgetting on
globally, and no way to make it happen without running the command — a policy
nobody asked for is a policy that must not fire.

A sweep deletes every memory in a bank whose `created_at` is *older* than
`now - ttl_days`; one created exactly at the cutoff survives. Deleting the row is
all it takes: the `memories_ad` trigger retires its FTS entry and the tags
cascade, so the index can never cite a memory that is gone. Each bank is one
transaction, so a bank that fails leaves every other bank swept and nothing half
done. Output is per-bank, then a total, and the exit code is 0:

```text
memory-wire sweep
  demo        ttl 7d  cutoff 2026-09-20T13:52:46.190Z  would delete 3
  scratch     skipped (no ttl_days)
total 3 would be deleted
```

`--dry-run` is the same arithmetic with the delete left out. A value this build
cannot use — a string, a negative, a window no date can reach — reads as *no
policy* rather than failing the write, because a typo must not be able to make
forgetting happen (or stop a config from being stored at all). The column that
carries the policy is added by migration, so a store written by an earlier
version opens, reads, and sweeps exactly as it was.

## MCP

`memory-wire mcp` serves MCP over stdio — JSON-RPC 2.0, newline-delimited, stdout
only for protocol, logs to stderr. `--bank` sets the default bank, falling back to
`$MEMORY_WIRE_BANK` and then to `memory-wire`; a per-call `bank` argument
overrides both.

| Tool | Writes? | Arguments |
|---|---|---|
| `memory_retain` | yes | `content` (required), `bank?`, `context?`, `tags?` |
| `memory_recall` | no | `query` (required), `bank?`, `budget?`, `tags?` |
| `memory_reflect` | no | `query` (required), `bank?` |
| `memory_bank_config_get` | no | `bank?` |

Every tool declares `destructiveHint: false`; `memory_retain` is the only one
without `readOnlyHint: true` — a repeated retain of the same content in the same
bank writes nothing and returns the id that already holds it, but a repeated
retain of *different* content is a second memory. Each tool is a thin adapter onto
the same `MemoryService` methods
the HTTP routes call — retrieval, redaction, and budgeting are not reimplemented
for MCP, so the two surfaces cannot drift. An unknown tool name is a JSON-RPC
`MethodNotFound`; a tool that runs and fails returns `isError: true` with the same
opaque message the HTTP surface returns, never a crash.

```json
{ "mcpServers": { "memory-wire": {
  "command": "memory-wire", "args": ["mcp", "--bank", "my-project"] } } }
```

Bank-in-path scoping (`/mcp/:bank`) belongs to a future HTTP transport and is not
built; the argument-resolution rule above is already the one that mode needs.

## How it works

```text
serve        → SQLite store (bank-isolated) + seeded `default` bank
retain       → ensure bank exists → redact_pii(content, context) → put
               (+ tags ∪ bank retainTags); byte-identical content already in the
               bank returns that row's id, no second copy
               document_id set → supersede that document's prior row in the same tx
recall       → candidate pool: newest 200 rows ∪ the BM25 hits (bounded, so a
               recall never reads the whole bank) → FTS5 BM25 (LIMIT 50 in SQL)
               + overlap rank (cap 200) → RRF k=60
               → token-budget trim (truncate, never drop) → take 100
               budget precedence: request > bank recallMaxTokens > 2000
               format=full → {id, score, content} instead of bare strings,
               score = the fused RRF value
reflect      → recall(2000).first() cited by id
memories     → SELECT … LIMIT ? OFFSET ? (50/0 default, 500 cap) → {id, content, created_at}
stats        → COUNT(*) + COUNT(DISTINCT tag) + MIN/MAX(created_at) for the bank
mcp          → same four calls, over JSON-RPC on stdio
```

`document_id` turns retain into an upsert: with the default
`"update_mode":"replace"`, each call deletes that document's previous row and
inserts the new one **inside one transaction**, so a reader never sees the new
revision beside the old tag set and a failure never leaves the document with no
revision at all. Three replaces under one `document_id` leave exactly one row,
holding the last content. `tags` are replaced wholesale on each write, so a
revision does not inherit the previous revision's tags. The third field,
`"update_mode":"append"`, only ever *adds* a memory — it never extends a row — so
it is **not usable for repeated writes**: a second append under a `document_id`
that already holds a row is refused with `409 document already exists; use
update_mode=replace` and the first row survives, unchanged. Treat `append` as
single-use per document id; if you are accumulating revisions, `replace` is the
mode that works today. Neither field appears in any response — a document id is a
write-side handle only, so keep it yourself.

Library in `0.2.0`: `src/{api,store,recall,capture,embed,lib,memory}.rs` —
`capture.rs` supplies the redaction filter. Binary-side modules (`connect`,
`doctor`, `guidelines`, `hooks`, `http`, `mcp`, `paths`, `sweep`) live in
`src/main.rs`; the HTTP surface is eight route groups (`/health`, and
`/{retain,recall,reflect,config,memories,memories/:mid,stats}` under `/banks/:id`)
plus four MCP tools.

There is no consolidation ladder and no vector stream in this build. Neither has
a scheduler, a flag, or a stored column, and neither is reachable from a request:
what ships is FTS5 BM25 plus token-overlap, fused by RRF. `src/embed.rs` is the
one deliberate exception — it holds the dependency-free cosine/rank kernel that a
future vector stream would consume, with no embedder and no producer behind it.

**Lifecycle ops are routes now — custody, not relevance.** `recall` answers "what is
relevant"; these answer "what did I actually store, and how do I take it back":
`GET /banks/:id/memories` pages a bank (`?limit=` default 50, capped at 500;
`?offset=` default 0) and `GET`/`DELETE /banks/:id/memories/:mid` fetch or drop one
by id. Every memory those routes serve carries `created_at` (RFC 3339 UTC,
stamped at insert), and `context` appears only when the memory actually has one —
so a memory stored without capture context does not claim an empty one. A missing
id is `404 unknown memory`; deleting an already-deleted one is `200` with
`{"deleted":false}`, which makes the delete idempotent rather than an error.
`GET /banks/:id/stats` returns `{memories, tags, oldest, newest}` for the bank.

**The unknown-bank rule.** An operation that *names one resource* answers `404`
when that resource is absent, so a typo cannot read back as a plausible empty
answer: `GET`/`PUT /banks/:id/config` on a bank that was never created is
`404 unknown bank`, and `GET /banks/:id/memories/:mid` is `404 unknown memory`. An
operation that reads a *collection or an aggregate* answers `200` with the empty
answer, because banks are created implicitly by their first `retain` and a
pre-retain recall is normal, not exceptional: `GET .../memories` → `[]`,
`GET .../stats` → zeros and `null`s, `POST .../recall` → `[]`, `POST .../reflect` →
`"no relevant memories"`. `POST .../retain` creates the bank it names, and
`DELETE .../memories/:mid` stays idempotent — `200` with `{"deleted":false}`.
One corollary: **`PUT /banks/:id/config` requires an existing bank** and will not
create one, so configure a bank after its first retain.

`tests/{e2e,scale,backup}.rs` · `examples/{longmemeval,coding_life,scale_sweep,soak}.rs`.

## Reproduce

```bash
./eval/download.sh            # official LongMemEval-S, 264 MB, once
cargo run --release --example longmemeval -- --data eval/data/longmemeval_s_cleaned.json --n 500
cargo run --release --example coding_life
cargo run --release --example scale_sweep
cargo test && cargo clippy --all-targets --locked -- -D warnings
```

The eval artifacts are generated from `--release`, the profile that ships; a
default-profile run reports the same retrieval metrics but roughly 2–5× the
latency, so quote the profile with any latency number.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/<owner>/<repo>/main/install/get-memory-wire.sh | sh
# sh get-memory-wire.sh --db <path>     # echoes the serve line with that --db
# sh get-memory-wire.sh --uninstall     # removes the binary only, keeps the db
```

Detects `linux`/`macos` × `x86_64`/`aarch64` and refuses anything else;
`MW_REPO`, `MW_VERSION`, `MW_INSTALL_DIR`, and `MW_LOCAL_ASSET` override the
defaults. **No GitHub release is published yet** (crate is `0.2.0`), so
`MW_REPO` has no working default — build with `cargo build --release` until one is
cut. Agent-facing runbook with per-step assertions: `INSTALL_FOR_AGENTS.md`.

## Roadmap

Shipped: `GET`/`PUT /banks/:id/config` (merged into the `serve` router) · the
`connect <agent>` / `hook` / `doctor` / `seed` CLI subcommands · the MCP stdio
server (`rmcp`, 4 tools, +1.6 MB binary) · tag filtering on retain and recall ·
the lifecycle routes (`GET .../memories`, `GET`/`DELETE .../memories/:mid`,
`GET .../stats`) with `created_at` on every served memory · `format: "full"` on
recall · `document_id` upsert on retain (replace works; a repeated `append` is a
`409`) · dedup-on-retain on the content hash (a repeated retain of the same
content returns the existing id; document-scoped retains are exempt so the
append/replace split is untouched) · a bounded recall candidate window, so recall
cost no longer grows with the bank · per-bank `ttl_days` plus the explicit
`memory-wire sweep` that enforces it (off by default, no scheduler) · a loopback
`--addr` default for `serve`, with a one-line stderr warning on any other bind ·
graceful shutdown on SIGINT/SIGTERM, draining in-flight requests.

Not built yet: a 4-tier consolidation ladder (promote/evict/retention had no
scheduler and no caller; the code was removed rather than left as a model of
behaviour that does not run) · vectors at retain + stored embeddings +
cross-encoder rerank, which would close the −2.2pp to agentmemory hybrid (the
`embed` feature and its ONNX dependency were removed; the cosine/rank kernel
survives, unused) · a `DELETE` for a whole bank (one memory at a time only) · `tags` in
the lifecycle responses (they serve `{content, created_at, id[, context]}`; tag
filtering stays on `recall`) · `document_id` visible in any response · a `backup`
subcommand (the `sqlite3 .backup` recipe is the interface) · Postgres/
pgvector backend · LLM-backed `reflect` synthesis · bank-in-path scoping for MCP
over HTTP (`/mcp/:bank`; stdio only today) · `connect` for hosts outside the five
detected (claude-code, codex, copilot-cli, cursor, opencode). Gaps that need live
LLMs/providers (Hindsight system-evals, agentmemory quality/real-embeddings) are
tracked in `docs/BENCHMARK.md`.

## Subagents

Project rule: delegate only with the `space-bunny` model.
