# Audit: hindsight + agentmemory

Sources cloned 2026-09-26 into `_audit/` (reference only, not part of the build):
- `_audit/hindsight` — https://github.com/vectorize-io/hindsight
- `_audit/agentmemory` — https://github.com/rohitg00/agentmemory

## 1. Hindsight (vectorize-io/hindsight)

- **What:** Agent memory that *learns*, not just remembers. SOTA on LongMemEval (Jan 2026, independently reproduced by Virginia Tech + WaPo). Paper: arxiv 2512.12818. Used in Fortune-500 prod. License **MIT**.
- **Core audited:** `hindsight-api-slim/hindsight_api/` (FastAPI + SQLAlchemy + asyncpg + pgvector, `pyproject` v0.10.1, Python ≥3.11). Engine dirs: `engine/`, `api/`, `worker/`, `admin/`, `webhooks/`, `extensions/`, `migrations/` (alembic). Embedded PG via `pg0.py` (default `DATABASE_URL=pg0`), external Postgres or Oracle AI DB for prod.
- **Ops (the API to copy):**
  - `retain(bank_id, content, context?, timestamp?)` — LLM extracts facts/entities/temporal/relations → normalize to canonical entities + time series + sparse/dense indexes.
  - `recall(bank_id, query)` — 4 strategies in parallel: semantic (vector), keyword (BM25), graph (entity/temporal/causal), temporal (range filter) → RRF merge + cross-encoder rerank + token-budget trim.
  - `reflect(bank_id, query)` — disposition-aware synthesis (skepticism/literalism/empathy per bank).
- **Memory model:** banks (strict isolation, background + disposition + templates) → world facts / experiences → **observations** (deduped beliefs with quotes + proof counts, refined not overwritten) → **mental models** (standing Q&A, DB-read cheap) → **knowledge pages** (wiki folders, markdown-mirrorable). Multilingual end-to-end (native script preserved). Memory Defense opt-in (45 PII/secret patterns, redact-or-block).
- **Interfaces:** REST :8888 + UI :9999; MCP per bank `GET /mcp/{bank_id}/` + `hindsight-local-mcp` stdio; 60+ integrations (coding agents via `hindsight-coding-agents`, LiteLLM wrapper `wrap_openai/wrap_anthropic` = 2-line adoption, frameworks, n8n/Zapier, Obsidian); clients Python/TS/Go/**Rust (thin, 5 files)**; **`hindsight-cli` is already Rust** (`hindsight-cli/src/`: banks, memory retain/recall/reflect/history, mental-models, knowledge-base, documents/entities/tags/chunks, webhooks, audit, TUI `explore`, `fs` knowledge mirror/sync daemon) — the natural donor for memory-wire's CLI.
- **Ops surface:** hierarchical config (env → tenant → bank), Prometheus metrics, admin CLI (migrations/repair), webhooks (retain/consolidation/refresh), tenant/auth/storage extension points.
- **Take for memory-wire:** retain/recall/reflect semantics; observation + mental-model learning loop; bank isolation + disposition; TEMPR recall (4-stream + RRF + rerank); Rust CLI command shapes; MCP-per-bank pattern.

## 2. agentmemory (rohitg00/agentmemory)

- **What:** Persistent memory for coding agents. Local-first, zero external DBs. License **Apache-2.0**. Audited version **0.9.29** (`package.json`), ~184 files / ~42k LOC / 1,674 tests. Runtime: **iii-engine v0.22.1** (worker/function/trigger primitives; KV state + streams + OTEL come from bundled workers, not Postgres/Redis/Express/pm2).
- **Pipeline (the capture to copy):**
  - `PostToolUse` → SHA-256 dedup (5-min) → privacy strip → raw observation → synthetic compression default (LLM only if provider + `AGENTMEMORY_AUTO_COMPRESS=true`) → embed if provider active → BM25 (+vectors) index.
  - `Stop/SessionEnd` → session summary → graph extraction (if `GRAPH_EXTRACTION_ENABLED`) → slot reflect (if enabled).
  - `SessionStart` → project profile + hybrid search → ≤2000-token injection.
- **Memory model:** 4 tiers working (raw) → episodic (summaries) → semantic (facts/patterns) → procedural (workflows); Ebbinghaus decay, strengthen-on-access, auto-evict, contradiction resolve, versioning/supersession (superseded leave index, history in KV chain), `similarTo` near-dup hints, origin-channel provenance (user/agent/tool/import/shared), git snapshots.
- **Recall:** triple-stream BM25 (stem + synonyms, CJK needs optional segmenters) + vector (local `all-MiniLM-L6-v2` opt-in or Gemini/OpenAI/Voyage/Cohere/OpenRouter) + graph (entity match + BFS) → **RRF k=60**, session-diversified (≤3/session). Keyless = BM25 (+existing graph in smart-search); lesson recall on dedicated BM25 index.
- **Interfaces:** 132 REST endpoints `:3111`; **54 MCP tools** (core-8 trimmable), 6 resources, 3 prompts, 17 skills (9 invocable + 8 reference); 12 auto-capture hooks + 20 `connect <agent>` adapters (Claude/Codex/Copilot/Cursor/OpenCode/pi/Hermes/…); viewer `:3113`, iii streams `:3112`, worker WS `:49134`; multi-agent via `AGENT_ID` + `shared|isolated` scope, team share, leases/signals/actions/routines/checkpoints/sentinels/mesh-sync.
- **Benchmarks:** LongMemEval-S R@5 **95.2%** / R@10 98.6% (reproducible `eval/` harness + `benchmark/COMPARISON.md` vs mem0/Letta/Zep/Cognee…); token story ~170K/yr (~$10, $0 local).
- **Take for memory-wire:** hook auto-capture protocol + dedup/privacy; 4-tier consolidation + decay/evict; triple-stream RRF recall; MCP tool registry shape (core vs all); `connect`-style agent wiring; eval harness design.

## 3. Complementarity (why integrate)

| Concern | Hindsight wins | agentmemory wins | memory-wire rule |
|---|---|---|---|
| Learning | observations w/ proof counts, mental models, reflect | — | adopt Hindsight learning loop |
| Capture | manual retain / wrapper | 12 hooks, zero-effort sessions | adopt agentmemory hooks |
| Retrieval | TEMPR + cross-encoder rerank | RRF k=60 + session-diversify + lesson index | union: 4-stream + RRF + optional rerank |
| Storage | Postgres+pgvector (prod-grade) | zero-dep SQLite/KV (local-first) | trait: SQLite default, Postgres when `DATABASE_URL` set |
| Interfaces | per-bank MCP, 60+ integrations, Rust CLI | 54 MCP tools, 17 skills, viewer | Hindsight ops × agentmemory tool breadth, one Rust CLI |
| Eval | LongMemEval SOTA + paper | reproducible harness + comparison matrix | one harness, both suites |

## 4. Risks / open questions

1. Cross-encoder rerank in Rust: `fastembed` covers embeddings; rerank may start as score-fusion only, ONNX reranker later.
2. iii-engine is a binary runtime, not a library — memory-wire re-implements the *protocol* (hooks/KV/streams), not the engine.
3. License mix MIT × Apache-2.0 → memory-wire is `MIT OR Apache-2.0`; no copied code in scaffold, clean-room interfaces only.
4. `_audit/` is reference; never compiled. Delete or gitignore before publishing.
