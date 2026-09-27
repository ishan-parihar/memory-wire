# Docs consistency audit — 2026-09-27

Phase 4 (H6 + H7) of `.omo/plans/production-readiness.md`. Every command below
was executed against a scratch `serve --db /tmp/canon.db` on this tree, with
`target/release/memory-wire` (`0.1.0`, built after R1–R3 and H4/H5), so the next
release has something to diff against instead of re-deriving it.

**Method.** Start a server on a scratch `--db`, run each documented command
verbatim, and paste what came back into the `Expect:` line. Flags are cross-read
against `src/main.rs:39-110` (the `clap` derive) and the HTTP claims against a
live server; anything that could not be executed on this machine is marked
*out of scope* rather than assumed. Files swept: `README.md`,
`INSTALL_FOR_AGENTS.md`, and `plugin/skills/memory-wire/SKILL.md`.

**Result: 12 wrong claims fixed, 2 doc files left alone, 6 open items.**

## 1. Fixed — were wrong, now observed output

| # | Where | Was | Now | Evidence |
|---|---|---|---|---|
| 1 | `README.md` CLI | "Six subcommands" + a table of six; `seed` "is **not built**" | Seven, with `seed` and its four flags | `memory-wire --help` lists `info serve connect hook doctor mcp seed` (+ clap's `help`); `seed --help` shows `--commits/--transcripts/--bank/--db` |
| 2 | `README.md` roadmap | "Not built yet: a `seed` subcommand" | moved to **Shipped** with the other subcommands | `memory-wire info` prints a `seed` line; `seed --db /tmp/mw-seed.db` runs and exits 0 |
| 3 | `INSTALL_FOR_AGENTS.md` step 9 | "the binary ships six subcommands" + a six-line `--help` transcript | seven + the verbatim transcript above | same |
| 4 | `INSTALL_FOR_AGENTS.md` "Not in this build" | row asserting `seed` → `error: unrecognized subcommand 'seed'` | row deleted; replaced by a `backup`-subcommand row saying there is deliberately none | `seed` runs |
| 5 | `README.md` "How it works" | a repeated `append` under a `document_id` "fails the uniqueness constraint and comes back `500 storage error`" | `409 document already exists; use update_mode=replace`, first row unchanged | observed `200` then `409`; the `plan` row still held `plan v1` |
| 6 | `README.md` bounds, `INSTALL` step 4 | "a budget of 1–3 tokens returns `[]`" | the top hit cut to the cap, unlabelled: `budget: 1` → `["auth"]`, `2` → `["auth use"]`, `3` → `["auth uses jo"]`; the marker lands at 4 | `src/recall.rs:135-140` gates the marker on `room > 12`; observed all four budgets |
| 7 | `README.md` connect | "`connect claude-code` → `wired  SessionStart, UserPromptSubmit, Stop`" | `claude-code  wired         SessionStart, UserPromptSubmit, Stop` (host column is real), plus `already-wired -`, the `left alone:` case and the `FAILED … left untouched` case | run against a clean `HOME`, then against this machine's real `~/.claude/settings.json` (restored afterwards) |
| 8 | `README.md`, `INSTALL` step 9 | "`connect` installs the three lifecycle hooks into every agent host" | three hooks into claude-code / codex / copilot-cli; an MCP entry into cursor (`mcpServers`) and opencode (`mcp`) | `HOME=/tmp/… memory-wire connect` on an empty home, all five hosts |
| 9 | `README.md`, `INSTALL` step 9 | "Put `{"background": "..."}` in the config" (implying step 6's `demo`) | the bank the hook resolves from its **own cwd** (git worktree name, else `memory-wire`); `MEMORY_WIRE_BANK` does not override it | `hooks.rs:182-193`; `hook session-start` printed `Project P: keep the rail thin.` only after the *resolved* bank's config carried the key |
| 10 | `README.md`, `INSTALL` step 7 | lifecycle row described as `{"id":…,"content":…,"created_at":…}` | `{"content":…,"created_at":…,"id":…}` — the serialised key order | observed response bytes |
| 11 | `README.md` tests line | `tests/{e2e,scale}.rs` | `tests/{e2e,scale,backup}.rs` (H7.2 added one) | `ls tests/` |
| 12 | `INSTALL` error contract | "`?limit=` that is not a number → axum's own deserialize message"; "unknown path → axum default" | the two literal bodies, so a reader can match them | `?limit=abc` → `400 Failed to deserialize query string: invalid digit found in string`; `/nope` → `404` empty body |

## 2. Verified — were right, re-stated from observed output

| Claim | Observed |
|---|---|
| `/health` body is exactly `ok`, plain text, `200` | `ok` / `200` |
| Retain returns `{"id":"<uuid>"}`; `sed` extracts it | `{"id":"3b3a923f-b3ce-48ed-8829-cf071d6accb3"}` |
| Redaction on write, both fields | `auth uses jose middleware; key [REDACTED:api_key]; mail me at [REDACTED:email]` |
| Recall default is a bare array of strings | `["auth uses jose middleware; key [REDACTED:api_key]; mail me at [REDACTED:email]"]` |
| `budget: 0` → `[]`; `budget: 5` → a 20-char string | `[]`; `["auth use…[truncated]"]` (8 + 12 chars) |
| `MAX_RESULTS` is 100 | 150-memory bank, `budget: 1000000` → exactly 100 |
| `memories` paging: 50 default, 500 cap, `offset` honoured | 50 / 150 (`?limit=99999`) / 5 (`?limit=10&offset=145`) |
| Reflect is top-hit citation, no LLM | `"based on [3b3a923f-…]: auth uses jose middleware; …"` |
| Config: `{}` before, echo on PUT, byte-for-byte on GET, `404 unknown bank` for a bank never created (GET **and** PUT), PUT does not create | all four observed |
| `retainTags` applies to a later retain; `tags` filter finds it | `["jose inherits the bank default tag"]` |
| Delete is idempotent | `{"deleted":true}` then `{"deleted":false}`, both `200` |
| `document_id` + `replace` leaves one row with the last content | `[{"content": "spec v2", …}]` |
| `format:"full"` → `{content,id,score}`; default stays strings | `[{"content":"spec v2","id":"272de770-…","score":1}]` |
| **Unknown-bank rule** (R2): single-resource → 404, collection/aggregate → 200 + empty | `config` GET/PUT `404 unknown bank`; `memories` → `[]`; `stats` → `{"memories":0,"tags":0,"oldest":null,"newest":null}`; `recall` → `[]`; `reflect` → `"no relevant memories"`; `memories/:mid` → `404 unknown memory`; `DELETE memories/:mid` → `200 {"deleted":false}` |
| **Append → 409** (R1), first row byte-unchanged | `409 document already exists; use update_mode=replace`; `plan` row still `plan v1` |
| Other 400s: `invalid bank id`, `invalid content`, `invalid bank config` | observed verbatim |
| `serve` seeds a `default` bank at boot | `select id,name from banks` → `default\|default` on a fresh db |
| `--db` optional; default `$XDG_DATA_HOME/memory-wire/memory.db`, relative `XDG_DATA_HOME` ignored per spec, parent dirs created | `XDG_DATA_HOME=relative/path` + fake `HOME` → `$HOME/.local/share/memory-wire/memory.db` created |
| `hook --help` lists `session-start`, `prompt`, `stop` | yes |
| All three hooks: exit 0, best-effort against a down server | `session-start` against `http://127.0.0.1:1` printed the local framing, exit 0; `prompt` printed the recall section for a bank with content; `stop` retained `session s1 ended; transcript /tmp/nope.jsonl (0 B)` |
| `connect --uninstall --guidelines` refused; unknown host is a clap error | exit 1 / exit 2, both messages observed |
| MCP: 4 tools, JSON-RPC 2.0, stdout protocol-only, stderr logs; unknown tool `-32601` | `tools/list` + `tools/call` over a pipe; stderr empty; `{"code":-32601,"message":"unknown tool: nope"}` |
| MCP annotations: every tool `destructiveHint:false`, only `memory_retain` lacks `readOnlyHint:true` | `{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false,"openWorldHint":false}` for retain, `readOnlyHint:true` for the other three |
| Installer: `MW_REPO`/`MW_VERSION`/`MW_INSTALL_DIR`/`MW_LOCAL_ASSET`; `--db` echoed in the serve line; `--uninstall` removes the binary only | `MW_LOCAL_ASSET` end-to-end installed `memory-wire 0.1.0` to a temp dir and printed `… serve --addr 127.0.0.1:8888 --db /tmp/mw-inst.db`; uninstall left the db alone |
| Installer: "No GitHub release is published yet", so `MW_REPO` has no working default | default `memory-wire-rs/memory-wire` → `memory-wire: no published release for memory-wire-rs/memory-wire.` exit 1 |
| Installer refusals | unknown flag / `--db` with no value / uninstall-when-absent / asset without the binary → all exit 1 with a named reason |
| Footprint: binary 8,593,368 B (**8.2 MB** MiB) | `stat -c %s target/release/memory-wire` → `8593368` |
| Footprint: idle RSS **10.3 MB** after 1 retain | `ps -o rss` 3 runs → 10368 / 10320 / 10500 KB (mean 10396 = 10.2 MiB) |
| Footprint: 10k storage 2,375,680 B (**2.3 MB**) | `eval/SCALE_SWEEP.md` 10000 row → `2375680` |
| Benchmark retrieval metrics unchanged | `eval/RESULTS.md` overall row: R@5 93.0 / R@10 97.4 / R@20 99.6 / NDCG@10 83.5 / MRR 83.9; `eval/CODING_LIFE.md` hit-rate 100%, R@5 100% |

## 3. Added (H7) — not a correction, a new claim

| Claim | Where | Evidence |
|---|---|---|
| Back up online with `sqlite3 "$MW_DB" ".backup '$MW_DB.bak'"`; restore with `rm -f` of the sidecars then `cp` | `INSTALL` "Back up and restore" | `tests/backup.rs` runs exactly these commands: `.backup` → delete a row → restore → 3 rows back, the row findable again by FTS, the post-backup config gone |
| A naive `cp` of a live WAL store is **not** a backup, and a naive restore is worse than none | same section | same test: the `cp` yields a file with no `memories` table at all; `cp bak db` with the stale `-wal` beside it replays the WAL and returns the live rows, while the documented restore returns the backup's |
| `doctor` runs `PRAGMA integrity_check` and `--strict` fails on a damaged page | `README`, `INSTALL`, `src/doctor.rs` | `doctor::tests::a_store_with_a_damaged_page_should_fail_strict` overwrites a page past the header and asserts `store unreadable: integrity_check` + nonzero strict |
| `doctor` folds an existing store's WAL on the way past (`PASSIVE`, never blocking) | same | `src/doctor.rs::fold_wal`; the missing-store test still asserts nothing is created |
| There is deliberately no `backup` subcommand (H7.4) | `README` CLI + roadmap, `INSTALL` "Not in this build" | `Cmd` in `src/main.rs:47-110` has no `Backup` variant |

## 4. Open — needs a decision this phase is not allowed to make

1. **`plugin/skills/memory-wire/SKILL.md` still carries three claims this sweep
   disproved, and `plugin/` is out of the write scope for Phase 4.** All three
   are one-line fixes: "A *second* `append` under an existing `document_id`
   returns `500 storage error`" (it is `409`); "budgets of 1–3 tokens return
   `[]` rather than a stub" (same defect as #6 — it returns the top hit cut to
   the cap, unlabelled); and "Note the asymmetry: `memories` and `stats` answer
   `200` with `[]` or zeros for a bank that was never created, while `config`
   still `404`s" (replace with the unknown-bank rule, the way README and INSTALL
   now phrase it). Everything else in that file was verified and is correct,
   including both MCP annotation claims and the eight-route list.
