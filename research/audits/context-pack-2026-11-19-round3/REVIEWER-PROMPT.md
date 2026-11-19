# Reviewer prompt — memory-wire, round 3

You are reviewing a **shipped** system, not a proposal. Its author has spent
three full work cycles on one subsystem and now suspects that subsystem was
already fine. You have no session history and you are not continuing anyone's
work.

## What the subject is

`memory-wire` is a single-binary agent-memory service: a bank-isolated SQLite
store with FTS5 full-text search, exposed over HTTP, MCP (stdio and HTTP), and a
CLI, with hooks that capture conversation prose into it. Its competitive
position is deliberate: **one static binary, ~10.6 MiB, three shared libraries,
no daemon required**, against competitors that ship a Python/Node runtime plus
Postgres and 0.8–1.0 GB of idle floor.

The pack states its own case in `A-STATE`. Treat it as a claim.

## The question this round exists to answer

The author's position, which you should test rather than accept:

> Three cycles (retrieval efficacy, a BEAM/battery dev-set cycle, and the W0–W7
> upgrade) went into **ordering** — which memory to surface, and in what order.
> The measured result is that ordering is largely solved: the candidate pool
> contains the gold memory in 500/500 LongMemEval questions, and LoCoMo
> `recall_any@10` is 94.2%. Even the remaining gap (`recall_all@10` 83.0% —
> questions with several gold sessions) is an ordering problem *inside* the pool,
> not a coverage problem.
>
> Meanwhile the **product** surface — what an agent can actually *do* with a
> memory once recalled — has not been re-derived since v0.4.0. `B-DELTA` D3
> shows the architecture document still describes a system with **five
> modules** and **three doors** when the tree has nine modules and four doors.

**So: is that reading right, and where should the next cycle go?**

## How to answer

**Treat the pack's claims as CLAIMS TO AUDIT, not as ground truth.** If a
figure, a count or a causal story does not hold up against the artefacts quoted,
say so and name the artefact.

**Use "not in evidence" when that is the truth.** If you cannot settle something
from the pack, label it `not in evidence` and say what would settle it. That is a
useful answer, not a failed one. Do not fill a gap with a plausible value.

**Every figure has three attributes, and you should check all three:** its
**value**, its **population** (what set it counts), and its **selection
constraint** (how the set was chosen). A right value on the wrong basis is a real
defect, and it is the kind this pack would rather you found than not. The pack
marks its evidence inline: `[LIVE]` (measured on this tree), `[CERT]` (a dated
committed artefact), `[IN-SAMPLE]`, `[PRIOR]`, `[UNCONFIRMED]`.

**Do not spend the turn on these, which are already settled by measurement and
recorded in the pack:**
- score thresholds or relevance cuts on auto-injection (`C-LAW`, `D-EVIDENCE`)
- re-running any benchmark again
- matching a competitor's tool or feature count

## What I want back

A **ranked** list. For each item:

1. **What** the defect, gap or redundancy is.
2. **Where** it lives — a file, a route, a documented decision. If you cannot
   point at one, mark it `not in evidence`.
3. **Why it matters to an agent using this product** — concretely: what does an
   agent fail to do, or do expensively, because of it?
4. **What would have to be true** for it to be worth a work cycle.

Rank by value per unit of work, and say what you are trading off. If your top
item is one the pack already lists (`TODO.md` row 31, the door-parity gap), say
so and argue for or against it — do not just restate it.

## Four specific things worth your attention

The pack's `F-ANALYSIS` and `G-LENS` sections are the artefacts, not a
curriculum. But these are the places where I am least sure:

1. **Is "ordering is solved" an artefact of the instrument?** Both the 500/500
   and 94.2% figures come from benchmarks the author chose and has re-run for
   months. `D-EVIDENCE` carries the caveats. Is the headroom claim overstated?

2. **`recall_all@10` 83.0% vs `recall_any@10` 94.2%.** The pack says the gap is
   an ordering problem. Is it? Or is it telling you something about how sessions
   are written that no amount of ordering fixes?

3. **The door asymmetry** (`F-ANALYSIS`, the 2026-11-19 addendum). `redact` —
   retroactive secret removal — has **no MCP tool and no HTTP route**. An agent
   that retained a credential through MCP cannot retract it through MCP. Is that
   the highest-value gap in the system, or is it worse or better than it looks?

4. **What the pack structurally cannot show you.** It is authored by the same
   person who wrote the system. What class of problem would that blind it to?

## Length and format

As long as the answer needs. Structure and arithmetic are judged; prose is not.
A short, precise report that names four real defects beats a long one that
restates the pack.