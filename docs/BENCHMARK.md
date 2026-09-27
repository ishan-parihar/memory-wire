# Benchmark + tool matrix

> **This file holds two records.** §1–§4 are the **0.1.0 audit, run 2026-09-27**,
> kept verbatim as evidence about that build — its "NOT IMPLEMENTED" rows and its
> debug-build numbers are *true of 0.1.0* and false of this tree. Do not read them
> as the current state. §5 is the current `0.2.0` matrix. Nothing below rewrites
> the 0.1.0 numbers.

## 1. Tool matrix (live, :8891)

| Tool | Result | Evidence |
|---|---|---|
| CLI `info` / `--help` / `serve --help` | PASS | version + pointers print; help renders |
| `GET /health` | PASS | `ok` (200) |
| `POST /banks/:id/retain` | PASS | returns uuid; persists; auto-creates bank |
| `POST /banks/:id/recall` | PASS | keyword hit returns content; unknown bank `[]` |
| redaction on retain | PASS | `sk-…` stored as `[REDACTED:api_key]` |
| bank isolation | PASS | `other` bank recall `[]` for `proj` content |
| `POST /banks/:id/reflect` | PASS | cited answer; empty bank → `"no relevant memories"` |
| malformed JSON / unknown route | PASS | 400 / 404 |
| MCP tools (rmcp) | **NOT IMPLEMENTED** | no `rmcp` dep, no MCP route — plan-only (PLAN.md §4) |
| CLI `bank/memory/mental-model/fs/explore` | **NOT IMPLEMENTED** | only `info` + `serve` exist |

## 2. Seeded benchmark (:8892, debug, in-memory SQLite)

- Corpus: 200 memories (20 topical × 4 subjects + 180 distractors)
- Queries: 20 (4 subjects × 5 repeats), budget 2000 tokens
- **hit_rate 1.00** (20/20 contain expected keyword)
- **recall p50 1.9 ms, p95 2.1 ms** (localhost, overlap-rank kernel)
- `cargo test`: 20 passed / 0 failed; clippy locked gate clean

## 3. Footprint

- Shipped runtime: one 76 MB debug binary (→ release+LTO will shrink) + SQLite
  file; zero daemons. Competitors need Python 3.11 + Node runtimes, Postgres/pg0
  or iii-engine workers, and 1,048 MB of source checkouts.
- Not yet comparable: official LongMemEval-S R@5/R@10 (needs tantivy +
  fastembed + dataset harness), 132-endpoint / 54-tool parity, viewer.

## 4. Gaps to production-grade (honest, as of 0.1.0)

1. Recall is token-overlap, not BM25/vector — tantivy + fastembed per PLAN.md §4.
2. No MCP server (rmcp), no per-bank MCP, no `connect <agent>` installers.
3. No Postgres/pgvector backend, no FTS5 migration, no mental-model cron.
4. No release-binary size, no CI, no LongMemEval harness run.

### 0.1.0 provenance (verbatim — evidence about that build, not to be re-pinned)

Baseline pins: `docs/VERSIONS.md` (hindsight `ccfe85b`, agentmemory `bcf4f0d`,
`memory-wire` 0.1.0, rustc 1.98.0). Binary: debug build, 76,178,192 bytes
(symbols included; release+LTO not yet measured).

## 5. Current matrix — memory-wire 0.2.0, 2026-09-27

Everything in §1–§4 that 0.2.0 has since closed, and where the evidence is. The
authoritative current numbers live in `eval/` and in the README footprint table;
this section exists so a reader who lands here does not take the 0.1.0 matrix for
the shipped product.

| Tool / surface | Status at 0.2.0 | Evidence |
|---|---|---|
| Retrieval stack | **FTS5 BM25 (LIMIT 50) + token-overlap (cap 200), fused by RRF k=60** over a candidate pool of the newest 200 rows ∪ the BM25 hits | `eval/RESULTS.md`: R@5 93.0 / R@10 97.4 / R@20 99.6 / NDCG@10 83.5 / MRR 83.9 on official LongMemEval-S, 500 questions |
| Official LongMemEval-S harness | **built and run** (gap 4 closed) | `examples/longmemeval.rs`, `eval/RESULTS.md`; per-question values reproduce bit-for-bit across runs |
| `GET`/`PUT /banks/:id/config` | shipped; PUT requires an existing bank | `docs/CONSISTENCY.md` §2, unknown-bank rule |
| Lifecycle routes | shipped: `GET .../memories` (50/500, offset), `GET`/`DELETE .../memories/:mid`, `GET .../stats` | README "Lifecycle ops are routes now" |
| `format: "full"` on recall | shipped; `score` is the fused RRF value, not a token count | `docs/CONSISTENCY.md` §2 |
| `document_id` upsert | shipped; `replace` works, a repeated `append` is a `409` | `docs/CONSISTENCY.md` §1 #5 |
| CLI | **eight subcommands**: `info serve connect hook doctor mcp seed sweep` | `memory-wire --help` |
| MCP server (rmcp) | **shipped** — 4 tools over stdio (gap 2 closed) | `memory-wire mcp`; `docs/CONSISTENCY.md` §2, 4 tools + annotations verified |
| `connect <agent>` / `hook` / `doctor` | shipped for claude-code, codex, copilot-cli (+ MCP entry for cursor, opencode) | `docs/CONSISTENCY.md` §1 #7–#8 |
| FTS5 | shipped — external-content `fts5` table with AI/AD/AU triggers, rebuilt on drift | `src/store.rs`; `migrate_should_rebuild_a_drifted_fts_index` |
| Bounded recall pool | shipped — cost no longer grows with the bank | `eval/SCALE_SWEEP.md`, `docs/BENCHMARK_SCALE.md` |
| `ttl_days` + `sweep` | shipped, off by default, no scheduler | README "Forgetting" |
| Release binary size | **8,816,120 B** (gap 4 closed) | `stat -c %s target/release/memory-wire` after `cargo build --release --locked` |
| Tests | **217 passed / 0 failed** (117 lib + 94 bin + 2 backup + 2 e2e + 1 scale + 1 doc-test) | `cargo test --locked --all-features` |

### Still not built — and formally declined, not in flight

- **Vector retrieval was DECLINED.** The `embed` feature and its `fastembed` /
  ONNX dependency were removed from the tree, not deferred; `Cargo.lock` has no
  `fastembed`, `ort` or `tokenizers` entry. `src/embed.rs` survives as a
  dependency-free cosine/rank kernel with no embedder and no producer behind it.
  PLAN.md §2/§4 still list `fastembed` because those sections describe the goal
  architecture, not this build.
- 4-tier consolidation ladder (deleted: no scheduler, no caller) · `DELETE` for a
  whole bank · `tags` in the lifecycle responses · `document_id` in any response ·
  a `backup` subcommand (the `sqlite3 .backup` recipe is the interface) ·
  Postgres/pgvector backend · LLM-backed `reflect` synthesis · bank-in-path MCP
  over HTTP (`/mcp/:bank`) · `connect` for hosts outside the five detected.

### Gaps that need live LLMs or providers

Hindsight system-evals and agentmemory quality/real-embeddings runs cannot be
reproduced from this tree; they need a hosted provider key. Their retrieval-only
LongMemEval-S numbers *are* comparable and are in the README benchmark table.
