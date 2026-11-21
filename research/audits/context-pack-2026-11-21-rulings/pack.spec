# Context pack — plan review, 2026-11-21
#
# Question: the plan for the remaining work, attacked before it is executed.
# Not a proposal -- a plan with three tiers, and the reviewer's job is to say
# which item is mis-ranked, what none of it catches, and what to cut.
#
# Rules applied:
#   - whole files, no slices: every section is read in full, so no anchor can
#     silently redirect after an edit
#   - no history: no worklogs, no superseded sections, no prior rounds
#   - the plan carries its own figures; the prompt names each one so the
#     reviewer checks it against the artefacts rather than against my prose
#
# Excluded, deliberately: target/, every prior round under research/audits/,
# eval/ outputs, and any file holding operator-private state or credentials
# (AGENTS 14). The PersonaMem run directory is gitignored because it wrote
# memory rows verbatim and one held a pasted token.

[A-STATE] the subject's own current self-description, so the reviewer sees what it believes about itself before it asks what is true
AGENTS.md

[B-ASK] the plan under review: what is proposed, in what order, and what each tier is blocked on
docs/plans/2026-11-21-remaining-gaps-plan.md

[C-LAW] the rules a proposal must survive, verbatim, so the reviewer attacks the real thing
docs/handoff/CLOSED.md

[D-EVIDENCE] what is open, what each open row is blocked on, and what the register already ruled out
TODO.md
