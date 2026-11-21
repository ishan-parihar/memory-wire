# Reviewer prompt — plan review, 2026-11-21

You are reviewing a plan for **memory-wire**, a Rust agent-memory service
(SQLite + FTS5, HTTP and MCP, hooks that capture conversation prose). The pack
is all you know. You have no session history and no repository access.

**Treat every claim in the pack as a CLAIM TO AUDIT, not ground truth.** Where
the plan asserts a number or a state, check it against the artefact the pack
carries. If they disagree, the artefact wins — say so.

**If the pack cannot settle something, answer "not in evidence — here is what
would settle it."** That is complete. A confident recommendation the pack cannot
support is a defect in your reply.

## Subject

memory-wire at `9a61762`. Not Hindsight, not agentmemory, not aroute. If your
reply is about something the state section does not name, say so and stop.

## The job

The plan (`# SECTION B-ASK`) is a plan for execution, not a proposal. It has
three tiers: build now, decide later, blocked on the operator. **Your job is to
attack it before it is executed**, not to summarise it.

Answer these, in this order:

1. **Is the top item the right top item?** The plan claims the highest-value
   remaining work is a per-host write-then-read smoke test, because nothing
   verifies a memory reaches a model's prompt on a real host. It cites TODO 35
   (a door that shipped dead) as the evidence. Is that reasoning sound, and is
   the claim that no such check exists correct on the pack's evidence?

2. **Which item is mis-ranked, and on what evidence?** The plan orders Tier 1 as
   count-gate, smoke test, doc change, parity rescoping. Say what you would
   reorder and why. A ranking argued rather than measured is the thing to attack.

3. **What will this plan fail to catch?** Each item names what it does not
   cover. Name what *none* of them covers.

4. **Is Tier 2 a real question or a way of not deciding?** The plan records two
   structural questions (why rendering policy lives in the doors; whether the
   opencode door still loads) without answering them. Say whether either should
   be decided now, and on what evidence.

5. **What is the cheapest thing on this list that is worth nothing?** Say which
   item you would cut, and what makes it not worth the cost.

## The request behind this round

*"Proceed with the full scale execution of the implementation plan"* — for *"the
remaining implementation gaps, redundancy consolidation/cleanup or
feature/operational upgrades."*

For each judgement, name **which defect class it reduces and which it adds**.

## Specific things to check

- The plan says `grep` across `tests/` for `before_agent_start`, `ev.system`,
  `inject` returns nothing, and that none of the 22 integration tests drives a
  host. Verify that reading against `# SECTION D-EVIDENCE`.
- The plan says the four doors total 4,037 lines and that a check exists in N-1
  of them. Check the arithmetic.
- The plan says release is measured (10.6515 MiB, `DT_NEEDED`=3, +3.92% over
  v0.7.0) and deliberately not cut, and that the operator's ruling is "release
  after development is final". Say whether deferring is right given what the
  pack carries.
- The plan corrected a host count (AGENTS said "Six", `Host::ALL` has eight).
  Check whether other counts in the pack are similarly stale.

## Scope

The pack alone must settle the plan. Outside sources only in a final section,
each with a URL, the date read, and its own marker.

## Output

```
## Verdicts
1. Top item right? — yes/no, with the evidence
2. Mis-ranked — which item, what order, why
3. What none of it catches
4. Tier 2 — decide now or defer, per question
5. Cut — which item, and why it is not worth it

## Ranking
<the plan's items, re-ordered if you disagree, by cost-of-being-wrong ×
difficulty-of-undo>

## Audit
<figures in the plan the pack's own artefacts contradict; stale counts;
premises never re-checked>
```

Name a file, a line, or a number where you can. That is worth more than
well-formed caution.
