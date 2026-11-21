# LENSES — the instruments this round must apply

The request named four defect classes plus one slogan. Each lens below must
produce **at least one finding**, and every finding must name **which artefact
and which anchor** it came from. A lens that returns "no finding" must say what
it looked at and why the class is absent — that is a result, not a failure.

## L1 — Bottleneck

Where does the system's throughput or its operator's attention actually stop?

Not "what is slow" — the pack carries no latency figures, and AGENTS §7 says a
number without a load reading is a range at best. Look for **structural**
bottlenecks instead:

- the one step every path must pass through, and what it costs
- a decision that cannot proceed without one person, and what queues behind it
- a resource that is exhausted and cannot be replenished (an attention budget, a
  counted consultation, a metered CI quota)

Name the step, name what queues behind it, and say whether the queue is
growing.

## L2 — Redundancy

Where does the same thing exist twice, and which copy is the authority?

Distinguish three cases and treat them differently:

- **Duplicated mechanism** — the same logic in two places, with a drift cost.
- **Duplicated record** — the same fact in two documents, one of which is stale.
- **Deliberate duplication** — a copy kept on purpose, with a named ceiling and
  an upgrade path (`ponytail:` / `shortcut:` markers in this codebase).

The third is not a finding. If the pack's own text claims a duplication is
deliberate and the evidence agrees, say so and move on.

## L3 — Fragmentation

Where is one capability split across surfaces so that no single caller can reach
all of it?

This codebase's own recurring shape is **one service, N doors** (HTTP, MCP, CLI,
and per-host plugins). A capability reachable from some doors and not others is
fragmentation, and the pack's `INTEGRATION_GAPS.md` records the pattern. Check
whether the fragmentation is:

- **absence** — the door was never built (a gap, but a documented one), or
- **drift** — the door exists and has fallen behind the others (a defect), or
- **deliberate asymmetry** — recorded with a reason (not a finding).

## L4 — Integration gap

Where two things that must agree do not, and nothing checks that they do?

Prefer cases where **both sides are individually correct** and the contradiction
only appears when they are compared. Those are the ones a reader cannot discover
from either artefact alone. `doctor` saying "server up" while `daemon status`
says "not running" is the pack's worked example — find others of that shape, or
say there are none.

## L5 — The slogan, as a per-item test

The request asks to *"address and operationalize the optimal-agentic-experience
effectively."* That is not testable as written. Convert it:

> For **every** recommendation you make, name **which defect class from L1-L4
> it reduces**, and **which it adds**. A recommendation that removes a
> bottleneck by adding a seam is a trade — the operator should see the trade,
> not the slogan.

If a recommendation reduces nothing and adds nothing, it is not a
recommendation; drop it. If it adds a class it does not reduce, say so plainly —
that is the most useful thing a review can produce.

## Output shape per lens

```
L<n> — <name>
  findings: <count>
  <finding>: <artefact path + anchor> — <one sentence> [class: L1|L2|L3|L4]
  adds: <which class this finding's implied fix would add, if any>
```

A lens with zero findings writes `findings: 0` and one line on what it examined.