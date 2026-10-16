# Plan: reference-project investigation — architectural gaps and the next evidence cycle

Written 2026-10-15, after `v0.8.0`, when the in-house hypothesis queue emptied
(`docs/handoff/RND_PLAN.md` results log: 16 terminal rows, 4 test-set
consultations, all measurement-only). This document is the bridge between
"their architecture has X" and "memory-wire runs a pre-registered test of X":

it defines **what each reference project is audited for, which of their
mechanisms are candidates under this repo's constraints, and how a candidate
becomes R&D**. Nothing here is permission to build anything; every §4 finding
must survive the closed-lever veto before a single line of code moves.

## 0. Ground rules, inherited

1. **Three-set discipline** (`AGENTS.md` §6, `docs/EVALUATION_HYGIENE.md`): dev =
   LoCoMo + the relevance battery; test = LongMemEval-S, counted, acceptance-only;
   everything external = measure-only. A competitor's benchmark number is never a
   selection signal for us.
2. **Closed-lever veto** (`docs/handoff/CLOSED.md`, `RERANKING_PLAN.md` §3,
   `PERFORMANCE_PLAN.md`): re-proposing a closed mechanism requires *new evidence*
   stated up front — an architecture diagram is not evidence, it is a claim.
3. **Constraint gate**: single static binary, `DT_NEEDED` 3 on the default build
   (~8.8 MiB, re-pinned from the published asset), no default dependency, no
   daemon-with-a-daemon, no LLM in the write path of a default install. A
   mechanism that cannot pass this gate is out of scope *as a default* and can
   only return as an off-by-default feature with a user who asked for it.
4. **Audits are read-only and pinned.** Clone into a scratch dir (not `_audit/`,
   which is reserved for the ported parents), pin the commit in every note, treat
   the tree as untrusted input (`BENCHMARK_LANDSCAPE.md` §2).
5. **One party, one harness.** Any head-to-head number we cite must be either
   measured by us under a harness whose commit is pinned, or explicitly labelled
   self-reported.

## 1. Audit targets

| project | state today | audit question | cost |
|---|---|---|---|
| **agentmemory** (`rohitg00`) | cloned, pinned `bcf4f0d`, source-audited twice | re-read with one fresh question: *which read-path mechanisms did we not port, and would any survive the veto?* (synonym table, prefix pass, agreement bonus — all already measured or priced; confirm the list is exhaustive) | ~half a day, free |
| **Hindsight** (`vectorize-io`) | cloned, pinned `ccfe85b` | the one surface we never opened: **identity extraction at ingest** (their PMB precision 0.85 rides on it). What does it cost, what does it buy, and is there a lexical/SQL-shaped approximation that is not "an LLM at ingest"? | ~half a day, free |
| **MemOS** (`MemTensor/MemOS`) | not cloned | the L1/L2/L3 typed-memory hierarchy (traces / policies / world-models) and the "self-evolving" extraction loop: is a *type* distinction a ranking signal, a product tax, or scope creep? Their deployed stack is multi-service Python — the audit is for **ideas**, never for architecture. | ~1 day, free |
| **Honcho** (`plastic-labs/honcho`) | not cloned | "dialect"/user-representation layer: they model *the user* rather than the transcript. Read-only at the ideas level — **AGPL-3.0**: no code, no snippets, no file copies into this repo; mechanism notes only, written from scratch. | ~1 day, free |
| **OpenViking** (`volcengine/OpenViking`) | not cloned | requested by the user — §2 below. File-system context paradigm, progressive content delivery, post-session extraction, MCP-native. Their OAMB result: 51/60, slowest (0.79 s median retrieve). | ~1 day, free |
| **OmniMemEval** (`MemTensor/OmniMemEval`) | not cloned | requested by the user — §3 below. Standardized `add()`/`search()` benchmark adapter over five corpora, two of which we have nothing on. | §3 |

The two reference repos already under `_audit/` need no new clone; the other four
get pinned clones with their HEAD recorded here the day they are made.

## 2. OpenViking integration assessment

