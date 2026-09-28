# PLAN: memory-wire (Rust) — Hindsight × agentmemory

Goal: the best memory infrastructure we can have — **Hindsight's learning memory
(retain/recall/reflect + observations + mental models) on agentmemory's
coding-agent capture (hooks + 4-tier consolidation + hybrid RRF recall)** — as one
Rust binary. Audit: `docs/AUDIT.md`.

Note on subagents: per request, any delegated work in this project must use the
`space-bunny` model. This scaffold + plan was done directly (no subagents fired).

## Status — 2026-09-28 · benchmark-integrity plan · **Phases A, B and D landed; C still BLOCKED**

**Status: Phase A done. Phase B done, with two of its four premises corrected.
Phase C BLOCKED. Phase D done.** Phase A touched no `.rs` file — it was
packaging and labelling only. Phases B and D are the SQLite/recall work
recorded in `docs/CONSISTENCY.md` §11 and §12.

The battery is now `examples/{longmemeval,coding_life,scale_sweep,soak,bench_footprint,bench_write,bench_recall_curve,bench_concurrency,bench_coldstart}.rs`
plus `tests/scale.rs`. **CI runs none of it**:
`.github/workflows/ci.yml` is quota-frugal by design and deliberately omits the
examples, so every harness is manual and nothing will catch rot in one
automatically. That is the premise of this plan — the numbers were only ever as
good as the discipline of whoever last ran them by hand, and the documents
outlived their builds. It has now cost real accuracy twice: a hand-measured
"flat 60 rec/s" that was a Python-client artifact, and a generator that deleted
a curated methodology note. `docs/CONSISTENCY.md` §12 is the record.

Reference point this plan is measured against (`0.2.0`, 2026-09-27, before this
plan's work): 217 tests green, clippy clean at `--all-features`, release binary
8,816,120 B, and R@5 93.0 / R@10 97.4 / R@20 99.6 / NDCG@10 83.5 / MRR 83.9.
The retrieval metrics are the invariant across all of it; everything else moved
and is re-pinned in `docs/CONSISTENCY.md` §12.9.

### Phase A — reproducible from a clean clone, documents labelled. **DONE**

`examples/coding_life.rs` reads `eval/data/sessions.json` and
`eval/data/queries.json` at hardcoded relative paths, and `.gitignore` excluded
the whole of `eval/data/`, so a clean clone could not run it at all. `.gitignore`
now ignores `eval/data/*` and re-includes exactly those two fixtures, so
`cargo run --release --example coding_life` works from a fresh clone with no
manual copy step. The 277,383,467 B LongMemEval-S dump stays untracked and is
still fetched by `eval/download.sh`; the two tracked fixtures cost 8,394 B total.

Every benchmark document now says whether it is current or historical:
`docs/BENCHMARK.md` and the pre-cut half of `docs/CONSISTENCY.md` carry
historical banners, `docs/BENCHMARK_SCALE.md` states its build profile
explicitly, and `docs/VERSIONS.md` §3 is marked as omitting `rmcp`. **No recorded
measurement was altered** — a stale number is marked, never replaced, because
re-measuring here would destroy the baseline the deltas are computed against.

### Phase B — throughput and the write path. **DONE (with two of the four premises corrected)**

Motivated by four measurements. **Two of the four were wrong, and one was a
prediction that did not reproduce.** All four are kept below, quoted as first
written, because a wrong premise is worth more on the record than a deleted one.

> **WRONG — RETRACTED.** "Recall throughput measured flat at ~60 rec/s whether 1
> or 64 concurrent clients hit the server, with p50 latency growing linearly
> 17.2 ms → 55.4 → 220.6 → 1012.3 ms — a serialization signature, caused by one
> `Mutex<Connection>`."
>
> That whole paragraph is an artifact of **how it was measured**: over HTTP, with
> Python clients, on a box at `loadavg` 105. In-process, on the same tree, the
> numbers are **380–575 ops/s** for 1 client, not 60 — the "flat 60" is the
> client and the scheduler, not the store. The *latency* growth was real and is
> why the pool was built; the *throughput* claim was not. Corrected and pinned in
> `docs/CONSISTENCY.md` §12.3.

> **PREDICTED, DID NOT REPRODUCE.** "Write path: WAL + `synchronous=FULL` (SQLite's
> default, which the store never overrides) = 1.980 ms/commit = 505 commits/s; WAL
> + `synchronous=NORMAL` = 0.099 ms/commit = 10,054 commits/s. A **19.9x**
> headroom from one pragma."
>
> The pragma shipped and it is a large win, but **19.9x is not a number this
> store produces.** The 1.980 ms figure is a bare single-row `INSERT`; the
> store's `put` also does SHA-256 of the content, the FTS5 insert trigger, three
> index updates and a `find_duplicate` seek, so the ratio is smaller. Measured,
> interleaved, on the store's own paths: **`synchronous=NORMAL` is 8–10x**
> (`put` p50 2,681.7 → 266.5 us @1k, 2,355.3 → 283.3 us @10k; 1,000-row bulk
> load 140–174 → 1,145–2,029 rows/s). Same direction, same mechanism, smaller
> number. See `docs/CONSISTENCY.md` §11.1 and §12.2.

