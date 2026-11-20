# Reviewer prompt — memory-wire rulings round

You are reviewing a Rust project called **memory-wire**: a single-binary agent
memory service (SQLite + FTS5, HTTP and MCP, hooks that capture conversation
prose). The pack is the only thing you know about it. You have no session
history and no access to the repository.

**Treat the pack's claims as CLAIMS TO AUDIT, not as ground truth.** Where the
pack asserts a number, a count, or a state of the world, check it against the
artefacts the pack actually carries. If the pack says a thing and the artefact
shows another, the artefact wins and you should say so.

**If a question cannot be answered from the pack, say "not in evidence — here is
what would settle it".** That is a complete answer. A confident recommendation
the pack cannot support is a defect in your reply.

## The subject, and what it is not

The subject is memory-wire, this repository, at commit `05672ec`. It is **not**
Hindsight, **not** agentmemory, **not** aroute, and **not** any earlier round of
this review. If your reply turns out to be about something the pack's state
section does not name, say so and stop.

## What this round is for

The project has a written plan for its remaining work
(`docs/plans/2026-11-20-remaining-gaps-plan.md`). That plan ends with **three
decisions waiting on one person**, and no artefact in the pack settles any of
them. They are in `# FILE: RULINGS.md`.

Your job is to recommend, not merely to audit. For each of the three rulings:

1. **Recommendation** — one of the row's options, or a named alternative with
   its claim. If a row offers options (a)/(b)/(c), pick one and say which.
2. **Reasoning** — which facts in the pack carry it, named by anchor
   (`# SECTION …`, `# FILE: …`).
3. **Reasoning gap** — what the pack does **not** settle, and **the one
   measurement that would settle it**. If your recommendation rests on an
   `[UNCONFIRMED]` or `[agent-held]` row, say so: the operator should know which
   preferences they are being asked to ratify.
4. **Confidence** — high / medium / low, and what would move it.

Then rank the three rulings by **cost-of-being-wrong × difficulty-of-undo**, and
close with the one or two that matter most and why. A reply that treats three
rulings as equal has not advised anyone: reversible decisions can be deferred,
irreversible ones cannot, and that ordering is the advice.

## The question behind the rulings

The request that produced this round was, verbatim: *"proceed with the full
scale execution of the implementation plan"*, for *"the remaining implementation
gaps, redundancy consolidation/cleanup or feature/operational upgrades"*.

For each recommendation, name **which defect class it reduces and which it
adds**. A recommendation that removes a bottleneck by adding a seam is a trade;
the operator should see the trade, not the slogan.

## What to do beyond the rulings

Expect the highest value to sit here rather than in the ranked advice:

- **The baseline audit.** Re-read the pack's evidence against its own artefacts.
  Counts go stale, premises go un-rechecked, and a plan inherits both. Name any
  figure in `# FILE: RULINGS.md` or `# SECTION B-MEASURED` that the pack's own
  artefacts contradict.
- **What the plan structurally does not see.** The plan was written by the same
  process that produced the code. Name the question it never asks.
- **The TS door duplication (R1) specifically.** The pack claims 13 identical
  constants and 8 identical functions across two TypeScript files. Verify that
  count from the pack. Then say whether removal (a), a fence (b), or acceptance
  (c) is right, and what would make (a) safe.
- **The parity suite (R2).** Its cost is estimated at 2-3 days and has never
  been scoped against the actual door APIs. Say whether that estimate is
  credible from what the pack carries, and what the cheapest version that still
  catches the failure class would be.

## Scope discipline

The pack alone must settle the rulings. You may draw on outside sources only for
the last section, and any outside claim must carry its URL, the date you read it,
and a marker of its own so it can be weighed apart from the pack's evidence.

## Output shape

```
## Rulings
R1 — recommendation / reasoning / gap / confidence
R2 — recommendation / reasoning / gap / confidence
R3 — recommendation / reasoning / gap / confidence

## Ranking
<the three, ordered by cost-of-being-wrong × difficulty-of-undo, with the reason>

## Baseline audit
<figures the pack's own artefacts contradict, and the premises never re-checked>

## What the plan does not see
<the question it never asks>
```

Be specific. A finding that names a file, a line, or a number is worth more than
a paragraph of well-formed caution.