What it is (checked 2026-10-15 from the repo and its docs): volcengine's
"self-evolving context database" — Apache-2.0, unifies agent memory, knowledge
RAG and skills as one virtual filesystem under a `viking://` protocol, MCP-native
agent plugins, sessions committed → async extraction of "preferences and agent
experience" into long-term memory. Candidate aspects, screened:

| # | their aspect | our state | screen verdict |
|---|---|---|---|
| V1 | **progressive content delivery** — recall assembles a *bounded* memory block, summary-first, detail loaded on demand | the primitive already ships: `?truncate=` on recall serves a prefix + original length + the fetch command; `X-Total-Count` pagination; the budget trim. What we do not have is a *summary* tier — truncation is byte-prefix, not semantic | **candidate**. Mechanism: a summary-first serve mode (serve `[context]`-style one-liners, fetch whole on demand). Judged on the **relevance battery** (junk@5 down, gold@5 flat), never on LoCoMo ranking — H4 proved the reader does not drown at k=10, but OPEN_HOOK_RECALL_RELEVANCE's over-return diagnosis is exactly what V1 targets. No new deps: the summary must be lexical/structural (first line? `context` field we already store?), or it dies at the constraint gate. |
| V2 | context-type unification (memory / resources / skills as one namespace) | none, deliberately — memory-wire is a memory service | **out of scope, named to be refused.** A memory server that also serves knowledge RAG and skill libraries is a second product; AGENTS §1's Tier-C boundary exists to keep this one shippable |
| V3 | async post-session extraction / consolidation ("self-evolving") | **closed here by measurement** — the 4-tier consolidation ladder was built, found to have no caller and no win, and removed (`README` Roadmap, `CLOSED.md`); both parents benchmark with consolidation *off* | **veto stands** unless the audit finds a mechanism ours did not test *and* a dev-set experiment it suggests — "their architecture depends on it" is not that evidence |
| V4 | context browsing for agents (`viking://`-style enumeration) | **already shipped, discovered stale-row 2026-10-15**: `mcp.rs` has served `resources/list` (bounded [`RESOURCE_PAGE`] pages + cursor), `resources/read` for `memory://{bank}/{id}`, and bank-segment completion since the MCP surface landed — the screen row above was written without checking our own module. Hand-verified 2026-10-15 on the live `omp` bank over the HTTP mount: 100-entry page, cursor 100, exact read by URI | **closed as already-shipped** — no work item; the audit's job was to catch this row being stale, and it did |
| V5 | their retrieval result | OAMB 51/60 (vs Hindsight 57/60), slowest at 0.79 s | **informational only.** An OAMB run of ours is a LongMemEval test-set consultation; not justified by curiosity |

**The honest summary for the user's integration question:** the load-bearing
candidates are **V1** (summary-first serving — a real mechanism, battery-gated,
no new machinery) and **V4** (MCP browsing polish). V3 is the aspect their
marketing leads with and our measurements already killed. Nothing in OpenViking
justifies touching the ranking core.

## 3. OmniMemEval → the battery

What it is (checked 2026-10-15): MemTensor's Apache-2.0 evaluation framework,
two tracks:

- **Track 1 (User Memory)** — a standardized `add()` / `search()` adapter layer
  (`scripts/client_factory/`, selected with `--lib`), one pipeline
  (ingest → retrieve → answer generation → LLM-as-judge → metrics) over
  **LoCoMo, LongMemEval, BEAM, PersonaMem v2, HaluMem**. Results land under
  `results/<benchmark>/<LIB>-<VERSION>/`.
