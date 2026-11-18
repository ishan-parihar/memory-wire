# Reviewer prompt — memory-wire upgrade, agentic experience

You are reviewing a small Rust agent-memory service. You have no session history and no repo.
The pack is all you know. **Treat the pack's claims as CLAIMS TO AUDIT, not as ground truth.**
Where the pack does not settle a question, say **"not in evidence"** and continue under a stated
assumption — do not fill the gap with plausible generalities.

## The decision this round exists to test

`memory-wire` stores an agent's memories and injects them back into the agent's context. A plan
(`E-ANALYSIS`) proposes six work items (W0–W6). **W0–W1 are correctness scaffolding; W2–W6 are
product.** The question is not "is the plan well-written". It is:

> Does W0–W6, as scoped, produce an *optimal agentic experience* — or does it optimise the
> subsystem that is already fine while leaving the defect that actually degrades the agent
> untouched?

The plan's own thesis is that ranking is near its ceiling and the loss moved to *serving shape*
(`C-EVIDENCE`, `docs/handoff/STATE.md`). Section **D-ASK** is a defect that was diagnosed on
2026-09-30 and deliberately never fixed. **W0–W6 does not list it.** Judge whether that omission
is right or wrong, and say which.

## What I want from you — five questions

1. **The omission.** Is D-ASK a defect worth a plan item, or a documented-and-closed record the
   plan correctly leaves alone? Note carefully: D-ASK's own doc says a *score threshold* is the
   wrong fix (a fitted constant, and `AGENTS.md` forbids it). If the defect is real but the
   threshold is not the fix, say what the fix *is*, and whether it fits the plan's invariants
   (no new dependency, no LLM in the binary, byte-identical output when a feature is off).

2. **The ranking.** Rank W0–W6 by what each buys an *agent*, not an engineer. Defend any
   reordering with a named anchor from `F-LENS`, `D-ASK` or `C-EVIDENCE` — a mechanism, not an
   intuition. Say which item is mis-ranked and in which direction.

3. **What W0–W6 structurally cannot see.** `AGENTS.md` §6 forbids selecting on the test set, and
   §8 forbids re-proposing closed levers. Both are load-bearing. But a plan that cannot move a
   number it claims to care about is a plan in denial. Which metric in `C-EVIDENCE` can *any* of
   W1, W2, W4, W5 move, and which of them cannot be moved by any item in this plan? Name the
   metric, not a feeling.

4. **The agentic-experience gap.** If you were the agent whose prompt was injected into, what
   would still be wrong after all six items shipped? Be concrete: what arrives in your context,
   at what moment, and what is still wrong with it? Do not propose a feature. Describe the
   experience.

5. **One thing missing from the pack.** What would you have needed to answer 1–4 properly, and
   what did this pack not carry?

## Constraints on your answer

- **Every finding names an anchor**: `# SECTION <NAME>` or `# FILE: <path> (lines a,b)`. A finding
  with no anchor is an assumption — label it one.
- **Every figure keeps its three attributes**: population, basis, selection constraint. A number
  without them is not usable.
- **Pack-only questions are 1, 3 and 4**: the pack is the only admissible evidence. If it does not
  settle them, say `not in evidence`.
- **Question 2 is open to outside sources** — if the wider agentic-memory field has a mechanism
  this plan should know about, cite it inline with its URL and the date you read it. Mark it as
  web-sourced. Do not use the web to answer 1, 3 or 4.
- **A rejection is as useful as a finding.** If you conclude W2–W6 are correctly scoped, say so
  and say why. Do not manufacture work to be helpful.

Reply in markdown, ≤ 2,500 words, numbered findings matching the questions above.