2. **`eval/SCALE_SWEEP.md` and `eval/CODING_LIFE.md` disagree with the README's
   Benchmarks table on latency, not on retrieval.** README says scale p50
   "2.6→90ms" and coding-life "p50 1.2ms"; the artifacts say search p50
   2863 µs → 115949 µs (2.9→116 ms) and 1311 µs (1.3 ms). Retrieval metrics are
   identical everywhere, which is what Phase 2 gated on, and this sweep was told
   not to alter benchmark metrics — so the two latency figures need whoever owns
   H2 to say which artifact is authoritative before either number moves.
3. **The pre-retain idle RSS figure.** README explains the 8.7 → 10.3 MB move as
   "8.7 MB *before* the first retain"; three fresh measurements here read 8416 /
   8368 / 8544 KB (8.2 MiB) before the first retain. The post-retain number Phase
   3 pinned is unaffected.
4. **The under-load RSS figure is harness-shaped.** Phase 3's 21.6 MB
   (22,116 KB) is the pinned number and the README states the method as "5,000
   retains + 200 recalls". Re-running exactly that with 8-way concurrent
   `curl` (`xargs -P 8`) read 36,900 KB, because the store serialises on one
   `Mutex<Connection>` and eight in-flight requests hold more per-request state.
   Sequential and concurrent loads are not the same measurement; the doc should
   say which one it means.