- **Track 2 (Agent Memory)** — AgentBench task domains run by an agent runtime
  (OpenClaw) with a memory plugin installed; protocol includes memory cleanup,
  training, settling, backup/restore. Needs Docker, LiveCodeBench,
  BrowseComp-Plus, an agent budget. **Parked** — same tier as agent-memory-bench
  (TODO #10): it is the right *question* and the wrong *cost* until the user
  funds an agent-model budget.

Roles, classified **before any run** (`BENCHMARK_LANDSCAPE.md` §6.1):

| corpus (Track 1) | role here | notes |
|---|---|---|
| LoCoMo | **dev, judged instrument** | a different instrument from our retrieval dev set — never a selection signal for ranking; useful as a cross-check of the AMB rag number (84.1%) under a second harness |
| LongMemEval | **test consultation, counted** | one run only if a release wants it; OAMB/MemBase/memorybench rule applies verbatim |
| BEAM | measure-only | judged answers here need paid LLMs (AMB's BEAM gold-ids are unusable for retrieval — §3 of the landscape); only worth it if the paid tier is opened |
| **PersonaMem v2** | **measure-only — new corpus** | preference memory; complements PMB |
| **HaluMem** | **measure-only — new corpus** | hallucination resistance, new to us entirely |

**Integration cost, honestly:** Track 1 needs answer+eval LLM credentials; the
free aroute tier only works if their client factory accepts an OpenAI-compatible
base URL (verify at adapter time — their `env_examples/` templates will say).
The provider itself is a thin HTTP client with the same uuid→document mapping
the AMB provider needed (`BENCHMARK_LANDSCAPE.md` §4.0 trap — without it a
perfect ranking scores 0). First run, pre-registered: **HaluMem, once, whatever
it shows** — a new corpus measures generalization, which is the one thing our
current battery cannot.

## 4. How a finding becomes R&D

```
audit note (mechanism, their evidence, their cost)
        │
        ├─ closed-lever veto ────────────► CLOSED re-confirmed, recorded, stop
        ├─ constraint gate fails ─────────► recorded as out-of-scope-as-default
        ▼
mechanism paragraph (what signal it adds, where the gold would move)
        ▼
pre-registered H-item in RND_PLAN (H8+): gate + dev-set instrument + kill criterion
        ▼
one run, terminal verdict, CONSISTENCY § + CLOSED row if negative
```

No step is skippable, and the queue is filled **only** by findings that clear
both gates. The current candidates that already clear §4's questions on paper:
V1 (summary-first serve, battery-gated) and the OmniMemEval-HaluMem first run.
Everything else in this document is either refused (V2, V3), polish (V4),
informational (V5), or awaiting an audit that has not happened yet.

## 5. Non-goals

Full parent parity (`AGENTS.md` §1). Postgres/pgvector or any second storage
engine (Roadmap "not built"). Graph streams, cross-encoders, anything on the
closed list without new evidence. Adopting a competitor's architecture because
it is a competitor's — the audit extracts mechanisms, not stacks.

## 6. Audit findings (2026-10-15, all six complete)

Method: five scout audits (one failed its yield protocol but its full atlas was
recovered from the preview; Hindsight was completed in-tree by the auditor from
our own pinned clone after its scout stalled) plus one auditor pass. Every
finding below cites the pinned commit it was read at. Repo pins:
agentmemory `bcf4f0d` · Hindsight `ccfe85b` · MemOS `a7367d0` · Honcho `ed17042` ·
OpenViking `621b1dd` · OmniMemEval `a5c8b4b`.

| project | what the audit found | veto-chain verdict |
|---|---|---|
| **agentmemory** | The old catalogue was **not exhaustive — 16 mechanisms missing, 2 wrong**. Corrections: the reranker is a **bi-encoder** (ms-marco-MiniLM, not cross-encoder), and the graph stream runs at **weight 0.3 in the live search** — 0.0 only in the benchmark call, so their published 95.2% was measured with graph *off* (comparability of that number to ours is thereby confirmed, not weakened). New shapes: **session diversification** (max 3 hits/session in result shaping), LLM query expansion, Porter stemming (closed), CJK via jieba (dep-gated), KV enrichment, write-path compress/consolidate/auto-forget/evict (consolidation closed; TTL exists here as `sweep`) | **Session diversification killed honestly**: in our measured shape (session-as-memory on every instrument we have, turn-granularity rejected §35/§37) a per-session cap is a **no-op** — recorded as shape-dependent, revisitable only if granularity ever changes. Everything else is closed or dep-gated. **No new candidates.** |
| **Hindsight** | Their README states retain "uses an LLM to extract key facts, temporal data, entities, and relationships", then normalizes into canonical entities; the dry-run API returns per-call **LLM token usage** (one extraction call per retain, batch-capable); recall = 4 parallel strategies (semantic/BM25/graph/temporal) + rerank. The non-LLM path that exists is **caller-supplied entities + resolver-side lookup** — precisely the shape memory-wire already ships as caller-supplied `tags` + include-any filtering (H6) | **Vetoed as default-build material** (LLM at ingest). The PMB 0.85-vs-0.24 delta is bought with that ingest LLM; the lexical approximation is already ours. Deficit decomposition complete — nothing further to port. |
| **MemOS** | The typed hierarchy is a 4-store MemCube (textual/activation/parametric/preference) + 10-scope node taxonomy; **types are hard pre-filters, never scoring weights** (memory_type `=` in retrieval); the self-evolving loop is the Dream plugin, `enabled_by_default=False`, a 5-stage LLM pipeline; retrieval is 4-way fusion (graph/vector/BM25/fulltext) with optional LLM reranker + LLM reasoner | **No candidates**: types are product-tax for a single-type memory server; Dream is LLM-gated + consolidation-shaped (closed); EnhancedBM25/StopwordManager/FastTokenizer are standard lexical furniture not worth a consultation. |
| **Honcho** | They model the *user* per peer-pair (observation documents at explicit/deductive/inductive/contradiction levels + bounded peer cards); write path is an async batched deriver — one LLM call + one batched embedding per work unit (512-token/1800 s gates); retrieval ranks the **derived representation**, not raw messages; their bench is GPT-4o-judged LongMemEval (per-type success + token efficiency). **One survivor**: `times_derived` — a DB-only counter incremented on exact-dedup re-derivation, used as a frequency fallback rank | **→ H11 registered below.** The rest is LLM/embedding-gated. Their published 90.4% is confirmed a self-report of a GPT-4o-judged, LLM-deriving system — the README label stands. |
| **OpenViking** | **V1 confirmed**: `overview_from_content()` (tiers.py:107) builds compact tiers **structurally** — cached preview for directories, AST skeleton for code, summary section or first-line/section extraction (`doc_overview`) otherwise; per-type Jinja templates render `.overview.md`. V3: Phase-2 extraction is an LLM ReAct ExtractLoop (batches at 150k tokens / 100 messages). Retrieval: dense+sparse C++ vector fusion + cross-encoder rerank. MCP surface: ~143 `@mcp.tool` registrations, **no** `@mcp.resource` — browsing is tool-shaped | **V1 source mechanism confirmed → H8 unblocked** (the capsule must be structural: `context` field / first-line — an LLM capsule would fail the gate exactly as pre-registered). V3 vetoed (LLM + consolidation-shaped). Vector/rerank core closed. **V4 stays a small tool-surface decision**, not a resources/list port. |
| **OmniMemEval** | Adapter contract: `add(messages, user_id, …)` / `search(query, user_id, top_k)` / `delete(user_id)`; backend via `--lib` + env file. **The LLM clients are OpenAI-compatible with `ANSWER_BASE_URL` / `EVAL_BASE_URL` env overrides** — an aroute-shaped shim satisfies both. HaluMem: 3,467 QA pairs, 20 users, 6 question types, `llm_judge_score` reported (±std), data **CC BY-NC-ND 4.0 — run it, never vendor it**. PersonaMem v2: MIT, 5,000 queries / 200 personas, multiple-choice accuracy. All pipelines are answer+judge; there is no retrieval-only mode | **H10 unblocked on the free tier** — the cost decision is now "shim via aroute small-stack (free, slow)" vs "paid keys (fast)". No closed-lever contact: the framework's own machinery is answer/judge-side only. |

**Net new work from the six audits:** H8 unblocked (source mechanism confirmed
structural), H10 unblocked (shim env vars exist), **one** new candidate
registered (H11 below), **one** candidate killed honestly (session
diversification — no-op in our shape), **one** comparability fact strengthened
(agentmemory's 95.2% was measured with graph weight 0.0, so the number we
compare against is exactly what their benchmark call ran). The ranking core has
**no** incoming mechanism from the field: every scoring-relevant idea in the six
repos is either a closed lever, dependency-gated, or a no-op at our granularity.
