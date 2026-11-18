# Context pack — round 1, 2026-11-18
# Question: is the W0-W6 plan the right shape for memory-wire's upgrade,
# judged on whether it produces an optimal agentic experience?
#
# Rules applied: slice by ANCHOR, never by line number. No history.
# Evidence classes are the subject's own. This file is the build input;
# PACK-AS-SENT.md is its one output.

[A-STATE] what memory-wire believes about itself, and the tree state this round must not misread
docs/handoff/STATE.md "@## 1. What is in the working tree..@### Product changes"
docs/handoff/STATE.md "@## 2. Current numbers..@## 3. Instrument caveats"

[B-LAW] the rules a proposal must survive, verbatim: evaluation set is not a training set, rejected work stays rejected, output conventions for agents
AGENTS.md "@## 6. The evaluation set is not a training set..@## 7. Artifacts and measurement"
AGENTS.md "@## 8. Rejected work stays rejected..@## 10. Output conventions for agents"
AGENTS.md "@## 10. Output conventions for agents..@## 11. Git and releases"

[C-EVIDENCE] what has already been measured, and what it bounds — closed levers, the four "inert" mechanisms' real verdicts, the benchmark record
docs/handoff/CLOSED.md
eval/LOCOMO_LEVERS.md
README.md "@## Benchmarks..@## What an answer"
README.md "@## What an answer actually costs..@## Footprint vs competitors"

[D-ASK] the live, diagnosed-but-unfixed defect on the agent experience path — the decision this round exists to test
docs/OPEN_HOOK_RECALL_RELEVANCE.md

[E-ANALYSIS] the artefact under review: the audit and the plan it produced
docs/audit/2026-11-18-surface-audit.md "@## Scope..@## Part II"
docs/audit/2026-11-18-surface-audit.md "@## Part II — Findings, ranked..@## Part III"
docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md "@## 0. What this plan..@## 1. The shape"
docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md "@## 1. The shape..@## 2. Items"
docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md "@## 4. Gates..@## 5. Cost"

[F-LENS] the surface the agent actually touches: what connect writes, what hooks inject, what MCP exposes
README.md "@## Connect, hooks, doctor..@## Bank config"
README.md "@## MCP..@## How it works"