5. **`install/get-memory-wire.sh:56-57` prints the *default* data directory in
   its uninstall message**, not the `--db` path the user actually passed. The
   docs now say so explicitly, but the script is the thing that is misleading,
   and `install/` is outside this phase's write scope.
6. **`docs/BENCHMARK.md` predates the MCP server, the `connect`/`hook`/`doctor`
   subcommands and FTS5** — its tool matrix still reports `MCP (rmcp)` and
   `CLI bank/memory/mental-model/fs/explore` as *NOT IMPLEMENTED*, and its
   footprint section says "no release-binary size". None of that is true of this
   tree, but the file is a dated audit record (2026-09-27, debug build) rather
   than a current claim, and it is outside this phase's write scope. It should
   either be re-run or banner-stamped as historical so the next reader does not
   take its matrix as current.

## 5. How to re-run this audit

```bash
cargo build --release
nohup ./target/release/memory-wire serve --addr 127.0.0.1:18899 --db /tmp/canon.db &
export MW=http://127.0.0.1:18899
# …walk INSTALL_FOR_AGENTS.md top to bottom, then:
memory-wire --help && for c in serve connect hook doctor mcp seed; do memory-wire $c --help; done
cargo test --locked && cargo clippy --all-targets --locked -- -D warnings
```

Note two things that will bite a re-run: `doctor` probes `127.0.0.1:8888` unless
`MEMORY_WIRE_URL` is set, and `connect` writes to the real `~/.claude` — point
`HOME` at a scratch directory to sweep it safely.