> **NOT DONE.** "FTS5 currently runs on all defaults, so there is no Porter
> stemmer. On a morphological-variant probe, `unicode61` retrieved the same
> documents as the base form for only 1 of 3 testable pairs; `porter unicode61`
> for 6 of 7."
>
> Still true: the table is still `detail=none` with no `porter` tokenizer. The
> morphology gap was **not** closed, and the probe was not re-run, so the 1-of-3
> / 6-of-7 pair is still the only evidence and it is a single unversioned
> observation. What *did* change on this axis is storage: `detail=none` removed
> 172,032 B of index at 10k memories (`docs/CONSISTENCY.md` §12.5). A stemmer is
> a separate, untested change and stays off.

> **DONE, partially.** "13 `.prepare(` call sites, zero `prepare_cached`."
>
> Six of the thirteen are now `prepare_cached` — the `get`/config/paging/TTL
> read paths. Seven stay on `prepare`, each with the reason in a code comment
> (`recall_pool_conn` and `keyword_search_conn` interpolate one `?` per id, so
> caching them thrashes the 16-slot LRU; the backfill, `has_column` and three
> test helpers are one-shot or test-only). `get` p50 12.8 → 4.2 us @1k. The
> remaining uncached parses on the retain path (`conn.execute` / `query_row`) are
> listed in `docs/CONSISTENCY.md` §11.2 as the bigger unclaimed fish.

Read together at the time: the read path is serialized, the write path is paying
a durability cost nobody asked for, retrieval has no morphological tolerance,
and every query re-parses its SQL. Each shipped separately, so each number is
attributable. What came out: a WAL read pool (Phase D harness), `synchronous=NORMAL`,
`prepare_cached` on the read paths, and `detail=none` — plus a no-pool control
that is the only reason the pool is kept.

### Phase C — LoCoMo retrieval-only parity run. **BLOCKED — do not schedule**

The goal is a retrieval-only LoCoMo run scored the same way as the LongMemEval-S
harness, so memory-wire's retrieval can be set against Hindsight's published
LoCoMo subset numbers. **The dataset is not present locally**; the only LoCoMo
material available is a small conversation-sample fixture in the Hindsight audit
checkout, which is a fixture and not the benchmark. Nothing in Phases A, B or D
changes that. Phase C stays blocked until the real LoCoMo dataset is obtained and
vendored, or fetched by a `download.sh` step the way LongMemEval-S is.

### Phase D — close the benchmark-surface gap. **DONE**

> The benchmark-surface gap: no harness emits ops/s; no soak result is persisted
> anywhere; all latency is measured in-process so the README's HTTP/RSS numbers
> are hand-measured with no committed script; and every quality suite uses a
> corpus under 200 memories, meaning the 200-row recall candidate pool "never
> binds" on them (this is stated in `eval/RESULTS.md` lines 5-11) so recall
> quality above 10k memories has never been measured.

Three distinct consequences, and the last one is the expensive one: the throughput
work in Phase B has nothing to report itself in, the soak harness produces numbers
that are lost when the terminal closes, and the pool bound that the whole recall
design rests on is unverified for quality on any corpus big enough to trigger it.
Phase D therefore gates Phase B: shipping a concurrency fix with no ops/s harness
means shipping it unmeasured.

## 0. Status

