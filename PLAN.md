# PLAN: memory-wire (Rust) — Hindsight × agentmemory

Goal: the best memory infrastructure we can have — **Hindsight's learning memory
(retain/recall/reflect + observations + mental models) on agentmemory's
coding-agent capture (hooks + 4-tier consolidation + hybrid RRF recall)** — as one
Rust binary. Audit: `docs/AUDIT.md`.

Note on subagents: per request, any delegated work in this project must use the
`space-bunny` model. This scaffold + plan was done directly (no subagents fired).

## 0. Status

- [x] Clone + audit both repos (`_audit/`, reference only)
- [x] Scaffold `memory-wire` (this crate, `cargo build` clean)
- [x] Phase 1 — Store (SqliteStore + bank isolation + redaction; 10 tests)
- [x] Phase 2 — Capture + Recall + Consolidation (RRF k=60 kernel, tiers/decay; 16 tests)
- [x] Phase 3 — Learning + Interfaces (MemoryService + axum retain/recall/reflect + health; e2e verified)
- [x] Phase 4 — Hardening + Eval (217 tests green, clippy locked gate clean, e2e regression suite)

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
