1. The omission (D-ASK)

Judgment: Wrong omission. D-ASK (# SECTION D-ASK, # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,163)) documents a live, open seam defect — not a closed record — and excluding it from W0–W6 while adding a new startup capsule (W5) misallocates work away from the primary per-turn context-degradation path.

Why D-ASK is not closed: # SECTION C-EVIDENCE (# FILE: docs/handoff/CLOSED.md (lines 1,45)) closes only score thresholds on auto-injection, citing docs/CONSISTENCY.md §32 (not in evidence) and D-ASK itself. Meanwhile D-ASK's 2026-10-08 status update explicitly states: "the scoring gating question stays OPEN by decision, not neglect" (# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,163)). Moreover, D-ASK's stated blocker against policy gating — absence of "labelled (query, relevant?) pairs over the real bank" — was lifted on 2026-10-16 when the frozen relevance battery ([CERT] | n=21 cases on a deterministic 1,413-row sha256 snapshot | dev corpus, W2 cycle) shipped specifically so "precision levers [are] registrable from here — none registered" (# SECTION A-STATE, # FILE: docs/handoff/STATE.md (lines 13,108)).

Audit of live state at HEAD 36bb947: Whether src/hooks.rs received the 2026-10-08 TypeScript plugin provenance header ((recalled by relevance from bank <id>, unverified)) by v0.10.0 is not in evidence (# SECTION E-ANALYSIS, # FILE: docs/audit/2026-11-18-surface-audit.md (lines 19,153) does not audit D-ASK). Under the stated assumption that D-ASK's code citation holds (src/hooks.rs:333–362), recall_at still deserializes Vec<String> — discarding the fused RRF score pinned at src/api.rs:1903 — and recall_section still gates solely on !is_empty(), injecting up to MAX_LINES ([LIVE] | n=2 observed failures of ~10,000 chars on 2026-09-30 | unselected live UserPromptSubmit hook traces). The 2026-10-08 borrowed-answer mitigation lives only in-process inside TS extensions (plugin/integrations/...) and fails on fresh processes or restarted TUIs (# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,163)).

Why a score threshold is wrong: Per-query min-max normalization in src/recall.rs:319–337 forces the top hit of any irrelevant set to score 1.0 by construction (# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,163)), RRF values (1/(60+rank)) are rank-only (# FILE: docs/audit/2026-11-18-surface-audit.md (lines 19,153)), and tuning a cut-off violates # SECTION B-LAW (# FILE: AGENTS.md (lines 203,263)).

What the non-threshold fix is, and invariant fit: CLOSED.md names the right levers: "composition, caps, labels, tags — not a cut-off" (# FILE: docs/handoff/CLOSED.md (lines 1,45)). Combining D-ASK and E-ANALYSIS yields a four-part deterministic fix:

Stop dropping scores at the seam (D-ASK): Deserialize the scored struct in recall_at (and pass an opt-in parameter such as ?format= if carrying un-normalized raw BM25 / matched-term count across the wire, preserving the frozen default JSON body per # SECTION B-LAW, # FILE: AGENTS.md (lines 282,312)).

Structural/scope gating via W1 + W5: Pass tags/exclude_tags (# SECTION F-LENS, # FILE: README.md (lines 654,700)), W1 (scope, lifecycle, trust), and W5 active_paths globs on UserPromptSubmit so cross-topic rows (e.g., trading notes inside a Telegram bug thread) are excluded structurally before ranking.

Query-substance & zero-overlap guard: Skip hook injection on empty/trivial continuation acknowledgements and when un-normalized lexical match is zero (single mechanistic constant / zero-signal rule checked once on the frozen relevance battery [CERT] | n=21 | dev set, never fitted on LongMemEval).

Mandatory provenance + summary caps: Ensure src/hooks.rs emits the unverified/trust label and defaults hook auto-injection to format: "summary" (MEMORY_WIRE_FORMAT, src/hooks.rs:533–574), which already cuts bytes 43% at identical ranking on the frozen battery ([CERT] | n=21 | dev set; # FILE: docs/handoff/STATE.md (lines 138,196)).

Invariant check (# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 11,57)): Pure Rust (0 new deps, DT_NEEDED=3 intact, < 9 MiB binary), no LLM in binary, FusionWeights::SHIPPED untouched, opt-in/config-gated for byte-identical off-path, and dev-set-only selection (0 LongMemEval consultations).

2. The ranking (agent value vs. plan order)

The plan's order — W0 → W1 → W2 → W3 → W4 → W5 → W6 (# SECTION E-ANALYSIS, # FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 58,81)) — reflects engineering dependency and security risk, not agentic experience. Re-ranked by what each buys an operating agent (highest to lowest):

W4 > W5 (when enabled) > W1 > W6 > W2 > W3 > W0

W4 (deterministic hygiene scan + apply) — MIS-RANKED DOWNWARD (plan #4 → agent #1).

Mechanism: # SECTION C-EVIDENCE (# FILE: docs/handoff/CLOSED.md (lines 1,45); # FILE: eval/LOCOMO_LEVERS.md (lines 1,76)) falsified the recency stream (H3: temporal R@1 −33.4pp, temporal recall_all@10 −7.8pp | [CERT] | n=1,531 queries, 9 dropped | deterministic LoCoMo H0-dated dev set, single pre-registered value). Because recency cannot rank memories, unresolved contradictions and unanchored relative dates ("last week") stay equally rankable. Meanwhile memory_retain deduplicates only exact string matches (# SECTION F-LENS, # FILE: README.md (lines 654,700)), allowing near-duplicate prose (# SECTION D-ASK) to consume the finite 8,000-token DEFAULT_RECALL_BUDGET (~3 median 2,626-token sessions, # FILE: README.md (lines 124,188)). When redundant rows crowd out distinct gold sessions, reader accuracy collapses (91.2% when all gold fits n=1,289 vs. 53.0% when some fits n=166 | [UNCONFIRMED] | n=1,540 AMB LoCoMo rag, free small-stack reader/judge | measure-only instrument artifact; # FILE: docs/handoff/STATE.md (lines 138,196)).

[web-sourced] Chao et al., "STALE: Can LLM Agents Know When Their Memories Are No Longer Valid?" (https://arxiv.org/abs/2605.06527, read 2026-11-18)[1][2] shows that when invalidated/contradictory memories stay retrievable alongside updates, even Gemini-3.1-pro reaches only 55.2% overall accuracy (n=1,200 queries across 400 conflict scenarios), because agents fail to adjudicate stale state at read time without write/maintenance-time consolidation.

W5 (startup grounding capsule) — MIS-RANKED DOWNWARD (plan #5 → agent #2; structurally coupled to W1).

Mechanism: # SECTION C-EVIDENCE (# FILE: README.md (lines 29,123)) documents an ordering collapse on single-session-preference (43.3% R@1 vs. 100.0% R@20 | [IN-SAMPLE] | n=500 LongMemEval-S | test-fitted overlap 0.25 over 46 configs), while # FILE: README.md (lines 124,188) shows serving shape/budget produced the project's largest empirical gain (40.0% → 60.0% accuracy at 2,000 → 8,000 token budget | [IN-SAMPLE] | n=25 LongMemEval slice, 92.0% gold presence unchanged, local free judge | mechanistic 3× median rule, measured once; similarly H12 in-band dates lifted HaluMem Dynamic Update 0.2056 → 0.4056, [PRIOR] | # FILE: docs/handoff/STATE.md (lines 13,108)). At SessionStart (# FILE: README.md (lines 511,576)) and on cold Turn 1 (# SECTION D-ASK), an opt-in bounded (≤40 lines / ≤2048 B), active_paths-scoped capsule surfaces standing constraints without requiring the agent to guess lexical keywords — though Gate 6 subtracts its bytes from DEFAULT_RECALL_BUDGET.

[web-sourced] The Agentic AI Foundation's memory architecture synthesis ("Agent memory: patterns, tradeoffs, open problems", https://aaif.io/blog/agent-memory-patterns-tradeoffs-open-problems, read 2026-11-18)[3][4] emphasizes separating always-loaded, scoped project/policy instructions from episodic retrieval corpora precisely so stable directives are not drowned by session search.

W1 (dimensional metadata: scope/aspect/lifecycle/trust) — #1 engineering foundation, #3 standalone agent value.

Mechanism: # SECTION C-EVIDENCE (# FILE: docs/handoff/CLOSED.md (lines 1,45)) shows honoring scope-tag filters (H6) lifted PrecisionMemBench 14/77 → 19/77 (precision 0.17 → 0.24 | [IN-SAMPLE] | n=77 | measure-only), and # SECTION F-LENS (# FILE: README.md (lines 654,700)) shows MCP currently exposes only untyped tags/exclude_tags. However, Gate 2 mandates that unfiltered recall remains byte-identical (# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 332,358)), so W1 pays off to the agent primarily through W4, W5, and W2.

W6 (connect --with-skills) — MIS-RANKED DOWNWARD (plan #6 → agent #4).

Mechanism: # SECTION F-LENS (# FILE: README.md (lines 511,576)) shows connect installs 5 hooks and 4 MCP tools, yet src/connect.rs deploys 0 SKILL.md files (# FILE: docs/audit/2026-11-18-surface-audit.md (lines 19,153)). Without explicit tool-use guidance, agents fall into the uniform-default trap and noisy retention (# SECTION D-ASK). [web-sourced] AAIF (https://aaif.io/blog/agent-memory-patterns-tradeoffs-open-problems, read 2026-11-18)[3][4] notes that an explicit capture policy (distinguishing durable norms from transient debugging notes) is essential to stop store accumulation.

W2 (mount profiles: read_only, trust ceiling) — MIS-RANKED UPWARD for routine capability (plan #2 → agent #5; #1 for untrusted-ingest safety).

Mechanism: Prevents untrusted external ingest from masquerading as scope=policy (# FILE: docs/audit/2026-11-18-surface-audit.md (lines 154,262) F7; # SECTION D-ASK), matching [web-sourced] AAIF's analysis of sleeper memory poisoning (https://aaif.io/blog/agent-memory-patterns-tradeoffs-open-problems, read 2026-11-18)[3][4]. Yet in normal single-repo coding loops without untrusted web scraping (# FILE: README.md (lines 511,576)), W2 adds write refusals rather than context quality.

W3 (retroactive redaction) — MIS-RANKED UPWARD (plan #3 → agent #6).

Mechanism: Fixes an operator/privacy compliance gap (# FILE: docs/audit/2026-11-18-surface-audit.md (lines 154,262) F6), adding negligible task utility to the agent beyond preventing accidental re-emission of a previously leaked secret.

W0 (test hermeticity + redactor tests) — plan #0 engineering gate, #7 agent value.

Mechanism: golden_install_shapes (copilot on PATH, F9) affects CI only; whether capture::redact_pii corrupts code/UUIDs in live recalls (F6) is not in evidence.

3. What W0–W6 structurally cannot see (C-EVIDENCE metrics)

(Strictly pack-only.)

Because W0–W6 preserves FusionWeights::SHIPPED (# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 58,81)), keeps unfiltered recall byte-identical (Gate 2), keeps the ranking path untouched (Gate 5), and keeps flag-off startup byte-identical (Gate 6, lines 332–358), almost every headline retrieval metric in # SECTION C-EVIDENCE is structurally immovable by this plan:

Metrics in C-EVIDENCE that NONE of W1, W2, W4, W5 can move:

All LongMemEval-S retrieval metrics (# FILE: README.md (lines 29,123)): R@1 (83.8%, and 43.3% on single-session-preference), R@5 (97.2% fitted / 93.0% unfitted), R@10 (98.6% / 97.4%), R@20 (99.6%, 100.0% preference), NDCG@10 (88.2% / 83.5%), MRR (89.2% / 83.9%), and the +16.2pp oracle R@1 ceiling ([IN-SAMPLE] fitted / [CERT] unfitted | n=500 | per-question fresh session-as-document index, 100k-token budget). Why: W1 adds no unfiltered effect (Gate 2); W2 is a write guard; W5 has zero pre-existing capsule rows or active_paths on LongMemEval (# FILE: docs/audit/2026-11-18-surface-audit.md (lines 19,153)); W4 scan leaves ranking untouched (Gate 5) and LongMemEval runs a fresh index per question without invoking W4 apply.

All LoCoMo dev retrieval metrics (# FILE: eval/LOCOMO_LEVERS.md (lines 1,76)): R@1 (60.4%), R@5 (86.0%), R@10 (94.2%), R@20 (98.0%), recall_all@10 (83.0% across 261 misses), NDCG@10 (73.6%), MRR (71.8%), R@pool (100.0%) ([CERT]/[IN-SAMPLE] | n=1,531 queries | deterministic retrieval, H0-dated dev set). Why: Unfiltered queries + untouched FusionWeights::SHIPPED + no SessionStart capsule. (Audit caveat: exact text mutations of W4 apply are not in evidence because plan lines 82–331 are absent; under the stated assumption from Gate 5 and # FILE: docs/handoff/STATE.md (lines 13,108) that the plan ships zero ranking changes and LoCoMo runs raw documents.json, W4 moves LoCoMo by 0.0pp.)

Coding-life retrieval (# FILE: README.md (lines 124,188)): hit-rate (100%) and R@5 (96.7%) ([IN-SAMPLE] | n=15 labeled queries | deterministic release run) — same unfiltered ranking invariance.

Metrics in C-EVIDENCE that ANY of W1, W2, W4, W5 CAN move:

PrecisionMemBench (14/77 → 19/77, precision 0.24, recall 0.95, active 3/43 | [IN-SAMPLE] | n=77 | measure-only; # FILE: docs/handoff/CLOSED.md (lines 1,45)): W1 (and conditionally W4 apply if it transitions contradicted rows via lifecycle) can move precision/active counts if the benchmark adapter populates and queries W1 scope/lifecycle/trust filters (AGENTS.md §6 forbids selecting on it).

LLM-judged answer_quality (40.0% → 60.0%, wrong despite gold served 52.0% → 32.0%, prompt tokens 315,343 | [IN-SAMPLE] | n=25 LongMemEval slice; # FILE: README.md (lines 124,188)) & AMB LoCoMo rag (84.1% | [UNCONFIRMED] artifact | n=1,540): W5 cannot move LongMemEval answer_quality (no capsule rows on fresh index). W4 apply could move a multi-turn answer_quality / prompt tokens reading only if apply shrinks bloat/duplicates inside DEFAULT_RECALL_BUDGET prior to recall — not in evidence for LongMemEval/LoCoMo harnesses.

Relevance-battery junk-injected (0/9 | [UNCONFIRMED] polarity, flagged as live-bank growth artifact in # FILE: docs/handoff/CLOSED.md (lines 1,45) and n=21 frozen dev set in # FILE: docs/handoff/STATE.md (lines 138,196)): Can only move via W1 filter wiring or W4 apply cleanup; untouched by default recall (Gates 2, 5, 6) and uncoupled from D-ASK.

W2 moves ZERO metrics in C-EVIDENCE: C-EVIDENCE contains no authorization or poisoning metric.

Footprint & speed (artifact size, idle RSS 9.88–10.7 MiB, write path latency, search p50 260–620 µs / 0.99–6.03 ms | [LIVE] | range-quality under load; # FILE: docs/handoff/CLOSED.md (lines 1,45), # FILE: README.md (lines 124,188)): All four (W1, W2, W4, W5) can move these negatively (regress) via new columns, guards, scan passes, and capsule reads, bounded by Gate 8 (binary ≤ 9 MiB, idle RSS ≤ 12 MiB, DT_NEEDED=3).

4. The agentic-experience gap

(Strictly pack-only; describing the agent's context arrival without proposing features.)

After W0–W6 ships, this is what arrives in my prompt across a session:

At SessionStart (hook session-start, # SECTION F-LENS, # FILE: README.md (lines 511,576)):

If W5's opt-in flag is off (the default per Gate 6), nothing from W5 arrives; I receive only the existing bank preamble (if configured) and a generic startup recall.

If W5 is on, I receive a ≤40-line / ≤2048-byte active_paths-filtered grounding block carved out of my 8,000-token DEFAULT_RECALL_BUDGET (Gate 6; # FILE: docs/audit/2026-11-18-surface-audit.md (lines 154,262) F10), followed by a correspondingly smaller generic startup recall. Whether stop/pre-compact/session-end hooks populate non-uniform W1 dimensions so W5 has rows to surface is not in evidence; under the assumption that only explicit MCP writes set them, W5 arrives thin or empty on hook-only repos. Worse, if the git remote changed (# FILE: README.md (lines 511,576)), bank identity silently moved to <owner>-<repo> while prior memories remain unmigrated in the old basename bank, leaving me amnesic without warning.

On every user turn (UserPromptSubmit, hook prompt, # SECTION D-ASK, # FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 1,163)):

Nothing in W0–W6 repairs src/hooks.rs:333–362. Every time the user submits an off-topic question or starts a fresh process / restarted TUI with a terse prompt ("Continue", D-ASK), recall_at hits /recall without filters, discards the fused RRF score into Vec<String>, and recall_section injects up to MAX_LINES (~10,000 chars observed [LIVE] | n=2) of the least-bad irrelevant memories (1.0 min-max BM25 at rank 1 by construction) straight above the user's prompt.

What arrives gives no match score, no W1 trust/lifecycle/scope badge in the hook block (whether hooks.rs carries the 2026-10-08 unverified header at 36bb947 is not in evidence), and no distinction between a confident hit and unrelated filler. Between external W4 apply runs (# FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md (lines 58,81)), unanchored relative dates ("last Tuesday") and conflicting statements still surface side-by-side.

Mid-task via MCP (memory_recall / memory_retain, # SECTION F-LENS, # FILE: README.md (lines 654,700)):

When I invoke memory_recall without manual W1 filters, Gate 2 guarantees byte-identical ranking (FusionWeights::SHIPPED): I still receive un-distilled 2,000-char pre-compact/session-end transcript dumps where short user preferences sit at 43.3% R@1 ([IN-SAMPLE] | n=500; # FILE: README.md (lines 29,123)), and hitting DEFAULT_RECALL_BUDGET (8,000 tokens minus W5) still truncates lower hits (# FILE: README.md (lines 124,188)).

When I discover an outdated or wrong memory inside my injected context, memory_retain can only append a second competing row (# FILE: README.md (lines 654,700)). Whether W1 or W4 adds a 5th MCP supersession/retraction tool is not in evidence (F5 leaves the 5th tool undecided); assuming the 4-tool surface remains, I cannot retract stale context mid-turn.

At Stop / PreCompact / SessionEnd (# FILE: README.md (lines 511,576)):

Silent hooks still retain raw 1-line stop notes and up to 2,000 characters of un-distilled conversation prose, continuously feeding noise back into my next UserPromptSubmit.

5. One thing missing from the pack

The single most critical omission is the design body of the upgrade plan itself — # FILE: docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md, lines 82–331 (Sections 2–3 specifying W0–W6), omitted between (lines 58,81) and (lines 332,358) in # SECTION E-ANALYSIS.

Without those 250 lines, every question turns on not in evidence details:

Question 1: Whether W1 or W5 alters src/hooks.rs (UserPromptSubmit, stop, pre-compact) or leaves D-ASK's Vec<String> score-drop untouched.

Question 2: How W4 performs deterministic contradiction detection without an LLM and whether W4 apply deletes, archives (lifecycle), or rewrites rows in-place; how W5 selects rows; and whether W1 adds a 5th MCP tool (F5).

Question 3: Whether W4 apply mutates stored text/counts in a way that could reach benchmark harnesses, and how W1 filter schemas map onto PrecisionMemBench (19/77) or the frozen relevance battery.

Question 4: What exact text/badges (scope, trust, lifecycle, unverified) W1 and W5 render into the agent's prompt, and whether an agent can supersede stale memories mid-turn.

(Secondary pack omissions reinforcing these gaps: docs/CONSISTENCY.md §32 and eval/relevance/BATTERY-2026-10-16-frozen.txt — needed to audit CLOSED.md's threshold closure and junk-injected 0/9 polarity — and current src/hooks.rs at 36bb947.)