- [x] Clone + audit both repos (`_audit/`, reference only)
- [x] Scaffold `memory-wire` (this crate, `cargo build` clean)
- [x] Phase 1 — Store (SqliteStore + bank isolation + redaction; 10 tests)
- [x] Phase 2 — Capture + Recall + Consolidation (RRF k=60 kernel, tiers/decay; 16 tests)
- [x] Phase 3 — Learning + Interfaces (MemoryService + axum retain/recall/reflect + health; e2e verified)
- [x] Phase 4 — Hardening + Eval (217 tests green, clippy locked gate clean, e2e regression suite)
- [x] Benchmark-integrity Phase A — clean-clone reproducibility + document labelling (2026-09-27, see the dated status section above).
- [x] Benchmark-integrity Phase B — write path + read path (2026-09-28: `synchronous=NORMAL`, `prepare_cached` on 6 read paths, 4-connection WAL read pool, `detail=none`, TTL sweep, graceful shutdown, loopback bind, dead-code removal). Two of its four motivating premises were wrong; both are corrected in the Phase B section above and in `docs/CONSISTENCY.md` §12.
- [x] Benchmark-integrity Phase D — benchmark surface closed (2026-09-28: five committed harnesses under `eval/`, plus a working soak artifact).
- [ ] Benchmark-integrity Phase C — LoCoMo retrieval-only parity. **Still BLOCKED — do not schedule.** The dataset is not present locally.

Execution evidence (2026-09-27, the Phase 4 close): `cargo test` 20 passed / 0
failed; `cargo clippy --all-targets --all-features --locked -- -D warnings` clean;
live serve e2e (health/retain/recall/reflect) verified on :8899. That 20 is the
count at the phase boundary and is kept as a record of it; the tree now stands at
**217 passed / 0 failed** (117 lib + 94 bin + 2 backup + 2 e2e + 1 scale +
1 doc-test), re-measured by the final regression gate on 2026-09-27 at `0.2.0`.

## 1. Architecture

```text
hooks (agentmemory protocol) ──> capture (dedup→privacy→raw obs) ──> store
retain (Hindsight op) ──> extract (facts/entities/temporal) ──> normalize ──> store + indexes
recall/reflect ──> hybrid retrieve (BM25+vector+graph+temporal → RRF → [rerank] → budget trim)
background ──> consolidate (working→episodic→semantic→procedural, decay/evict) ──> observations + mental models
interfaces ──> REST (:8888-compat subset) + MCP (per-bank + tools) + CLI + viewer-lite
```

Crate layout (`src/`), lib side: `memory.rs` (Bank/Memory) · `store.rs` (`Store`
trait, SQLite+FTS5, bank isolation, migrations) · `recall.rs` (candidate pool,
BM25 + token-overlap, RRF, budget trim) · `capture.rs` (PII redaction) ·
`embed.rs` (cosine/rank kernel for a future vector stream) · `api.rs`
(retain/recall/reflect) · `lib.rs`. Binary side, in `main.rs`: `connect`, `doctor`,
`guidelines`, `hooks`, `http`, `mcp`, `paths`, `seed`, `sweep`. The 4-tier ladder
and the `Observation`/`MentalModel` types above are target state, not built: the
types were deleted for having no constructor and no caller, and `consolidate.rs`
was deleted for having no scheduler behind it. `capture.rs` supplies the
redaction filter; the hook *protocol* lives in the binary-side `hooks.rs`.

## 2. API contract (v1, Hindsight-compatible subset)

- `POST /banks` / `GET /banks` — isolated stores (id, name, background, disposition).
- `POST /banks/{id}/retain {content, context?, timestamp?}` → `{memory_id}`.
- `POST /banks/{id}/recall {query, budget?}` → ranked memories (each: content, score, provenance).
- `POST /banks/{id}/reflect {query}` → synthesized answer + cited memory ids.
- `GET /banks/{id}/models` + knowledge pages as markdown (`/pages`, mirror to disk).
- agentmemory-compat: `POST /agentmemory/observe|smart-search|session/start|session/end` mapped onto the same core (thin shim, not 132 endpoints on day one).
- MCP: per-bank `retain/recall/reflect` tools first; extend toward agentmemory's core-8 (`save, recall, consolidate, smart-search, sessions, diagnose, lesson-save, reflect`).

