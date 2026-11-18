# Context pack — round 2, 2026-11-18
#
# Question: given the W0-W7 upgrade SHIPPED and the five audit-fix batches
# committed, where is the architecture now — what are its bottlenecks,
# redundancies, fragmentations and integration gaps on the path to an optimal
# agentic experience, and what should the next iteration actually be?
#
# Round 1 asked whether a PLAN was the right shape. Round 1's answer: yes on
# substance, no on process (the pack omitted the plan body — my defect, fixed
# here). The plan is now implemented. This round audits the SHIPPED thing.
#
# Rules applied:
#   - slice by ANCHOR, never by line number (round 1 measured the failure)
#   - pack the artefact under review WHOLE (round 1's lesson, §5)
#   - no history; the register rows carry the readings
#   - the measurement behind every packed figure ships beside the spec
#     (measure_surface.sh), so the reviewer re-derives rather than trusts
#   - evidence classes are the subject's own, inline
#
# Size: built at --max-bytes 110000. Round 1's lesson (§5 of its VERIFICATION.md)
# is that the artefact under review must be packed WHOLE, and the plan is 24.7 kB
# of that. On the aistudio/Hydrogen venue the 1M window is shared by input,
# thinking and output, so 110 kB (~30k tokens) still leaves the reviewer room to
# reason. The cap's purpose on that venue is "will it still think", not "will it
# fit".

[A-STATE] what the subject says it is, and what it believes about itself
README.md "@## The wider field, checked 2026-10-15..@## Quick start"
docs/ARCHITECTURE.md

[B-LAW] the rules any proposal must survive, verbatim
AGENTS.md "@## 1. What this project is..@## 2. Build, test, run"
AGENTS.md "@## 6. The evaluation set is not a training set..@## 8. Rejected work stays rejected"
AGENTS.md "@## 10. Output conventions for agents..@## 11. Git and releases"

[C-EVIDENCE] what has been measured and what it bounds — the closed levers, the benchmark record, the instrument caveats
docs/handoff/CLOSED.md
docs/handoff/STATE.md "@## 2. Current numbers..@## 3. Instrument caveats"
eval/baseline/README.md "@## What the battery contains, and what it deliberately leaves out..@## Baselines"

[D-ASK] the open decision this round exists to test — what the next iteration should be
TODO.md "@### The hierarchical-grounded-memory cycle..@EOF"

[E-ANALYSIS] the artefact under review: the upgrade as SHIPPED, in full
docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md
docs/audit/2026-11-18-surface-audit.md "@## Part II — Findings, ranked..@## Part III"
docs/audit/2026-11-18-surface-audit.md "@## Part III — What is genuinely unbuilt..@## Part IV"

[F-LENS] the surfaces an agent actually touches — what connect writes, what hooks inject, what MCP exposes, what the CLI does
README.md "@## Connect, hooks, doctor..@## Bank config"
README.md "@## MCP..@## How it works"
README.md "@## CLI..@## The daemon"
docs/OPEN_HOOK_RECALL_RELEVANCE.md

[G-MEASURE] the authored-surface measurement this round cites, with its exclusions named
research/audits/context-pack-2026-11-18-round2/measure_surface.sh
research/audits/context-pack-2026-11-18-round2/SURFACE.md