---

# Final regression gate — 2026-09-27, `0.2.0`

The pre-cut sweep above is the **baseline this round is measured against**; its
numbers are left exactly as recorded, including §2's 8,593,368 B / 10.3 MB /
2,375,680 B rows and the `MW_LOCAL_ASSET` install record. What follows is the
post-cut state: what changed, what it cost, and the two places where the code and
the brief disagree on purpose.

## 6. Retrieval did not regress — measured, not assumed

Two changes could have moved ranking: the `recall.rs` overlap scorer (one
reusable scratch buffer per candidate pool instead of a `String` per token per
document) and `RECALL_POOL_LIMIT` bounding the overlap candidate pool to the
newest 200 rows ∪ the BM25 top-50. Corpus size decides whether the second could
have mattered, so it is stated per suite:

| Suite | Corpus | Over the 200-row bound? | Verdict |
|---|---|---|---|
| longmemeval | 38–62 sessions/question (mean 47.7), 500 questions | **no** — max 62 | pool bound cannot apply |
| coding-life | 15 sessions, 15 queries | **no** | pool bound cannot apply |
| `tests/scale.rs` | 5,000 memories, 100 signals inserted *first* | **yes**, and adversarially | this is the probe |
| `eval/SCALE_SWEEP.md` | 240 / 1k / 5k / 10k | **yes** from 240 up | latency + storage |

So the two quality suites **cannot** detect a pool-bound regression, and this file
says so rather than letting a green longmemeval run stand in for the bound. The
bound is covered by `tests/scale.rs`, whose 100 signals are inserted before 4,900
distractors and therefore sit *outside* the newest-200 window at query time — they
can only enter the pool through the BM25 union clause. It held at 20/20.