## 3. Phases

### Phase 1 — Store (SQLite first, Postgres when configured)
- `Store` trait + `SqliteStore` (rusqlite/SQLite FTS5 for BM25 start) + migrations.
- Bank isolation enforced in every query; secret/PII redaction on write (45-pattern starter set from Hindsight Memory Defense).
- `DATABASE_URL` set → `PgStore` (sqlx + pgvector) with same trait; single-binary switch.
- Done when: bank CRUD + retain-persist + redaction unit-tested, `cargo test` green.

### Phase 2 — Capture + Recall + Consolidation
- Capture: hook event JSON (SessionStart/PostToolUse/Stop) → 5-min SHA-256 dedup → privacy filter → raw observation → index. File-watcher + `import-jsonl` (Claude Code transcripts) port.
- Recall: Tantivy BM25 + fastembed HNSW vector + SQLite entity-graph + temporal filter → **RRF k=60**, session-diversify ≤3/session, token-budget trim. Rerank = score-fusion first, ONNX cross-encoder later.
- Consolidation: working→episodic→semantic→procedural job (tokio scheduler) with Ebbinghaus decay, strengthen-on-access, auto-evict, contradiction flag, supersession chain (superseded leave index).
- Done when: keyless BM25 recall works; vector recall behind `EMBEDDING_PROVIDER=local`; consolidation produces an observation with proof count.

### Phase 3 — Learning + Interfaces
- Observations (refine-not-overwrite, quotes + proof counts) + mental models (cron refresh, DB-read serve) + knowledge pages (wiki folders, `fs` markdown mirror like `hindsight-cli fs sync`).
- REST (axum) per §2 + MCP (rmcp) + CLI (clap; port `hindsight-cli` shapes: `bank|memory|mental-model|knowledge-base|fs|explore`) + `connect <agent>` installer for 5 agents first (claude-code, codex, opencode, cursor, copilot-cli).
- Done when: `memory-wire serve` + `memory-wire memory retain/recall/reflect` + one agent wired end-to-end.

### Phase 4 — Hardening + Eval
- Auth (bearer), multilingual preservation (no transliteration), disposition traits in reflect, Prometheus metrics, viewer-lite (read-only obs/graph/replay page).
- Eval harness: LongMemEval-S R@5/R@10/MRR + coding-agent-life P@5/R@5/hit-rate + p50 latency, same adapter pattern as agentmemory `eval/`; publish scorecards under `benchmark/`.
- Done when: eval runs in CI, numbers recorded, gaps filed as issues.

## 4. Dependencies (researched 2026-09-26, added per phase — not all upfront)

