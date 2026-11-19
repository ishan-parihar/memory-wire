# Reviewer prompt — round 4

You are reviewing a Rust agent-memory service called `memory-wire`. You have no
prior context on it. Everything you know is in the pack.

**Treat the pack's claims as CLAIMS TO AUDIT, not as ground truth.** Where you
cannot find support in the pack, say **not in evidence** rather than accepting
the claim or inventing support. Where two numbers in the pack disagree, name
which one is wrong.

## What the system is

A single-binary memory service: a bank-isolated SQLite store with FTS5, exposed
over HTTP and MCP, plus hooks that capture conversation prose into it. Four
"doors" reach the same core service: the Rust hooks, an MCP server, the HTTP
API, and plugin adapters for agent hosts (two TypeScript, one Python).

## The question

Five times, the same defect shape appeared in this repo:

1. A security policy (withhold `untrusted_external` rows from auto-injection)
   existed in 2 of 4 doors.
2. A measurement script counted `src/` and stopped, so 1,622 lines of
   injection-path code were never in any surface number.
3. A register row said a feature was open a month after it shipped.
4. A gaps document said a capability was missing after it was implemented.
5. A benchmark (PersonaMem v2) ran to completion on 2026-10-17 and its score
   (37.8%) was never computed or recorded.

Each was found by an external review, not by the system's own checks. Items 1–4
are now fixed. Item 5 is not.

The pack's `C-ASK` frames the decision as a choice between two readings of the
same fact — that the tree is 35,298 lines across 32 modules, of which 6,609 are
per-host `connect_*` adapters:

- **(i)** the per-host family is the right shape, and the duplication is the
  honest cost of being correct for each host's different config format;
- **(ii)** the per-host family is where the N−1 pattern recurs next, because
  seven files that each answer "what does this host need" are seven places a
  capability can be added to six and forgotten in one.

**Decide which, on the pack's evidence, and say what would change your mind.**
If the answer is (ii), propose the *smallest* structural change that would make
the pattern detectable rather than discoverable. The pack contains one worked
example of such a check: `trust_gate_test.sh` enumerates the doors and fails if
one lacks the filter.

## What to look for, in priority order

1. **Bottlenecks.** What actually limits an agent using this product? Name the
   artefact. `G-OPEN` lists five open work items with their blockers; if a
   different item is the real bottleneck, say so and why the register is wrong.
2. **Redundancy and fragmentation.** The measured surface is in `B-DELTA` §2
   and `measure_surface.sh`. Is the 98-of-741-line duplication between the two
   TypeScript plugins (i) or (ii)? Is 6,609 lines of `connect_*` justified?
3. **Integration gaps.** `F-LAW` states the interface rules. Does the code the
   pack shows honour them? The pack includes the hermes provider's injection
   section and one TypeScript plugin's whole injection half.
4. **The optimal-agentic-experience question.** An agent meets this system
   through 13 CLI subcommands, 9 HTTP routes, 5 MCP tools, 6 hosts, and 4
   plugin doors that are documented as asymmetric. What would an agent hit
   first, and where would it get stuck?

## Constraints you must respect when proposing anything

- **No new ranking lever.** The rejected-lever list (`H-DECIDED`) is complete;
  proposing one without new evidence is out of scope.
- **No score threshold on injection.** Closed twice by measurement.
- **The cheap-option position is load-bearing.** One static binary, ~10.7 MiB
  RSS, 3 shared libraries, 17 dependencies. A proposal that grows the default
  build materially is a proposal against the product's reason to exist.
- **No benchmark run can be proposed as a fix.** The blockers are real and
  external (`G-OPEN`).

## What I already know and do not need re-derived

- The four batches since round 3 are done and verified (`B-DELTA` §1).
- The retrieval ranking is measured and settled.
- PersonaMem v2 at 37.8% is a single-harness reading with no comparator.

## The output I want

A ranked list. For each finding: **the artefact it comes from**, **what it costs
an agent using the product**, **what would have to be true for it to be worth
doing**, and **which of the two readings in `C-ASK` it supports**. Be concrete
about file and section. If a finding depends on a number, give the number and
its population. If you think the framing of the question itself is wrong, say
that first — a rejected framing is worth more to me than a confident answer to
the wrong question.