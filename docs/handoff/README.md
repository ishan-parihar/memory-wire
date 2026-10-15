# Handoff — continuing the memory-performance R&D

Written 2026-10-14 for a fresh agent with no session context. Everything here is
checked against the working tree and committed artifacts on that date; where a
claim could not be checked it says so. **Artifacts win over prose** (AGENTS §7):
if a number here disagrees with an artifact, the artifact is right and this file
is stale.

## What the job is

Raise memory-wire's *memory performance* — retrieval ordering/coverage and, through
it, the judged answer accuracy of a reader — **without** (a) breaking the
single-small-binary position (AGENTS §1), or (b) overfitting the evaluation set
(AGENTS §6). The second is the rule most easily broken by accident and the one this
whole R&D process exists to enforce.

## Read in this order (≈20 min)

| # | file | why |
|---|---|---|
| 1 | `AGENTS.md` §1, §2, §6, §7, §8, §11, §12, §13 | the rules. §6 (three-set discipline) is non-negotiable |
| 2 | `docs/handoff/STATE.md` | what exists, what is uncommitted, every current number and where it lives |
| 3 | `docs/handoff/RND_PLAN.md` | the hypotheses, in order, each with mechanism, test, gate and recording rule |
| 4 | `docs/handoff/RUNBOOK.md` | exact commands, environment traps, how to run the baseline and the LLM instrument |
| 5 | `docs/handoff/CLOSED.md` | levers already measured and rejected — do not re-propose without new evidence |
| 6 | `TODO.md` | the implementation register (rows 7, 9, 12, 14, 17 are the open R&D-adjacent ones) |

Deeper references, only when a hypothesis points there: `docs/EVALUATION_HYGIENE.md`,
`docs/BENCHMARK_LANDSCAPE.md`, `docs/RERANKING_PLAN.md`, `docs/PERFORMANCE_PLAN.md`,
`docs/EXCEED_PLAN.md`, `docs/NEXT_ITERATION.md`, `docs/CONSISTENCY.md` (append-only;
§34–§35 are the latest).

## The five rules that bite

1. **LoCoMo and the relevance battery choose. LongMemEval-S only measures.** A change
   picked by looking at a LongMemEval number is *fitted*, not earned (AGENTS §6). Any
   harness that runs LongMemEval (OAMB, MemBase, memorybench, AMB `longmemeval`,
   `answer_quality`) *is* a test consultation: count it in `docs/CONSISTENCY.md`.
2. **Decide by mechanism, then measure once.** Write the mechanism argument *before*
   the number. Never iterate until a metric improves. A null result is a result;
   record it.
3. **Do not commit, tag, push or release** unless the user says so (AGENTS §11). The
   tree is deliberately uncommitted at handoff; leave the decision to the user.
4. **Do not edit the aroute repo** (`agentic-utility/proxy/artificial-route`) or restart
   its service; do not look at its credentials. Its bugs are handed off in
   `docs/AROUTE_BUGS.md`. Never send real API keys to harnesses: `env -u GOOGLE_API_KEY`.
5. **Delegate only with the `space-bunny` model** (AGENTS §11), and never cite a
   subagent's summary as a measurement — run the gate yourself (AGENTS §12).

## Decisions that belong to the user, not to you

Stop and ask (in your final report, not mid-run) before any of these:

- turning the dense-vector arm on by default (costs binary size and a model download;
  `docs/EXCEED_PLAN.md` Phase D);
- a paid/frontier reader or judge for any LLM run (the instrument is free-tier
  `small-stack`);
- committing, tagging, pushing, publishing;
- changing a shipped default in `FusionWeights::SHIPPED` (even with dev-set evidence:
  report it, mark it `provisional:` if it was ever LongMemEval-informed, and let the
  user decide).

## Definition of done for a hypothesis

A hypothesis is closed when (1) its mechanism argument is written, (2) the dev-set
measurement exists as a committed artifact under `eval/`, (3) `docs/CONSISTENCY.md`
has an appended section with the numbers, the n, the loadavg, and the
test-consultation count, (4) the gate in `AGENTS.md` §2 passes, and (5) `TODO.md` and
`docs/handoff/RND_PLAN.md` status lines are updated. "It looked better" is not done.