| Layer | Crate (version) | Why (evidence) |
|---|---|---|
| async runtime | `tokio` 1, `features=["full"]` | Required by axum, sqlx, rmcp; already in scaffold |
| REST | `axum` 0.7 (stay; 0.8.x stable, 0.9 in dev) + `tower` + `tower-http` | Tokio-team standard, hyper-based, tower middleware (timeouts/tracing/compression/auth) free; `axum::serve` + `State<PgPool>` pattern |
| DB toolkit | `sqlx` (postgres + sqlite + migrate features, runtime-tokio, rustls) | One async toolkit for both backends; `PgPoolOptions`, `query!` macros, `migrate!` |
| PG vectors | `pgvector` 0.4 (`features=["sqlx"]`) | `<->`/`<=>`/`<#>` distance ops, HNSW/IVFFlat, `Vector::from(Vec<f32>)`, sqlx + postgres + diesel examples incl. hybrid RRF |
| embedded PG (dev/test) | `pglite-oxide` 0.5.1 | Embedded Postgres + bundled pgvector, no Docker; SQLx connects via normal URL; temp-DB-per-test + persistent path |
| SQLite + BM25 day-one | `rusqlite` (bundled) + SQLite FTS5 external-content table + triggers | Zero-daemon BM25: `CREATE VIRTUAL TABLE … USING fts5(…, content=…, content_rowid=…)` + AI/AD/AU triggers, `bm25()` rank, `highlight`/`snippet`; bound MATCH param (never interpolate) |
| full-text upgrade | `tantivy` 0.26 | Lucene-class BM25, 17 Latin stemmers + CJK (jieba/lindera/tiny-segmenter), mmap, <10 ms startup; replaces FTS5 when ranking/tokenizer needs grow |
| embeddings + rerank | `fastembed` 7.x | Local ONNX (ort + tokenizers), sync → wrap in `spawn_blocking`; `TextEmbedding` (BGE-small default), `Bgem3Embedding` (dense+sparse+ColBERT one pass), `TextRerank` cross-encoder — one crate covers Phase 2 recall + Phase 4 rerank |
| vector-index alternative | `embedvec` 0.10 (evaluate) | Pure-Rust HNSW + AVX2/FMA SIMD + RaBitQ/E8/H4 quantization, Fjall/Sled/RocksDB/pgvector backends, tokio async; adopt only if hand-rolled HNSW becomes load-bearing |
| MCP | `rmcp` (modelcontextprotocol/rust-sdk, `features=["server","macros","schemars"]`) | Official SDK, tokio, MCP 2026-07-28 + 2025-11-25 compat, `ServerHandler` + `#[tool]`, stdio + streamable-HTTP (stateless) transports |
| CLI | `clap` 4 derive (done) + `clap_complete`, `trycmd`/`snapbox` | Polished help/completions + snapshot tests for CLI output |
| time/ids/hash | `chrono`, `uuid` (`v4,serde`), `sha2`, `regex` + `secrecy` | Timestamps, bank/memory ids, 5-min SHA-256 dedup, Memory-Defense patterns, secret wrappers |
| pages/scheduling | `pulldown-cmark`, `tokio-cron-scheduler` (or `tokio::interval` first) | Knowledge-page render, consolidation cron |
| errors/logging | `thiserror` 2 (lib) + `anyhow` 1 (bin only) — done; `tracing` + `tracing-subscriber` EnvFilter — done | Enforce boundary per §7 |
| Phase 4 | `prometheus`, `argon2`, `subtle` | Metrics, bearer-hash, constant-time compare |

Per-phase adds: P1 `sqlx,pglite-oxide,rusqlite,chrono,uuid,regex,secrecy` → P2 `+tantivy,fastembed,sha2` (evaluate `embedvec`) → P3 `+rmcp,tower,tower-http,clap_complete,pulldown-cmark,tokio-cron-scheduler` → P4 `+prometheus,argon2,insta/trycmd`.

## 5. Non-goals (v1)

No iii-engine re-implementation, no 132-endpoint parity, no 60-integration parity, no hosted cloud, no ONNX reranker on day one, no Oracle backend (trait allows it later).

## 6. Verification

- Each phase: `cargo build` + `cargo test` + `cargo clippy --all-targets --all-features --locked -- -D warnings` clean (skill gate; add `-W clippy::pedantic` in CI by Phase 3).
- Phase 2+: recall quality gates (LongMemEval-S sample) before merging retrieval changes.
- Never commit `_audit/`; it is local reference. `.gitignore`: `_audit/`, `target/`, `.env`.

## 7. Rust conventions (rust-best-practices skill — enforced from Phase 1)

- Errors: `thiserror` enums per module with `#[from]` hierarchies in lib; `anyhow` only in `main.rs`/bin. No `unwrap`/`expect` in prod (tests only); propagate with `?`, early-return via `let … else`.
- Borrowing: `&str`/`&[T]` params, no redundant `.clone()` (watch `redundant_clone`, `clone_on_copy`, `needless_collect`, `large_enum_variant`); `Cow` where ownership is ambiguous; small `Copy` (≤24 B) by value.
- Dispatch: generics (static) on hot paths (`Store`, recall fusion); `dyn Trait` only for heterogeneous plugin/tool surfaces; box at API boundaries.
- Docs: `//` = why (safety/rationale), `///` = what/how on all public APIs; `#![deny(missing_docs)]` on lib; `TODO(#n):` always linked.
- Tests: descriptive names (`recall_should_fuse_bm25_and_vector_via_rrf`), one assertion per test, doc-tests for public API, `insta` snapshots for CLI/recall output.
- State safety: type-state pattern for serve lifecycle (`Unbound`→`Serving`) and consolidation jobs where invalid ops must not compile.
- Fix lints, don't silence: `#[expect(clippy::…)]` + reason only, never bare `allow`.
