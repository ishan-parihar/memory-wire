# RAW PAGE — the scrape as it stood at archive time

## SECTION 1: the DOM extraction (complete — not capped)

Architectural Verdict (Four Findings at a Glance)
Hook/Context-Budget Bottleneck — Worth fixing (context side); compute paths not a bottleneck. Storage/retrieval compute is already sub-30 ms ([PRIOR]), whereas every prompt turn spawns memory-wire hook and unconditionally injects up to the 8,000-token recall budget with best-of-bad-set noise (# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md, # FILE: docs/audit/2026-11-18-surface-audit.md). Post-upgrade (555d1aa) runtime performance is unmeasured and the binary (10,937,128 B) now breaches Gate 8's 9 MiB cap (# SECTION G-MEASURE).
Integration-Stack & Dead-Vector Redundancy — Worth fixing selectively. hygiene.rs, grounding.rs, and sweep.rs do not duplicate one another, and HTTP /mcp vs stdio mcp is host-driven. Real duplication sits in maintaining both Rust CLI hooks (src/hooks.rs + connect*) and external TS plugins (plugin/integrations/*) that diverge on bug fixes, plus dual instruction installers (--guidelines vs --with-skills) and 1,142 LOC of disabled vector.rs/embed.rs (# SECTION G-MEASURE, # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md).
Door & Capability Fragmentation — Worth fixing. "One service, three doors" is contradicted by the pack: capabilities are siloed across HTTP, MCP, CLI, hooks, and TS plugins. Retroactive redact (W3) has no MCP/CLI door or listed /redact HTTP route; grounding (W5) and BankConfig writes lack MCP/CLI controls; CLI lacks recall/retain; src/mcp.rs defines 6 tool constants while README.md claims 5 and tabulates 4 (# SECTION G-MEASURE, # FILE: README.md (lines 426,453), # FILE: README.md (lines 654,700)).
Per-Turn Context & Trust Integration Gap — Worth fixing. Even after W0–W7, hooks.rs injects un-gated min-max normalized noise, fails to exclude trust=untrusted_external on read, silently drops empty-framing output when no daemon runs, and ships W5 grounding off and W6 skills opt-in. Both OPEN_HOOK_RECALL_RELEVANCE.md and TODO.md miss several of these defects (# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 1,482), # FILE: TODO.md (lines 56,76)).
1. Bottlenecks
Note on evidence class across this section: G-MEASURE (555d1aa) measurements are [CERT]; v0.7.0/v0.8.0 baselines are [PRIOR] (and [UNCONFIRMED] range-quality where measured at load 11–28 on 24 cores); live omp observations are [LIVE].
Write path (capture + BankConfig + dedup + memory_versions): From the pack (# FILE: docs/ARCHITECTURE.md (lines 1,85), # FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 1,482) W1–W3): every write runs regex PII redaction (capture::redact_pii), read_only and max_trust checks, SHA-256 content hashing/dedup, and an append-only row insert into memory_versions. Dense-vector inference (19–30× write cost per # FILE: docs/handoff/CLOSED.md (lines 1,45)) is avoided because --features embed is off by default (# SECTION G-MEASURE, # FILE: research/audits/context-pack-2026-11-18-round2/SURFACE.md (lines 1,142)).
Measured cost: From the pack (# FILE: docs/handoff/STATE.md (lines 169,227) [PRIOR]/[UNCONFIRMED]): 2,018 ops/s retain (v0.7.0, synthetic write bench, load 11–28 on 24 cores, range-quality); ~283 µs puts (# FILE: README.md (lines 323,363) [PRIOR], v0.8.0, basis/load unspecified). Concurrency failed its 2-client gate (1.30× vs 1.50×, v0.7.0 under load 11–28).
Unmeasured cost: Not in evidence at 555d1aa — post-W1–W3 write latency/throughput with memory_versions writes and BankConfig checks, and quiet-box bench_concurrency (TODO.md #14), are absent. Not worth optimizing without a quiet-box measurement showing regression.
Recall path (SQL filters + FTS5 BM25 + RRF + min-max + budget trim): From the pack (# FILE: docs/ARCHITECTURE.md (lines 1,85), # FILE: docs/handoff/STATE.md (lines 169,227)): W1 narrows scopes, aspects, lifecycles, and excludes superseded/quarantined in SQL before ranking, then FTS5 BM25 + overlap + RRF score the candidate pool (fused list = whole bank on 261/261 LoCoMo misses, n=261, 19–32 sessions/bank) and DEFAULT_RECALL_BUDGET (8,000 tokens per # FILE: docs/audit/2026-11-18-surface-audit.md (lines 154,262) F10) truncates at the end.
Measured cost: From the pack: 7.2 ms p50 at 10k synthetic memories (# FILE: docs/handoff/STATE.md (lines 169,227), v0.7.0, [PRIOR]/[UNCONFIRMED] load 11–28 on 24 cores); 26.5 ms end-to-end LoCoMo retrieve (# FILE: README.md (lines 323,363), v0.8.0, [PRIOR], AMB rag, n=1,540 queries).
Unmeasured cost: Not in evidence at 555d1aa — post-W1 SQL filter overhead and index coverage for the 6 new columns are absent. Not a latency bottleneck worth tuning.
Hook path (five lifecycle events spawning OS processes + context-budget bottleneck): From the pack (# FILE: README.md (lines 511,576), # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180)): CLI-wired hosts (claude-code, codex, copilot-cli) spawn a new OS process for SessionStart, UserPromptSubmit, Stop, PreCompact, and SessionEnd — at least two spawns per turn (prompt + stop). Each spawn links 3 shared libraries (libgcc_s.so.1, libm.so.6, libc.so.6, # SECTION G-MEASURE), parses .git/config, and makes an HTTP call (recall_at(endpoint, ...)) or reads transcript_path (capped at 2,000 chars).
Measured cost: From the pack (# FILE: docs/handoff/STATE.md (lines 169,227)): cold start 13–16 ms (v0.7.0, [PRIOR]/[UNCONFIRMED], synthetic BENCH_COLDSTART, load 11–28 on 24 cores) — roughly 2× the 7.2 ms recall p50. Worse, UserPromptSubmit (recall_section) reads ScoredHit.content and created_at without gating on score, forcing up to 8,000 tokens (~10,000 characters observed [LIVE] in bank omp, 2026-09-30, # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180)) into the agent's context window on off-topic turns.
Unmeasured cost: Not in evidence at 555d1aa — end-to-end wall time of hook prompt (spawn + HTTP + recall) and pre-compact/session-end transcript parsing. The forced context-window bloat is worth fixing immediately; the 13–16 ms process spawn is secondary unless latency dominates host UX.
Startup path (grounding capsule + binary footprint): From the pack (# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 1,482) W5): grounding.rs (349 LOC) queries active preference|invariant|trap|map rows at policy|global|domain scope with scope_globs matching.
Measured cost: Capsule output is capped at ≤40 lines and ≤2048 bytes (~500 tokens in plan W5, vs ~1,000 tokens stated in surface-audit.md F10) and subtracted from DEFAULT_RECALL_BUDGET, defaulting to off (0 cost when disabled). Separately, binary size at 555d1aa is 10,937,128 B (10.43 MiB) ([CERT], # FILE: research/audits/context-pack-2026-11-18-round2/SURFACE.md (lines 1,142), default release build excluding --features embed), breaching plan §4 Gate 8 (≤ 9 MiB (from 8.8)).
Unmeasured cost: Not in evidence — grounding.rs scan latency at scale and 555d1aa idle RSS (Gate 8 ≤ 12 MiB). Worth verifying Gate 8 footprint/RSS once, not optimizing startup capsule compute.
2. Redundancies
Parallel hook & plugin integration stacks (Worth fixing — real duplication with bug drift): From the pack (# SECTION G-MEASURE, # FILE: research/audits/context-pack-2026-11-18-round2/SURFACE.md (lines 1,142)): across the authored src/ population (34,891 LOC excluding target/, research/, _audit/, eval/, genome/, tests/fixtures/), integration/connect/hook modules total 8,802 LOC (25.2%): connect.rs (2,841), hooks.rs (1,664), connect_plugin.rs (1,103), connect_codex.rs (857), connect_ext.rs (830), connect_opencode.rs (348), connect_claude.rs (324), connect_copilot.rs (306), skills.rs (278), guidelines.rs (251). Alongside Rust hooks.rs, # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180) documents two TypeScript integrations (plugin/integrations/extension/memory-wire.ts, plugin/integrations/opencode-plugin/memory-wire.ts) plus operant's in-process provider (crates/operant-core/src/memory_wire.rs).
Concrete duplication: operant independently reproduced hooks.rs's un-gated injection defect, and the 2026-10-08 mitigation (borrowing the prior turn's answer for prompts <160 chars and turn-<hash> retention dedup) shipped in the two TS plugins, not in src/hooks.rs. In addition, connect deploys two parallel static-guidance mechanisms: --guidelines (guidelines.rs, 251 LOC) and --with-skills (skills.rs, 278 LOC).
Dead vector/embedding arm (1,142 LOC) (Worth pruning or strictly isolating): From the pack (# SECTION G-MEASURE, # FILE: docs/handoff/CLOSED.md (lines 1,45)): src/vector.rs (965 LOC) + src/embed.rs (177 LOC) total 1,142 LOC (3.3% of src/), yet --features embed is off in all shipped builds and FusionWeights::vector is 0.0 (closed by H1d: recall_all@10 +1.8pp at vec 0.75 cost −9.9pp R@1, eval/SELECTION_VECTOR_AXIS_ALL10.md). How much of vector.rs compiles into the default binary vs behind #[cfg(feature = "embed")] is not in evidence, though default users never reach it.
Dual startup-context channels (Minor redundancy — watch, not urgent): From the pack (# FILE: README.md (lines 511,576), # FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 1,482) W5): hook session-start already reads BankConfig's static background/preamble/system_prompt from GET /banks/:id/config, and W5 adds grounding.rs (349 LOC, opt-in, default off) synthesizing a startup index from memories rows. My own inference: with W5 off by default and unexposed on MCP/CLI, grounding.rs risks becoming dead weight unless surfaced or unified with preamble.
Non-duplications (Explicit rejections — do NOT remove):
sweep.rs (276 LOC), hygiene.rs (645 LOC), and grounding.rs (349 LOC) do not duplicate capabilities: sweep hard-deletes by ttl_days, hygiene detects duplicate/contradiction/relative-date/bloat candidates non-destructively, and grounding formats active policy/invariant capsules (# FILE: TODO.md (lines 56,76) #26–27).
Stdio mcp.rs (1,873 LOC) vs HTTP mcp_http.rs (484 LOC) is host-required because Codex requires URL-based MCP (/mcp/{bank}) (# FILE: README.md (lines 654,700)).
daemon.rs (1,132 LOC) duplicates OS service managers, as docs/ARCHITECTURE.md concedes ("a deployment, not a dependency"), which is non-critical dead weight unless trimming binary size below 9 MiB.
3. Fragmentations
Testing "one service, three doors" and "there is no second implementation of recall" (# FILE: docs/ARCHITECTURE.md (lines 1,85)):
The core ranking math inside recall.rs is shared once calls reach MemoryService ([PRIOR]/[CERT] architecture claim). However, the "three doors" claim fails on evidence: there are at least four shipped doors (HTTP REST, MCP stdio/HTTP, CLI, and hooks.rs) plus external TS plugins, and capabilities are asymmetrically gated and formatted across them (worth fixing):
Capability	HTTP (api.rs / SURFACE.md)	MCP (mcp.rs / README.md)	CLI (README.md)	Hook / Plugin (hooks.rs / OPEN_HOOK...)
Recall	POST /banks/{id}/recall; W1 filters (plan W1)	memory_recall; W1 filters undocumented in README.md (not in evidence in src/mcp.rs)	None directly (only via hook)	hooks.rs calls HTTP /recall (no daemon = no recall); TS plugins rewrite <160-char queries, hooks.rs does not
Hygiene (W4)	Listed (id/hygiene in SURFACE.md)	Mismatched: plan W4 specified memory_hygiene (5th tool); SURFACE.md has memory_historian, no memory_hygiene	None (only TTL sweep)	None
Grounding (W5)	Modified src/api.rs; no /grounding route in SURFACE.md	None (and no config_set to opt in)	None	Intended on session-start, default off
Redact (W3)	Not listed in SURFACE.md Public HTTP routes	None	None	Write-time capture::redact_pii only
Delete vs Supersede	delete_memory is HTTP-only (TODO.md #29); no /supersede route in SURFACE.md	memory_supersede present; no hard delete	Only bulk TTL sweep	None (TS plugin uses document_id = "turn-..." upsert)
Bank Config / Stats	GET|PUT /banks/:id/config, /stats, /memories (read_only, max_trust)	memory_bank_config_get only (read-only); no stats or memories list	doctor prints summary counts only	session-start reads preamble via GET; read_only refusal swallowed by exit 0 hook contract
Seed / Sweep	None	None	seed, sweep CLI-only	stop/pre-compact/session-end retain transcripts
Surface-count drift (From the pack): # FILE: research/audits/context-pack-2026-11-18-round2/SURFACE.md (lines 1,142) (grep -oE 'pub const [A-Z_]+: &str = "memory_[a-z_]+"' src/mcp.rs at 555d1aa) finds 6 MCP tool constants (memory_retain, memory_recall, memory_reflect, memory_bank_config_get, memory_supersede, memory_historian). Yet # FILE: README.md (lines 654,700) says "The same five tools", lists 4 in its table, and # FILE: TODO.md (lines 56,76) labels both W4 and W7 "the 5th tool". (Whether all 6 constants are registered in tools/list is not in evidence; likewise README.md (lines 426,453) lists 9 CLI subcommands while README.md (lines 323,363) and src/update.rs (613 LOC) also reference memory-wire update.)
4. Integration Gaps Toward an Optimal Agentic Experience
Walking a live agent session (from the pack, primarily # FILE: README.md (lines 511,576), # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180), # FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 1,482)):
Install (connect): Unless the user adds --with-skills, SKILL.md is never copied (TODO.md #28), so the agent receives no guidance on scope/aspect/lifecycle/trust or memory_supersede. connect also splits hosts unevenly (claude-code, codex, copilot-cli get hooks; cursor, opencode get MCP; codex gets HTTP /mcp/{bank}).
Session Start (SessionStart hook -> hook session-start): Injects static preamble (if configured) + recall. Because W5 grounding.rs defaults off and MCP lacks config_set, no structured invariant/trap/preference capsule arrives out of the box. If serve/daemon is down, the hook prints empty local framing and exits 0, indistinguishable from an empty bank (violating AGENTS.md §10). If a repo's origin remote changed the derived bank ID from directory basename to owner-repo, the agent silently starts in an empty bank (only doctor warns).
Every Prompt Turn (UserPromptSubmit hook -> hook prompt): Injects up to DEFAULT_RECALL_BUDGET (8,000 tokens) formatted as ### memory-wire recall — \<bank>`with(recalled by relevance from bank `<id>`, unverified)` and per-entry dates. Three concrete defects hit the agent's context:
Irrelevant best-of-bad-set flood: recall_section ignores ScoredHit.score (W7b closed without gating because min-max normalization forces the top hit of noise to 1.0). Off-topic prompts inject up to ~10k chars of irrelevant memories ([LIVE] in omp).
No read-time trust gate: W2 only downgrades untrusted_external on write so it cannot become scope=policy; not in evidence that hooks.rs excludes trust=untrusted_external from UserPromptSubmit recall. Poisoned external writes are still injected directly into context.
Short-prompt blindness on CLI hooks: Prompts <160 chars (e.g. "Continue") borrow the previous answer only in TS plugins (2026-10-08 mitigation), not in src/hooks.rs or on turn 1 of a fresh session.
Mid-Turn Reasoning (MCP tools): The agent cannot retroactively redact a leaked secret, hard-delete, inspect bank stats/rows, or enable grounding/read_only via MCP. Automatic hook retention writes untyped prose (defaults local/episodic), leaving W5 capsules empty unless the agent manually curates or runs an external hygiene apply loop.
Turn End / Compaction (Stop, PreCompact, SessionEnd hooks): stop logs a one-line pointer; pre-compact/session-end retain raw transcript prose capped at 2,000 chars (README.md) — losing claim boundaries (CLOSED.md showed turn fragments perform worse than whole sessions on LoCoMo) and silently dropping memories when the server is down or read_only (hooks exit 0).
Completeness audit of docs/OPEN_HOOK_RECALL_RELEVANCE.md and TODO.md:
Both documents are incomplete (and TODO.md rows 21–29 are stale open despite plan §6 marking W0–W7 shipped). Missing from neither list (or punted unresolved):
Read-time exclusion/quarantine of trust=untrusted_external on automatic hook recall (hooks.rs). (W2 guarded writes only.)
Surface parity & doc truth: MCP/CLI exposure of W3 redact and BankConfig writes (W5 opt-in), documenting memory_historian/memory_supersede, and reconciling the 4-vs-5-vs-6 MCP tool count (SURFACE.md vs README.md vs TODO.md).
Silent failure modes on hooks: indistinguishable server-down empty framing (AGENTS.md §10 violation) and silent bank-ID shift on origin migration.
Post-W0–W7 555d1aa Gate-8 verification: binary grew to 10.43 MiB vs ≤9 MiB gate, with zero 555d1aa latency/RSS numbers.
Unblocking recall_section relevance: OPEN_HOOK_RECALL_RELEVANCE.md noted raw un-normalised BM25 would be needed across the wire, yet W7b closed (eb95a0a) with only a no-raw-score-gate decision doc, leaving the relevance defect unfixed without adding raw BM25 or a hook relevance dev set.
5. What the Next Iteration Should Be
Single highest-value next move: Ship a read-time relevance & trust gate on the per-turn hook path (src/hooks.rs + src/api.rs/src/recall.rs), evaluated solely on the frozen relevance battery + a labelled hook-injection dev set.
Concretely (one cohesive change):
Carry un-normalised lexical relevance signals (e.g., raw FTS5 BM25 + matched-term coverage ratio, alongside the existing normalized RRF score) across ScoredHit (# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180)).
In hooks.rs (recall_at / recall_section), filter out trust=untrusted_external on automatic prompt injection by default (extending W2 from write-time downgrade to read-time isolation) and suppress injection completely (emitting nothing rather than best-of-bad-set prose) when raw lexical relevance falls below a threshold pre-registered on a labelled hook dev set (# FILE: AGENTS.md (lines 203,281), # FILE: eval/baseline/README.md (lines 21,89) battery / BATTERY-2026-10-16-frozen.txt).
Why this beats every alternative (pack + web-sourced anchors):
Pack anchor: F10 (# FILE: docs/audit/2026-11-18-surface-audit.md (lines 154,262)) proves the 8,000-token context budget is the dominant cost axis; # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180) records ~10,000 chars of live irrelevant context poisoning real turns; and # FILE: docs/handoff/CLOSED.md (lines 1,45) already closed retrieval-weight, stemming, vector, and granularity tuning. Every other improvement (W5 capsules, W4 hygiene, MCP verbs) is degraded while UserPromptSubmit floods every turn with un-gated, potentially untrusted text.
From a cited source (web-sourced, read 2026-11-19): In When Malicious Instructions Persist: Persistent Memory Poisoning Attack on Harness-Based Agents (https://doi.org/10.48550/arXiv.2609.13889), Huang et al. (Sep 2026) show that untrusted external content written into persistent memory achieves 81.7% cross-session attack success on Claude Code (66.9% injection success) while preserving normal benign task behavior, and explicitly find that prompt-level defenses provide limited protection once poisoned memories are retrieved [[1](https://www.google.com/url?sa=E&q=https%3A%2F%2Fvertexaisearch.cloud.google.com%2Fgrounding-api-redirect%2FAUZIYQHPuX3phuNx3A_yakar-ucFJWsvX7Wt9Rluo651SFHas34yOiiWclr9HQMAK-xRw32oCM_aKqOFQBpBLeXrS7OtkC-FlHawHlM8jmkeYKbveTUm3u6U5Q%3D%3D)]. That directly invalidates relying on W2's write-downgrade + (..., unverified) banner alone while hooks.rs still injects untrusted_external rows. Likewise, Corrective Retrieval Augmented Generation (CRAG, https://api.semanticscholar.org/CorpusID:267312595, read 2026-11-19) establishes placing a retrieval-confidence evaluator between retrieval and generation to reject/suppress poor retrievals instead of treating top-K rank as relevance [[2](https://www.google.com/url?sa=E&q=https%3A%2F%2Fvertexaisearch.cloud.google.com%2Fgrounding-api-redirect%2FAUZIYQFYvDcjXbBivHqyjyLn5UjSmYSOvpKEWkL4ix0GuvyXXYsnHlvb4eD9qGNYdJije2R_5WxV_u88_ZT2FubhYoGfHHoNs9kHowCT1dCnYaNkNnFB4m13bQk%3D)].
Single-binary discipline preserved: Pure Rust, zero new dependencies, DT_NEEDED=3, no LLM, no daemon required (# FILE: AGENTS.md (lines 26,63)).
The cheapest thing that would be a mistake to do instead:
Adding a 3-line score cut on the already-returned ScoredHit.score in src/hooks.rs (recall_section). It is the cheapest possible code edit, and it is mathematically broken: # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180) (recall.rs:319-337) shows BM25 magnitude is min-max normalized per query, forcing the best hit of a completely irrelevant corpus to 1.0. (Equally fatal: turning on --features embed / dense vectors for hooks, which failed gate H1d in # FILE: docs/handoff/CLOSED.md (lines 1,45) and adds ~3× binary size and cold ONNX load risk on per-turn spawns.)
What would have to be true for the opposite choice to be right:
For leaving recall_section ungated / prioritizing missing MCP CRUD (memory_redact, config_set) instead to be right: (a) off-topic turns in real agent workflows would have to rarely trigger false-positive keyword overlap against large developer banks (contradicted by omp's ~10,000-char injections in # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md), AND (b) agents would have to never retain untrusted_external content (e.g., web fetches/PR comments) into shared project banks (contradicted by W2's motivating threat model in # FILE: TODO.md (lines 56,76) #24 and PMPA [[1](https://www.google.com/url?sa=E&q=https%3A%2F%2Fvertexaisearch.cloud.google.com%2Fgrounding-api-redirect%2FAUZIYQHPuX3phuNx3A_yakar-ucFJWsvX7Wt9Rluo651SFHas34yOiiWclr9HQMAK-xRw32oCM_aKqOFQBpBLeXrS7OtkC-FlHawHlM8jmkeYKbveTUm3u6U5Q%3D%3D)]).
For replacing automatic per-turn hook injection with agent-initiated memory_recall + W5 capsule only to be right: coding agents would have to reliably invoke memory_recall unprompted before answering (not in evidence in the pack, and directly why hooks.rs exists).
(All findings above are from the pack unless explicitly tagged not in evidence, my own inference, or from a cited source (web-sourced).)
Sources
help
arxiv.org
arxiv.org
Google Search Suggestions
Display of Search Suggestions is required when using Grounding with Google Search. Learn more
"Corrective Retrieval Augmented Generation" OR "adaptive retrieval" skip retrieval threshold BM25 agent hook
"Memory poisoning" "81.7" "Claude Code" arXiv OR paper 2026

## SECTION 2: the markdown read (may be truncated by the read tool)

ExplorePlaygroundHistoryBuildNew appMy appsGalleryManageDashboardDocumentation

# Reviewing Rust Agent-Memory Architecture

66,971  tokens

 User 3:59 AM

# Reviewer prompt — memory-wire, the shipped architecture, round 2

You are reviewing a small Rust agent-memory service that has just finished a
multi-quarter upgrade. You have no session history and no repo. The pack is all
you know. **Treat the pack's claims as CLAIMS TO AUDIT, not as ground truth.**
Where the pack does not settle a question, say **"not in evidence"** and continue
under a stated assumption — do not fill the gap with plausible generalities.

## What changed since the last round

Round 1 reviewed a plan. That plan has since been **implemented and shipped**:
schema columns, mount profiles, retroactive redaction, a deterministic hygiene
scan, a startup grounding capsule, an MCP supersede verb, a connect --with-skills
installer — then five batches fixing audit findings in those changes. The tree is
555d1aa. **This round reviews the thing, not the plan.**

## The decision this round exists to test

The project's stated goal is to be **the cheap option**: a single static binary
(10,937,128 B per G-MEASURE), 3 shared libraries, no daemon required, retrieval
that beats the one competitor we can compare against. The upgrade added a lot to
that binary.

The question is not "is each feature correct" — the repo says each is tested. It
is:

> **Where is the architecture now — as a system an agent actually lives inside —
> and what should the next iteration be?**

Name four things, and for each say whether it is a **bottleneck**, a
**redundancy**, a **fragmentation**, or an **integration gap**, and whether it is
worth fixing at all. A "none here, and here is why" is a valid answer.

## Five questions

1. **Bottlenecks.** Where does the system force work onto a path that cannot
avoid it? Think about: the write path (redaction, hashing, dedup, version
rows), the recall path (FTS5, fusion, budget trim), the hook path (five
events, each spawning a process), the startup path (grounding capsule).
Name the measured cost where the pack gives one; name it as unmeasured
where it does not.
1. **Redundancies.** The surface now carries HTTP routes, 5 MCP tools, a CLI,
a daemon, an installer across six hosts, a plugin, a skill file, a hygiene
module, a grounding module, a sweep module. Which of these duplicate a
capability another already has? Which are dead weight a user never reaches?
Name the duplication, not a style preference.
1. **Fragmentations.** The architecture doc says "one service, three doors" and
"there is no second implementation of recall". Test that claim. Is
recall really reached identically from HTTP, MCP, the CLI and the hook?
Does hygiene have a surface? Does grounding? Does redact? Where does
a capability exist in one door and not another?
1. **Integration gaps toward an optimal agentic experience.** If you were the
agent whose context this fills, what is still wrong after everything
shipped? Be concrete: what arrives in your context, at what moment, from
which door, and what is wrong with it. The pack carries
docs/OPEN_HOOK_RECALL_RELEVANCE.md — a defect diagnosed and deliberately
unfixed — and it carries TODO.md, which lists what the project thinks is
left. Judge whether those lists are **complete**: name what is on neither.
1. **What the next iteration should be.** Given 1–4, what is the single
highest-value next move — one item, not a roadmap — and what is the
cheapest thing that would be a mistake to do instead? Defend the choice
with a named anchor, and say what would have to be true for the opposite
choice to be right.

## Constraints on your answer

- **Every finding names an anchor**: # SECTION <NAME> or # FILE: <path>.
A finding with no anchor is an assumption — label it one.
- **Every figure keeps its three attributes**: population, basis, selection
constraint. A number without them is not usable. G-MEASURE carries the
surface count with its exclusions named; if you cite a count, say which
population it is over.
- **Questions 1–4 are pack-only.** The pack is the only admissible evidence.
If it does not settle one, say not in evidence — do not answer it from the
web.
- **Question 5 is open to outside sources.** If the wider agentic-memory field
has a mechanism this system should know about, cite it inline with its URL
and the date you read it, and mark it as web-sourced. A web-sourced finding
without a URL is an assumption.
- **A rejection is as useful as a finding.** If you conclude the architecture
is correctly scoped and the lists are complete, say so and say why. Do not
manufacture work to be helpful.

Reply in markdown, ≤ 2,500 words, numbered findings matching the questions above.
For each finding, say which it is: **from the pack** (name the # SECTION or
# FILE: anchor), **from a cited source** (give the URL), **my own inference**,
or **not in evidence**.

--- PACK (inline) ---

# CONTEXT PACK

**NOTE: the markdown read above is TRUNCATED** (it returned 4966 chars, at the read tool's ~5 kB cap). Section 1 is the complete extraction and is the one to re-read.