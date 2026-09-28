> # ⚠️ TWO RECORDS IN ONE FILE — READ THIS BEFORE QUOTING ANY NUMBER
>
> **§1–§5 are the PRE-CUT `0.1.0`-era sweep. They are kept verbatim as the
> baseline that the final gate was measured against, and they are NOT current
> claims.** Every count, size and version in §1–§5 describes `0.1.0`:
>
> - §1 "Now" column says the CLI has **seven** subcommands. This tree ships
>   **eight** — `sweep` was added at `0.2.0`.
> - §2 records the binary at **8,593,368 B** and idle RSS at **10.3 MB**. Both
>   were superseded; §9 carries the current figures.
> - §2 records 10k storage at **2,375,680 B**. Also superseded — §9.
> - §2's `MW_LOCAL_ASSET` row is a past-tense install record: it really did
>   install `memory-wire 0.1.0`, and it is true of that run.
> - §4's six open items were closed at the cut; §10 says which.
>
> **§6–§10 are the post-cut `0.2.0` state** (final regression gate, 2026-09-27):
> 217 tests, release binary **8,816,120 B**, idle RSS 10,704 KB post-retain,
> 10k store 3,194,880 B settled / 7,442,440 B with the WAL unflushed, and the
> retrieval metrics in `eval/RESULTS.md`. Those are the numbers as they stood at
> the `0.2.0` cut.
>
> **§11–§12 are the current state** (measured 2026-09-27 and 2026-09-28, and
> published in `v0.3.0`). Read those: **237** tests, release binary
> **8,836,032 B**, idle RSS 10,716–11,080 kB post-retain, 10k store 3,022,848 B
> settled / 7,237,496 B with the WAL unflushed, 4-connection WAL read pool, FTS5
> `detail=none`. The earlier "the tree is ahead of the published release" banner
> is retired for the same reason `docs/VERSIONS.md` retired its copy: `v0.3.0`
> is published and `Cargo.toml` says `0.3.0`, so the version string describes the
> tree as well as the release. §12 also carries the honest negatives and the two
> places where an earlier claim in this file or in the README was simply wrong.
>
> **Do not edit a §1–§5 number to make it current.** They are the baseline, and
> §9 is the single place movement is recorded — that is what makes the deltas
> legible. Add a row to §9 instead.
>
> **CI runs no benchmarks and no doc sweeps** (deliberate — Actions quota; see
> `.github/workflows/ci.yml`), so nothing re-verifies any figure here
> automatically. `docs/CONSISTENCY.md` is a record, not a live check.

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


---

# SQLite layer: `synchronous`, statement cache, page cache — 2026-09-28

Three changes at the pragma/statement level in `src/store.rs`, appended rather
than folded into §9 so the pre-cut baseline and the `0.2.0` gate above stay
exactly as recorded. Nothing else moved: no schema object, index, column, API
signature, or ranking behaviour was touched, and `eval/results.json` is
byte-identical to the committed artifact (see §11.4).

**Method, and the caveat that governs every number below.** This box is shared
and was under sustained external load the whole session — `loadavg` between 42
and 98 on 24 cores, and per-round means for the *same binary* varied by up to 3x.
Absolute ops/s is therefore not a usable statistic on this machine; **p50 is**,
because it is a median over 3,000–45,000 samples. Before/after runs were
interleaved (base, change, base, change, …) so drift in that load lands on both
arms, and each configuration was also A/B'd **within one binary** where the
choice allowed it. The `3x` mean swings in the raw logs are the load, not the
change; the p50s do not move that way.

## 11.1 D1 — `PRAGMA synchronous=NORMAL` — the whole win

`configure()` set three pragmas and let SQLite default `synchronous` to `FULL`,
which fsyncs the WAL on every commit. Adding `NORMAL` is the one durability trade
in the diff, and it is documented on the function: at `NORMAL` in WAL mode a
**power loss or OS crash can lose transactions committed in the last few
seconds**; a process crash cannot (the WAL is in the page cache and the next
opener replays it), and the database is never corrupt either way.

| Write path, 9,000 samples per arm | ops/s base | ops/s after | p50 base | p50 after | p50 gain |
|---|---|---|---|---|---|
| `put` @1k corpus | 207.3 | 988.9 | 2,681.7 us | **266.5 us** | **10.1x** |
| `put_tagged` @1k (1 tag) | 141.6 | 1,322.4 | 2,953.6 us | **321.3 us** | **9.2x** |
| `put` @10k corpus | 195.7 | 697.1 | 2,355.3 us | **283.3 us** | **8.3x** |
| `put_tagged` @10k | 241.1 | 756.1 | 2,450.0 us | **339.1 us** | **7.2x** |
| 1,000-row bulk load (the `seed` path) | 140–174 rows/s | 1,145–2,029 rows/s | — | — | **8–11x** |

The brief's raw-SQL figure was 1.98 ms -> 0.099 ms per commit (19.9x). The store's
`put` does more than the bare insert — SHA-256 of the content, the FTS5 insert
trigger, three index updates, and a `find_duplicate` seek — so the *ratio* is
smaller and the *absolute* p50 lands at ~270 us rather than ~99 us. Same
direction, same mechanism.

**Batched write, as a control rather than a claim.** `Store` has no batch-insert
API, so a multi-row transaction was priced through raw `rusqlite` against the
same schema and the same three original pragmas: one commit per 1,000 rows was
already 16.0 tx/s (~60 us/row) before and 16.7 tx/s after, i.e. **unchanged**,
because a per-commit fsync amortized over 1,000 rows was never the cost. That is
the point of D1: it moves exactly the per-commit cost and nothing else.

RSS: 10,720 KB -> 10,729 KB idle (3 runs each). No change.

## 11.2 D3 — `prepare_cached` on 6 of 13 sites; 7 left on `prepare`, with reasons

`Rows::drop` calls `sqlite3_reset` (rusqlite `src/row.rs:109`), so a row iterator
that goes out of scope always returns a *reset* statement to the cache. That
makes the `get()`/`get_bank_config()` "read one row and return" shape safe to
cache — verified directly rather than assumed, since an un-reset cached
statement would continue the previous cursor and answer with a stale row.

| Read path, 45,000 samples per arm | ops/s base | ops/s after | p50 base | p50 after |
|---|---|---|---|---|
| `get` @1k | 72,241 | 193,824 | 12.8 us | **4.2 us** (3.0x) |
| `get` @10k | 54,692 | 197,449 | 10.7 us | **4.7 us** (2.3x) |
| `list_page` @1k | 8,056 | 11,013 | 86.7 us | 71.6 us (1.21x) |
| `list_page` @10k | 2,350 | 2,924 | 321.1 us | 287.0 us (1.12x) |

Converted: `list_conn`, `list_page_conn`, `get`, `get_bank_config` (on both the
retain and recall path), `bank_ttls`, and the per-tag `memory_tags` insert inside
`put_doc`. Six static SQL strings against rusqlite's 16-slot LRU, so nothing
evicts anything.

Left on `prepare`, each with the reason now in a code comment:

| Site | Why not |
|---|---|
| `recall_pool_conn` | SQL text carries one `?` per hit id and per tag: up to ~1,000 distinct strings against 16 LRU slots. Caching it thrashes and pins 16 prepared statements alive. |
| `keyword_search_conn` | Same shape, narrower: 21 keys (0–20 tags) against 16 slots. The common tag-free shape is one key, but a tag-filtered recall would evict it. |
| backfill `SELECT … content_hash IS NULL` | Marker-gated: runs at most once per database, ever. Nothing to amortize against. |
| `has_column` (`PRAGMA table_info`) | ~5 calls per process *open*, never on a request path, and the text interpolates the table name. |
| 3 test helpers (`tags_of`, `bank_columns`, `has_background_column`) | Test-only. Editing test source for zero production benefit is diff noise. |

On the write path D3 is small and honestly so: `put_tagged` p50 321.3 -> 322.7 us
@1k and 339.1 -> 321.9 us @10k, because after D1 a single commit costs ~270 us and
one saved parse is a small fraction of it. **D3's real win is the `get` route
(2.3–3.0x), not the retain.** `recall` p50 was 11,365.6 -> 9,966.9 us @1k and
25,079.4 -> 22,572.5 us @10k — flat, as it must be, since that code is unchanged;
it is the control that says the arm was not simply quieter.

**Not done, and it is the bigger fish:** `conn.execute(...)` and `conn.query_row(...)`
prepare internally on every call and are on the retain path (`put_bank`,
`find_duplicate`, the tag delete, `set_bank_config`, the sweep). Roughly six
uncached parses per retain survive this change. Converting them is the same
one-word edit at ~8 more sites, and it is a separate change from this one.

## 11.3 D4 — `cache_size` and `mmap_size`: one shipped, one refused

