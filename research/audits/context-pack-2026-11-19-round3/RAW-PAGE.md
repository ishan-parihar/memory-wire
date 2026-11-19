# RAW PAGE — the scrape as it stood at archive time

## SECTION 1: the DOM extraction (complete — not capped)

Verdict on the Author’s Reading
Half right, half falsified by the pack’s own artefacts:
"Ordering is largely solved" is overstated. It holds on whole-session, tiny-bank QA benchmarks (19–32 sessions on LoCoMo [CERT], and LongMemEval-S [IN-SAMPLE] at overlap 0.25) and breaks down the moment the corpus resembles production granularity or bank size (BEAM-100k: 39.9% R@1 [CERT]; LoCoMo Turns: -5.8 pp R@1 [CERT]; PrecisionMemBench: 0.24 precision [CERT]). Stop spending cycles tuning recall.rs, not because ordering is solved everywhere, but because single-shot ranking tweaks over raw prose (H1a–H1d, H2, H3) have hit a structural ceiling.
The product-surface gap is real, but B-DELTA D3 is already stale. # FILE: research/audits/context-pack-2026-11-19-round3/B-DELTA.md (lines 1,83) claims docs/ARCHITECTURE.md says "five modules" and "three doors", whereas # FILE: docs/ARCHITECTURE.md (lines 1,100) in # SECTION A-STATE already reads "nine modules" and "four doors" and cites docs/INTEGRATION_GAPS.md (2026-11-19 addendum) and TODO.md row 31. Do not spend cycle effort on D3.
Pack-level evidence-class audit: # FILE: research/audits/context-pack-2026-11-19-round3/C-ASK.md (lines 1,70) falsely asserts "Every number in this pack is [LIVE] and traceable to an artefact." Only B-DELTA and SURFACE.md are [LIVE]; STATE.md, CLOSED.md, and RND_PLAN.md hold October [CERT] snapshots (some superseded, e.g., PMB 14/77 → 19/77, 17.0% → 18.6%, 10.25 MiB → 10.625 MiB [LIVE]); overlap 0.25 LongMemEval scores (97.2% R@5) are [IN-SAMPLE]; and README.md’s ≈94.4% out-of-sample est (# FILE: README.md (lines 323,363)) has no committed artefact ([UNCONFIRMED] / not in evidence).
Where the next cycle should go: Complete and harden the shipped MemoryService door/host surface (Rank 1–2), then close the upgrade migration cliff (Rank 3). Do not run another FusionWeights R&D cycle, re-run benchmarks, add score thresholds, or redo D3.
Audit of the Four Specific Questions
1. Is "ordering is solved" an artefact of the instrument?
Yes. Auditing value, population, and selection constraint on every headline number shows four instrument distortions:
Metric	Value & Class	Population	Selection Constraint / Failure Mode
LongMemEval pool 500/500, R@20 99.6% (# FILE: AGENTS.md (lines 26,63), # FILE: docs/handoff/CLOSED.md (lines 1,45))	500/500 [CERT]	500 LongMemEval-S questions (including 30 _abs abstention questions vs 470 in # FILE: docs/handoff/STATE.md (lines 218,276)) stored as whole sessions	Whole-session ingest; LongMemEval-S bank size and pool cutoff are not in evidence (would settle: inspect examples/longmemeval.rs). Counting _abs questions in 500/500 treats unanswerable prompts as having gold in-pool.
LongMemEval R@1 83.8%, R@5 97.2%, R@10 98.6% (# FILE: docs/handoff/STATE.md (lines 218,276))	97.2% [IN-SAMPLE]; 93.0% unfitted [CERT]	n=500 whole sessions	overlap 0.25 was fitted over 46 configs on LongMemEval itself (CLOSED.md). STATE.md places 83.8% / 98.6% / 88.2% in the fitted row without [IN-SAMPLE] tags or unfitted R@1/R@10/NDCG@10 equivalents (not in evidence).
LoCoMo recall_any@10 94.2%, pool 100% (# FILE: docs/handoff/STATE.md (lines 218,276))	94.2% [CERT] (1,442/1,531)	1,531 queries across 19–32 whole-session banks (9 of 1,540 AMB queries dropped)	STATE.md H1.0 admits fused list = whole bank on 261/261 misses: on 19–32 document banks, 100% pool coverage means the pool equals the entire bank.
Production granularity & larger banks (# FILE: TODO.md (lines 18,46) #9/#12, # FILE: docs/handoff/STATE.md (lines 218,276))	LoCoMo Turns R@1 54.6% (-5.8 pp) [CERT]; W=2 R@pool 98.7% [CERT]; BEAM-100k R@1 39.9% (R@5 63.6%, R@10 76.4%) [CERT]	Production hooks write turn pairs or 2,000-char slices (# FILE: README.md (lines 514,579)); BEAM-100k has n=313/400 resolved turn-pair queries over 27–50 docs	When LoCoMo is segmented closer to production writes, both ranking and pool reachability drop (W=2 blows past pool ceiling). On BEAM-100k, fitted overlap 0.25 underperforms unfitted 1.00 (39.9% vs 42.5% R@1; 63.6% vs 67.1% R@5).
Precision & drowning (# FILE: TODO.md (lines 18,46) #7/#17, # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180))	PMB 19/77, precision 0.24 [CERT]; H4 92–94% [CERT]	77 PMB cases; 200 H4 LoCoMo queries	H4 rejected context-drowning by selecting 200 queries whose gold was already inside top-3 on 19–32 session banks (k=10 = 31%–53% of bank). Meanwhile PMB precision is 0.24 (563/567 pre-filter failures = unexpected beliefs) and live hooks dumped ~10k chars of off-topic logs.
2. recall_all@10 83.0% vs recall_any@10 94.2% — ordering or architectural/session-writing?
Architectural / representation problem, not ordering. Check the arithmetic from # FILE: docs/handoff/RND_PLAN.md (lines 45,64) and # FILE: docs/handoff/STATE.md (lines 218,276) ([CERT], n=1,531, 261 misses):
Decomposing the 1,531 queries (89 multi-hop + 268 single-hop >=2 gold + 42 residual single-hop 1 gold + 292 temporal + 840 open-domain):
Multi-hop misses: 13/58 (1 gold) + 10/14 (2) + 6/7 (3) + 10/10 (>=4) = 39/89 (43.8%).
Single-hop >=2 gold misses: 75/163 (2) + 40/56 (3) + 43/49 (>=4) = 158/268 (59.0%).
Combined plural (>=2 gold) miss rate: 184 / 299 = 61.5% (>=4 gold: 53 / 59 = 89.8%). Plural queries account for 184 / 261 = 70.5% of all recall_all@10 misses.
Why ordering cannot fix it: Even inside 19–32-session banks, 99 / 261 = 37.9% of worst-ranked gold sessions sit at rank 21+ (bottom third of the bank). Six independent ranking/granularity interventions failed (# FILE: docs/handoff/CLOSED.md (lines 1,45), # FILE: docs/handoff/RND_PLAN.md (lines 134,209)): ExactAndPrefix (+0.5 pp), BestSentence (-0.3 pp), bm25_magnitude (-0.2 pp), Recency (-7.8 pp), Turns (-2.3 pp), Windows(2) (-0.4 pp), and even dense vectors (H1d, vec=0.75) rescued only 4 / 99 deep 21+ misses while costing -9.9 pp R@1.
Root cause: Multi-topic conversational logs dilute individual facts across whole sessions, single-shot retrieval cannot chain intermediate facts across hops (Session A → Session B) when Session B lacks query terms, and k=10 over raw sessions breaks on exhaustive list-all queries. On AMB LoCoMo rag ([CERT], n=1,540), judge accuracy drops from 91.2% (1,289) when all gold sessions are served to 53.0% (166) when only some are served (# FILE: docs/handoff/STATE.md (lines 218,276)), confirming the penalty is real — and unreachable by FusionWeights.
3. The door asymmetry (redact / TODO.md row 31) — worse or better than it looks?
It is both worse structurally and riskier to patch naively than the prompt’s framing suggests:
Worse than it looks (for an agent):
memory_supersede is a secret-retention trap on MCP. MCP exposes memory_supersede (W7a), which marks a row superseded while explicitly keeping it in SQLite and include_history (# FILE: README.md (lines 657,709)). An MCP agent with no redact tool that tries to retract a leaked credential via memory_supersede leaves the secret on disk.
The maintenance workflow is severed across incompatible doors. # FILE: docs/INTEGRATION_GAPS.md (lines 446,489) ([LIVE]) shows hygiene (W4, finding duplicate/merge candidates) is HTTP-only, memory_supersede (W7a, resolving them) is MCP-only, and CLI has neither. No caller on any single door can inspect hygiene and supersede candidates.
grounding (W5) is inaccessible on hookless MCP hosts. grounding is CLI + SessionStart (default off); hosts without hooks (cursor, opencode, omp in # FILE: docs/INTEGRATION_AUDIT.md (lines 231,274)) have no HTTP route or MCP tool to pull the startup capsule. And CLI lacks recall entirely (INTEGRATION_GAPS.md).
Better than it looks (in local deployments):
Primary hosts (claude-code, codex, opencode, omp, copilot-cli) run on the local machine where memory-wire serve binds loopback (rmcp rejects non-loopback Host headers, # FILE: README.md (lines 657,709)), so local shell-capable agents can invoke memory-wire redact via CLI, and write-time PII redaction (memory module) handles known patterns upfront.
Hidden hazard if redact is exposed blindly:
# FILE: README.md (lines 657,709) documents that both HTTP and /mcp are completely unauthenticated and every MCP tool currently sets destructiveHint: false. Exposing a bank-wide or arbitrary-pattern destructive redact over unauthenticated HTTP/MCP without destructiveHint: true, narrow per-memory_id / explicit scoping, and verifying REST Host-header protection widens a prompt-injection / local-process blast radius.
4. What the pack structurally cannot show you
End-to-end coding-agent utility and harm: All retrieval benchmarks test offline single-prompt QA on whole dialogue transcripts. TODO.md #10 (agent-memory-bench Claude Code arm — "the only external benchmark that grades the product's actual job — task success with hooks+MCP wired — and it has a harm suite") is open, and # FILE: docs/INTEGRATION_AUDIT.md (lines 231,274) §4 #6 notes tests run against fixture directories (1 real omp assertion in history; tests/*.rs is 1,307 LOC vs 11,648 LOC of benchmark examples/*.rs in # FILE: research/audits/context-pack-2026-11-19-round3/SURFACE.md (lines 1,104) [LIVE]). Whether real turn-pair noise (pre-compact 2,000-char tails, stop markers) helps or actively misleads multi-session coding tasks is not in evidence.
Non-Rust client drift outside measure_surface.sh: # FILE: research/audits/context-pack-2026-11-19-round3/measure_surface.sh (lines 1,69) counts only src/*.rs (35,295 LOC), omitting plugin/ (Python/TS integrations) and install/. Because non-Rust plugins replicate recall/injection logic (D1 had to separately patch plugin/integrations/hermes/memory-wire/__init__.py), silent divergence across plugin/ files is structurally invisible to Rust tests and surface scripts.
Multi-process SQLite concurrency & REST security boundaries: Because CLI and stdio mcp run without a required daemon while serve + hooks hit HTTP (README.md), concurrent WAL locking / stale process state under simultaneous serve + mcp stdio + doctor (PASSIVE checkpoint) is unmeasured by single-process bench_concurrency (TODO.md #14). Likewise, whether Axum REST routes enforce rmcp's loopback Host-header guard against DNS rebinding/CSRF is not in evidence.
Selection bias in rejecting capabilities (CLOSED.md): Turning off finer granularity (Turns / W=2) because it scored lower on 19–32-session LoCoMo ignores that production hooks already write turn pairs; rejecting context-drowning via H4 (top-3 guaranteed subset) ignores 0.24 PMB precision; rejecting extraction/consolidation because competitors turn it off on QA ignores 61.5% plural misses and 0.85 vs 0.24 PMB precision.
Ranked Action Plan (by Value per Unit of Work)
Rank 1 — Complete the MemoryService Door Surface & Scoped redact Contract (TODO.md row 31, expanded)
What: Close the arbitrary door fragmentation across one MemoryService:
Expose hygiene on MCP (memory_hygiene) and CLI (Cmd::Hygiene) so callers that can supersede can discover duplicates.
Expose supersede on HTTP (POST /banks/:id/memories/:mid/supersede) and CLI (Cmd::Supersede), and recall on CLI (Cmd::Recall) to honour # FILE: AGENTS.md (lines 294,327) §10 fetch workflows.
Expose grounding (W5) on MCP (memory_grounding) and HTTP (GET /banks/:id/grounding) so hookless MCP hosts can fetch startup capsules.
Add narrowly scoped redact to MCP (memory_redact, with destructiveHint: true, scoped by memory_id or explicit target rather than silent bank-wide wipe) and HTTP (POST /banks/:id/memories/:mid/redact or verified pattern endpoint) after confirming REST routes share rmcp's non-loopback Host rejection (currently not in evidence).
Fix memory_supersede documentation/description to state explicitly that superseding retains the original row in history and does not purge secrets.
Where: TODO.md row 31, docs/INTEGRATION_GAPS.md (2026-11-19 addendum), src/mcp.rs, src/api.rs, src/http.rs, src/main.rs, README.md (lines 679–693).
Why it matters to an agent: Today, no door lets an agent run the full lifecycle (grounding → recall → hygiene → supersede / redact). An MCP agent cannot run duplicate cleanup (hygiene missing), cannot receive grounding on hookless hosts, and if it accidentally retains a credential via memory_retain, cannot purge it via MCP and risks calling memory_supersede while leaving plaintext credentials on disk.
Argue for/against as #1 & what would have to be true: All five underlying operations already exist and are tested in MemoryService (src/api.rs), making this thin, dependency-free adapter work (0 binary-footprint or retrieval-latency penalty) — the highest value per unit of work on the table. Trade-off: Giving up a smaller 5-tool MCP prompt budget (adding hygiene, grounding, redact → 8 tools) and adding explicit destructive-verb surface on an unauthenticated loopback server. Worth a cycle iff REST Host-header validation is verified/added alongside destructiveHint: true (if you keep redact CLI-only for security on unauthenticated servers, you must still ship hygiene/supersede/grounding parity and document memory_supersede non-deletion).
Rank 2 — Audit TS Plugin D1 Trust-Gate Parity & Resolve the Host Hook/MCP Split
What:
Audit and close the D1 (untrusted_external) check on non-Rust plugins: # FILE: research/audits/context-pack-2026-11-19-round3/B-DELTA.md (lines 1,83) (D1) patched src/hooks.rs and plugin/integrations/hermes/memory-wire/__init__.py to drop untrusted_external before auto-injection. # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,180) names two other in-repo auto-injecting plugins — plugin/integrations/extension/memory-wire.ts (before_agent_start) and plugin/integrations/opencode-plugin/memory-wire.ts (experimental.chat.system.transform). Whether they request format carrying trust and drop untrusted_external is not in evidence (settled by reading those two files and adding regression tests).
Resolve the host capability split (INTEGRATION_AUDIT.md §3): Verify and converge connect*.rs so supported hosts get both auto-injection hooks and MCP tools (reconciling TODO.md #4's connect_codex.rs / connect_claude.rs / connect_copilot.rs against # FILE: docs/INTEGRATION_AUDIT.md (lines 231,274) §3 and # FILE: README.md (lines 514,579)), and ship golden-fixture / host-smoke verification (TODO.md #4 P2.1).
Where: plugin/integrations/extension/memory-wire.ts, plugin/integrations/opencode-plugin/memory-wire.ts, docs/INTEGRATION_AUDIT.md §3–4, src/connect*.rs, TODO.md #4/#30, README.md (lines 514–520, 673).
Why it matters to an agent: Rank 1's MCP tools are unreachable on any host whose connect installs hooks only (❌ tools), while tools-only hosts start blind with zero SessionStart grounding or PreCompact/SessionEnd capture. Worse, if either TS plugin still injects untrusted_external rows verbatim, the Round-2 indirect prompt-injection vulnerability remains live there.
What would have to be true: The TS audit takes hours; full host convergence is worth the remaining cycle iff those hosts (claude-code, codex, copilot-cli, cursor, opencode, omp) are actively supported product targets (P2.1 golden fixtures required so host config drift stops recurring). Trade-off: Ongoing maintenance against 7 upstream host config formats (P2.1 fixture drift) vs concentrating on fewer verified hosts.
Rank 3 — Explicit Bank-Migration Command for the basename → owner/repo Upgrade Cliff
What: Provide an explicit, transactional CLI migration/merge command (memory-wire bank migrate <from> <to> with --dry-run, content-hash dedup, and explicit confirmation) alongside doctor's warning when a git origin bank replaces an existing directory-basename bank.
Where: # FILE: README.md (lines 514,579) (Upgrading moves your bank name, and nothing migrates for you), src/doctor.rs, src/main.rs, src/store.rs (docs/BANK_IDENTITY.md referenced).
Why it matters to an agent: Immediately after memory-wire update, hooks and MCP resolve a fresh empty owner-repo bank. An agent does not run doctor on its own, so from the agent’s perspective all project memories vanish silently across an upgrade until a human intervenes with --bank <old>.
What would have to be true: Small, deterministic SQLite work (preserves the author's correct ban on silent auto-migration). Worth half a cycle iff pre-remote-bank installs exist in the wild (install base is not in evidence). Trade-off: Minor CLI surface expansion for upgrade continuity.
Rank 4 — Zero-LLM Scope Precision (narrow_any) + Live Coding-Agent Validation (TODO.md #10)
What: Replace RND_PLAN.md H-items with two non-ordering checks that stay within C-LAW (~10.6 MiB, 3 shared libs, no LLM on ingest/recall):
Honour narrow_any / deterministic alias-normalized tag matching flagged in TODO.md #7 (PrecisionMemBench precision 0.24, 563/567 pre-filter failures = unexpected beliefs).
Run TODO.md #10 (agent-memory-bench Claude Code arm with harm suite) — a new end-to-end coding harness, not a benchmark re-run — to measure whether production turn-pair/2k-char hooks actually degrade coding-agent task success or recall R@1.
Where: TODO.md #7, #10, #12; eval/amb/memory_wire.py / src/recall.rs (narrow_any); docs/handoff/RND_PLAN.md §1.
Why it matters to an agent: BEAM-100k (39.9% R@1), LoCoMo Turns (-5.8 pp R@1), and PrecisionMemBench (0.24 precision) all signal that real multi-turn fragmented banks perform far worse than whole-session QA suggests, yet zero evidence exists on real coding tasks.
What would have to be true: Worth a cycle iff external agent-model budget for TODO.md #10 is approved and you commit to halting FusionWeights tuning (RND_PLAN.md H-items). Trade-off: External API spend for agent-memory-bench (not in evidence how much) and risking proof that turn-pair capture needs structural context headers (file/branch/topic) at capture time.
Rank 5 (Bundle into Ranks 1–2, Do Not Spend a Standalone Cycle) — Reconcile Doc/Script Contradictions
What: Fix verified pack/doc drift, while explicitly dropping B-DELTA D3 (already updated in docs/ARCHITECTURE.md):
docs/ARCHITECTURE.md: remove stale claim "Redaction happens on write, not on read. A secret is not stored, so it cannot be recalled" (contradicts Cmd::Redact W3).
README.md (lines 323–363, 515, 673) & AGENTS.md §1 & INTEGRATION_AUDIT.md §3: reconcile v0.8.0 vs v0.10.0, 10.25 MiB / musl DT_NEEDED=0 vs 11,141,240 B (10.625 MiB) / 3 glibc libs (SURFACE.md [LIVE]), "the two that speak it (cursor, opencode)" vs codex/claude/copilot connectors, 4 vs 5 MCP tools, and anchor or drop ≈94.4% out-of-sample est.
STATE.md: tag 83.8% / 98.6% / 88.2% on LongMemEval-S as [IN-SAMPLE] (overlap 0.25) or publish unfitted baselines.
measure_surface.sh: exclude memory_historian (prompts/list, not a tool) from tool count, capture /mcp/{bank} and HTTP methods, and measure plugin/ + install/.
Where: B-DELTA.md D3, docs/ARCHITECTURE.md, README.md, AGENTS.md §1, docs/INTEGRATION_AUDIT.md §3, docs/handoff/STATE.md, measure_surface.sh.
Why it matters to an agent: Coding agents and maintainers relying on README.md/INTEGRATION_AUDIT.md get false host capabilities, outdated tool counts, and unflagged [IN-SAMPLE] accuracy claims.
What would have to be true: Zero standalone cycle justification; update inline as closeout to Ranks 1–2.
Not Funded / Settled (Excluded by Evidence)
Score thresholds on hooks::recall_section: Settled and rejected (C-LAW, D-EVIDENCE CLOSED.md; per-query min-max normalization makes best-of-bad-set score 1.0).
Additional recall.rs / FusionWeights cycles (RND_PLAN.md H1–H3) or dense-vector default (--features embed): Closed by LoCoMo/BEAM measurements (H1d +1.8 pp recall_all@10 at -9.9 pp R@1, 4/99 deep rescue, ~3× binary size).
Re-running LongMemEval / LoCoMo / BEAM / H4 or matching competitor feature counts: Explicitly out of scope (C-LAW, E-ASK).
B-DELTA D3 (docs/ARCHITECTURE.md 5 modules / 3 doors): Already updated in # FILE: docs/ARCHITECTURE.md (lines 1,100).

## SECTION 2: the markdown read (may be truncated by the read tool)

ExplorePlaygroundHistoryBuildNew appMy appsGalleryManageDashboardDocumentation

# Reviewing Memory-Wire System Strategy

75,124  tokens

 User 4:04 PM

# Reviewer prompt — memory-wire, round 3

You are reviewing a **shipped** system, not a proposal. Its author has spent
three full work cycles on one subsystem and now suspects that subsystem was
already fine. You have no session history and you are not continuing anyone's
work.

## What the subject is

memory-wire is a single-binary agent-memory service: a bank-isolated SQLite
store with FTS5 full-text search, exposed over HTTP, MCP (stdio and HTTP), and a
CLI, with hooks that capture conversation prose into it. Its competitive
position is deliberate: **one static binary, ~10.6 MiB, three shared libraries,
no daemon required**, against competitors that ship a Python/Node runtime plus
Postgres and 0.8–1.0 GB of idle floor.

The pack states its own case in A-STATE. Treat it as a claim.

## The question this round exists to answer

The author's position, which you should test rather than accept:

> Three cycles (retrieval efficacy, a BEAM/battery dev-set cycle, and the W0–W7
> upgrade) went into **ordering** — which memory to surface, and in what order.
> The measured result is that ordering is largely solved: the candidate pool
> contains the gold memory in 500/500 LongMemEval questions, and LoCoMo
> recall_any@10 is 94.2%. Even the remaining gap (recall_all@10 83.0% —
> questions with several gold sessions) is an ordering problem inside the pool,
> not a coverage problem.
> 
> 
> 
> Meanwhile the **product** surface — what an agent can actually do with a
> memory once recalled — has not been re-derived since v0.4.0. B-DELTA D3
> shows the architecture document still describes a system with **five
> modules** and **three doors** when the tree has nine modules and four doors.

**So: is that reading right, and where should the next cycle go?**

## How to answer

**Treat the pack's claims as CLAIMS TO AUDIT, not as ground truth.** If a
figure, a count or a causal story does not hold up against the artefacts quoted,
say so and name the artefact.

**Use "not in evidence" when that is the truth.** If you cannot settle something
from the pack, label it not in evidence and say what would settle it. That is a
useful answer, not a failed one. Do not fill a gap with a plausible value.

**Every figure has three attributes, and you should check all three:** its
**value**, its **population** (what set it counts), and its **selection
constraint** (how the set was chosen). A right value on the wrong basis is a real
defect, and it is the kind this pack would rather you found than not. The pack
marks its evidence inline: [LIVE] (measured on this tree), [CERT] (a dated
committed artefact), [IN-SAMPLE], [PRIOR], [UNCONFIRMED].

**Do not spend the turn on these, which are already settled by measurement and
recorded in the pack:**

- score thresholds or relevance cuts on auto-injection (C-LAW, D-EVIDENCE)
- re-running any benchmark again
- matching a competitor's tool or feature count

## What I want back

A **ranked** list. For each item:

1. **What** the defect, gap or redundancy is.
1. **Where** it lives — a file, a route, a documented decision. If you cannot
point at one, mark it not in evidence.
1. **Why it matters to an agent using this product** — concretely: what does an
agent fail to do, or do expensively, because of it?
1. **What would have to be true** for it to be worth a work cycle.

Rank by value per unit of work, and say what you are trading off. If your top
item is one the pack already lists (TODO.md row 31, the door-parity gap), say
so and argue for or against it — do not just restate it.

## Four specific things worth your attention

The pack's F-ANALYSIS and G-LENS sections are the artefacts, not a
curriculum. But these are the places where I am least sure:

1. **Is "ordering is solved" an artefact of the instrument?** Both the 500/500
and 94.2% figures come from benchmarks the author chose and has re-run for
months. D-EVIDENCE carries the caveats. Is the headroom claim overstated?
1. **recall_all@10 83.0% vs recall_any@10 94.2%.** The pack says the gap is
an ordering problem. Is it? Or is it telling you something about how sessions
are written that no amount of ordering fixes?
1. **The door asymmetry** (F-ANALYSIS, the 2026-11-19 addendum). redact —
retroactive secret removal — has **no MCP tool and no HTTP route**. An agent
that retained a credential through MCP cannot retract it through MCP. Is that
the highest-value gap in the system, or is it worse or better than it looks?
1. **What the pack structurally cannot show you.** It is authored by the same
person who wrote the system. What class of problem would that blind it to?

## Length and format

As long as the answer needs. Structure and arithmetic are judged; prose is not.
A short, precise report that names four real defects beats a long one that
restates the pack.

--- PACK (inlin

**NOTE: the markdown read above is TRUNCATED** (it returned 4974 chars, at the read tool's ~5 kB cap). Section 1 is the complete extraction and is the one to re-read.