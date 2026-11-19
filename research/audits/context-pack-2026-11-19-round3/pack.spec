# Context pack — round 3, 2026-11-19
#
# Question: the W0-W7 cycle is shipped and three cycles of work have gone into
# retrieval ORDERING. The candidate pool holds gold in 500/500 LongMemEval
# questions and LoCoMo recall_any@10 is 94.2% — ordering is largely solved, and
# the 83.0% recall_all@10 gap is an ordering problem too. Meanwhile the PRODUCT
# surface (what an agent can DO with a recalled memory) has not been re-derived
# since v0.4.0. This round asks where the NEXT cycle's effort goes, and whether
# it should keep going where the last three went.
#
# Round 2 asked where the shipped architecture was and what to build next; it
# produced four verified structural findings, two now closed (see B-DELTA D1,
# D2). This round is the delta round: it carries the NEW evidence (D1-D4) plus
# the artefacts needed to judge the next-cycle question, and it deliberately
# does NOT re-pack what round 2 already established unless the delta changes it.
#
# Rules applied:
#   - slice by ANCHOR, never by line number (round 1 measured the failure)
#   - no history; the register rows carry the readings
#   - the measurement behind every packed figure ships beside the spec
#     (measure_surface.sh + SURFACE.md)
#   - evidence classes are the subject's own, inline
#   - the pack carries ARTEFACTS and RULINGS; the synthesis is in the prompt
#
# Size: built at --max-bytes 110000, the same cap as round 2. The aistudio
# venue shares one 1M window between input, thinking and output, so the cap's
# job there is "will it still reason", not "will it fit".

[A-STATE] what the subject says it is, and what it believes about itself
README.md "@## The wider field, checked 2026-10-15..@## Quick start"
docs/ARCHITECTURE.md

[B-DELTA] what changed since round 2, figures verbatim with owners
research/audits/context-pack-2026-11-19-round3/B-DELTA.md

[C-LAW] the rules any proposal must survive, verbatim
AGENTS.md "@## 1. What this project is..@## 2. Build, test, run"
AGENTS.md "@## 6. The evaluation set is not a training set..@## 8. Rejected work stays rejected"
AGENTS.md "@## 9. Interface rules..@## 11. Git and releases"

[D-EVIDENCE] what has been measured and what it bounds — the closed levers, the instrument caveats
docs/handoff/CLOSED.md
docs/handoff/STATE.md "@## 2. Current numbers..@## 3. Instrument caveats"

[E-ASK] the open decision this round exists to test
research/audits/context-pack-2026-11-19-round3/C-ASK.md

[F-ANALYSIS] the artefacts under review: the product surface as SHIPPED
docs/INTEGRATION_AUDIT.md "@## 3. The capability matrix, honestly..@## 5. Where we are better than the competitors"
docs/INTEGRATION_GAPS.md "@## Addendum 2026-11-19 — the doors are asymmetric, and that is the honest state..@EOF"
TODO.md "@## Open..@## Deliberately not doing"
docs/handoff/RND_PLAN.md "@## 1. What the evidence says about where headroom is..@## 2. Order of work"
docs/handoff/RND_PLAN.md "@## H1 — Coverage levers for multi-session questions..@## H3"

[G-LENS] the surfaces an agent actually touches — the injection path, the doors, the wire
README.md "@## Connect, hooks, doctor..@## Bank config"
README.md "@## MCP..@## How it works"
docs/OPEN_HOOK_RECALL_RELEVANCE.md

[H-MEASURE] the authored-surface measurement this round cites, with its exclusions named
research/audits/context-pack-2026-11-19-round3/measure_surface.sh
research/audits/context-pack-2026-11-19-round3/SURFACE.md