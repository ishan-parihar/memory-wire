# The ask — what this round exists to decide

Round 1 asked "is the plan the right shape?" and answered yes on substance, no
on process. Round 2 asked "where is the shipped architecture, and what should
the next iteration be?" and produced four verified structural findings, two of
which are now closed.

This round asks the question neither of those reached, because both of them
could only be answered from inside the system's own framing:

> **The W0–W7 cycle optimised a subsystem that was already fine.**
>
> Round 1's reviewer said this about the *plan*, and the plan's own author
> agreed. It is now true of the shipped tree: three full cycles (retrieval
> efficacy, BEAM/battery, W0–W7) have gone into **ordering**, and the measured
> position is that ordering is largely solved — the candidate pool contains the
> gold memory in **500 of 500** LongMemEval questions (`AGENTS.md` §1).
> LoCoMo, the **dev** set, is less clean and the pack states it rather than
> quoting only the flattering half: `recall_any@10` is **94.2%** but
> `recall_all@10` is **83.0%**, i.e. questions with several gold sessions still
> miss at k = 10 (`AGENTS.md` §1, `RND_PLAN.md` §1). That gap is an *ordering*
> problem inside the pool, which is the same subsystem again.
>
> Meanwhile the **product** surface — what an agent can actually *do* with the
> memory once it is recalled — has not been re-derived since v0.4.0.

So the decision this round tests is **where the next cycle's effort goes**, and
specifically whether it should keep going where the last three went.

## What the reviewer is asked to weigh

1. **Is the "ordering is solved" reading right, or is it an artefact of the
   instrument?** The 500/500 pool figure and the 94.2% `recall_any@10` are
   measured on LongMemEval-S and LoCoMo. `docs/CONSISTENCY.md` records that
   every LongMemEval number is a **test** consultation, never a selection
   target (§6). If the reviewer thinks the headroom claim is overstated, that is
   the finding.

2. **Given that reading, what is the highest-value next cycle?** Candidates on
   the table, none of them chosen here:
   - the **door-parity** gap (`TODO.md` row 31) — `redact` is reachable only
     from the CLI, so an MCP client that stored a secret cannot retract it
   - the **architecture doc**, which no longer describes the system (D3)
   - the **remaining R&D queue** (`RND_PLAN.md` §2, H-items)
   - something the reviewer can see that this pack cannot

3. **Is there a structural problem none of the four lenses names?** The pack
   carries four lenses (below) because those are the ones the record already
   argues about. A reviewer who sees a fifth is answering the question the round
   was opened for.

## What is NOT being asked

- Whether to add a score threshold or relevance cut to injection. **Rejected
  twice by measurement** (`CLOSED.md`; `CONSISTENCY.md` §32/§48). Do not
  re-propose it; the pack carries the rejection so the reviewer does not spend
  its turn rediscovering it.
- Whether to re-run any benchmark. Every number in this pack is `[LIVE]` and
  traceable to an artefact.
- Whether to match a competitor's feature count. Explicitly not recommended,
  with reasons (`AGENTS.md` §8, `docs/INTEGRATION_GAPS.md` "Explicitly not
  recommended").

## The shape of the answer wanted

A ranked list, each item carrying: **what** the defect or gap is, **where** it
lives (a file or a documented decision), **why it matters to an agent using the
product**, and **what would have to be true** for it to be worth a cycle. An
item the reviewer cannot point at an artefact for should be labelled "not in
evidence" rather than asserted — that label is useful, and it is not a failure.