| Metric | Baseline | Measured (release) | Delta |
|---|---|---|---|
| longmemeval R@5 | 93.0% | 93.0% | **0.0** |
| longmemeval R@10 | 97.4% | 97.4% | **0.0** |
| longmemeval R@20 | 99.6% | 99.6% | **0.0** |
| longmemeval NDCG@10 | 83.5% | 83.5% | **0.0** |
| longmemeval MRR | 83.9% | 83.9% | **0.0** |
| coding-life P@5 / R@5 / hit-rate | 24.0% / 100% / 100% | 24.0% / 100% / 100% | **0.0** |
| `tests/scale.rs` R@1 / R@5 @5k | 20/20 · 20/20 | 20/20 · 20/20 | **0.0** |

The longmemeval match is not an aggregate coincidence: all **2,500** per-question
values (`eval/results.json`, five metrics × 500 questions) are identical to the
committed artifact, and all six question-type slices match. The rewritten scorer
is bit-identical in aggregate and per-question.

Latency improved everywhere it was measured, which is the point of both changes:

| Latency | Baseline | Measured (release) | Delta |
|---|---|---|---|
| longmemeval p50 | 44 ms (debug) | 6 ms | −86% |
| coding-life p50 | 1311 µs (debug) | 266–616 µs over 5 runs | profile-dominated |
| scale 240 / 1k / 5k / 10k p50 | 2863 / 11828 / 51358 / 115949 µs (debug) | 588 / 893 / 2578 / 4903 µs | −79% / −92% / −95% / −96% |
| `tests/scale.rs` p50 / p95 @5k | 45 / 48 ms (debug) | 13–17 / 16–23 ms | −65% |
| under-load recall p50 / p95 | 45 / 132 ms | 6 / 10 ms | −87% / −92% |
| under-load RSS | 22,116 KB | 12,964 KB | **−9,152 KB (−41.4%)** |

Debug-profile latency is **not** a usable gate: five identical runs of the same
binary span 1.96–2.46 ms on coding-life, against the 1311 µs the old artifact
recorded. The eval artifacts are now generated from `--release` and say so in
their headers; a debug run reports the same retrieval metrics at 2–5× the
latency. `eval/CODING_LIFE.md`'s **1.3 ms was the measured figure and README's
1.2 ms was simply wrong** — no run produced 1.2 ms.

## 7. Where the code and the brief disagree — kept, deliberately

1. **A malformed `update_mode` returns `400 invalid content`.** The frozen error
   table has no field-specific message, so an unknown mode string rides the
   content error even though the content was fine. Observed:
   `{"content":"spec v1","document_id":"plan","update_mode":"upsert"}` →
   `400 invalid content`. A wrong *type* is different and better: axum's
   deserializer answers first, `422 Failed to deserialize the JSON body …
   update_mode: invalid type: boolean`, before any of our code runs. Kept as-is:
   the error table is frozen and adding a field-specific code is a contract
   change, not a doc fix. A repeated `append` under an existing `document_id` is
   still the documented `409`.
2. **The recall `contents` map is one pass, not zero maps.** Eliminating it would
   mean changing `trim_to_budget`'s signature (`src/recall.rs:177`, still
   `pub fn … contents: &HashMap<&str, &str>`), and that signature is frozen. The
   overlap scorer itself is now buffer-reusing — one owned scratch buffer for the
   whole candidate pool, hoisted query index and dedup flags, no per-document
   `HashSet` — and it carries a differential A/B against the previous
   per-document implementation. The map survives as the one remaining allocation,
   and eliminating it is a follow-up that needs the unfreeze.

## 8. Removals, and one construction worth keeping

- **`fastembed` and its ONNX runtime are gone**, not deferred: no `embed`
  feature, and no `fastembed` / `ort` / `tokenizers` entry in `Cargo.lock`. The
  vector arm was formally DECLINED, so PLAN.md §2/§4 still naming `fastembed`
  describe the goal architecture, not this build. `src/embed.rs` survives as the
  dependency-free cosine/rank kernel with no embedder behind it.
