# Context pack — rulings round, 2026-11-20
#
# Question: three decisions wait on the operator and no artefact in the repo
# settles any of them. What should be decided, what would change each answer,
# and which matters most?
#
# This is a RULINGS round, not an audit round. RULINGS.md is the ask section and
# carries the decisions; REVIEWER-PROMPT.md states the per-ruling output
# contract. The pack carries facts; the prompt carries the question.
#
# Rules applied:
#   - slice by ANCHOR, never by line number (round 1 measured that failure)
#   - no history: no worklogs, no superseded sections
#   - every figure keeps its producer beside the spec (measure_rulings.sh), so
#     the reviewer re-derives a number instead of trusting a claim
#   - the pack's own framing stays out of the evidence: RULINGS carries the
#     decisions, B-MEASURED carries the measurements, nothing else interprets
#   - authored surface only: target/, research/ prior rounds, eval outputs and
#     .git are excluded, and the count names that exclusion
#
# Excluded, deliberately: target/ (build residue), every prior round under
# research/audits/ (history, and the drift gate holds sent rounds anyway), eval/
# outputs (generated), and any file holding operator-private state or
# credentials (AGENTS 14). The PersonaMem run directory is gitignored because it
# wrote memory rows verbatim and one held a pasted token -- it is NOT in this
# pack and is not available to the reviewer.

[A-STATE] the subject's own current self-description -- claims to audit, not facts
AGENTS.md "@## 1. What this project is..@## 2. Build, test, run"
AGENTS.md "@## 3. Where things live..@## 4. The surface"

[B-MEASURED] the live surface, every figure re-derivable from the script kept beside this spec
research/audits/context-pack-2026-11-20-rulings/measure_rulings.sh

[C-RULINGS] the three decisions waiting on the operator, with options, cost of deferral, and owner
research/audits/context-pack-2026-11-20-rulings/RULINGS.md

[D-PLAN] the plan whose execution is in question, verbatim
docs/plans/2026-11-20-remaining-gaps-plan.md

[E-LAW] the rules a recommendation must survive, verbatim: eval is not a training set, rejected stays rejected, output conventions, git and releases
AGENTS.md "@## 6. The evaluation set is not a training set..@## 7. Artifacts and measurement"
AGENTS.md "@## 8. Rejected work stays rejected..@## 10. Output conventions for agents"
AGENTS.md "@## 11. Git and releases..@## 12. Honesty rules for reports"

[F-CLOSED] every lever already measured and removed. Do not re-propose any of these.
docs/handoff/CLOSED.md

[G-DOORS] the four auto-injection doors and the two TypeScript doors R1 is about -- read them and verify the duplication count
src/hooks.rs "@/// **The trust gate lives here.**..@fn is_untrusted"
plugin/integrations/hermes/memory-wire/__init__.py "@class MemoryWireProvider(MemoryProvider):..@def _hermes_home"
plugin/integrations/extension/memory-wire.ts "@var UNTRUSTED_TIER..@EOF"
plugin/integrations/opencode-plugin/memory-wire.ts "@var UNTRUSTED_TIER..@EOF"

[H-REGISTER] what is open, what each open row is blocked on, and what the plan already ruled out
TODO.md "@## Open..@## Deliberately not doing"

[I-GAPS] the integration gaps as the repo itself records them, including the door-asymmetry addendum
docs/INTEGRATION_GAPS.md "@## Addendum 2026-11-19 — the doors are asymmetric, and that is the honest state..@EOF"