RSS by `ps -o rss` against a `serve` on a scratch `--db`, read after `/health` +
1 retain (the README's own idle definition), mean of 3 runs per setting:

| `cache_size` / `mmap_size` | idle RSS | vs unset | RSS under load | vs unset |
|---|---|---|---|---|
| unset / 0 (shipped) | **10,639 KB** | — | **13,540 KB** | — |
| -8192 (8 MB) / 0 | 10,728 KB | +89 KB | 13,664 KB | +124 KB |
| -32768 (32 MB) / 0 | 10,744 KB | +105 KB | 13,720 KB | +180 KB |
| unset / 64 MB | 10,716 KB | +77 KB | **15,472 KB** | **+1,932 KB** |
| unset / 256 MB | 10,679 KB | +40 KB | **15,876 KB** | **+2,336 KB** |
| -8192 (8 MB) / 64 MB | 10,819 KB | +180 KB | 15,556 KB | +2,016 KB |
| -32768 (32 MB) / 256 MB | 10,689 KB | +51 KB | 15,868 KB | +2,328 KB |

**`mmap_size=0` is a refusal, and a large one.** Mapped database pages count
toward RSS. 64 MB and 256 MB cost **+1.9 MB and +2.3 MB under load — +15% / +17%**
— for no throughput gain at any setting measured: with a 32 MB cache, `put` p50
271.0 -> 275.8 us, `put_tagged` 314.5 -> 320.4 us, and a 1,000-row batched
transaction 19.2 -> 15.2/s. Unlike a cache ceiling, an mmap region is paid in
resident pages whether or not it earns its keep.

**`cache_size` stays at the default (-2000), and the reason is structural.**
Same binary both arms, 9,600 samples per arm, 4 alternating rounds, 10k corpus:

| | default | 32 MB |
|---|---|---|
| `put` p50 | 287.7 us | 286.1 us (**-0.6%**) |
| `put_tagged` p50 | 330.7 us | 320.6 us (-3.1%, better in 3 of 4 rounds) |
| idle RSS | 10,639 KB | 10,744 KB (**+105 KB**) |

An earlier 2-round sweep read this as -5.0% / -8.4%; the 4-round same-binary
A/B does not reproduce it, and the 4-round reading is the one to believe. So the
honest summary is: **at most a 3% median gain on one metric, and a reproducible
+105 KB of RSS.** A bigger page cache would pay off if recall read the whole
bank; it does not. The candidate pool is the newest 200 rows plus the BM25 hits,
so the read working set is bounded however large the bank grows and 2 MB already
holds it. 8 MB was measured too (-1.8% / -3.9%) for +89 KB — the same RSS within
noise, for less than half an already-unresolvable gain. A knob that costs RSS
and buys nothing measurable is a knob to leave alone; it is stated explicitly so
the choice is pinned rather than inherited.

**Shipped config, RSS re-measured (4 runs):** idle 10,760 / 10,652 / 10,580 /
10,748 KB, **mean 10,685 KB (10.43 MiB)**; under load 13,316 KB. Against
§9's pinned 10,704 KB post-retain, and against the 10,720 KB this same harness
measured on the **unmodified** `0.2.0` binary on this box, that is **-19 KB** —
inside the run-to-run band (10,512–10,884 KB across every configuration
measured today). **The README's headline 10.5 MB idle-RSS claim is not
invalidated and was left untouched.**

> **SUPERSEDED — see §12.4.** "Left untouched" was true when this section was
> written. The read pool has since added four more connections' worth of page
> cache, and the claim no longer holds: measured 2026-09-28 over 7 rounds, idle
> post-retain is **10,716–11,080 kB, mean 10,911 kB (10.65 MiB)**, and the
> README now publishes that range instead of a point value. This section is left
> exactly as recorded so the page-cache deltas below stay legible.

## 11.4 Gates — verbatim, and one thing the eval run broke

| Gate | Result |
|---|---|
| `cargo test --locked` x3 | **217 passed, 0 failed**, 3/3 runs: 117 lib + 94 bin + 2 backup + 2 e2e + 1 scale + 1 doc-test |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | clean, exit 0 |
| `cargo doc --no-deps --all-features --locked` | 0 warnings |
| longmemeval R@5 / R@10 / R@20 / NDCG@10 / MRR | **93.0 / 97.4 / 99.6 / 83.5 / 83.9 — all unchanged** |

The retrieval match is not an aggregate coincidence: `eval/results.json` is
**byte-identical** to the committed artifact after the run (`git diff` empty;
500 records, 5 metrics each = 2,500 values), and all six question-type slices
match. Nothing about ranking can have moved, because nothing that feeds it did.

**`cargo run --release --example longmemeval` has a side effect worth flagging.**
Running the documented reproduce command **overwrote `eval/RESULTS.md`**:
it deleted the hand-written methodology note (the one stating that the 200-row
candidate pool never binds on this suite, 38–62 sessions per question, which
§6 above depends on) and replaced the pinned 5–7 ms p50 column with this run's
11–14 ms — a load artifact of a box at `loadavg` 84. It was reverted
(`git checkout eval/RESULTS.md`); the retrieval metrics it produced were
identical, so nothing was lost but the note. **The generator should not be
rewriting a curated note, and it should not overwrite a latency column from a
single run.** Left alone here: `examples/longmemeval.rs` was outside this
change's write scope and another task owns `examples/`.

**Binary:** 8,816,120 B -> **8,824,696 B**, +8,576 B (+0.097%). Still "8.4 MB".
The README's exact byte count was updated to match; its RSS row was not, because
§11.3 shows the claim still holds.

---

# §12 — the current published state (`v0.3.0`) — measured 2026-09-27 and 2026-09-28

§11 recorded the SQLite pragma/statement work. §12 records everything else that
landed on top of it, every number that moved, and — deliberately, at length —
every measurement that **did not** work out. Nothing here is a new claim about
the product's shape; it is the re-pin and the honest negatives.

## 12.0 What this section is, and the machine it was measured on

**Everything in §12 is in the published `v0.3.0` tag and release.**
`Cargo.toml` says `0.3.0` and `v0.3.0` is cut, so the version string describes
the tree and the release alike. The earlier framing here — "the tree on `main` is
substantially ahead of the published `v0.2.0` tag and release, and `Cargo.toml`
still says `0.2.0`" — was true when written and is now false on both counts; the
3db88b4 removal of the E3/E4 levers and 3e902ff's version bump are both inside
the tag. `main` is one oracle-rerank harness ahead of the tag, which touches no
number below.

**The machine, because it governs every latency number in this section.** The
box is shared and was under sustained load from unrelated work for the whole
period — a fleet of `ultra_granular_001_regional_hpo.py` processes at ~91% CPU
each plus an API service at ~470%. Observed `loadavg` across this work ranged
**20.9 to 48.1 on 24 cores**, and per-round means for the *same binary* varied by
up to 3x. Therefore:

- **Structural measurements are trustworthy and are pinned exactly**: byte
  counts, page counts, row counts, test counts, recall metrics, index sizes.
- **Wall-clock latency is not.** It is reported here as a measured **range with
  its load window**, never as a point value. Where a before/after exists it is
  interleaved across >=3 alternating rounds.
- The battery's per-harness load is recorded in `/tmp/mw-battery.log` for this
  session; the committed artifacts each carry their own date/profile/machine
  provenance line, and the four latency-heavy ones now carry an explicit
  "do not pin this" note in their own text.

## 12.1 Feature work — the complete list

Everything here is code that already existed before this record was written; this
section is the index, not the changelog.

| # | Change | What it is | Where it is recorded |
|---|---|---|---|
| 1 | TTL sweep | per-bank `ttl_days` plus the explicit `memory-wire sweep` that enforces it. Off by default, no scheduler, one transaction per bank, FTS entry retired by the `memories_ad` trigger. A value this build cannot use reads as *no policy* rather than failing the write | README "Forgetting"; `src/sweep.rs` |
| 2 | Graceful shutdown | SIGINT/SIGTERM stop accepting and drain in-flight requests. The shutdown test builds its `Expect: 100-continue` request by hand rather than sleeping, so the ordering assertion is a round-trip, not a duration | §8; README CLI |
| 3 | Loopback-default bind | `serve --addr` defaults to `127.0.0.1:8899` because there is no authentication. A non-loopback address still binds and says so once on stderr first | README CLI |
| 4 | Dead-code removal | `consolidate.rs`, `Observation`, `MentalModel`, `DedupWindow` deleted; the `fastembed` dependency, the `ort`/`tokenizers` transitive entries and the `embed` cargo feature removed. Only the dependency-free `cosine`/`rank_by_cosine` kernel survives, with no embedder and no producer behind it | §8; `docs/VERSIONS.md` §3 |
| 5 | Poisoned-lock fix | a poisoned writer lock used to panic on the next `lock()`, so one panic in any writer took the store down permanently. Fixed so a poisoned lock is recovered and reported | `src/store.rs` |
| 6 | Swallowed-error fix | storage errors that were being dropped rather than propagated; every one now surfaces as the same opaque `500 storage error` the HTTP contract promises, so a failure cannot read as an empty success | README error contract |
| 7 | `UpdateMode` enum | `document_id` upsert made explicit: `replace` (the default, works repeatedly) and `append` (single-use per document id — a second one is a `409` and the first row survives). Replaces stringly-typed mode dispatch | README "How it works" |
| 8 | `created_at: Option<String>` | the insert path stamps RFC 3339 UTC or does not; the type now says so, and a memory stored without capture context no longer claims an empty one | README lifecycle routes |
| 9 | Recall scorer rewrite | token-overlap ranking rewritten so the candidate pool is newest-200-rows ∪ BM25 top-50, fused by RRF k=60 with a 100-result cap. Recall cost stopped growing with the bank | `eval/BENCH_RECALL_CURVE.md` |
| 10 | `synchronous=NORMAL` | §11.1. No fsync per commit. **8-10x on the write path, not the predicted 19.9x** | §11.1, §12.2 |
| 11 | `prepare_cached` | 6 of 13 static-SQL read sites; the other 7 stay on `prepare` with a recorded reason. `get` p50 12.8 → 4.2 us @1k | §11.2 |
| 12 | 4-connection WAL read pool | writer-first with spill-on-contention. **Kept because the no-pool control collapses; the cost is RSS** | §12.3 |
| 13 | FTS5 `detail=none` | no positional data in the index. **Index 434,176 → 262,144 B, −39.6%. No latency win is possible or claimed** | §12.5 |
| 14 | `doctor` store integrity | `PRAGMA integrity_check`; a damaged page is reported `unreadable` and fails `--strict` rather than being counted | §3 |
| 15 | Five benchmark harnesses + tracked fixture | `bench_footprint`, `bench_write`, `bench_recall_curve`, `bench_concurrency`, `bench_coldstart`, plus a working soak artifact; the 8 KB coding-life fixture is tracked so a clean clone can run the suite | `eval/`; `eval/README.md` |
| 16 | Generator owns its own artifacts | three harnesses were destroying curated prose in their own committed output. Fixed — see §12.6 | §12.6 |

## 12.2 The predicted 19.9x that reproduced as 8-10x

`PLAN.md` quoted a raw-SQL measurement of 1.980 ms/commit at
`synchronous=FULL` against 0.099 ms/commit at `NORMAL` and called it "a 19.9x
headroom from one pragma". **19.9x is not a number this store produces, and it
was never going to be.** The raw figure is a bare single-row `INSERT`; the
store's `put` additionally does SHA-256 of the content, the FTS5 insert trigger,
three index updates and a `find_duplicate` seek, so the per-commit *fraction* that
a pragma can remove is smaller and the absolute p50 lands ~270 us rather than
~99 us.

Measured, interleaved, 9,000 samples per arm (full table in §11.1):
`put` p50 2,681.7 → 266.5 us @1k, 2,355.3 → 283.3 us @10k; `put_tagged`
2,953.6 → 321.3 us @1k, 2,450.0 → 339.1 us @10k; 1,000-row bulk load
140–174 → 1,145–2,029 rows/s. That is **7.2x-10.1x**, and it is what the docs now
say. The direction and the mechanism are the same; the number is smaller, and the
19.9x figure has been struck from `PLAN.md` rather than left as a prediction.

## 12.3 The read pool: kept for throughput, paid for in memory

`READ_POOL_SIZE = 4`, chosen by measurement rather than taste:

| Pool size | 1.5x scaling floor at 64 clients | Soak RSS |
|---|---|---|
| 2 (writer only) | **1.23x — FAILS** | — |
| **4 (shipped)** | 1.89x — passes | within ceiling |
| 8 | nothing further to gain | **23.7–24.2 MB against a 23.8 MB ceiling** |

**The measurement that justifies keeping it** is the no-pool control. With the
pool, aggregate in-process recall throughput at 64 clients is **772.0 ops/s
(760.9–791.8)** against **408.7 ops/s (351.3–464.9)** for 1 client — a **1.89x**
gain, clearing the 1.5x floor. The no-pool control collapses to **243–496 ops/s
at 64 clients**, i.e. *below* its own 1-client figure, and fails the gate at
0.87x. That control is the whole argument: without it a 1.89x number means
nothing.

**The measurement that justifies the "writer-first" policy is a memory one, and
it points the other way.** Under a 16-client mixed soak, total RSS is
**19.0–20.9 MB writer-first against 25.3–27.4 MB pool-first** against a 23.8 MB
ceiling. So the shipped design is deliberately *not* the one that maximises
parallel recall: a reader takes the writer's connection when it can, and spills
to a pool neighbour only under contention. Idle RSS is unaffected either way.
Today's committed soak run (`eval/SOAK.md`, loadavg 47.6) shows RSS 7.6 → 27.2 MB
against the 23.8 MB ceiling and **PASS** — the gate is on RSS *growth* against
the page-cache ceiling, not on the total.

**A 4-5x single-client read regression was predicted and did not reproduce.** A
second connection ought to mean a second, cold page cache: the 2,000-memory store
is **729,088 B**, which fits inside *any* connection's default 2 MB page cache, so
there is no cold-cache penalty to pay and no regression to observe. The mechanism
was real; its precondition was not met by a corpus this small. Recorded as a
prediction that did not reproduce, not as a finding.

## 12.4 RSS: re-pinned as a range, and the two settings that were refused

| Figure | Previously pinned | Measured 2026-09-28 | Load | Rounds |
|---|---|---|---|---|
| idle RSS, pre-retain | 9,123 kB (8.91 MiB) | **9,196–9,536 kB, mean 9,351 kB (9.13 MiB)** | loadavg 25.0 | 7 |
| idle RSS, post-retain | 10,704 kB (10.45 MiB) | **10,716–11,080 kB** (7-round mean 10,911 kB / 10.65 MiB, plus the committed harness's own 11,080 kB read) | loadavg 21.0–25.0 | 8 |
| RSS under load (5k retains + 200 recalls, HTTP, sequential) | 12,964 kB (README) / 13,316 kB (§11.3) | **13,084–13,360 kB, mean 13,249 kB (12.94 MiB)** | loadavg 20.9–29.9 | 3 |
| release binary | 8,824,696 B | **8,836,032 B** (8.43 MiB) | — | exact |
| 10k storage, settled main | 3,194,880 B | **3,022,848 B** (2.88 MiB) | — | exact |
| 10k storage, settled total | 3,227,648 B | **3,055,616 B** (2.91 MiB) | — | exact |
| 10k storage, WAL unflushed | 7,442,440 B | **7,237,496 B** (6.90 MiB) | — | exact |
| index bytes per 1k memories | 322,764 B | **305,561 B** | — | exact |
| tests | 217 (117 lib) | **237** (137 lib + 94 bin + 2 backup + 2 e2e + 1 scale + 1 doc-test) | — | exact |

**"Idle RSS is flat" is no longer a true sentence, and the README now says so.**
The old pin sits at the very bottom of today's band, so the claim moved up by
roughly 200 kB; the plausible cause is the read pool's extra page caches. This was
*not* an interleaved pre-pool/post-pool A/B — the pre-pool binary was not rebuilt
for this measurement — so the honest claim is "the range moved up by about
200 kB", not an attributed delta.

**The structural numbers were bit-stable across two independent runs, which is
the check that the wall-clock split is real.** `eval/BENCH_FOOTPRINT.md` was run
twice today at `loadavg` 34.4 and 21.1 and returned *identical* byte counts both
times — 7,237,496 unflushed, 3,055,616 settled total, 3,022,848 settled main,
305,561 per 1k — while its RSS rows moved (11,020 → 11,080 kB) and
`eval/BENCH_RECALL_CURVE.md`'s p50 column moved 2-3x against a fixed set of
quality metrics. Byte counts and recall metrics are pinnable; latency is not.

**The page-cache tuning was tried and rejected, and the rejection is the result.**
Same binary both arms, 9,600 samples per arm, 4 alternating rounds, 10k corpus:
`cache_size=32MB` moved `put` p50 287.7 → 286.1 us (**−0.6%**) and cost a
reproducible **+105 KB** of idle RSS. `mmap_size=64MB` and `256MB` cost
**+1,932 KB and +2,336 KB under load — +15% and +17% RSS — for no throughput gain
at any setting measured**, because mapped database pages count toward RSS whether
or not they earn their keep. An earlier 2-round sweep read `cache_size` as
−5.0%/−8.4%; the 4-round same-binary A/B does not reproduce it, and the 4-round
reading is the one to believe. Both knobs therefore stay at the SQLite defaults.
A bigger page cache would pay off if recall read the whole bank; it does not —
the read working set is bounded however large the bank grows, and 2 MB already
holds it.

## 12.5 `detail=none`: a real storage win, and a latency win that cannot exist

`memories_fts` is now created with `detail=none`, gated by a one-shot
`fts_detail_none` marker in the existing `schema_markers` table so the migration
runs once per database and never re-runs. Byte-deterministic on every measurement.

| Object | Before | After | Delta |
|---|---|---|---|
| whole FTS index | 434,176 B | **262,144 B** | **−172,032 B (−39.6%)** |
| `memories_fts_data` | 331,776 B | **159,744 B** | −172,032 B |
| `memories_fts_docsize` | 94,208 B | 94,208 B | unchanged |

`docsize` is unchanged **because `bm25()` needs it** — that is the reason this
trade is possible at all, and the reason it stops there.

**There is no latency win here and none is claimed.** The tempting story is that
a smaller index is a faster index. It cannot be, for this store: recall's
candidate pool is bounded by construction — the newest 200 rows ∪ at most 50 BM25
hits — so there is **no full index scan for `detail=none` to accelerate**. The
`eval/BENCH_RECALL_CURVE.md` artifact is the proof, and it is the same shape
before and after: the scan behind the pool *is* the FTS5 `MATCH` walk, and that
walk still grows with the bank (see the curve's "bounded fusion, unbounded scan"
paragraph). The correct statement is: **−172,032 B of resident and on-disk index
at 10k memories, and no latency change, because the index is not the bottleneck
and never was.**

## 12.6 Three generators were destroying their own committed output

This is the defect class that motivated the work, and it had **three** instances,
not one.

1. **`examples/longmemeval.rs` deleted the methodology paragraph** in
   `eval/RESULTS.md`. The generator emitted a heading, a one-line methodology
   note and a table; everything between the note and the table was hand-written
   and was silently lost on every run. §11.4 recorded the incident and left it
   unfixed as out-of-scope. **Fixed**: the paragraph is now part of the
   generator's template, and the two figures inside it that could go stale — the
   per-question haystack range and mean, and the count of per-question values —
   are **computed from the run** rather than hand-pinned, so they cannot
   contradict the run that wrote them. Verified: a fresh run reproduces the
   committed file with exactly three differences, all of them intended — the run
   date, the newly added "do not pin the p50 column" caveat, and the p50 column
   itself. The computed session range reproduces the hand-written one exactly:
   **38-62 sessions, mean 47.7**.
2. **`examples/scale_sweep.rs` had the same defect.** Its committed
   `eval/SCALE_SWEEP.md` carried a hand-written paragraph explaining that the
   `DB bytes` column is the main file only, with three hard-coded byte counts in
   it. A bare run deleted the paragraph, and when re-run the byte counts would
   have been a month stale. **Fixed**: the paragraph is in the template, and the
   three stale numbers are **gone from it entirely** — the WAL and
   post-checkpoint figures are delegated to `eval/BENCH_FOOTPRINT.md`, which
   measures both on every run. A second copy of a byte count is a second thing to
   go stale.
3. **`examples/coding_life.rs` had it too**, and this one had already fired: its
   committed artifact carried a run-date line and a "p50 moves ±40% run to run"
   caveat that the generator did not emit. Regenerating it during this work
   deleted them, which is how the third instance was found. **Fixed** the same
   way, with the caveat text in the template.

**The `--out-md` default, and the approach taken.** Every harness defaulted
`--out-md` to a path under `eval/`, so a bare `cargo run --release --example
soak` silently replaced a reviewed artifact. The fix is **one shared rule**:
`bench_common::out_md(args, name)` returns the caller's `--out-md` when given, and
otherwise writes to a scratch file under `$TMPDIR` and prints where. Updating a
committed artifact now requires naming it (`--out-md eval/SOAK.md`).

*Why this approach and not the alternative.* The other option — keep the
`eval/` default and require an explicit `--out-md` to write at all — was rejected
because it makes the *common* case a failure and the *destructive* case easy: a
caller who wants a quick look at the numbers would have to learn a flag before
their first run, and the flag's absence would then be the thing that stops a
silent overwrite. Writing to a temp path by default means a bare run is always
harmless and always tells you where it put the file; the cost of being wrong is
one extra flag on the run you actually meant to keep. One definition, shared by
all eleven harnesses, because five separate copies of this rule is how the rule
drifted.

**Also fixed while in the examples**, both cases of a generated artifact asserting
something false about the build it was generated from: `bench_write`'s header
claimed the store "never sets `synchronous`", which stopped being true when
`synchronous=NORMAL` shipped (it now says what the store *does* set, and that the
artifact's numbers are the *after* side of §11.1); and the `soak` and
`bench_concurrency` headers described the store as "a single `Mutex<Connection>`",
which stopped being true when the read pool landed.

## 12.7 Two claims that were simply wrong

**"Recall throughput measured flat at ~60 rec/s whether 1 or 64 concurrent clients
hit the server."** This was quoted in `PLAN.md` as the motivating measurement for
the whole of Phase B, and it is an **artifact of how it was measured**: over HTTP,
with Python clients, on a box at `loadavg` 105. In-process, on this tree, a single
client does **408.7 ops/s (351.3–464.9)** — not 60. The 60 is the Python client
and the scheduler. The *latency* growth in that same measurement (17.2 ms → 55.4 →
220.6 → 1012.3 ms) was real, and it is why the pool exists; the *throughput*
claim was not. `PLAN.md` now quotes the original and retracts it in place.

**`docs/BENCHMARK.md`'s "MCP tools (rmcp) — NOT IMPLEMENTED" row.** This is a
true record of the `0.1.0` audit and a false statement about the current tree,
and it was sitting in a table a reader could take at face value. **It has been
labelled in place, not deleted**: the row reads `NOT IMPLEMENTED *(true at 0.1.0
only; shipped at 0.2.0 — §5)*`, the section heading now carries the version and
the date, and §1's banner says so too. A historical number that gets deleted is a
number nobody can check; one that gets labelled is a number nobody can
misread.

## 12.8 Other stale numbers, and what happened to each

| Where | Was | Now | Why |
|---|---|---|---|
| `PLAN.md` status | "217 tests green ... release binary 8,816,120 B" | kept as the *reference point* the plan was measured against, with a pointer forward | it is the pre-plan baseline; moving it would destroy the delta |
| `PLAN.md` Phase A | "13 `.prepare(` call sites, zero `prepare_cached`" | 6 of 13, 7 with recorded reasons | §11.2 |
| `PLAN.md` Phase B | "flat ~60 rec/s" | retracted in place with the in-process number | §12.7 |
| `PLAN.md` Phase B | "19.9x headroom" | struck, replaced with 8-10x | §12.2 |
| `PLAN.md` Phase B | the Porter-stemmer morphology probe | **not done**; the probe was not re-run, so 1-of-3 / 6-of-7 is the only evidence and is unversioned | the storage win on the same axis shipped instead |
| `docs/BENCHMARK.md` §5 | 217 tests, binary 8,816,120 B | 237 tests, binary 8,836,032 B | re-measured |
| `docs/VERSIONS.md` §3 | "INCOMPLETE — omits `rmcp`" | **complete**, regenerated; `fastembed`/`ort`/`tokenizers` confirmed absent | §12.4 |
| `docs/BENCHMARK_SCALE.md` | the 13-17 ms / 16-23 ms p50 pair, profile unverified | **left exactly as recorded**, with its own standing note that the profile is unverified | not re-measured here; inventing a replacement would be worse than the ambiguity |
| `docs/CONSISTENCY.md` §2 | 8,593,368 B, 10.3 MB, 2,375,680 B, 20 tests | left verbatim | §1-§5 is the baseline §9's deltas are computed from |
| `eval/CODING_LIFE.md` | p50 "266 us" in a table whose own prose said 364-616 us | regenerated, self-consistent, with the range caveat in the template | the two halves of that file disagreed with each other |
| `eval/SCALE_SWEEP.md` | 3,194,880 B and 7,442,440 B hand-written in prose | regenerated; the stale copies removed from the template | §12.6 |
| `eval/BENCH_RECALL_CURVE.md` | p50 1,366 → 97,986 us | p50 2,443 → 131,642 us at loadavg 36 | **quality rows are bit-identical** (R@1 78.1/75.0/81.2/75.0, R@5 100% at every size, 32 gold in BM25 at every size — these are the **pre-E1** figures; the shipped weights read R@1 71.9/75.0/75.0/71.9, R@5 96.9/84.4/90.6/90.6%, see §13); only wall-clock moved. This is the clearest single demonstration of the structural/wall-clock split |
| `README.md` "RSS under load" | "recall p50 6 ms / p95 10 ms" | **not re-pinned**; the equivalent HTTP round trip measures 13.8-32.2 ms per `curl`, which is the client, not recall. The store's own recall figures are cited instead | §12.4 |

## 12.9 Gates — verbatim, 2026-09-28

| Gate | Result |
|---|---|
| `cargo test --locked` | **237 passed, 0 failed**: 137 lib + 94 bin + 2 backup + 2 e2e + 1 scale + 1 doc-test |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | clean, exit 0 |
| `cargo doc --no-deps --all-features --locked` | 0 warnings |
| `cargo run --release --example longmemeval -- --data eval/data/longmemeval_s_cleaned.json --n 500` | R@5 93.0 / R@10 97.4 / R@20 99.6 / NDCG@10 83.5 / MRR 83.9 — **the pre-E1 control row.** This run was taken while the overlap stream still voted at 1.00; the shipped `overlap: 0.25` reads 97.2 / 98.6 / 99.6 / 88.2 / 89.2, which is what `eval/RESULTS.md` publishes |

**The retrieval match is not an aggregate coincidence.** All 500 per-question
values for `recall_any_at_5`, `recall_any_at_10`, `recall_any_at_20`, `mrr` and
`ndcg_at_10` — 2,500 numbers, plus `question_id` and `question_type` — were
compared field by field against the pre-change `eval/results.json` and are
**identical, 0 differences out of 3,500 compared values**. All six question-type
slices match too.

`eval/results.json` itself is deliberately **not** claimed byte-identical: it
embeds a per-question `latency_ms` recorded from the wall clock, which is
load-dependent by construction and differs between runs. The per-question
*metric* fields are the invariant, and those are what is asserted. Quoting the
byte-identity of a file that carries a timestamp would be quoting a coincidence
as a guarantee.

> **Correction, 2026-09-28.** This paragraph used to give that field's median as
> "6 ms in the committed artifact's run, 8 ms today, 14 ms at loadavg 44". **None
> of the three is a committed number.** `eval/results.json` is gitignored
> (`.gitignore:13`) — it is a local byproduct, not an artifact of record — and the
> copy in this working tree now reads a median of 9 ms, so the "8 ms" was stale
> as well as unciteable. The committed latency for this suite is the `p50 ms`
> column of `eval/RESULTS.md`, which reads **11 ms** for the same run, and that
> is a different quantity from the median of a per-question field: one is the
> harness's own p50 over the whole run, the other is the median of 500
> per-question observations. The figures are deleted rather than re-pinned
> because there is no committed artifact to re-pin them from.

---

## §13 — Idle-box re-measurement of the recall curve (2026-09-28)

Every latency figure in §12 was measured on a box this record elsewhere bounds at
**loadavg 20.9–48.1 on 24 cores** (§12.0), under unrelated
`ultra_granular_001_regional_hpo.py` processes and `hindsight-api`, and was
published as a range for that reason. The machine later rebooted and came back
quiet, so the recall curve was re-measured properly.

> **Correction, 2026-09-28.** This paragraph used to say §12 was measured at
> "26–112 on 24 cores". That window belongs to no §12 figure: §12.0 records
> 20.9–48.1, §12.4's per-figure windows are 20.9–29.9, 21.0–25.0 and 25.0, and
> the two heaviest `eval/` runs it cites are `eval/BENCH_FOOTPRINT.md` at
> loadavg 34.4 and 21.1 and `eval/SOAK.md` at 47.6. **The 26–112 window is
> unaccounted for and has been dropped rather than re-attributed.** The two load
> windows that *are* recorded — §11's own session at 42–98 and §12's at 20.9–48.1
> — are left exactly as recorded in their own sections; both are per-session
> observations whose raw log (`/tmp/mw-battery.log`) is not in the repository, so
> neither can be re-derived from here.

Three `bench_recall_curve` rounds at load **8.36–8.61**, p50 in µs. The
`§12 loaded range` column is the only one of the two ranges here whose source run
is not in the repository — it appears in no other file and no longer matches
`eval/BENCH_RECALL_CURVE.md` at any generation. It is left exactly as recorded and
flagged rather than reconciled, because the run that produced it cannot be
re-derived from this tree.

| Memories | r1 | r2 | r3 | idle median | §12 loaded range | inflation |
|---|---|---|---|---|---|---|
| 1,000 | 1,015 | 1,010 | 1,132 | **1,015** | 1,137–2,519 | 1.1–2.5× |
| 10,000 | 6,129 | 5,918 | 6,454 | **6,129** | 6,394–17,197 | 1.0–2.8× |
| 50,000 | 35,959 | 33,412 | 34,999 | **34,999** | 36,142–87,910 | 1.0–2.5× |
| 100,000 | 70,489 | 71,331 | 73,839 | **71,331** | 80,169–172,403 | 1.1–2.4× |

**The measurement noise, not just the level, was load.** Within-arm spread at
100k collapsed from 2.9× (58,444–172,403) to **1.05×** (70,489–73,839). At
1k–50k the idle spread is ±4–6%. Any §12 comparison that rested on a difference
smaller than ~2.5× was not resolvable and should be read as "no difference".

Retrieval quality was invariant across all nine runs: R@1 78.1 / 75.0 / 81.2 /
75.0 and R@5 100.0% at every size, all 32 gold rows in BM25's top-50. Latency
grows **70× for a 100× larger bank** — sublinear, and consistent with §12's
shape claim once the load is removed.

> **These nine runs were all pre-E1**, taken while the overlap stream still voted
> at weight 1.00. The figures above are a correct record of *that* configuration on
> that date and are kept for that reason — but they are **not** the shipped
> numbers, and Phase E1 moved them. The shipped configuration scores R@1
> **71.9 / 75.0 / 75.0 / 71.9** and R@5 **96.9 / 84.4 / 90.6 / 90.6%** at
> 1k/10k/50k/100k, which is what the committed
> `eval/BENCH_RECALL_CURVE.md` reads. The invariance claim was always scoped to a
> fixed configuration; E1 is what made the configuration move, not a counterexample
> to the invariance.

**The table above is the only record of the idle-box runs, and it is not a
committed artifact.** Those three rounds were not written to `eval/`, so nothing
under version control carries them. The committed
`eval/BENCH_RECALL_CURVE.md` as it now stands was generated at load
**24.64 33.47 31.95** and reads p50 **2,164 / 15,172 / 54,500 / 102,450 µs** at
1k/10k/50k/100k, with R@1 71.9/75.0/75.0/71.9 and R@5
96.9/84.4/90.6/90.6% on its `shipped` rows. That artifact says of itself that "the
absolute microseconds still need an idle box", which is what the table above is
the missing half of.

> **Correction, 2026-09-28.** This paragraph used to read: "`eval/BENCH_RECALL_CURVE.md`
> was regenerated at load 14.23 and reads 1,090 / 7,019 / 38,366 / 79,205 µs. It
> sits inside the idle band and above its median, which is the expected direction
> for a busier box." **None of that describes the committed file.** The artifact in
> the tree is a later generation, from a run at load 24.64/33.47/31.95 with the
> shipped 0.25 overlap weight, and it reads the p50 column quoted above. The
> 14.23 generation and its figures are gone from the repository, so the
> sentence was describing a file that does not exist rather than a stale number
> in one that does.

**This does not reopen §12's conclusions.** Every §12 finding that was structural
— FTS index 434,176 → 262,144 B, the pool's soak-RSS effect, the no-pool control
collapsing at 64 clients, the write-path commit shape — was measured in bytes or
ops and is unaffected. What the load *did* corrupt was the latency layer, and the
honest statement is that the latency layer was under-powered and is now better,
not that any earlier latency conclusion was wrong.

## §14 — The dev set lands: replication, a null result, and R@1 (2026-09-28)

Tree `90c5510`, `loadavg` 55.92 on 24 cores. **No latency or RSS figure in this
section is a measurement** — the box was not quiet, and nothing here needs one.

### 14.1 `overlap: 0.25` replicated on independent data, at a third of the size

`eval/LOCOMO.md`, from `examples/locomo.rs`, on 1,531 LoCoMo queries that never
chose the value (9 of 1,540 excluded, empty `gold_ids`; `gold_answers` is not used
— the harness never generates an answer). One bank per `user_id`, 19–32 documents
each.

| overlap | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR | R@pool | R@5 up/down/same | n |
|---|---|---|---|---|---|---|---|---|---|
| 1.00 (unfitted) | 54.8% | 84.5% | 93.3% | 98.0% | 70.7% | 68.0% | 100.0% | 0/0/1531 | 1531 |
| 0.75 | 57.0% | 85.8% | 94.0% | 98.0% | 72.0% | 69.5% | 100.0% | 22/3/1506 | 1531 |
| 0.50 | 59.0% | 86.2% | 93.8% | 98.0% | 72.9% | 70.7% | 100.0% | 39/13/1479 | 1531 |
| 0.25 (shipped) | 60.4% | 86.0% | 94.2% | 98.0% | 73.6% | 71.8% | 100.0% | 57/35/1439 | 1531 |
| 0.00 (stream dropped) | 60.4% | 86.3% | 94.1% | 98.2% | 73.6% | 71.6% | 100.0% | 69/41/1421 | 1531 |

**Direction replicates; magnitude does not.** On LongMemEval the move from 1.00 to
0.25 was worth +4.2pp R@5 and +4.8pp NDCG@10. Here it is worth +1.4pp and +2.9pp.
That is the shape a real-but-modest effect takes after the maximum of 46 noisy
draws, and it is the first evidence that the E1 sweep's headline was inflated.

**The `provisional:` marker on `overlap: 0.25` is discharged — as a replication,
not as an endorsement.** `docs/EVALUATION_HYGIENE.md` §2.2 asked for independent
confirmation; this is it, and it confirms the direction. It does not re-open the
weight for tuning: a weight that wins on LoCoMo is a replication, and picking a
new value off that table is a selection that would have to be counted here.

**Revised out-of-sample estimate.** The clean measurement is the equal-weight
93.0% R@5 on LongMemEval. The independently-measured value of the shipped delta
is +1.4pp, not +4.2pp. So the best current estimate of out-of-sample R@5 is
**≈94.4%**, not 97.2% — which puts memory-wire ≈0.8pp *behind* agentmemory's
committed 95.2% hybrid rather than 2.0pp ahead of it. Both numbers are recorded
and neither replaces the other: 97.2% is what this build measures on these 500
questions, 93.0% is what the unfitted configuration measures, and 94.4% is an
estimate built from a delta measured elsewhere.

### 14.2 The token-overlap stream does not earn its weight

`overlap: 0.00` ties `0.25` on R@1 (60.4% both) and NDCG@10 (73.6% both), edges it
on R@5 by 0.3pp, and loses MRR by 0.2pp — while churning **69 queries up and 41
down**. A tie with that much movement is noise, not signal. The E1 sweep found a
**plateau, not a peak**, and `FusionWeights::overlap` may be a dead parameter.

This is recorded as a null result, and **not acted on**, for a reason the artifact
states itself: LoCoMo banks hold 19–32 documents against LongMemEval's ~50 and
`tests/scale.rs`'s 5,000. A second lexical voter has far more room to break ties
in a large pool than in a 25-document one, so the dev set under-measures exactly
the regime where the stream might still pay. Re-test at pool scale before deleting
it. `0.00` is a diagnostic bound and was never a candidate.

### 14.3 R@1 is measured, and it is the number that matters

`eval/RESULTS.md` now carries R@1 — the gap P0 was opened for.

| slice | R@1 | R@5 | R@20 | n |
|---|---|---|---|---|
| knowledge-update | 94.9% | 100.0% | 100.0% | 78 |
| multi-session | 87.2% | 97.0% | 99.2% | 133 |
| single-session-assistant | 85.7% | 100.0% | 100.0% | 56 |
| **single-session-preference** | **43.3%** | 86.7% | 100.0% | 30 |
| single-session-user | 87.1% | 98.6% | 100.0% | 70 |
| temporal-reasoning | 80.5% | 96.2% | 99.2% | 133 |
| **overall** | **83.8%** | 97.2% | 99.6% | 500 |

Overall R@1 83.8% re-derives `eval/ORACLE_RERANK.md`'s independent 419/500 — two
harnesses, same number. `single-session-preference` is the target: gold in the
pool 100% of the time, ranked first 43.3% of the time. That is an ordering
collapse, not a coverage gap, and it is invisible in every K≥5 column.

### 14.4 The recency stream was unmeasurable on LongMemEval, and why

`memories.created_at` existed, was indexed, and was referenced nowhere in
`src/recall.rs` — which is what made a recency stream look free. It was not
measurable: `examples/longmemeval.rs` passed `created_at: None` and
deserialised neither `haystack_dates` nor `question_date`, so SQLite stamped every
row with the wall clock at insert and all ~50 sessions in a bank landed
microseconds apart in `haystack_session_ids` order. A recency stream over that
ranks by insertion order.

Fixed by indexing sessions at the dataset's own `haystack_dates`
(`2023/05/20 (Sat) 02:21` → `2023-05-20T02:21:00.000Z`, millisecond width to match
what `store.rs` writes, since `expire_before` compares `created_at` as bytes and
assumes a fixed width). **Verified behaviour-neutral: 0 per-question differences
across all 500 rows and all 6 metrics** between the committed table and a fresh
run. `eval/RESULTS.md` was not regenerated, because the only column that moved was
p50 wall-clock and that file says not to pin it.

**The empirical basis for the recency lead does not check out.** It was cited from
agentmemory's reported "f2–f5 at 27% with token recency vs 14% without". A search
of `_audit/agentmemory` — `benchmark/`, `docs/`, every markdown file — did not find
that claim. The `R@1 43.3%` evidence that ordering is broken on
`single-session-preference` stands on its own; the claim that recency is the fix
does not, and is no longer cited as if it were.

### 14.5 First end-to-end answer-quality number — and the assembly bug it exposed

`examples/answer_quality.rs` exists to measure the thing the whole project has
optimised a proxy for. LongMemEval's actual metric is LLM-judged answer accuracy;
every number until this point was `recall_any@K`.

The first attempt at **n=20** gave conditioned 35% / closed-book 10%, then hung: the
process sat blocked with 1.7s of CPU across 28 minutes. Cause identified — the
endpoint it used (`gemini-web2api` on `:8081`) is not slow, it **times out**, and
at 180s per request and 4 calls per question a 25-question run will spend two hours
waiting on nothing. The harness was not at fault; it bounds every request. Re-run
against `hermes-router/small-stack`, which answers: a second trap there is
`max_tokens` — at 8 it returns an empty completion with `finish_reason: 'length'`,
which looks exactly like a broken endpoint.

**Clean run, n=25, seed 42, tree `91b5597`, loadavg 57.63 → 48.67, 286.5s over
100 LLM calls, 346,517 prompt tokens:**

| arm | accuracy | correct | scored | unscored |
|---|---|---|---|---|
| retrieval-conditioned | **40.0%** | 10 | 25 | 0 |
| closed-book control | **8.3%** | 2 | 24 | 1 |
| **delta** | **+31.7pp** | | | |

**The +31.7pp is the load-bearing result of this section: memory is doing real
work, and the feared "retrieval is at parity with no memory at all" did not
happen.** The 40.0% is not a memory-system quality score, and must not be quoted as
one — see below and `eval/ANSWER_QUALITY.md`, which says so at length.

**The 40.0% is floored by the default recall budget, and that is a product finding
rather than a harness artifact.** The harness uses `DEFAULT_RECALL_BUDGET` = 2,000
tokens. The median LongMemEval session is **2,543 tokens** (n=2,913 sessions: p10
814, median 2,561, p90 4,223, max 10,721 at 4 chars/token). **A 2,000-token budget
does not hold one median session.** The budget trim truncates the top hit to the cap
and skips every lower hit that does not fit, so the answerer received a partial
first session and essentially nothing else — while gold at rank 2–5 was
structurally unreachable.

The run's own disagreement table, in the committed `eval/ANSWER_QUALITY.md`, shows
exactly that, and it is the most important number in this section:

| on this slice | count | share of n=25 |
|---|---|---|
| gold session present in the **served context** | 23 | 92.0% |
| … answer judged correct | 10 | 40.0% |
| … answer judged **wrong despite gold being served** | 13 | **52.0%** |
| gold absent, answer correct anyway | 0 | 0.0% |

Per type: `single-session-assistant` 100% (4/4), `single-session-user` 100% (3/3),
`knowledge-update` 50% (6/6) — but `multi-session` **0%** (5/5 gold served),
`temporal-reasoning` **0%** (5/5 gold served), and `single-session-preference` 0/2
with gold **not in context at all** despite that slice's R@5 being 86.7%.

So the loop loses the answer in three distinguishable places: retrieval sometimes
misses (4%), the budget then discards what retrieval found, and the answerer fails
on what survived. **The middle one is ours and is fixable without touching the
ranking.** The 2,000-token default is tuned for short atomic memories
("auth uses jose"); against session-scale memories it silently under-serves, and
no ranking work in `eval/RESULTS.md` can show that, because `recall_any@K` counts
the gold row before the budget touches it.

A budget sweep (2,000 / 20,000 / 50,000 on the same 25 questions) is the direct
test; it was started, abandoned, and replaced with a deterministic measurement in
§14.9.

### 14.6 `eval/CODING_LIFE.md` is stale against its generator — do not regenerate

Found while auditing, and now written into `eval/README.md`.

The committed artifact declares 10 columns
(`configuration | BM25 | overlap | coverage | idf | P@k | R@k | Hit rate | p50 | n`)
and carries 12 rows. `examples/coding_life.rs:218` writes 8 columns (no `coverage`,
no `idf`) from the 4-entry grid at lines 154–157. A re-run would **silently delete
8 rows and 2 columns** — the 5 `idf overlap=*` rows and 3 `shipped + coverage=*`
rows that are the evidence for the E3/E4 removals.

This is the same hazard `README.md` already documents for `SWEEP_FUSION.md`, and
that file was not covered by the warning. The set of such artifacts is now
enumerated in `eval/README.md`.

### 14.7 Gate, on my own run rather than a subagent's word

`cargo test --locked` — **258 passed / 0 failed** (158 lib + 94 bin + 2 + 2 + 1 + 1;
baseline was 237, +21 from the mechanism tests).
`cargo clippy --all-targets --all-features --locked -- -D warnings` — clean, exit 0.
`cargo doc --no-deps --all-features --locked` — 0 warnings. (A subagent reported 3
transient warnings in `src/recall.rs`; that was mid-write state, not a regression.)

### 14.8 Selection count, unchanged

No parameter was selected in this section. The four new mechanisms ship with every
weight at an inert default and each is proved inert by a mutation check. LoCoMo
measured; it did not choose. The count of test-set consultations is unchanged from
`docs/EVALUATION_HYGIENE.md` §2.1.

The `--budget` sweep in §14.9 is the one thing in this section that could be
mistaken for a selection. It is not: it varies one *harness input* to explain an
observed failure, on 25 questions, and the value it points at is the same for
every consumer regardless of what the answer loop scores — the median session does
not fit in 2,000 tokens. Decided on that arithmetic, not on a metric.

### 14.9 The budget finding, measured without an LLM

The LLM sweep was the wrong instrument and was killed. At 20,000-token budgets
each of the 100 calls carries a 20k-token prompt through a local free-tier
gateway, and the harness caps every request at 180s — so it would have spent hours
and plausibly produced nothing but timeout rows. The mechanism does not need a
model; it is arithmetic over the corpus, and that arithmetic does not depend on
which judge produced the 40.0%.

Session sizes in `longmemeval_s_cleaned.json`, at the store's own 4-chars-per-token
rule, over all **23,867** haystack sessions in the 500 questions:

| percentile | p10 | p25 | median | p75 | p90 | p99 | max |
|---|---|---|---|---|---|---|---|
| tokens | 811 | 1,498 | **2,626** | 3,640 | 4,252 | 5,181 | 19,534 |

What fits whole in a budget of each size:

| budget | sessions that fit whole | median-size sessions that fit |
|---|---|---|
| **2,000** (the shipped default) | **35.6%** | **0** |
| 4,000 | 84.8% | 1 |
| 8,000 | 100.0% | 3 |
| 20,000 | 100.0% | 7 |

**The shipped default budget does not hold one median session, and 64.4% of
sessions are truncated by it.** This is not a harness artefact and it is not about
this benchmark: `DEFAULT_RECALL_BUDGET` = 2,000 is a product default, correct for
the short atomic memories the README's quick start uses ("auth uses jose", tens of
tokens) and silently wrong for session-scale memories, where it truncates the top
hit at the cap and skips every lower hit that does not fit. A consumer storing
conversations gets an answer loop starved of its own second-best memory.

The right follow-up is a budget-sweep *regression*, not another LLM run: assert
that the served set holds the gold row for a slice whose R@K is high, at a budget
that admits the corpus's median session. That is a `tests/` assertion, costs no
tokens, and would have caught this.

**The two n=25 runs are bit-identical.** Committed at `eval/ANSWER_QUALITY.md`
(tree `45ac189`); the earlier run at tree `91b5597` wrote to `$TMPDIR` and is
recorded in §14.5. They agree on every number —
40.0% / 8.3% / +31.7pp, 346,517 prompt tokens, the same 2 row errors, and the same
per-question verdicts — at temperature 0. The 40.0% is reproducible on this slice;
the open questions are the slice size (n=25) and the judge, neither of which a
repeat run narrows. Wall clock differed sharply between the two (286.5s vs 21.1s)
on an identical 100-call workload, which is a caution against reading anything into
per-call timing from that gateway.

---

# §15 — closeout inventory, and the doc claims that were false (2026-09-29)

Read-only audit of the tree at `edd5acb` plus a large uncommitted wave, plus
corrections to the doc claims it falsified. **No measurement in this section is a
measurement** — nothing was built or run. Every number below is a byte count, a
row count, a version string or a file size read off the tree.

## 15.1 `fastembed` / the `embed` feature — the removal claims are false, and were

`§8`, `§12.1` #4, `§12.8`, `README.md`, `docs/BENCHMARK.md` §5 and
`docs/VERSIONS.md` §3 all recorded, as fact, that the `fastembed` dependency and
its ONNX runtime were *removed, not deferred*, and cited the lockfile as proof:
"no `embed` feature and no `fastembed`, `ort` or `tokenizers` entry in
`Cargo.lock`" (`README.md`), "zero matches for all three names … The `embed` cargo
feature is gone from `Cargo.toml`" (`docs/VERSIONS.md` §3), "`Cargo.lock` has no
`fastembed`, `ort` or `tokenizers` entry" (`docs/BENCHMARK.md` §5).

**All three were true at the `v0.3.0` cut and are false of the tree now.**
Re-verified immediately before correcting, 2026-09-29:

| Claim | Tree state |
|---|---|
| "`embed` feature is gone" | `Cargo.toml:57` — `embed = [\"dep:fastembed\"]` |
| "`fastembed` is not a dependency" | `Cargo.toml:44` — `fastembed = { version = \"7.1\", optional = true, default-features = false, features = [\"ort-download-binaries-rustls-tls\"] }` |
| "no `fastembed`/`ort`/`tokenizers` in `Cargo.lock`" | all four present — `fastembed` (553), `ort` (1331), `ort-sys` (1344), `tokenizers` (2159), plus `ndarray` (1236) |

The arm behind the feature is not a stub: `src/vector.rs` (45,697 B) is a
retain-time dense arm with an embedder, a `memory_vectors` column and a
query-path branch, and `models/` carries 6 files / **23 MB** of vendored int8
MiniLM weights plus a `PROVENANCE.md`. `src/main.rs` grows a ninth CLI
subcommand, `embed <text>`, behind `#[cfg(feature = \"embed\")]`.

**What did *not* move, and why the correction is narrower than it looks.**
`default = []`, and nothing outside the `embed` feature references `fastembed`, so
a default build activates none of it. Every footprint figure this file and the
README publish — 8,836,032 B binary, 10,716–11,080 kB idle RSS, 3,022,848 B 10k
store, and the rest — remains a **default-build** figure and is unaffected. The *decision* to decline vector retrieval also stands:
`FusionWeights::vector` ships at `0.0` in every build, so no ranking and no HTTP
request reaches the stream.

**Corrected in place** (these files are not historical records): `README.md`
(×2), `docs/BENCHMARK.md` §5, `docs/VERSIONS.md` §3, `docs/BENCHMARK_SCALE.md`.
**Deliberately left as written:** §8, §12.1 #4 and §12.8 above. They are the
record of what was true at the `v0.3.0` cut, and `AGENTS.md` §2 keeps a
deliberate dated record. This section is the correction, appended rather than
back-edited.

## 15.2 The "the tree is ahead of the published release" banner is retired, and it is true again

The top-of-file banner (and the copy in `docs/VERSIONS.md` §0, and §12.0 below)
retires the "the tree on `main` is substantially ahead of the published `v0.2.0`
tag" framing on the grounds that `v0.3.0` is published and `Cargo.toml` says
`0.3.0`, "so the version string describes the tree as well as the release." That
grounding no longer holds. `v0.3.0` is tagged at `3e902ff`; `HEAD` is `edd5acb`,
which is **19 commits past the tag**, on top of the uncommitted wave in §15.3.
`Cargo.toml` still says `0.3.0`, so the version string no longer describes the
tree.

The §12 sections are left alone — they are correctly scoped to the release. This
paragraph restores the pointer.

## 15.3 The uncommitted wave

`git status --porcelain` at `edd5acb`: **9 modified tracked files, 8 untracked
paths.** `git diff --stat HEAD` totals **2,775 insertions / 347 deletions** across
9 files.

| | |
|---|---|
| modified | `Cargo.lock` (+714), `Cargo.toml` (+20), `src/api.rs` (+478), `src/recall.rs` (+533), `src/store.rs` (+900), `src/embed.rs` (+95), `src/main.rs` (+26), `src/lib.rs` (+6), `examples/locomo.rs` (−350 net, mostly deletions) |
| untracked, source | `src/vector.rs` (45,697 B), `examples/select_fusion.rs` (118,999 B), `eval/locomo_dev.rs` (22,139 B) |
| untracked, data | `models/` — 6 files, **23 MB** |
| untracked, artifacts | `eval/SELECTION.md` (36,059 B), `eval/SELECTION_VECTOR_AXIS_FIXED.md` (35,926 B), `eval/results_selection.json` (1,543,396 B), `eval/results_selection_vector_axis_fixed.json` (423,078 B) |

`Cargo.lock` grew by 714 lines — the fastembed/ort/tokenizers/ndarray subtree.
None of the four `eval/SELECTION*` or `results_selection*` paths is tracked, and
`.gitignore` does not cover them, so they are all staged-for-nothing and at risk.
`models/` at 23 MB is a poor commit candidate for a repository whose headline is
an 8.4 MB binary; `.gitignore` does not cover it either.

## 15.4 `eval/SELECTION.md` is not a regeneration hazard, and `eval/README.md` is wrong about its own list

`eval/README.md`'s "Artifacts that must NOT be regenerated" section states the
list "is the complete set" and names two artifacts. **It is neither complete nor
accurate for the new work, and the premise of the section does not cover what is
actually in the tree.**

`eval/SELECTION.md` (36,059 B, untracked) does **not** have the `SWEEP_FUSION.md` /
`CODING_LIFE.md` hazard. Those two lose evidence: a re-run drops 8 of 12 rows and 2
of 10 columns because the arms were removed from the generator. `SELECTION.md`'s
axis table declares four pass-2 axes — `overlap`, `k`, `bm25_magnitude`,
`agreement` — while `examples/select_fusion.rs:178` now defines a fifth,
`const VECTOR: &[f64] = &[0.00, 0.10, 0.25, 0.50, 0.75, 1.00, 1.50]`. A re-run
would **add** a vector axis, not delete rows. The failure mode is the opposite
one, and it is not a hazard: it is a stale artifact.

Two further facts, neither determinable from a re-run:

- `eval/SELECTION_VECTOR_AXIS_FIXED.md` is **newer** (02:08 vs 21:43) and carries
  a section the older file does not, explaining that the original vector axis
  would have read as a null **for the wrong reason** — the dev corpus ingests via
  `store.put`, which writes no vector, and `vector::vector_stream` returns an
  empty stream for a vectorless bank by design. The fix embeds all 272 documents
  before the sweep and aborts rather than publishing a grid if the counts
  disagree. So `SELECTION.md` was produced by a harness that did **not** have that
  guard, and `SELECTION_VECTOR_AXIS_FIXED.md` supersedes it for the vector axis.
- Both artifacts carry the same **commit-under-test `edd5acb`**, and both disclose
  in their own provenance block that 14 and 15 paths respectively were uncommitted
  at run time, so neither is attributable to any commit. Neither is a committed
  artifact at all, so `AGENTS.md` §2's rule about not hand-editing them does not
  apply — the honest disposition is to regenerate with `--out-md` named, or to
  leave both untracked and unclaimed.

`eval/README.md` also never mentions `select_fusion.rs` at all, and its
do-not-regenerate list predates both artifacts. It should name the new harness in
its harness inventory and correct "the complete set" — **not** by adding
`SELECTION.md` (which would be the wrong warning) but by saying the new grid
artifacts are superseded-by-regeneration rather than preserved-because-lost.

## 15.5 The consultation count is not recorded anywhere

`docs/EVALUATION_HYGIENE.md` §3.3 is the **counting rule**, not a count. It says
"Every selection made on the test set is logged in `docs/CONSISTENCY.md` with the
date, the parameter, the search space size, and the delta. The count is the
project's overfitting budget and it is meant to be visible and finite."

`AGENTS.md` §1 says `docs/CONSISTENCY.md` records the count. It does not. §14.8 —
the section that exists to address exactly this — says "The count of test-set
consultations is unchanged from `docs/EVALUATION_HYGIENE.md` §2.1", and §2.1 is a
prose audit of **one** confirmed violation, not a tally. So the budget is
described as visible and finite in two governing documents and is stated as a
number in none of them.

**The count is determinable by hand and is 1.** The only parameter ever selected
on the test set is `overlap: 0.25` (E1, 46 configurations on the 500 questions).
§14.2's `overlap: 0.00` is a diagnostic bound, not a candidate. The `--budget`
sweep in §14.9 varied a harness input, not a shipped value, and §14.8 already
rules it a non-selection on stated grounds. §14.4's recency fix was verified
behaviour-neutral at 0 per-question differences. The k-axis in the new
`select_fusion.rs` grid is the live risk: `docs/EVALUATION_HYGIENE.md` §2.6 records
`k = 60` as borrowed-and-never-fitted, and a grid that scores six `k` values on
the dev set is fine, but the moment a `k` is chosen that is a second selection and
the count becomes 2.

Left as a gap rather than written in, because this section is the wrong place to
open a tally: §3.3 asks for per-selection entries (date, parameter, search-space
size, delta) and the audit of 2026-09-28 recorded the E1 sweep in prose without
them. Adding the number without the log it counts would make the budget look
tighter than the record supports.

## 15.6 The `provisional:` marker — promoted in the docs, still live in a generated artifact

`§14.1` already decided it: the marker on `overlap: 0.25` "is discharged — as a
replication, not as an endorsement", on the strength of the LoCoMo result
(+1.4pp R@5, +2.9pp NDCG@10 over 1,531 queries that never chose it). That
decision had not been propagated to the documents that still assert the opposite.

**Promoted** in `docs/EVALUATION_HYGIENE.md` §2.1, §2.2 and §3.2, which claimed
"The shipped value is provisional", described independent confirmation as
outstanding, and described the harness as unbuilt work. The rule that governs the
promotion is `AGENTS.md` §1 — a fitted value carries a `provisional:` marker
"until an independent set confirms it" — and an independent set has confirmed it.
Promotion retires the **marker only**: §2.1 now states the measured
out-of-sample delta (+1.4pp R@5 / +2.9pp NDCG@10) and the ≈94.4% best-estimate
alongside the historical [0, +4.2pp] bound, so the +4.2pp cannot be quoted
detached from its correction.

**Not promoted, and this is a real remaining inconsistency.** The marker's primary
carrier is generated. `eval/RESULTS.md:3` still opens with the `provisional:`
paragraph, and that text is emitted by
`examples/bench_common::fitted_weight_note` (`examples/bench_common/mod.rs:278`),
which hard-codes "**fitted, not earned** … do not quote the difference from an
unfitted configuration as earned". `AGENTS.md` §2 forbids hand-editing a generated
artifact — "Fix the generator" — and the generator is under `examples/`, which
this change does not own and did not touch. So a reader of `eval/RESULTS.md` still
meets the discharged marker stated at full volume.

**This is the one doc fix in this pass that cannot be completed from here.** It
needs `fitted_weight_note` to either re-word to "replicated on independent data,
magnitude not earned" or to take the promotion as a parameter, and then
`eval/RESULTS.md` regenerated. Until then the honest state is: the decision is
recorded in §14.1 and in `docs/EVALUATION_HYGIENE.md`, and one generated artifact
has not caught up.

## 15.7 Test counts — a null result, and the reason it is one

Three documents carry **237**: the top banner (describing §12.9's gate),
`docs/BENCHMARK.md:114`, and `docs/NEXT_ITERATION.md:7`. §14.7 records a later,
larger gate: **258 passed / 0 failed** (158 lib + 94 bin + 2 backup + 2 e2e + 1
scale + 1 doc-test), "baseline was 237, +21 from the mechanism tests".

**None of the three 237s is stale, and none was changed.** `docs/BENCHMARK.md` §5
is headed "Current matrix — memory-wire 0.3.0 tree" and its rows are the
`v0.3.0` release figures (8,836,032 B binary included). §14.7's 258 was measured
*after* the tag, at a later commit. Moving the 0.3.0 matrix to 258 would make a
table headed "Status at 0.3.0" describe a tree that `v0.3.0` does not contain —
the same error §12.8 fixed in the other direction when it moved 217 → 237. The
counts are consistent once their scopes are read; they only look like a conflict
side by side.

**The count for the tree as it stands is not determinable without a build**, and
nothing here was built. The uncommitted wave adds `src/vector.rs` and ~2,000 lines
across `src/{api,recall,store}.rs`, so the count has almost certainly moved again
past 258 in both directions. **Needs a `cargo test --locked` on the working tree
before any release claim quotes a count.**

## 15.8 Retracted and load-confounded numbers — nothing re-quotes them

Checked every occurrence of the two figures this file records as retracted or
load-confounded: the pre-correction README "recall p50 6 ms / p95 10 ms"
(`§12.8`), the 22,116 KB under-load RSS and the 1311 µs coding-life p50 (§6, §9),
and README's former "p50 1.2ms" (§10. All ten hits are either inside this file's
own append-only record, where a retracted number belongs as history, or in
`README.md:201`, which quotes the 6 ms / 10 ms pair **in order to say it is not
re-pinned** and to point at the store's own committed recall figures instead. No
document re-publishes a retracted number as a current claim. Nothing to fix.

## 15.9 Deferred decisions — the register, as of 2026-09-29

Five, of which two are the ones previously known and three were not recorded in
one place before:

1. **A bad `update_mode` string answers `400 invalid content`.** §7.1. The frozen
   error table has no field-specific message, so `update_mode: "upsert"` rides the
   content error even though the content was fine. Kept because adding a
   field-specific code is a contract change, not a doc fix. Deferred at least
   twice — recorded in §7.1 and carried in the README's error contract.
2. **`trim_to_budget`'s signature is frozen**, so the duplicate `contents` map
   cannot collapse to one. §7.2. Eliminating the map means changing
   `pub fn … contents: &HashMap<&str, &str>`, and the overlap scorer was already
   made buffer-reusing around it. The map is the last remaining allocation and
   removing it needs the unfreeze.
3. **`install/get-memory-wire.sh:56-57` prints the default data directory** in its
   uninstall message, not the `--db` path the user passed. Recorded in §4.5, still
   open in §10, and never re-examined since — `install/` has been outside every
   phase's write scope across four of them.
4. **The E1 open contradiction**, `docs/NEXT_ITERATION.md:127`: the claim that the
   two streams already agree on their ordering "cannot be true, and the sweep
   artifact proves it two ways". The data needed to settle it was never collected.
   `docs/PERFORMANCE_PLAN.md` P1 is the phase that settles it, and every fusion
   decision rests on it.
5. **The vector arm's product trade**, `docs/EXCEED_PLAN.md:218`: a model download
   on first use breaks the "one static binary, offline" property. The plan records
   it as "a real product trade and the user's call, not mine." The uncommitted
   wave has now taken that decision unilaterally in one direction — vendored and
   offline, ~23 MB of weights in-tree — without the trade being recorded anywhere
   as answered.

**Not deferred, and worth saying so:** the `overlap: 0.00` null result (§14.2) and
the `detail=none` latency claim that could not exist (§12.5) are recorded results,
not open questions. `§12.5` is a negative finding that stands, and re-proposing a
latency win for `detail=none` would be re-litigating a rejected lever under
`AGENTS.md` §4.

---

# §16 — the `embed` build's `libstdc++.so.6`, and what it took to remove it (2026-09-29)

§15.1 corrected the *bookkeeping* about the `embed` feature being present. This
section is the part that could only be answered by building: **what the feature
does to the shipped artifact's shared-library dependencies, and what a plain
`cargo build --release --features embed` now costs.**

Every number here is a byte count, an ELF `DT_NEEDED` entry, a test count or a
verbatim command. **No latency and no RSS is quoted anywhere in this section**,
so `AGENTS.md` §3 does not apply to any of it; the box was at `loadavg` 21.29 on
24 cores throughout and that is recorded only so nobody assumes the number means
anything it does not.

## 16.0 The toolchain this was measured on, because two of the findings are toolchain-specific

`rustc 1.98.0` (`88d9e12ae`, 2026-08-18), `cargo 1.98.0`, LLVM 22.1.8, host
`x86_64-unknown-linux-gnu`, **gcc 16.2.1**. rustc links through
`rust-lld` via rustup's `gcc-ld` wrapper with `-fuse-ld=lld` and `-nodefaultlibs`;
the release profile is `lto = true`, `codegen-units = 1`. `bubblewrap 0.12.0` for
the minimal-root harness. The glibc static caveats named in `16.4` are
toolchain-independent; the `-static-libstdc++` no-op in `16.2` is a consequence
of the LLD/`gcc-ld` path and would not reproduce on a toolchain that still uses
GNU `ld` as the default linker.

## 16.1 The regression, reproduced before anything was changed

| Configuration | Size (B) | `readelf -d` `NEEDED` | minimal-root |
|---|---|---|---|
| default, no features | **8,874,144** | `libgcc_s.so.1`, `libm.so.6`, `libc.so.6` — **3** | **passes** |
| `--features embed` | **62,632,504** | `libstdc++.so.6`, `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, `ld-linux-x86-64.so.2` — **5** | **fails** |

The failing run, verbatim:

```text
$ memory-wire --version          # inside the minimal root
/memory-wire: error while loading shared libraries: libstdc++.so.6: cannot open shared object file: No such file or directory
exit 127
```

**The mechanism, read out of the dependency's own source rather than guessed.**
`ort-sys 2.0.0-rc.13` fetches a prebuilt *static* `libonnxruntime.a` and emits
`cargo:rustc-link-lib=static=onnxruntime`, so the ONNX runtime itself needs no
`.so`. But `build/static_link/mod.rs:21-34` also emits
`cargo:rustc-link-lib=stdc++`, and that resolves to the **shared**
`libstdc++.so.6`. The captured link line shows the two regions:

```text
[3]  -Wl,-Bstatic
[4]  …/libonig_sys-….rlib
[5]  …/libort_sys-….rlib          <- carries libonnxruntime.a
[6]  …/libsqlite3_sys-….rlib
[7]  …/libcompiler_builtins-….rlib
[8]  -Wl,-Bdynamic
[9]  -lstdc++                      <- shared; becomes a DT_NEEDED
[10] -lgcc_s … -lc
```

so ONNX Runtime's C++ symbols are satisfied *by the shared library*, which is
precisely what puts a C++ runtime on a Rust binary's load list.

## 16.2 Two options that do not work, kept here so they are not re-proposed

**`-static-libstdc++` is a no-op, and the previous attempt's own figure was
wrong.** Built with `RUSTFLAGS="-C link-arg=-static-libstdc++"`:

| | Size (B) | `NEEDED` |
|---|---|---|
| `--features embed` | 62,632,504 | 5, `libstdc++.so.6` first |
| the same build + `-static-libstdc++` | **62,632,504** | **identical, 5** |

**0 bytes, not the +128 B previously recorded.** The conclusion ("a driver flag
cannot override an explicit `rustc-link-lib`") reproduces; the magnitude does
not, so the +128 B figure is wrong and is not carried forward. The reason is more
specific than "a driver flag cannot override": rustc links through
`-B …/bin/gcc-ld -fuse-ld=lld`, and that wrapper forwards the library list to
LLLD without expanding gcc's link specs, so `-static-libstdc++` is parsed and
then ignored. `-C link-arg` position is not the issue — the flag never reaches a
linker that would act on it.

**`ORT_CXX_STDLIB=static` is not the value that suppresses anything.** From
`ort-sys`'s own code, `static_link_prerequisites` does:

```rust
if let Some(stdlib) = vars::get_any(vars::CXX_STDLIB) {
    if stdlib.is_empty() { None } else { Some(stdlib) }
}
```

so the value is used **verbatim** as a library name. `ORT_CXX_STDLIB=static`
would emit `cargo:rustc-link-lib=static` and ask the linker for `libstatic`.
The only value that suppresses the directive is the **empty string**, and it
suppresses it completely — which is also why it is not the answer here, because
it removes the C++ runtime without replacing it. Verified rather than reasoned:
replaying the captured link line with `-lstdc++` deleted and nothing put in its
place **fails to link**, with `undefined symbol: std::__throw_out_of_range_fmt(…)`
among a cascade the linker declined to finish ("too many errors emitted,
stopping now").

## 16.3 What actually works: a static `libstdc++.a`, in position

Two link-line shapes were measured by replaying the captured `cc` invocation
against a snapshot of its own inputs, so each variant is the real link and not a
reconstruction. Both leave `-lstdc++` exactly where rustc put it:

| Variant | Size (B) | `NEEDED` | minimal-root |
|---|---|---|---|
| baseline | 62,632,504 | 5 | fails |
| static `libstdc++.a` **replacing** `-lstdc++` at its own position | 63,748,384 | 4 | **passes** |
| static `libstdc++.a` **inside the `-Bstatic` group**, `-lstdc++` left in place | **65,006,360** | **4** | **passes** |

The second shape is the one shipped, and the reason is that it needs no
cooperation from ort-sys at all. rustc already links with `-Wl,--as-needed`, so
if every C++ symbol ONNX Runtime needs is satisfied *before* the shared
`libstdc++.so.6` is reached, the linker declines to record a `DT_NEEDED` for it.
The same measurement, twice, on a real build and on a replayed link, gave
65,006,360 and 65,007,536 — the 1,176 B difference is link-layout slack between
two different `rcgu.o` sets, not a different result.

**`libsupc++.a` is deliberately not linked.** With and without it the binary is
byte-identical: modern `libstdc++.a` already contains the C++ ABI objects. The
only symbol-level claim worth making is a negative one that survives a partial
scan: `libonnxruntime.a` does reference `dlopen`, `dlsym`, `dlclose`, `dladdr`
and `dlerror`, which matters for `16.4` and nowhere else.

## 16.4 `crt-static` works, and is still the wrong answer for this repository

| Form | Result |
|---|---|
| `RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --features embed` | **does not build**: `error: cannot produce proc-macro for async-trait v0.1.92 as the target x86_64-unknown-linux-gnu does not support these crate types` |
| the same **plus `--target x86_64-unknown-linux-gnu`** | **65,214,520 B, zero `NEEDED`**, static-PIE, minimal-root passes, `memory-wire embed` returns a real 384-d vector inside the sandbox |

So the glibc-static caveats were checked rather than assumed, and they do not bite
here. `libonnxruntime.a` links into a static-PIE with no relocation errors
(it is built `-fPIC`, and the bundle is the same object set the `.so` is built
from). `dlopen`/`dlsym`/`dlclose`/`dladdr` *are* referenced by the ONNX bundle
(they are in the 1,030), so under static glibc they are the stub
implementations that fail gracefully rather than load anything; the CPU execution
provider is compiled in rather than loaded, which the `embed` subcommand
returning a correct vector inside the sandbox is direct evidence of. NSS is not
reached: nothing in this crate resolves a user or a hostname.

**It is still rejected**, for a reason that has nothing to do with glibc: cargo
has no per-feature `rustflags`, so `-C target-feature=+crt-static` lands on the
default build too and turns a 3-`NEEDED` dynamic binary into a 0-`NEEDED` static
one. That is a different artifact, not a fix to this one, and it needs a user-set
environment variable and an explicit `--target`. Recorded as working, rejected
for scope.

## 16.5 The shipped fix: `build.rs`, and what it does and does not touch

`build.rs` (new, 111 lines) resolves the C++ runtime with
`cc -print-file-name=libstdc++.a` and emits
`cargo:rustc-link-search=native=<dir>` + `cargo:rustc-link-lib=static=stdc++`,
**gated on `#[cfg(feature = "embed")]` and on a `*-linux-gnu` target**. A plain
`cargo build --release --features embed` needs **no environment variable, no
`RUSTFLAGS`, and no `.cargo/config.toml`**. If the runtime cannot be located it
emits one `cargo:warning` and leaves the toolchain default in place rather than
breaking a build that used to work.

rustc places the archive in the `-Wl,-Bstatic` group ahead of the shared
`-lstdc++`, which is the position `16.3` measured. Confirmed in the link line of
the real build — the group grew from four entries to six:

```text
[2]  -Wl,--as-needed
[3]  -Wl,-Bstatic
[4]  …/libmemory_wire-….rlib      <- carries the bundled libstdc++.a
[5]  …/libonig_sys-….rlib
[6]  …/libort_sys-….rlib          <- carries libonnxruntime.a
[7]  …/liblibsqlite3_sys-….rlib
[8]  …/libcompiler_builtins-….rlib
[9]  -Wl,-Bdynamic
[10] -lstdc++                      <- reached with nothing left undefined; --as-needed declines it
```

**The default build is not degraded, and the claim is measured rather than
asserted.** Building the same tree with `build.rs` moved aside reproduces the
pre-`build.rs` binary **byte for byte** (`cmp` clean, 8,874,144 B), so the
toolchain here is deterministic and the comparison is meaningful. With
`build.rs` present the default binary is **8,874,128 B** — 16 B *smaller* — and
the only section whose size changes is **`.strtab`, by −12 B**; the remaining 4 B
are the ELF header and build-id. `.text`, `.rodata` and `.data.rel.ro` are
byte-for-byte the same size. A build script changes the crate's `-C metadata`
hash, which changes mangled symbol names, which shortens the string table. **No
instruction changed.** The `DT_NEEDED` list is still exactly three.

## 16.6 The minimal-root test, both configurations, and proof the C++ runtime is real

The root contains `ld-linux-x86-64.so.2`, `libc.so.6`, `libm.so.6` and
`libgcc_s.so.1` and nothing else. No `libstdc++.so.6` is present, and it was
deliberately left out rather than included to make the test pass.

| Configuration | Size (B) | `NEEDED` | `memory-wire --version` in the root |
|---|---|---|---|
| **default** | **8,874,128** | `libgcc_s.so.1`, `libm.so.6`, `libc.so.6` — **3** | `memory-wire 0.3.0`, exit 0 |
| **`--features embed`** | **65,007,536** | `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, `ld-linux-x86-64.so.2` — **4** | `memory-wire 0.3.0`, exit 0 |

**A passing `--version` is not evidence the C++ runtime works**, so it was
exercised, in the sandbox, on the shipped artifact:

```text
$ memory-wire embed "auth uses jose middleware and rotating keys"
[-0.030153006,-0.022352213,-0.047021147,-0.0957627,0.03199739,0.012406587,…]
$ memory-wire embed "the rate limiter is a token bucket"
[-0.093209974,0.0070842616,-0.010345894,-0.04167332,0.06556005,-0.03236901,…]
```

384 floats each, and different — that is real ONNX Runtime inference from the
23 MB vendored int8 MiniLM, in a root with no C++ shared library. Separately, the
six `vector::tests` that load the model (`the_embedder_returns_384_dimensions`,
`the_embedder_separates_a_paraphrase_from_an_unrelated_pair`,
`the_stored_vector_is_the_embedding_of_the_redacted_text`,
`the_vendored_model_is_the_file_the_provenance_records`,
`the_vendored_tokenizer_is_the_fast_tokenizers_serialisation`,
`the_default_weight_never_embeds_anything`) were run **inside the same sandbox**
and all six pass. A full `memory_retain` → `memory_recall` MCP round trip on the
same binary, same root, also passes, so the non-C++ paths are unaffected.

**One difference is not fixed and is not claimed to be:** the embed build
carries a fourth `DT_NEEDED` entry, `ld-linux-x86-64.so.2`, that the default
build does not. It is present in the *unfixed* embed build too, so it comes from
the dependency graph rather than from the C++ runtime, and the minimal root
carries the loader, so it costs nothing. Its origin was not chased further; the
default build's three-entry list is unchanged, which is the property that
matters.

## 16.7 Gates, verbatim, on this tree

| Gate | Result |
|---|---|
| `cargo test --locked` | **288 passed, 0 failed** — 188 lib + 94 bin + 2 backup + 2 e2e + 1 scale + 1 doc-test |
| `cargo test --release --locked --features embed --lib` | **203 passed, 0 failed** — the 15 extra are `vector::tests` |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | clean, exit 0 |
| `cargo doc --no-deps --all-features --locked` | 0 warnings, exit 0 |
| `cargo build --release --locked` | ok, 8,874,128 B, 3 `NEEDED` |
| `cargo build --release --locked --features embed` | ok, 65,007,536 B, 4 `NEEDED` |
| minimal-root, both configurations | both exit 0, `memory-wire 0.3.0` |

**288 is not comparable to any count earlier in this file.** §15.7 already
recorded that the uncommitted wave moves the count and that "the count for the
tree as this stands is not determinable without a build". 288 is that build,
on the tree as it stood at 2026-09-29 03:0x, with `src/vector.rs` and the
`src/{api,recall,store,main,lib,embed}.rs` wave present. §12.9's 237 and §14.7's
258 describe different trees at different commits and stay as they are.

## 16.8 Feature disposition: **keep-and-working**

The decision the user took is that **the deployment ships the lexical arm only**.
That is a statement about the deployment, and it was already true before this
section: `default = []`, `install/get-memory-wire.sh` downloads a release
tarball rather than building, and nothing in the release path passes
`--features embed`. What changed is that "opt-in, and starts on a minimal root
like the default build does" is now a **measured, tested** property rather than a
hope.

Kept, because it is real work that is now correct:

- `src/vector.rs` (45,697 B) with a test that runs the vendored model, a
  `memory_vectors` column, a retain-time arm and a query-path branch.
- 23 MB of vendored int8 MiniLM with a provenance table and a SHA-256 test that
  re-derives the digest, satisfying the Apache-2.0 §4 obligation in source.
- 15 unit tests that exercise ONNX Runtime end to end, six of which pass inside
  a root with no C++ shared library.
- A `memory-wire embed <text>` subcommand that makes the weights reachable from
  the shipped executable rather than stripped by LTO.

A working fix existed. Descoping would have discarded correct, tested work and
the measurement record that justifies keeping it, and the brief's own condition
for descoping — no option works — was not met.

**The vector arm still ships inert.** `FusionWeights::vector` is `0.0` in every
build, so no ranking and no request reaches the stream. Nothing in this section
changes that, and nothing here is evidence for any fusion configuration.

## 16.9 Left for a human

1. **The `ld-linux-x86-64.so.2` fourth `NEEDED`** (`16.6`) is untraced. Harmless
   on the minimal root, but "three dependencies" and "four dependencies" are
   different sentences and the second one now applies to the embed build.
2. **Cross-compiling the `embed` feature.** `build.rs` asks `$CC` (default `cc`)
   for the static runtime, which is right when the target is the host and wrong
   when it is not. `CC=aarch64-linux-gnu-gcc` is the fix and is not wired up or
   tested, because the release pipeline builds each architecture on its own
   runner and never crosses. Worth a line in the release docs if that changes.
3. **`ORT_CXX_STDLIB=` remains unset**, deliberately: the shipped fix does not
   need it (`16.3`), and setting it globally in `.cargo/config.toml` would break
   a macOS embed build, where `libc++` is the right answer. If a future change
   wants the shared library *kept* on Linux, that is the knob and it is one line.
4. **The six "static binary" / "no runtime" assertions** elsewhere in the docs
   (`README.md:3`, `README.md:116`, `docs/RERANKING_PLAN.md:5`,
   `docs/PERFORMANCE_PLAN.md:213-215`, `docs/EXCEED_PLAN.md:216-218`,
   `plugin/skills/memory-wire/SKILL.md:3` and `:8`) are outside this section's
   write scope. None of them is newly false — the default build has always been
   dynamically linked to three libraries — but none of them says so either, and
   the `embed` build's four-library list is now on the record here for the first
   time. Left for the task that owns those files.
5. **Two doc claims that are false against the tree and are not covered by
   §15.1's disclaimer.** `§15.1` lists "deliberately left as written" as §8
   (`:263`), §12.1 #4 (`:558`) and §12.8 (`:782`). Two more are not on that list:
   - **`docs/CONSISTENCY.md:289`** — "The `fastembed` removal is real, but it is
     more than offset by the MCP SDK landing…". False (the removal is not real)
     and nothing disclaims it. This is the one line in the file that a reader
     could take at face value today.
   - **`docs/RERANKING_PLAN.md:67` and `:91`** — "ort needs ONNX Runtime, whose
     static library is **108 MB** uncompressed". The `.a` really is 105,481,448 B,
     so the arithmetic is roughly right, but the *artifact* is not: with
     `ort-download-binaries` plus `--gc-sections` it lands inside a 65,007,536 B
     binary, and the cross-encoder remains disqualified either way. A dated
     rejection record, so it stays; it just should not be read as a claim about
     the current binary's size.

---

# §17 — the assertions that were false, closed out, and the consultation budget stated (2026-09-29)

Continues §16, which closed item 4 of its own §16.9 register. **Appended, like
every section here.** §16's measurements are not repeated, only pointed at; nothing
was built and no test was run for this section, except the one `readelf -d` call
below, which reads an existing binary and contends on nothing.

Two of the four work items here are corrections to claims that were *never*
false — they were never *true*, and no document had said otherwise. One is a
number no document stated. One is a framing a prior commit retired correctly and
which has since become true again.

## 17.1 The "static binary" assertions: closed, with the measurement

`§16.9` item 4 left six assertions to the task that owns those files. Re-verified
on this tree, from the release binary that exists in `target/release/`:

```text
$ readelf -d target/release/memory-wire | grep -c NEEDED
3
$ readelf -d target/release/memory-wire | grep NEEDED
 (NEEDED)  Shared library: [libgcc_s.so.1]
 (NEEDED)  Shared library: [libm.so.6]
 (NEEDED)  Shared library: [libc.so.6]
$ file target/release/memory-wire
ELF 64-bit LSB pie executable, x86-64, dynamically linked,
interpreter /lib64/ld-linux-x86-64.so.2, … not stripped
```

**The default build has never been statically linked.** It is a dynamically
linked PIE with three `DT_NEEDED` entries, all of which are part of any glibc
system. That is a strong and *checkable* claim — three is a number, and a reader
can run the same command — and it is what the docs now say. The pitch is not
weakened by it: the real advantage was never staticness, it is that the whole
service is one file with no daemon, no language runtime, no database server and
three libraries the OS already has.

Corrected, in the four files their own owners hold:

| file:line | before | after |
|---|---|---|
| `README.md:3` | "Agent memory in one 8.4 MB static binary" | "one 8.4 MiB binary, three shared libraries", and "No runtime" → "No language runtime, no database server" |
| `README.md:116` (footprint table) | no dependency row at all | a **Shared libraries** row naming all three, plus a **Minimum box** cell that stops implying you install SQLite — `rusqlite` is declared `features = ["bundled"]`, so the database is compiled in |
| `docs/RERANKING_PLAN.md:5` | "the single ~8.4 MB static binary, offline" | "the single ~8.4 MiB binary, offline … **It is not a statically linked ELF**" + the three names |
| `docs/PERFORMANCE_PLAN.md:213-215` | "the '8.4 MB, one static binary, nothing to install' claim" | "the '8.4 MiB, three shared libraries, one file, nothing to install' claim" |
| `docs/EXCEED_PLAN.md:216-218` | "breaks `docs/PERFORMANCE_AUDIT.md`'s 'one static binary, offline' property" | the same trade, stated without the false attribution — see §17.6 |

**`docs/EXCEED_PLAN.md`'s citation was false in a second, separate way, and that
is the more serious half.** `docs/PERFORMANCE_AUDIT.md` never states any such
property — a case-insensitive search of that file for `static`, `offline`,
`single file` and `install` returns **nothing**. The document was invoked as the
authority for a constraint it does not contain. The line now names the property
and its real home instead of citing a file that never made the claim.

**Two places still say it, and neither is a doc fix.**

- **`plugin/skills/memory-wire/SKILL.md:3` and `:8`** — "a single static Rust
  binary" in the front-matter description, and "One binary, no daemon, no
  runtime" in the body. Both outside every phase's write scope to date, and
  outside this one. The second is defensible on its own terms ("one binary" is
  true; "no runtime" means no language runtime); the first is not.
- **The published `v0.3.0` release body** — "Upgrades are a single static binary —
  no runtime, no daemon, no database to install." It is published, so no edit
  reaches it. It is also the sentence the retired banner in this file's header
  was written against, which is worth knowing before anyone reads the retirement
  as agreement with the release page.

## 17.2 The embed build is now reachable from the front door

§16 is the record — the `libstdc++.a` link mechanism, the four-entry
`DT_NEEDED`, 65,007,536 B against 8,874,128 B, the `bwrap` minimal root, the two
rejected alternatives. What §16 could not do is put any of it where a reader
starts. Three pointers added, no numbers moved:

- `README.md`'s `embed` paragraph now carries the cost of the feature in the same
  breath as its existence: 65,007,536 B vs 8,874,128 B, the `build.rs` static
  `libstdc++.a` trick, no `libstdc++.so.6` recorded at all, and the minimal-root
  test running real ONNX inference rather than `--version`.
- The footprint table's new **Shared libraries** row carries the 3-vs-4 split.
- `docs/VERSIONS.md` §0 now says the pins do not cover the `embed` feature,
  `build.rs` or `src/vector.rs`, because all three are post-`v0.3.0`.

`§16.4` and `§16.2` are the "do not re-propose this" half and are unchanged; the
point of the pointers is that a reader arriving at the README now hits the
rejected alternatives on the way out rather than re-deriving them.

## 17.3 The consultation count, reconstructed

`§15.5` found that `docs/EVALUATION_HYGIENE.md` §3.3 states a counting *rule* and
no document states a *number*, and left the number unwritten because opening a
tally needs a log to count. Here is the log, reconstructed from the record, with
the arithmetic `§15.5` did by hand checked against the artifacts.

**The rule, as `§3.3` words it:** *every selection made on the test set* is
logged with date, parameter, search-space size and delta. The budget is the count
of those occasions.

### The three occasions the test set was consulted

| # | date | phase | harness | search space | what was decided | resulting delta | where the record is |
|---|---|---|---|---|---|---|---|
| 1 | 2026-09-28 | **E1** | `examples/sweep_fusion.rs` | **27 configurations**, 500 LongMemEval-S questions, seed 42, one index per question | `overlap` → **0.25 shipped** (axis A, 9 values 0.00→2.00); `k` → 60 retained (axis B + corner E); `agreement` → 0.0 retained (axis C, 4 values) | `overlap`: R@5 93.0 → **97.2** (+4.2pp), NDCG@10 83.5 → **88.2** (+4.8pp), R@20 flat at 99.6%. `k`: 0.0 — no corner beats 0.25/k=60. `agreement`: 0 of 500 questions moved at any magnitude | `eval/SWEEP_FUSION.md` at `b3ca603`; provenance: loadavg 20.03, index build 160.6s for all 27 |
| 2 | 2026-09-28 | **E3 + E4** | same harness, same 500 questions | **46 configurations** — occasion 1's 27 re-measured plus **19 new**: axis F `idf` (9 weights) and axis G `coverage` (10 rows) | `idf` overlap → **rejected**; `coverage` stream → **rejected** | `idf` at the shipped 0.25: **−0.4pp R@5** (97.2→96.8) for +1.7pp NDCG@10. `coverage=0.05`: **−0.2pp R@5** (97.0 vs 97.2) | `eval/SWEEP_FUSION.md` at `993134d`; provenance: loadavg 60.42, index build 398.8s for all 46 |
| 3 | 2026-09-28 | **E2** (Porter stemming) | `longmemeval` | **2 configurations** (E1-only vs E1+stemming), 500 questions, seed 42, all 2,500 per-question values diffed; plus a 2-point sub-comparison at equal weight | `tokenize='porter unicode61'` → **rejected and reverted** | R@5 **+0.00** (4 up / 4 down / 492 same), NDCG@10 +0.65; at equal weight R@5 93.0 → 94.2, but E1 had already taken that headroom | `docs/NEXT_ITERATION.md` §E2 — **prose only, not in a committed artifact** |

### The count, three ways, because "consultation" and "selection" differ

- **Occasions the test set was consulted: 3.** All 2026-09-28, all
  `examples/sweep_fusion.rs` or `longmemeval`, all 500 questions, seed 42.
- **Parameters whose shipped value was *chosen* by looking at those numbers: 1.**
  `overlap: 0.25`. This is the one `docs/EVALUATION_HYGIENE.md` §2.1 audits, the
  one that carries the historical `provisional:` marker, and the one `§14.1`
  discharged on independent data. **`§15.5`'s hand count of 1 is confirmed.**
- **Levers *rejected* on test-set evidence: 2, plus 1 that was algebra.** `idf`
  and Porter stemming were turned down because a test-set number said so. The
  `coverage` stream is not one of them: `docs/EVALUATION_HYGIENE.md` §1 exempts it
  because the decisive argument was algebraic (`coverage: w ≡ overlap: 0.25 + w`,
  RRF being linear in the weights), and axis G merely agrees. Listing it as a
  consultation would overstate the budget; omitting axis G would understate the
  evidence.

**The two retentions are not selections, and saying so is the point.** `k = 60`
was inherited from Hindsight and nothing changed; the corner sweep only confirms
it. `agreement` moved 0 of 500 questions at every magnitude, which is a null
(`AGENTS.md` §6) rather than a choice. Neither burns budget. That is also the
narrow sense in which `docs/EVALUATION_HYGIENE.md` §2.6's "borrowed, not fitted —
and therefore also unvalidated here" still stands: §2.6 is about the *origin* of
`k = 60`, and after occasion 1 the value has been measured here without having
been chosen here. A reader should not take §2.6 to mean the constant has never
been swept — it has, in axis B and corner E, and that sweep is what this table
counts.

**One adjacent measurement, recorded because the rule covers it and it is easy to
mis-file.** E1's decision also weighed the *synthetic* recall curve
(`bench_recall_curve`, 32 fixed queries, overlap 0.50/0.75/1.00, R@5
96.9/90.6/96.9/96.9 at four sizes). That is not the test set, so it is not a
consultation under §3.3, but `AGENTS.md` §1 says the synthetic set is not to be
tuned against either, and `docs/EVALUATION_HYGIENE.md` §2.3 is where the
resulting trade-off is framed. It is listed here so nobody has to rediscover it.

**The live risk is the `k` axis of the dev-set grid, and it has not fired.**
`eval/SELECTION.md` scores six `k` values on LoCoMo, and
`docs/EVALUATION_HYGIENE.md` §2.6 records `k = 60` as borrowed. Both artifacts are
grids with no recommended configuration — `SELECTION_VECTOR_AXIS_FIXED.md` says so
in its own text — so nothing has been selected off them and the count is still 3/1.
**The moment a `k` is chosen from either table, the selection count is 2 and the
occasion count is 4**, and the choice must be made on the dev set and never
checked against LongMemEval.

### What the record does not settle

1. **Occasion 1 and occasion 2 are two runs, or one run reported twice.** The
   committed artifact says 27 configurations at `b3ca603` and 46 at `993134d`, an
   hour apart, with two different provenance lines and two different index-build
   times (160.6s vs 398.8s) at two different loads. That is two runs. But
   `docs/NEXT_ITERATION.md` §E3 describes "**the same 46 configurations** took
   398.8s of index build here against 160.6s in the E1 run", which cannot be
   literally true of a 27-configuration E1 run. **Both readings are left standing,
   per `AGENTS.md` §6** — the artifact headers and the two commits are the stronger
   evidence, and the prose sentence is the weaker one. The count does not turn on
   it: 3 occasions and 1 selection hold either way, because both readings contain
   the same single shipped selection.
2. **Everything before `v0.2.0`.** This repository's history is **29 commits** and
   begins at `50614e4` ("memory-wire 0.2.0", 2026-09-27). §1–§5 of this file
   describe a `0.1.0`-era sweep whose commits are not in this repository's log, so
   no pre-`0.2.0` selection can be reconstructed from the record — and no
   pre-`0.2.0` parameter is named as selected anywhere in §1–§5 either. The honest
   statement is that the count above is complete **for the history this repository
   holds**, and that the `0.1.0` era is out of reach rather than clean.
3. **Whether E2's numbers ever reached an artifact.** They did not, as far as this
   tree goes: `eval/BENCH_RECALL_CURVE.md` records the E1-only recall curve, and
   E2's stemming run is described as byte-identical to it, so nothing was written
   for it. `AGENTS.md` §2 requires prose figures to say so; §E2 does not say so.
   That is a live inconsistency in `docs/NEXT_ITERATION.md`, and that file was not
   in this pass's write scope.

## 17.4 `eval/SELECTION*.md` are stale, not hazardous

`§15.4` reached the same verdict and left the edit to the file's owner. It is now
made, in `eval/README.md`, and the supporting axis comparison is a table there:
`SELECTION.md`'s pass-2 table declares six axes, the generator's constants are
seven, the six match value-for-value, and the seventh is `vector`. A re-run
therefore **adds** rows — the opposite of the `SWEEP_FUSION.md` /
`CODING_LIFE.md` hazard, where a re-run deletes evidence. Neither artifact is on
the do-not-regenerate list, and the reason is stated there rather than implied.

One thing found while making that comparison, and reported rather than fixed:
`SELECTION_VECTOR_AXIS_FIXED.md`'s §5 renders the walk's range bounds as
`f64::MAX` where the measured set is empty. It is a formatting bug in
`examples/select_fusion.rs`, which `AGENTS.md` §2 says to fix in the generator, and
`examples/` was not in this pass's write scope. It is not a measurement.

## 17.5 The tree *is* ahead of the published release, and the tag is `v0.3.0`

`§15.2` restored the pointer. What the docs did not do is act on it, and one brief
instructing this pass asserted that "`v0.2.0` is the published tag". **That is
wrong, and by name: `v0.3.0` is the published release.** Checked against the
GitHub API, not inferred from the docs — `releases/latest` for
`ishan-parihar/memory-wire` returns `tag_name: v0.3.0`, `draft: false`,
`prerelease: false`, `published_at: 2026-09-28T08:21:33Z`, one asset,
`memory-wire-linux-x86_64.tar.gz`. `v0.2.0` is 28 commits back.

So the accurate position, which is a *different* statement from the one that was
retired at `c56b1f1`:

- `v0.3.0` is published. The installer's `latest` resolves to it. That part of
  the retirement was right and stays right.
- `HEAD` (`edd5acb`) is **19 commits past the `v0.3.0` tag**, on top of an
  uncommitted wave (`§15.3`: 9 modified tracked files, 8 untracked paths).
- `Cargo.toml` still says `0.3.0`, so **the version string describes the release
  and not the tree** — which is exactly the gap the retirement claimed did not
  exist. The embedding of the `embed` feature, `build.rs` and `src/vector.rs` in
  the shipped artifact is a consequence: `v0.3.0` is the lexical arm only.
- The header of this file still carries the retired banner, because this file is
  append-only and that line is history. **This section is the correction**; the
  banner at `:22-30` should be read as superseded on this one point.

Fixed where it could be: `docs/VERSIONS.md` §0, which is not append-only, now
states the position instead of the retirement; and `README.md`'s install section
now says plainly that `curl … | sh` gives you `v0.3.0` while the rest of the file
describes later work.

## 17.6 §16.9 item 5: both claims resolved, one by an addendum

- **`docs/CONSISTENCY.md:289`** ("the `fastembed` removal is real, but it is more
  than offset by the MCP SDK landing") — false, and inside an append-only file, so
  it stays. `§15.1` is the correction and `§17.1`'s shared-library row is the
  consequence; this line is the third pointer. It is the one line in this file a
  reader could still take at face value, and it will keep being takeable — that is
  the cost of the append-only rule, paid knowingly.
- **`docs/RERANKING_PLAN.md:67` and `:91`** ("ONNX Runtime, whose static library is
  **108 MB** uncompressed") — a dated audit record, kept as written, with an
  addendum under §3.2 giving the measured figure (a 65,007,536 B binary with 4
  `NEEDED` entries) and stating that the disqualifier is the ~600–800 ms, which
  packaging does not touch. The §3.2 conclusion's "breaks the single-binary claim
  by 3–11×" was rewritten to point at the addendum rather than restate a ratio
  that no longer has a measured denominator.

## 17.7 What was not fixed, and why

1. **`plugin/skills/memory-wire/SKILL.md:3`** — "a single static Rust binary".
   Outside the write scope of this pass and of every prior one. One line, one
   front-matter string.
2. **The published `v0.3.0` release body** — "a single static binary". Published;
   unreachable by an edit. Fixable only by a later release's notes.
3. **`eval/SELECTION_VECTOR_AXIS_FIXED.md`'s `f64::MAX` bounds** — generator bug,
   `examples/` not in scope. §17.4.
4. **`docs/NEXT_ITERATION.md` §E2's prose-only figures** — no committed artifact
   behind them, which `AGENTS.md` §2 requires the prose to say. §17.3 item 3.
5. **`docs/PERFORMANCE_AUDIT.md` has no dependency, offline or install claim at
   all**, so nothing there needed correcting — but that is why `EXCEED_PLAN.md`'s
   citation was false, and it means the file cannot serve as the citation for any
   such property. §17.1.