- **`consolidate.rs`, `Observation`, `MentalModel` and `DedupWindow` are
  deleted** — the ladder had no scheduler and no caller, the types had no
  constructor, and `DedupWindow` no longer appears anywhere in the tree. Deleted
  rather than left as a model of behaviour that does not run. PLAN.md §1 and the
  README roadmap say so.
- **The shutdown test builds its `Expect: 100-continue` request by hand**
  (`src/main.rs:1626`, `:1674`) instead of sleeping. The ordering is a round-trip,
  not a duration, so the test is deterministic rather than timing-dependent.

## 9. Footprint, re-measured — including one number that moved the wrong way

| Figure | Pinned | Measured | Delta |
|---|---|---|---|
| release binary | 8,593,368 B (8.20 MiB) | **8,816,120 B** (8.41 MiB) | **+222,752 B (+2.59%)** |
| idle RSS, post-retain | 10,396 KB mean | 10,704 KB mean (10.45 MiB) | +308 KB (+3.0%) |
| idle RSS, pre-retain | 8,443 KB mean | 9,123 KB mean (8.91 MiB) | +680 KB (+8.1%) |
| 10k storage, settled | 2,375,680 B | **3,194,880 B** (3.05 MiB) | +819,200 B (+34.5%) |
| 10k storage, WAL unflushed | *not previously measured* | 7,442,440 B (7.10 MiB) | new figure |
| tests | 20 (phase-4 record) / stale 205, 222 | **217** | — |

**The binary grew, and the brief expected it to shrink.** The `fastembed`
removal is real, but it is more than offset by the MCP SDK landing between the
`0.1.0` and `0.2.0` measurements — `rmcp` + `schemars` cost more than `fastembed`
ever did. Recorded as measured; no baseline was moved to hide it.

The 10k store grew for a legible reason. A drop-index-and-`VACUUM` ladder on a
real 10k store attributes it to the two indexes the `document_id` upsert and
content-hash dedup introduced — `idx_memories_bank_hash` 565,248 B (17.7%) and
`idx_memories_bank_doc` 106,496 B (3.3%) — plus the `content_hash` and
`document_id` columns themselves. The pre-existing `idx_memories_bank_time` is
344,064 B, 10.8% (README previously said 10.4%, measured on a 5k store).

**And the storage figure was understating the store all along.** The store runs in
WAL mode and `eval/SCALE_SWEEP.md` reports the *main file* only. At 10,000
memories the real on-disk footprint while the process holds it open is
7,442,440 B (main 3,194,880 + `-wal` 4,214,792 + `-shm` 32,768), collapsing to
3,194,880 B at a clean `wal_checkpoint(TRUNCATE)`. The README now publishes the
settled figure and names the transient one, so neither reader is misled.

## 10. Open items from the sweep above, now closed

- §4.2 (the 1.2 vs 1.3 ms latency contradiction) — **closed**: the artifact was
  right, README was wrong, both now carry a `--release` figure and a profile note.
- §4.3 (pre-retain idle RSS) — **closed**: re-measured at 9,123 KB mean and
  re-pinned, with the pre/post distinction kept explicit.
- §4.4 (under-load RSS is harness-shaped) — **closed by stating the harness**:
  the README now says one process, sequential, over HTTP. The 8-way-concurrent
  reading (36,900 KB) remains true of a concurrent load and is left as a known
  difference rather than a contradiction.
- §4.6 (`docs/BENCHMARK.md` predates MCP) — **closed**: the 0.1.0 matrix is kept
  verbatim as a dated record and banner-marked historical, with a current `0.2.0`
  matrix added below it.
- §4.1 (`plugin/skills/memory-wire/SKILL.md`) — **closed**: the three disproved
  claims are fixed (the `500` is a `409`; budgets 1–3 return the top hit cut to
  the cap, not `[]`; the unknown-bank rule replaces the `memories`/`stats` vs
  `config` asymmetry note).
- §4.5 (`install/get-memory-wire.sh:56-57` prints the default data dir in its
  uninstall message) — **still open**; `install/` was out of this phase's write
  scope and the script is still the misleading thing.

