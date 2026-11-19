# C-ASK — the decision this round exists to test

Round 1 asked "is the plan the right shape?" — yes on substance, no on process.
Round 2 asked "where is the shipped architecture, and what is the next
iteration?" — four structural findings, two now closed.
Round 3 asked "should the next cycle keep optimising the subsystem the last three
optimised?" — no, and it named the enforcement asymmetry between doors.

That asymmetry is now closed in all four doors (`B-DELTA` §1). So the question
this round puts to a reviewer who has never seen this repo is the next one out:

> **Five times in one repo, the same defect shape appeared: a claim or a policy
> that exists in N places, is checked in N−1, and is recorded in N−2. Round 3
> found it in code. Fixing it found it in docs, in a measurement script, in the
> register, and in a benchmark that ran and was never scored.**
>
> That is not five bugs. It is one property of how this system is built, and it
> is the thing that would most affect an agent trying to use it.

## The specific tension to weigh

The system's stated competitive position is that it is **the cheap option**
(`AGENTS.md` §1): one static binary, ~10.7 MiB RSS, 3 shared libraries, 17
dependencies, no daemon required. Every structural change has to be paid for
against that. The tree is now **35,298 lines across 32 modules** against a
document that says 24.8k and 22 — and **6,609 of those lines are `connect_*`**
adapters, seven files, each pairing a host with a surface.

So there are two readings of the same fact, and the pack does not pick one:

- **(i) The per-host adapter family is the right shape.** Each host's config
  format is genuinely different (JSON, JSONC, TOML, a plugin directory, a
  provider slot), each has its own ownership and refusal rules, and a shared
  abstraction would have to model all of them to save the duplication. The
  duplication is the cost of being correct per host.
- **(ii) The per-host family is where the N−1 pattern will recur next.** Seven
  files that each answer "what does this host need" are seven places a new
  capability can be added to six and forgotten in one. That is exactly what
  happened to the trust gate across four doors.

Both are consistent with the evidence in this pack. **Choosing between them is
the decision.**

## What the reviewer is asked to weigh

1. **Which reading is right, and on what evidence in the pack** — not on
   intuition about either.
2. **If (ii), what the smallest structural change is that would make the N−1
   pattern detectable rather than discoverable.** A shared abstraction is one
   answer and probably the wrong one; a *check* that enumerates the places a
   capability must appear is another. The pack contains one worked example of
   the latter (`trust_gate_test.sh` enumerates three doors and fails if one
   lacks the filter).
3. **Whether the 98-of-741-line duplication between the two TypeScript plugins
   is (i) or (ii) in miniature.** They are the same file with two host
   adapters; 59% is literally identical.
4. **What an agent using this product would hit first**, given the surface:
   13 CLI subcommands, 9 HTTP routes, 5 MCP tools, 6 agent hosts, 4 plugin
   doors, and a documented asymmetry between them.

## What this round is NOT asking

- **Not** whether the retrieval ranking is good. Ordering is measured and
  settled (`AGENTS.md` §1: gold in pool 500/500; `CLOSED.md` is the complete
  rejected-lever list). Do not re-litigate it; `H-DECIDED` carries the record.
- **Not** whether to add a score threshold or relevance cut to injection.
  Rejected twice by measurement (`CONSISTENCY.md` §32/§48).
- **Not** whether to run more benchmarks. The blockers are real and stated in
  `G-OPEN`; a reviewer cannot resolve them.
- **Not** a re-derivation of the four batches. They are done and verified; the
  question is what they imply about the *next* structural change.
- **Not** whether the PersonaMem v2 number (37.8%) is good. It is a reading
  from one harness with no second system to compare against, which the pack
  states.

## The form of the answer wanted

For each finding: **the artefact it comes from**, **why it matters to an agent
using this product**, and **what would have to be true** for it to be worth
doing. A finding with no artefact should be labelled *not in evidence* rather
than asserted. Where the pack's own numbers disagree with each other, say which
one is wrong rather than reconciling them.