# Reviewer prompt — memory-wire rulings round

You are reviewing **memory-wire**, a Rust agent-memory service (SQLite + FTS5,
HTTP and MCP, hooks that capture conversation prose). The pack is all you know.
You have no session history and no repository access.

**Treat every claim in the pack as a CLAIM TO AUDIT, not ground truth.** Where
the pack asserts a number or a state, check it against the artefact the pack
carries. If they disagree, the artefact wins — say so.

**If the pack cannot settle something, answer "not in evidence — here is what
would settle it."** That is complete. A confident recommendation the pack cannot
support is a defect in your reply.

## Subject

memory-wire at `05672ec`. Not Hindsight, not agentmemory, not aroute, not an
earlier round. If your reply is about something the state section does not name,
say so and stop.

## The job

The plan (`# SECTION D-PLAN`) ends with **three decisions waiting on one person**
(`# FILE: RULINGS.md`). No artefact settles any of them. Recommend, do not
merely audit. For each ruling give exactly four things:

1. **Recommendation** — one of the row's options, or a named alternative.
2. **Reasoning** — the pack facts that carry it, by anchor (`# SECTION …`).
3. **Reasoning gap** — what the pack does not settle, and **the one measurement
   that would settle it**. If your answer rests on an `[agent-held]` row, say so.
4. **Confidence** — high/medium/low, and what would move it.

Then rank the three by **cost-of-being-wrong × difficulty-of-undo**, and name the
one or two that matter most. Treating three rulings as equal is not advice:
reversible decisions can be deferred, irreversible ones cannot.

## The request behind this round

*"Proceed with the full scale execution of the implementation plan"* — for *"the
remaining implementation gaps, redundancy consolidation/cleanup or
feature/operational upgrades."* For each recommendation, name **which defect
class it reduces and which it adds**. Removing a bottleneck by adding a seam is
a trade; show the trade.

## Beyond the rulings — expect the most value here

- **Baseline audit.** Re-read the pack's evidence against its own artefacts.
  Name any figure in `# FILE: RULINGS.md` or `# SECTION B-MEASURED` that the
  pack's own artefacts contradict, and any premise the plan never re-checked.
- **What the plan structurally does not see.** It was written by the process
  that produced the code. Name the question it never asks.
- **R1 (TS door duplication).** The pack claims 13 identical constants and 8
  identical functions across two TypeScript files. Verify that from
  `# SECTION G-DOORS`. Then pick (a) remove, (b) fence, or (c) accept, and say
  what would make (a) safe.
- **R2 (parity suite).** Its 2-3 day estimate has never been scoped against the
  real door APIs. Say whether that is credible from the pack, and what the
  cheapest version that still catches the failure class would be.

## Scope

The pack alone must settle the rulings. Outside sources only in the last
section, each with a URL, the date read, and its own marker.

## Output

```
## Rulings
R1 — recommendation / reasoning / gap / confidence
R2 — recommendation / reasoning / gap / confidence
R3 — recommendation / reasoning / gap / confidence

## Ranking
<ordered by cost-of-being-wrong × difficulty-of-undo, with the reason>

## Baseline audit
<figures the pack's artefacts contradict; premises never re-checked>

## What the plan does not see
<the question it never asks>
```

Name a file, a line, or a number where you can. That is worth more than
well-formed caution.
