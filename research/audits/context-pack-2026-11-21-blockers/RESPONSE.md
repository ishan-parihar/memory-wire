Subject verified: memory-wire (single-binary Rust agent-memory service: SQLite + FTS5, HTTP/MCP/hooks, DT_NEEDED = 3) as described in # SECTION C-STATE (AGENTS.md lines 26–69).
Audit of Pack Claims vs. Artefacts (Discrepancies Found)
Before the rulings, seven claims in the pack disagree with the artefacts or rules carrying them:
Commit-count and version-direction errors (B-DELTA.md vs. ASK.md): # SECTION B-DELTA (B-DELTA.md (lines 1,63)) says "Sixteen commits after 303a096"—yet its own log lists 16 commits including 303a096 (position 14) and two predecessors (3a4b1a6, 3050ab2), leaving 13 commits after 303a096. It also says installed 0.9.0 is "89 commits behind v0.10.0" (repeated in plan-v3.md as 82 and plan-v4.md as 87). Per # SECTION A-ASK (ASK.md R1), main is 89 commits past v0.10.0 (Oct 27), while 0.9.0 (Oct 20) predates v0.10.0—so 0.9.0 is >89 commits behind main.
Missing evidence body for CONSISTENCY.md (# SECTION F-EVIDENCE): # SECTION F-EVIDENCE quotes only the two heading lines (docs/CONSISTENCY.md (lines 5610,5610) and (lines 5668,5668)). Consultation count 5, LoCoMo 94.2%, and 16,034 vs 27,766 appear only secondhand in B-DELTA.md, ASK.md, and plan-v3.md, not directly verified from §74–§75 text.
Un-commanded footprint delta (AGENTS.md §1 vs. B-DELTA.md): # SECTION C-STATE (AGENTS.md §1) states "Do not trust a size quoted here without re-running stat — quote the command, not the number." Both B-DELTA.md and plan-v4.md §4 (# SECTION G-ANALYSIS) report 10.73 MiB without quoting stat or byte counts.
Stale plan-v3.md metrics and missing supersession header (# SECTION G-ANALYSIS vs. # SECTION H-ANALYSIS): plan-v4.md (# SECTION G-ANALYSIS) claims ...-plan-v3.md "now carry[s] supersession headers", yet # SECTION H-ANALYSIS (plan-v3.md (lines 1,276)) opens with **Status:** proposal and no supersession header. plan-v3.md also reports 4,241 memories in omp and 41% auto-injection reduction, vs. 4,246 (ASK.md R2 / plan-v4.md) and 42% (1 - 16,034/27,766 = 42.25% in B-DELTA.md).
Door-parity overclaim (plan-v4.md §0 vs. plan-v3.md §4 + B-DELTA.md): plan-v4.md §0 claims "Door parity is closed for everything except hygiene's MCP tool", but plan-v3.md §4's parity table shows supersede missing CLI (❌) and grounding missing HTTP and MCP (❌), neither of which is closed in B-DELTA.md (which only added redact HTTP in ed96bbd, plus supersede HTTP / hygiene CLI in 34237bf).
connect vs. get-memory-wire.sh conflation (ASK.md R2 vs. AGENTS.md §11): ASK.md R2(a) describes memory-wire connect --bank omp as forwarding the pin "to every connect it runs, including the --uninstall that unwires hosts", which # SECTION D-LAW (AGENTS.md §11) attributes to install/get-memory-wire.sh --bank <id>, not connect.
LoCoMo metric nuance (AGENTS.md §1 vs. B-DELTA.md): B-DELTA.md reports R@10 94.2% at load 23.71 on 24 cores (a ranking measurement, not latency); AGENTS.md §1 (# SECTION C-STATE) clarifies 94.2% is recall_any@10, while multi-session recall_all@10 is 83.0%.
Rulings on the Five Blocker Decisions
R1 — The release
Recommendation. Named alternative (a-gated): Land R4(b) (doctor fix) + bump Cargo.toml (> 0.10.0) → cut the three-Linux release via scripts/build-release.sh (with CI workflows disabled) → install/upgrade the local 0.9.0 binary. (Argues against cutting (a) literally right now before R4(b)/version bump, and against waiting in (b) on externally blocked items.)
Reasoning.
Option (c) is invalid: the recent fixes (ed96bbd HTTP redact, 34237bf HTTP supersede + CLI hygiene, 6df26dd doctor bank input in # SECTION B-DELTA) sit atop the W1–W7 feature build (plan-v3.md §1.1, # SECTION H-ANALYSIS), and # SECTION D-LAW (AGENTS.md §11) defines no backport/patch-branch workflow.
Option (b) stalls indefinitely if tied to the open TODO register: 4 of 6 open rows are externally blocked and "Nothing here is a feature gap" (plan-v3.md §2).
However, [MINE] (a) ("Cut now from main") is incomplete as written: Cargo.toml is already 0.10.0 and tag v0.10.0 already exists (ASK.md R1), so scripts/build-release.sh will refuse to run without a version-bump commit + cargo check (AGENTS.md §11); GitHub Actions quota is exhausted so ci.yml and release.yml must be disabled around the tag push (AGENTS.md §11); cutting a GitHub release alone leaves the local 0.9.0 binary unchanged; and plan-v4.md §5 (# SECTION G-ANALYSIS) identifies one <1h Tier-1 code defect (doctor stale state file = R4(b)) that should land before tagging.
Reasoning gap.
[OPERATOR] Whether a release is wanted before remaining non-blocked work (and whether the bump should be 0.10.1 vs 0.11.0) is not in evidence — here is what would settle it: operator sign-off on R4(b)-first + version number.
Missing machine evidence: what binary/commit pid 1295 is executing and the exact stat byte size (AGENTS.md §1). Not in evidence — settled by: ls -l /proc/1295/exe && stat -c '%s %n' target/release/memory-wire $(command -v memory-wire).
Confidence. Medium-High (High against (c) and open-ended (b); Medium on exact sequence pending [OPERATOR] confirmation and /proc/1295/exe).
R2 — The bank
Recommendation. (a) memory-wire connect --bank omp (agreeing with [MINE], once authorized, and auditing all IMPLICIT = 7 hosts rather than only the 5 named in ASK.md).
Reasoning. memory-wire doctor shows ishan-parihar-memory-wire holds 0 memories while omp holds 4,246 across 9 banks, causing silent empty recall on every hook firing (# SECTION A-ASK R2; # SECTION G-ANALYSIS §1). Option (b) (MEMORY_WIRE_BANK=omp per shell) fails silently in IDE/non-shell-launched hosts, and (c) (migrating memories) risks breaking any existing omp readers with no migration tooling evidenced in the pack. Note that ASK.md R2 names 5 host configs while plan-v4.md §4 (# SECTION G-ANALYSIS) records Host::ALL = 8 and IMPLICIT = 7 (e.g., Codex in plan-v4.md §0).
Reasoning gap.
[OPERATOR] Whether the live host configs may be rewritten is not in evidence — here is what would settle it: explicit operator authorization to edit ~/.claude.json, ~/.copilot/, ~/.cursor/, ~/.config/opencode/, ~/.hermes/config.yaml (and check Codex).
Also not in evidence: what the other 7 of 9 banks hold and whether any host intentionally relies on a non-omp bank. Settled by: listing all 9 banks with memory counts and checking existing bank pins in the 7 IMPLICIT host configs.
Confidence. High on (a) once authorized (moves to Low only if the store listing reveals omp is deprecated or belongs to a different project/host).
R3 — agent-memory-bench
Recommendation. (b) Defer until the release (R1) and bank (R2) decisions land. (Disagreeing with [MINE] (a) to fund/run it now.)
Reasoning.
ASK.md R3 (# SECTION A-ASK) states agent-memory-bench is not cloned, is measure-only (AGENTS.md §6, # SECTION E-LAW2), and has zero technical deferral cost.
[WEB] (read 2026-11-22, https://github.com/GiulioDER/agent-memory-bench[[1](https://www.google.com/url?sa=E&q=https%3A%2F%2Fvertexaisearch.cloud.google.com%2Fgrounding-api-redirect%2FAUZIYQEBvwuS6mxqEEMGfafWZtaT0Wef8fm9oOoW891o3vCgyOrKIHDDSpfuaDknn6Kpataiuy-4YGhwcLJYV7r8KONCfA7JTBzKMW_OXkao-2vnxDl8Mub-l1fuQrFCz76pGkC0lXZkFRiy4Ay_jAWmr1_Xl2VIcu0apBgzPk_JqEmuvaQjDzDQLYJz_Q48Au4QOBK8fZToTEpTfrPBDLE0lbkknAFWf_vUug%3D%3D)][[2](https://www.google.com/url?sa=E&q=https%3A%2F%2Fvertexaisearch.cloud.google.com%2Fgrounding-api-redirect%2FAUZIYQHx6uyfICLFgSrAF9gVV2QuBxFx7qmOj-uGupZj4u_Zcums2KgA0rqh7vWnq7fzTqSA-Tt0bs6L_lypAqmv1OmWXl9kEs-nEaqdG25YnzncSfbnDY6S7YDLchlhjMz9EKzdAFhRCRdLYiWq7v_MdkRi5tCyORGwvD9_z_iI6xtU7cktqiTcv9fvv4TKZoFnenpoKWMH)]): GiulioDER/agent-memory-bench runs Claude Code inside Docker against executable task tests across 195 session transcripts (plus a 4,900-doc hard corpus) constructed specifically to test absent, stale, superseded, contradictory, adjacent, and irrelevant memories, and mainly measures the read path.
Running it now against local 0.9.0 (which lacks W1 dimensional schema, memory_redact, memory_supersede, and grounding per ASK.md R1 and plan-v3.md §1.1) or an untagged working tree violates # SECTION C-STATE (AGENTS.md §1: "Do not publish a number you cannot trace to a committed artifact") and wastes money measuring old retrieval/hook behaviour before R1+R2 land.
Reasoning gap.
[OPERATOR] Available API budget for the answering model is not in evidence — here is what would settle it: operator stating a dollar cap.
Also not in evidence: per-task token cost in Claude Code for memory-wire and whether an adapter already exists. Settled by: cloning GiulioDER/agent-memory-bench (free) to inspect the adapter contract and pricing a 1–5 task dry run against the R1 release candidate.
Confidence. High on (b) over (c); Medium-High on (b) over (a) (would move to (a) only if the operator holds expiring API credits and explicitly accepts throwing away the pre-R1 baseline).
R4 — The unadopted server
Recommendation. (b) Fix only the doctor's silence (agreeing with [MINE] against (a), and rejecting (c)).
Reasoning. # SECTION A-ASK R4 and # SECTION G-ANALYSIS §1 (plan-v4.md) prove doctor (server up) and daemon status (not running, stale serve.json pid 1371296 from Oct 17) contradict each other because stale_serve_at line 397 (let running = exe_of(pid)?;) aborts silently on a dead pid. Option (c) leaves tooling actively misleading. Option (a) (daemon adopt) is unsafe without knowing how pid 1295 was launched, risks disrupting an unknown-load-bearing server, and adds permanent CLI surface (AGENTS.md §1). Option (b) is a Tier-1 helper fix (plan-v4.md §5) that names the stale serve.json and unadopted server without touching pid 1295.
Reasoning gap.
[OPERATOR] Whether pid 1295 is load-bearing (serves live traffic) is not in evidence — here is what would settle it: operator confirmation plus ps -fp 1295 / /proc/1295/cmdline.
Hidden integration gap not in evidence: plan-v4.md §0 states paths::endpoint() resolves env -> daemon state file -> DEFAULT_ADDR. What serve.json currently contains is unknown—if serve.json specifies a port other than 8888, hooks are currently misrouted! Settled by: cat /home/ishanp/.local/share/memory-wire/serve.json.
Confidence. High on (b) over (a) and (c) (unchanged even if pid 1295 is load-bearing, because (b) is read-only; if serve.json points off-8888, immediate manual cleanup of serve.json is needed alongside (b)).
R5 — The benchmark battery
Recommendation. (b) Run LongMemEval-S at the release, not before. (Agreeing with [MINE] against (a), and rejecting (c).)
Reasoning. Recent commits (B-DELTA.md, # SECTION B-DELTA) touched doors, a doctor line, docs, and a count gate—no ranking code—and both free harnesses re-ran unchanged (LoCoMo R@10 94.2% at load 23.71 on 24 cores; relevance battery H8 held, 42% fewer auto-injected chars). Running LongMemEval-S now ((a)) violates # SECTION E-LAW2 (AGENTS.md §6: test set is for "measurement and release acceptance only") and spends a counted consultation (5 per ASK.md R5). Option (c) ("Never re-run") forecloses release-acceptance verification across the 89-commit +36,278/-2,627 diff (plan-v3.md §1) and conflicts with AGENTS.md §1 ("Retrieval reports are the record").
Reasoning gap. Because # SECTION F-EVIDENCE omits the body of docs/CONSISTENCY.md §74–§75, the consultation count 5 and the commit SHA of the last LongMemEval-S run are not in evidence — here is what would settle it: reading docs/CONSISTENCY.md §74–§75 and eval/RESULTS.md (to confirm whether LongMemEval-S already ran after W1–W7).
Confidence. High against (a); Medium-High on (b) over (c) (would move to (c) only if eval/RESULTS.md shows LongMemEval-S already ran after all W1–W7/auto-injection changes and R1 adds zero retrieval-path changes).
Priority Ranking: Cost-of-Being-Wrong × Difficulty-of-Undo
Ordered highest to lowest on (cost of being wrong) × (difficulty of undo):
Rank	Ruling	Cost of being wrong	Difficulty of undo	Why it sits here
1	R2 — The bank	Very High: silent 0-recall across all hooks today; (c) risks cross-bank fragmentation / breaking omp readers	Medium–High: (a) rewrites 5+ host configs; wrong writes/migrations split 4,246+ memories across banks	Silent live failure right now; store splits are messy to reconcile later
2	R4 — The unadopted server	High: disrupting live pid 1295 loses live service; (a) bloats CLI contract	High for (a)/restart, Low for (b): shipped CLI commands & killed live state don't undo cleanly	serve.json vs paths::endpoint() could already be misrouting hooks, and pid 1295 safety is unknown
3	R5 — The benchmark battery	Medium: wastes a finite release-acceptance check on a non-ranking diff (or skips release proof)	Irreversible: a burned/counted LongMemEval-S consultation cannot be un-counted (AGENTS.md §6)	Non-replenishable evaluation resource must be preserved for the R1 tag
4	R3 — agent-memory-bench	Medium: spending cash to benchmark unreleased 0.9.0 yields an untraceable/obsolete result	Irreversible (cash): spent API budget cannot be refunded, though deferring (b) has 0 technical cost	Pure spend decision that gets strictly better data if delayed until after R1 + R2
5	R1 — The release	Low–Medium: growing drift (>89 commits, 0.9.0 lacks memory_redact), though secret-scan.sh gates leaks	Low: "not irreversible either way — a later cut supersedes an earlier one" (ASK.md R1)	Reversible via subsequent cut, though operationally needed to unblock R3 and R5
The three that matter most:
R2 (The bank) — It is the only active, silent product failure on the host (0 recalled vs 4,246 stored), and wrong migration choices fragment data.
R4 (The unadopted server) — Resolving (b) safely surfaces whether serve.json is misrouting paths::endpoint() and avoids breaking an unknown-load-bearing pid 1295 or locking in daemon adopt.
R5 (LongMemEval-S) — On pure irreversibility, burning the finite test-set instrument before R1 freezes is the hardest mistake to undo. (In execution order, the path is R4(b) → R1 (cut + local upgrade) → R2, then R5(b) and R3(b) once the local stack is actually healthy and tagged.)
Application of the Five Lenses (L1–L5)
L1 — Bottleneck
code
Code
download
content_copy
expand_less
L1 — Bottleneck
findings: 2
single-operator-queue: research/audits/context-pack-2026-11-21-blockers/ASK.md (# SECTION A-ASK, R1–R5) — All five decisions queue behind one operator while installed-binary drift (>89 commits past v0.10.0) and silent 0-recall hook failures keep accumulating. [class: L1]
adds: Resolving R1/R2 by rewriting 5 host configs and bumping versions adds cross-file configuration coupling (L4) if any of the IMPLICIT=7 hosts is missed.
ci-quota-exhaustion: AGENTS.md §11 (# SECTION D-LAW, lines 365–370) — Exhausted GitHub Actions quota forces every release through a single local host (scripts/build-release.sh plus manual workflow disable/re-enable), blocking automated CI verification. [class: L1]
adds: Local-only gh REST cuts remove clean-room CI checks and preserve host-environment coupling such as the glibc 2.44 / floor >= 2.39 constraint (L4).
L2 — Redundancy
code
Code
download
content_copy
expand_less
L2 — Redundancy
findings: 3
open-hook-self-contradiction: docs/plans/2026-11-21-remaining-gaps-plan-v4.md §2 (# SECTION G-ANALYSIS) — docs/OPEN_HOOK_RECALL_RELEVANCE.md carries a correction block at line 27 stating recall_at keeps scores, while §"Mechanism" item 2 eight lines below still asserts recall_at drops scores and cites a wrong line (src/api.rs:1903). [class: L2]
adds: Adding the optional symbol-line gate proposed in plan-v4.md §5 #4 introduces a new test-battery and gate-maintenance burden (L1).
plan-v3-stale-copy: docs/plans/2026-11-21-remaining-gaps-plan-v3.md (# SECTION H-ANALYSIS, lines 1–276) vs. plan-v4.md (# SECTION G-ANALYSIS) — plan-v3.md lacks the supersession header plan-v4.md claims it has and preserves stale counts (4,241 memories vs 4,246; 41% vs 42% auto-injection cut). [class: L2]
adds: Maintaining manual supersession headers across rapid same-day plans adds documentation attention overhead (L1).
unverified-footprint-prose: AGENTS.md §1 (# SECTION C-STATE) vs. research/audits/context-pack-2026-11-21-blockers/B-DELTA.md (# SECTION B-DELTA) — Both quote binary size in prose (10.65 MiB vs 10.73 MiB) without the stat command AGENTS.md §1 requires. [class: L2]
adds: None (removing un-commanded prose numbers in favour of running stat adds zero structural defect).
(Deliberate duplication examined: Tier-C extraction vs. _audit/ parents in AGENTS.md §1 and non-extraction of the TypeScript door core in B-DELTA.md 59a2919 are intentional with recorded reasons—not findings.)
L3 — Fragmentation
code
Code
download
content_copy
expand_less
L3 — Fragmentation
findings: 2
door-parity-asymmetry: docs/plans/2026-11-21-remaining-gaps-plan-v3.md §4 (# SECTION H-ANALYSIS) & B-DELTA.md (# SECTION B-DELTA, ed96bbd) — Even after redact gained an HTTP route, supersede lacks CLI and grounding lacks HTTP and MCP (contradicting plan-v4.md §0's claim that only hygiene MCP remains). [class: L3]
adds: Implementing every missing door would duplicate door handlers/tests (L2) and grow the 10.73 MiB / DT_NEEDED=3 footprint (L1/constraint risk).
local-vs-tree-split: research/audits/context-pack-2026-11-21-blockers/ASK.md (# SECTION A-ASK, R1) — Installed 0.9.0 serves 0 memory_redact occurrences while the tree serves 6 MCP tools including memory_redact and memory_supersede, splitting capability by binary path. [class: L3]
adds: Upgrading local 0.9.0 via R1 creates a transient mismatch with any un-restarted server process such as pid 1295 (L4).
L4 — Integration gap
code
Code
download
content_copy
expand_less
L4 — Integration gap
findings: 3
doctor-vs-daemon-silence: docs/plans/2026-11-21-remaining-gaps-plan-v4.md §1 (# SECTION G-ANALYSIS) & ASK.md (# SECTION A-ASK, R4) — doctor reports "server up" (pid 1295) while daemon status reports "not running" (dead pid 1371296 in serve.json), and stale_serve_at line 397 returns None when the pid is dead. [class: L4]
adds: Fixing doctor's reporting without adding daemon adopt (R4(b)) leaves lifecycle intervention manual (L1) and preserves daemon/serve asymmetry (L3).
unverified-endpoint-routing: docs/plans/2026-11-21-remaining-gaps-plan-v4.md §0–§1 (# SECTION G-ANALYSIS) — paths::endpoint() resolves env -> daemon state file -> DEFAULT_ADDR, yet nothing verifies whether stale serve.json points hooks away from pid 1295's port 8888. [class: L4]
adds: Inspecting or clearing stale serve.json before knowing pid 1295's role requires operator verification (L1).
pack-count-drift: research/audits/context-pack-2026-11-21-blockers/B-DELTA.md (# SECTION B-DELTA) vs. ASK.md (# SECTION A-ASK, R1) — B-DELTA.md asserts "Sixteen commits after 303a096" (list shows 13 after) and "0.9.0 is 89 commits behind v0.10.0" (ASK.md shows main is 89 past v0.10.0 while 0.9.0 predates it), with no gate checking pack prose. [class: L4]
adds: Extending count gates to arbitrary prose would create high false-positive maintenance overhead (L1), which plan-v3.md §3.4 explicitly advises against.
L5 — Per-Recommendation Trade Summary
R1 ((a-gated): R4(b) + bump Cargo.toml → three-Linux release → local upgrade):
Reduces L3 (local 0.9.0 vs main capability split) and L1 (clears growing >89-commit release queue).
Adds L1 (manual version bump, CI workflow disable/re-enable, local reinstall) and transient L4 (if pid 1295 stays running on the prior binary after local install).
R2 ((a): connect --bank omp across live hosts after authorization):
Reduces L4 (aligns hook resolution with the omp bank holding 4,246 memories instead of empty ishan-parihar-memory-wire).
Adds L4/L2 coupling across 5–7 external host config files outside the repo if any IMPLICIT host is omitted or later diverges.
R3 ((b): Defer agent-memory-bench until R1 + R2 land):
Reduces L1 (keeps un-scoped adapter/Docker/API spend out of the critical path) and L4 (prevents benchmarking an unreleased 0.9.0 binary untraceable to a tag).
Adds L1 (leaves the execution-graded product evidence gap queued behind R1/R2).
R4 ((b): Fix doctor's silence on stale serve.json + unadopted pid 1295):
Reduces L4 (removes the silent doctor vs daemon status contradiction safely without touching pid 1295).
Adds L1 (manual operator restart/inspection if action is needed) and L3 (retains intentional asymmetry where daemon cannot adopt external serve processes).
R5 ((b): Run LongMemEval-S once at the R1 release boundary):
Reduces L1 (preserves finite counted test-set budget at count = 5) and L2 (avoids duplicating ranking checks just run on LoCoMo + relevance battery).
Adds L4 (short deferral gap between B-DELTA.md and release-boundary test-set confirmation on the final R1 tag).
Sources
help
huggingface.co
github.com
Google Search Suggestions
Display of Search Suggestions is required when using Grounding with Google Search. Learn more
"GiulioDER/agent-memory-bench"