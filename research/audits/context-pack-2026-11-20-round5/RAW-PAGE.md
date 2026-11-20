# RAW PAGE — the scrape as it stood at archive time

## SECTION 1: the DOM extraction (complete — not capped)

Pack & Claim Audit (Disagreements and Unverified Claims)
Before ranking, several numbers and claims in the pack disagree or lack raw evidence in this context file:
Prompt counts vs. pack evidence:
13 CLI subcommands and 10 HTTP routes: not in evidence (src/main.rs, src/api.rs, and the stdout of # FILE: research/audits/context-pack-2026-11-20-round5/measure_surface.sh (lines 1,101) are omitted).
17 dependencies: reported in prose (# SECTION C-ASK, # SECTION B-DELTA D7 [IN-SAMPLE]), while Cargo.toml and measure_surface.sh stdout are absent.
8 agent hosts (prompt) disagrees with measure_surface.sh line 11 ("the 9 hosts connect.rs actually declares" [IN-SAMPLE]). src/connect.rs (Host, ALL, IMPLICIT) is omitted, so neither 8 nor 9 can be verified against code here.
6 MCP tools, 1 prompt and 4 auto-injection doors: in evidence (# SECTION B-DELTA D3; # FILE: docs/INTEGRATION_GAPS.md (lines 285,323) and (lines 460,530); # SECTION I-DOORS). Note that INTEGRATION_GAPS.md line 322 inside historical prose still says "4 tools" (explicitly flagged at line 297 as kept for record).
Binary footprint disagreement:
# FILE: AGENTS.md (lines 26,67) reports 10.63 MiB (11,141,240 B) measured 2026-11-19 [PRIOR]. # FILE: research/audits/context-pack-2026-11-20-round5/B-DELTA.md (lines 1,148) D7 reports 11,166,048 B = 10.64 MiB at 6a2a125 [IN-SAMPLE] and claims "the delta is D2–D4 and is +5 KB".
Which is wrong: 11,166,048 - 11,141,240 = 24,808 B (+24.2 KiB), not +5 KB (and 11,166,048 / 1,048,576 = 10.6488 MiB, which rounds to 10.65 MiB, not 10.64 MiB). Either D7's +5 KB delta is wrong or AGENTS.md §1 was already ~19.8 KB stale before D2–D4; either way AGENTS.md §1 is stale at 6a2a125.
connect_* module count error in B-DELTA D6:
B-DELTA.md D6 lists 8 modules (connect.rs, connect_plugin.rs, connect_codex.rs, connect_ext.rs, connect_opencode.rs, connect_claude.rs, connect_copilot.rs, connect_common.rs), matching # FILE: AGENTS.md (lines 103,148) (33 modules, verified sum = 35,674 lines), yet immediately writes "7 modules, 6,718 lines: 3,229 code / 3,489 test — 52% test".
Which is wrong: "7 modules" is wrong — summing all 8 listed rows yields 6,718 total / 3,229 code / 3,489 test (51.94%).
B-DELTA D2 table vs. prose: D2 states "six were byte-identical bodies", yet its table lists 5 helper names (mcp_url, baked, carries_marker, is_ours, preserve_comment); the sixth is not in evidence.
Register & doc contradictions in # FILE: TODO.md (lines 20,52) and INTEGRATION_GAPS.md:
TODO.md row 31 (open) and INTEGRATION_GAPS.md line 471 (redact (W3) | no route | no tool) still say MCP lacks redact, which is wrong/stale after B-DELTA D3 (b3ecc3b, memory_redact).
TODO.md row 34 (done) says "redact is reachable from the CLI and HTTP but not from MCP", which disagrees with INTEGRATION_GAPS.md line 471 (no route), lines 480–482 ("the only way to reach it is the CLI"), and TODO.md row 31 ("no MCP tool or HTTP route"). Because B-DELTA D3 added only an MCP tool over MemoryService::redact_memory (src/api.rs), row 34's "and HTTP" is wrong (conflating MemoryService in src/api.rs with an HTTP route).
TODO.md row 36 calls (hermes, omp) "the two TypeScript doors" while also noting hermes is Python, and omits opencode-plugin/memory-wire.ts from where; row 35's where cites src/connect_plugin.rs (hermes) instead of src/connect_opencode.rs.
Verdict Between Reading (A) and Reading (B)
Reading (B) — Verification is the real bottleneck, not the per-turn injection relevance mechanism.
Causal pattern in the pack: Every recent high-severity defect came from checks that either ran nowhere, verified self-authored mock files instead of real host/surface contracts, or covered N−1 doors:
Trust::UntrustedExternal was unreachable on write until verified end-to-end (TODO.md row 30).
trust_gate_test.sh existed without running in CI/pre-commit until 131cffb (B-DELTA D5), and missed post-slice starvation (extension/memory-wire.ts, B-DELTA D0) and export-shape checks (4646e7a).
opencode-plugin/memory-wire.ts was completely dead on opencode 2.x (B-DELTA D1) despite connect::tests::golden_install_shapes (TODO.md row 4, P2.1) passing, because golden bytes verified the repo's own assumptions rather than host loading.
memory_redact's first test used sk-live-deadbeef (which capture::redact_pii already scrubbed at retain time, B-DELTA D3) and documented regex as "literal text".
Even after D5, trust_gate_test.sh is gated behind command -v node in .githooks/pre-commit (B-DELTA line 83), only exercises formatRecall + export shape (B-DELTA lines 85–86), misses a live injectedQuery caching bug in the newly rewritten opencode-plugin/memory-wire.ts (see Rank #2), has no cross-door parity test (TODO.md row 36 open), and has no register/doc consistency check (leaving rows 31/34/36 and B-DELTA D6/D7 contradictory).
Reading (A) is weaker on the pack's own measurements: A score threshold is prohibited (# SECTION D-LAW; # SECTION F-CLOSED; # SECTION G-RELEVANCE lines 103–109). On the existing dev measurements, H4 (TODO.md row 17, 200-q LoCoMo subset) failed to reproduce context drowning (92.0% / 94.0% / 94.0% from 4.3k to 14.6k tokens), and the frozen 21-case relevance battery (TODO.md row 6) passes (gold 16/17, junk 0/9, D-case passes). Meanwhile the only end-to-end measurement that tests real agentic task impact with hooks+MCP (TODO.md row 10, agent-memory-bench) has never been run — itself a verification gap.
What would change my mind (falsifier): If all 4 auto-injection doors and all declared host layouts passed automated runtime hook+retain smoke tests with zero silent-dead-door failures across a release cycle, and running agent-memory-bench (TODO.md row 10) or an expanded multi-turn battery showed measurable task-success drops or harm-suite regressions (>5%) caused by unconditional recall injection vs. W7/W5-grounded injection, then (A) would become the #1 bottleneck.
Ranked Architectural Findings
1. Reactive, incomplete verification across host contracts, multi-turn plugin runtime, and surface/doc parity
Axis: Bottleneck (primary driver of silent failures and N−1 regressions).
Artefact & lines:
# FILE: research/audits/context-pack-2026-11-20-round5/B-DELTA.md (lines 1,148) (D1, D5):
"The shipped plugin exported a v1 bare function... It was installed as a flat file... a flat file is skipped with no error and no log line at all."
131cffb (trust_gate_test.sh wired "behind command -v node") and 4646e7a (export-shape check added reactively after D1).
# FILE: TODO.md (lines 20,52) row 4 (golden_install_shapes checked self-authored bytes and missed D1), row 10 (open: agent-memory-bench with hooks+MCP never run), row 36 (open: no drift check on shared plugin core), plus un-caught stale/wrong claims in rows 31, 34, 36.
# FILE: plugin/integrations/opencode-plugin/memory-wire.ts (lines 176,462) (session.hook("context"), ~lines 369–397 & 420–444): D1 verified export shape and a single stub-ctx turn, missing the multi-turn/failure-caching bug at:
code
Js
download
content_copy
expand_less
if (query === injectedQuery) {
if (injectedBlock) ev.system.push(injectedBlock);
return;
}
...
var block = Array.isArray(items) ? formatRecall(items) : "";
injectedQuery = query;
injectedBlock = block;
(See Rank #2 for exact mechanics.)
Cost against "cheap option": 0 binary bytes, 0 added [dependencies] (stays 17 [IN-SAMPLE]), 0 added DT_NEEDED (stays 3 [IN-SAMPLE]), 0 ms runtime latency (B-DELTA.md D7; measure_surface.sh lines 78–89) — CI/dev harness scripts and external agent-memory-bench adapter run completely outside target/release/memory-wire.
Falsifier: If CI executes a real runtime contract matrix across all 4 doors + all host install layouts (and fails loudly rather than skipping when node/host tooling is absent) with zero silent regressions over a full release cycle, while TODO.md row 10 shows injection quality causes >5% task failure, verification ceases to be #1.
2. Fragmentation and live behavioral drift across the four auto-injection doors
Axis: Fragmentation (with redundancy of a ~101-line shared domain core across 3 languages).
Artefact & lines:
# FILE: plugin/integrations/opencode-plugin/memory-wire.ts (lines 176,462) (~lines 369–397, 422): Sets injectedQuery = query; injectedBlock = block; unconditionally even when post(...) fails (items non-array -> block = ""), and never resets injectedQuery in session.execution.succeeded (lastPrompt = ""). A transient server outage freezes empty recall ("") for that query even after the server restarts, and repeated prompts > SHORT_PROMPT_CHARS replay stale blocks across turns.
# FILE: plugin/integrations/extension/memory-wire.ts (lines 205,356) (~lines 289–303) & opencode-plugin/memory-wire.ts (~lines 374–388):
"MEMORY_WIRE_FORMAT=summary ... comes from the environment rather than the bank's injectionFormat config key ... The Rust hook reads both and the bank wins there."
# FILE: src/hooks.rs (lines 737,789) (lines 755–781) & # FILE: plugin/integrations/hermes/memory-wire/__init__.py (lines 508,573) (~lines 552–572) vs. extension/memory-wire.ts (~lines 239–256) & opencode-plugin/memory-wire.ts (~lines 212–229): TS skips blank/whitespace content before taking RECALL_ENTRIES and hard-slices the full block at INJECT_CAP_CHARS - 1 + "…" without a footer; Rust/Python include blank rows in MAX_LINES, break on whole entries against SECTION_CAP_CHARS (ignoring header/footer chars), and emit - …and {hits.len() - MAX_LINES} more even after an early left break.
# FILE: TODO.md (lines 20,52) row 36: "101 lines of genuine duplication (markers, cap derivation, tier handling) ... a change to the cap shape must be made in two places or the doors diverge silently."
Cost against "cheap option": 0 new [dependencies], 0 added DT_NEEDED, 0–negative net binary bytes if the shared TS core is deduplicated via include_str! (TODO.md row 36 Option 1 removes one ~3–4 KB duplicate embedded TS string from connect_ext.rs/connect_opencode.rs; Option 3 drift test is 0 bytes). Latency caveat: aligning injectionFormat bank-config parity in plugins via an extra GET costs +1 loopback HTTP round trip per turn (extension/memory-wire.ts line 293; TODO.md row 33 Batch 4); carrying it in /recall metadata or connect env avoids that round trip (0 ms per-turn overhead, unmeasured exact header byte delta).
Falsifier: If an automated cross-door parity test (driving hooks.rs, hermes, extension, and opencode against a single fixture suite covering failed-then-recovered /recall, repeated prompts, blank content, SECTION_CAP_CHARS overflow, and format=summary) runs in CI and passes cleanly without recurring drift, and users never expect bank-level injectionFormat to affect plugin hosts, extracting the shared TS core (TODO.md row 36 Options 1–2) is unnecessary.
3. Cross-door capability asymmetry across MCP and HTTP
Axis: Integration gap (and surface fragmentation).
Artefact & lines:
# FILE: docs/INTEGRATION_GAPS.md (lines 285,323) (lines 302–303, 320):
"Our surface: memory_retain, memory_recall, memory_reflect, memory_bank_config_get, memory_supersede, memory_redact (src/mcp.rs)."
"SKILL.md:49-50 admits list/get/delete/stats are HTTP-only."
# FILE: docs/INTEGRATION_GAPS.md (lines 460,530) (lines 469–490):
hygiene (W4): HTTP GET /banks/:id/hygiene only (no tool on MCP, no on CLI).
supersede (W7a): MCP memory_supersede only (no route on HTTP, no on CLI) — "so a non-MCP, non-local caller cannot retract a row."
redact (W3): CLI + MCP (B-DELTA D3), no route on HTTP.
grounding (W5): CLI + default-off SessionStart hook (no route on HTTP, no tool on MCP).
recall: no on CLI.
# FILE: research/audits/context-pack-2026-11-20-round5/B-DELTA.md (lines 1,148) (D3): MemoryService already implements these core methods (MemoryService::redact_memory + readOnly guard needed only registration + dispatch arm).
Cost against "cheap option": 0 added [dependencies] (17), 0 added DT_NEEDED (3), 0 ms latency on recall/hook paths. Per-tool/route dispatch overhead in binary bytes is absent per item from the pack, bounded under the stated assumption of a few KiB per dispatch arm (<0.05% of 11,166,048 B, given D2–D4 combined measured +24,808 B max [IN-SAMPLE] vs. AGENTS.md §1 [PRIOR], vs. +1,084,720 B only when adding an entire transport in G5 at INTEGRATION_GAPS.md line 292).
Falsifier: If agent traces (agent-memory-bench, TODO.md row 10) show MCP agents never need list/get/delete/stats/hygiene/bank_config_set and HTTP-only hosts (opencode, omp, pi, hermes) never need supersede or redact — or if expanding MCP beyond ~6–10 tools measurably degrades LLM tool-selection accuracy (INTEGRATION_GAPS.md line 322) — keeping doors asymmetric is right.
4. Unconditional per-turn hook injection ignoring score + default-off startup grounding (W7/W5)
Axis: Bottleneck (context-token noise on unrelated queries, bounded by D-LAW/F-CLOSED).
Artefact & lines:
# FILE: docs/OPEN_HOOK_RECALL_RELEVANCE.md (lines 6,85) (lines 8–43):
"On 2026-09-30, twice, the UserPromptSubmit hook injected roughly 10,000 characters of quantitative-trading cell-architecture notes into a conversation about Telegram agentic-loop bugs."
"The open defect is narrower and still real: recall_section (:680-706) reads content and created_at and never reads score. The only consumer of score is a tracing::debug! (:640) whose own comment says 'Nothing branches on this and nothing is gated by it'."
# FILE: src/hooks.rs (lines 737,789) (lines 747–782), hermes/__init__.py (lines 522–530), extension/memory-wire.ts (lines 216–224), opencode-plugin/memory-wire.ts (lines 187–198).
# FILE: docs/INTEGRATION_GAPS.md (lines 460,530) (line 472): grounding (W5) is "SessionStart, default off".
Cost against "cheap option": 0 added [dependencies], 0 added DT_NEEDED (no score threshold per D-LAW/F-CLOSED, and no default dense vectors/cross-encoder per F-CLOSED). src/grounding.rs (350 lines, AGENTS.md §3) already compiles into the binary (0 extra bytes to enable by default, aside from small config/flag glue). Latency/token cost: currently pays 1 /recall request per turn (+1 config GET in hooks.rs) and up to ~10k chars of prompt context; non-threshold gating/W5 grounding can save per-turn prompt tokens (exact token/latency delta is unmeasured in the pack).
Falsifier: If agent-memory-bench (TODO.md row 10) confirms on live coding sessions what H4 (TODO.md row 17: 92.0% / 94.0% / 94.0%) and the 21-case battery (TODO.md row 6: gold 16/17, junk 0/9) showed — that irrelevant top-K injection does not materially degrade live agent task completion — then W7 needs no priority bump.
5. connect_* per-host adapter family (residual redundancy only — do NOT abstract into a trait)
Axis: Redundancy (measured as low/already harvested in production code).
Artefact & lines:
# FILE: research/audits/context-pack-2026-11-20-round5/B-DELTA.md (lines 1,148) (D6, D2):
6,718 lines across 8 connect*.rs files: 3,229 code (48.1%) / 3,489 inline #[test] (51.9%), with connect_ext.rs at 67.8% test (539/795).
D2 diffed all repeated fn signatures across connect*.rs and found only 6 (5 tabulated) byte-identical helpers, removing -138 lines across 6 modules while adding +177 lines for connect_common.rs (+39 net lines). Two collisions (is_ours on &Value vs &Path; version/prune_empty_dirs) differed structurally and were rightly left separate.
Cost against "cheap option": Enumerated parity checks cost 0 binary bytes, 0 dependencies, 0 DT_NEEDED, 0 ms runtime latency (#[test] / CI-only). A forced Rust trait over heterogeneous JSON/TOML/Python/TS installers adds abstraction indirection for negligible binary-size change (~0 to negative savings on 1,793 lines of per-host production code).
Falsifier: Only if a new fn/AST diff across the 1,793 production lines of the 6 per-host adapters (555 + 387 + 256 + 258 + 160 + 177) uncovers >300 lines of identical state-machine logic (e.g., atomic write + re-read verification + pin preservation) causing N−1 production bugs across >=3 adapters.
Specific Questions Answered
1. The connect_* question (52% inline tests: shared abstraction vs. enumerated parity check)
The enumerated parity check is decisively the better instrument.
Measurement (B-DELTA.md D6 & D2 [IN-SAMPLE]): Of 6,718 lines across the 8 connect*.rs modules, 3,489 lines (51.9%) are inline #[test] fixtures (connect_ext.rs is 67.8% test, connect.rs 53.3%, connect_codex.rs 52.9%) which validate exact host config bytes and cannot be deduplicated without obscuring golden expectations. Of the remaining 3,229 production lines, 1,328 sit in connect.rs and 108 in connect_common.rs, leaving only 1,793 production lines across all 6 per-host modules (plugin 555, codex 387, opencode 258, ext 256, copilot 177, claude 160).
Empirical extraction yield (D2): Diffing every repeated function across connect*.rs already extracted the byte-identical helpers into connect_common.rs — saving only 138 lines across 6 modules at the cost of 177 lines in connect_common.rs (+39 lines net) while proving two name collisions (is_ours, version/prune_empty_dirs) could not share code cleanly.
Why parity wins: A shared Rust trait cannot see host-side load failures like B-DELTA D1 (opencode 2.x requiring <dir>/index.ts + {id, setup}), whereas an enumerated parity check sourced from Host::ALL in src/connect.rs directly forces every host to pass install/uninstall/pin-preservation/doctor/host-contract checks at 0 binary bytes and 0 deps.
2. The four auto-injection doors: where they still disagree
From # SECTION I-DOORS, here are the specific remaining disagreements (excluding D0, which is fixed):
opencode-plugin/memory-wire.ts (~lines 369–397, 422) — injectedQuery failure-caching & stale-turn bug: opencode alone caches injectedQuery = query; injectedBlock = block; even when post(...) fails (!Array.isArray(items) -> block = "") and never clears injectedQuery in session.execution.succeeded (lastPrompt = ""). A transient outage caches "" until the user sends a different query, and repeated prompts > SHORT_PROMPT_CHARS across turns replay stale recall instead of hitting /recall.
injectionFormat bank config vs. MEMORY_WIRE_FORMAT env (extension/memory-wire.ts ~lines 290–303; opencode-plugin/memory-wire.ts ~lines 375–388; TODO.md row 33 Batch 4): src/hooks.rs reads both bank config (injectionFormat) and env MEMORY_WIRE_FORMAT (with bank config winning), whereas the TS plugins (and plugin surfaces per Batch 4) read only MEMORY_WIRE_FORMAT. Setting injectionFormat = "summary" on a bank changes Rust-hook hosts and silently leaves plugin hosts on full memories.
Blank-content handling, section cap enforcement, and "…and N more" footer (src/hooks.rs:755–781 & hermes/__init__.py:552–572 vs. TS formatRecall):
TS (extension:240, opencode:213) skips empty/whitespace content before counting RECALL_ENTRIES; hooks.rs and hermes inject blank entries (- \n) and consume MAX_LINES slots.
hooks.rs (:772–780) and hermes (:566–571) break cleanly on whole-entry boundaries against SECTION_CAP_CHARS, omit header/footer from that budget, and print - …and {hits.len() - MAX_LINES} more even when breaking early due to left (misstating omitted count); TS hard-slices the entire rendered block at INJECT_CAP_CHARS - 1 + "…" with no footer (and uses ## memory-wire long-term memory + 1. vs. ### memory-wire recall — \{bank}`+- `).
Syntactic / unverified-slice differences (flagged honestly):
opencode (~line 443) sets lastRetained on if (stored !== null) while extension (~line 351) uses if (stored), and opencode's prompt hook (if (text) lastPrompt = text) does not clear lastPrompt on a blank/reminder-only prompt if a prior turn aborted before session.execution.succeeded.
hermes/__init__.py:539,545,565 passes _strip_reminders(query) directly to module-level _recall(...) with no last_answer parameter visible and assumes hits elements are dicts (h.get("trust"), hit['content']) without TS's bare-string guard. Because lines 1–507 and 574+ of hermes/__init__.py, lines 1–736 of src/hooks.rs, and the numeric constants (MAX_LINES, RECALL_ENTRIES, SECTION_CAP_CHARS, INJECT_CAP_CHARS, SHORT_PROMPT_CHARS) are not in the pack, whether hermes caches last_answer/normalizes strings inside _recall and whether cap constants match numerically is not in evidence.
3. What an agent hits first (reaching across the exposed surfaces)
(Counting audit noted above: 13 CLI and 10 HTTP are not in evidence; 8 hosts conflicts with 9 in measure_surface.sh:11; 6 MCP tools, 1 prompt, and 4 plugin doors are verified.)
Over MCP (src/mcp.rs — 6 tools, 1 prompt):
Memory inspection & lifecycle (list, get, delete, stats): Explicitly HTTP-only (INTEGRATION_GAPS.md:320). An agent inside an MCP session (Claude Code, Codex, Copilot) cannot browse memories, fetch one by ID, delete a bad row by ID, or check bank stats without falling back to CLI/HTTP.
Bank config writes (memory_bank_config_set): MCP exposes only memory_bank_config_get (INTEGRATION_GAPS.md:303), so an agent can read settings like injectionFormat or recallSynonyms and cannot update them.
Hygiene (memory_hygiene) & startup grounding (grounding): hygiene is HTTP-only (GET /banks/:id/hygiene); grounding is CLI/default-off-hook only (INTEGRATION_GAPS.md:472–473).
Over HTTP (used by remote callers and plugin hosts hermes/omp/pi/opencode):
supersede (W7a), redact (W3), and grounding (W5) have no HTTP routes (INTEGRATION_GAPS.md:471–475, 480–490). A non-MCP HTTP agent cannot retract (supersede) or scrub (redact) a memory at all.
Over CLI:
hygiene, supersede, and recall have no CLI subcommands (INTEGRATION_GAPS.md:473–475).
One Thing the Pack Does Not Contain That I Need Before Committing
The agent-memory-bench Claude Code evaluation results (TODO.md row 10, status: open) — live task-success and harm-suite measurements with hooks + MCP actually wired together.
Right now the pack contains static retrieval benchmarks (LongMemEval, LoCoMo, BEAM) and unit/syntax checks, with zero live end-to-end measurements of whether unconditional per-turn hook injection plus the 6-tool MCP surface helps or harms real multi-turn agent tasks. That single missing measurement is what separates a confident ranking of (B) verification above (A) per-turn injection policy (#4) from an unmeasured assumption about downstream agent behavior.

## SECTION 2: the markdown read (may be truncated by the read tool)

ExplorePlaygroundHistoryBuildNew appMy appsGalleryManageDashboardDocumentation

# Architectural Bottlenecks and Remediation Priorities

88,519  tokens

 Model 1:50 AM Thoughts 

Expand to view model thoughts

### Pack & Claim Audit (Disagreements and Unverified Claims)

Before ranking, several numbers and claims in the pack disagree or lack raw evidence in this context file:

- **Prompt counts vs. pack evidence:**

  - 13 CLI subcommands and 10 HTTP routes: **not in evidence** (src/main.rs, src/api.rs, and the stdout of # FILE: research/audits/context-pack-2026-11-20-round5/measure_surface.sh (lines 1,101) are omitted).
  - 17 dependencies: reported in prose (# SECTION C-ASK, # SECTION B-DELTA D7 [IN-SAMPLE]), while Cargo.toml and measure_surface.sh stdout are absent.
  - 8 agent hosts (prompt) **disagrees** with measure_surface.sh line 11 ("the 9 hosts connect.rs actually declares" [IN-SAMPLE]). src/connect.rs (Host, ALL, IMPLICIT) is omitted, so neither 8 nor 9 can be verified against code here.
  - 6 MCP tools, 1 prompt and 4 auto-injection doors: **in evidence** (# SECTION B-DELTA D3; # FILE: docs/INTEGRATION_GAPS.md (lines 285,323) and (lines 460,530); # SECTION I-DOORS). Note that INTEGRATION_GAPS.md line 322 inside historical prose still says "4 tools" (explicitly flagged at line 297 as kept for record).
- **Binary footprint disagreement:**

  - # FILE: AGENTS.md (lines 26,67) reports 10.63 MiB (11,141,240 B) measured 2026-11-19 [PRIOR]. # FILE: research/audits/context-pack-2026-11-20-round5/B-DELTA.md (lines 1,148) D7 reports 11,166,048 B = 10.64 MiB at 6a2a125 [IN-SAMPLE] and claims "the delta is D2–D4 and is +5 KB".
  - **Which is wrong:** 11,166,048 - 11,141,240 = 24,808 B (**+24.2 KiB**), not +5 KB (and 11,166,048 / 1,048,576 = 10.6488 MiB, which rounds to **10.65 MiB**, not 10.64 MiB). Either D7's +5 KB delta is wrong or AGENTS.md §1 was already ~19.8 KB stale before D2–D4; either way AGENTS.md §1 is stale at 6a2a125.
- **connect_* module count error in B-DELTA D6:**

  - B-DELTA.md D6 lists **8** modules (connect.rs, connect_plugin.rs, connect_codex.rs, connect_ext.rs, connect_opencode.rs, connect_claude.rs, connect_copilot.rs, connect_common.rs), matching # FILE: AGENTS.md (lines 103,148) (33 modules, verified sum = 35,674 lines), yet immediately writes **"7 modules, 6,718 lines: 3,229 code / 3,489 test — 52% test"**.
  - **Which is wrong:** **"7 modules" is wrong** — summing all **8** listed rows yields 6,718 total / 3,229 code / 3,489 test (51.94%).
- **B-DELTA D2 table vs. prose:** D2 states "six were byte-identical bodies", yet its table lists **5** helper names (mcp_url, baked, carries_marker, is_ours, preserve_comment); the sixth is **not in evidence**.
- **Register & doc contradictions in # FILE: TODO.md (lines 20,52) and INTEGRATION_GAPS.md:**

  - TODO.md row 31 (open) and INTEGRATION_GAPS.md line 471 (redact (W3) | no route | no tool) still say MCP lacks redact, which is **wrong/stale** after B-DELTA D3 (b3ecc3b, memory_redact).
  - TODO.md row 34 (done) says "redact is reachable from the CLI and HTTP but not from MCP", which **disagrees** with INTEGRATION_GAPS.md line 471 (no route), lines 480–482 ("the only way to reach it is the CLI"), and TODO.md row 31 ("no MCP tool or HTTP route"). Because B-DELTA D3 added only an MCP tool over MemoryService::redact_memory (src/api.rs), row 34's "and HTTP" is **wrong** (conflating MemoryService in src/api.rs with an HTTP route).
  - TODO.md row 36 calls (hermes, omp) "the two TypeScript doors" while also noting hermes is Python, and omits opencode-plugin/memory-wire.ts from where; row 35's where cites src/connect_plugin.rs (hermes) instead of src/connect_opencode.rs.

---

### Verdict Between Reading (A) and Reading (B)

**Reading (B) — Verification is the real bottleneck, not the per-turn injection relevance mechanism.**

1. **Causal pattern in the pack:** Every recent high-severity defect came from checks that either ran nowhere, verified self-authored mock files instead of real host/surface contracts, or covered N−1 doors:

  - Trust::UntrustedExternal was unreachable on write until verified end-to-end (TODO.md row 30).
  - trust_gate_test.sh existed without running in CI/pre-commit until 131cffb (B-DELTA D5), and missed post-slice starvation (extension/memory-wire.ts, B-DELTA D0) and export-shape checks (4646e7a).
  - opencode-plugin/memory-wire.ts was completely dead on opencode 2.x (B-DELTA D1) despite connect::tests::golden_install_shapes (TODO.md row 4, P2.1) passing, because golden bytes verified the repo's own assumptions rather than host loading.
  - memory_redact's first test used sk-live-deadbeef (which capture::redact_pii already scrubbed at retain time, B-DELTA D3) and documented regex as "literal text".
  - Even after D5, trust_gate_test.sh is gated behind command -v node in .githooks/pre-commit (B-DELTA line 83), only exercises formatRecall + export shape (B-DELTA lines 85–86), misses a live injectedQuery c

**NOTE: the markdown read above is TRUNCATED** (it returned 4980 chars, at the read tool's ~5 kB cap). Section 1 is the complete extraction and is the one to re-read.