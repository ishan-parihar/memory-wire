# Context pack — round 5, 2026-11-20
#
# Question: where is memory-wire's architecture actually bottlenecked,
# redundant, fragmented, or gapped on the path to an optimal agentic
# experience -- and which one should be fixed next?
#
# Rules applied:
#   - slice by ANCHOR, never by line number (round 1 measured that failure)
#   - no history: no worklogs, no superseded sections. The delta carries what
#     changed and the register rows carry the readings.
#   - every figure keeps its producer beside the spec (measure_surface.sh), so
#     the reviewer re-derives a number instead of trusting a claim.
#   - evidence classes are the subject's own, inline.
#   - the pack's own framing stays out of the evidence: C-ASK carries the
#     question, B-DELTA carries facts, and nothing else interprets.
#
# Excluded, deliberately: target/, research/, eval/ outputs, .git, and every
# file holding operator-private state or credentials (AGENTS 14). The
# PersonaMem run directory is gitignored because it wrote memory rows verbatim
# and one held a pasted token -- it is NOT in this pack and is not available
# to the reviewer.

[A-STATE] the subject's own current self-description -- these are claims to audit, not facts
AGENTS.md "@## 1. What this project is..@## 2. Build, test, run"
AGENTS.md "@## 3. Where things live..@## 4. The surface"

[B-DELTA] what landed after round 4 was sent, and round 4's own claims re-checked
research/audits/context-pack-2026-11-20-round5/B-DELTA.md

[C-ASK] the decision this round exists to test, and what it is NOT asking
research/audits/context-pack-2026-11-20-round5/C-ASK.md

[D-LAW] the rules a proposal must survive, verbatim: the eval set is not a training set, rejected work stays rejected, output conventions
AGENTS.md "@## 6. The evaluation set is not a training set..@## 7. Artifacts and measurement"
AGENTS.md "@## 8. Rejected work stays rejected..@## 10. Output conventions for agents"

[E-SURFACE] the measured surface: modules, code-vs-test split, doors, hosts, duplication, footprint -- every figure re-derivable
research/audits/context-pack-2026-11-20-round5/measure_surface.sh

[F-CLOSED] every lever already measured and removed. Do not re-propose any of these.
docs/handoff/CLOSED.md

[G-RELEVANCE] the relevance question round 2 adopted and nobody has re-measured: what is actually wrong, and why a threshold is not the fix
docs/OPEN_HOOK_RECALL_RELEVANCE.md "@## Symptom..@## The same defect exists independently, twice"
docs/OPEN_HOOK_RECALL_RELEVANCE.md "@## What is NOT wrong, and must not be \"fixed\"..@## Where the fix belongs"

[H-GAPS] the integration gaps as the repo itself records them, including the 2026-11-19 addendum on door asymmetry
docs/INTEGRATION_GAPS.md "@## G1 — No persistent service, and our hooks fail silently without one..@## G2 — We capture nothing at the two moments worth capturing"
docs/INTEGRATION_GAPS.md "@## G5 — 4 MCP tools against 8, 39 and 54, and stdio only..@## G6 — Smaller items, each cheap"
docs/INTEGRATION_GAPS.md "@## Addendum 2026-11-19 — the doors are asymmetric, and that is the honest state..@EOF"

[I-DOORS] the four auto-injection doors, the policy they must all implement -- read all four and say where they disagree
src/hooks.rs "@The trust gate lives here..@fn is_untrusted"
plugin/integrations/hermes/memory-wire/__init__.py "@def prefetch..@def sync_turn"
plugin/integrations/extension/memory-wire.ts "@var UNTRUSTED_TIER..@EOF"
plugin/integrations/opencode-plugin/memory-wire.ts "@var UNTRUSTED_TIER..@EOF"

[J-REGISTER] what is open, and what each open row is blocked on
TODO.md "@## Open..@## Deliberately not doing"
