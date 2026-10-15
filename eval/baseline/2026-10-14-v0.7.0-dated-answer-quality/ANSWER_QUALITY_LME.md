# LongMemEval-S answer quality (memory-wire)

## This number is not a benchmark score

**It is not comparable to any published LoCoMo or LongMemEval figure, and it is not a competitor score.** LongMemEval's published metric is LLM-judged *answer* accuracy over a two-stage protocol: generate an answer from the retrieved evidence, then grade it against the gold answer with an LLM judge. This harness reproduces the shape of that protocol against memory-wire's own shipped retrieval, but the judge here is `small-stack`, reached over an OpenAI-compatible chat endpoint, with the prompt printed below — not the official grader, not the paper's judge model, and not run over the official evidence formatting. A different judge, prompt, model or truncation rule is a different instrument, so the accuracy below belongs to this harness alone.

**It is also not `recall_any@K`.** `eval/RESULTS.md` asks whether the gold session landed in the top K. This asks whether an LLM, handed what `recall` actually returned, produced an answer a grader accepted. The two can diverge in both directions, and the disagreement table below is why this harness exists.

PII redaction is not exercised: the haystack is indexed through `Store::put`, as `examples/longmemeval.rs` does, so this measures the ranking and the answer loop, not the redacted write path.

## Provenance

| field | value |
|---|---|
| commit | `84580d577cb3d0299da37ca9636081b1f3d30a2d` |
| tree state | dirty - 39 uncommitted path(s): AGENTS.md, INSTALL_FOR_AGENTS.md, README.md, TODO.md, docs/CONSISTENCY.md, eval/LOCOMO.md, eval/README.md, eval/RESULTS.md, eval/download.sh, eval/locomo_dev.rs, eval/results_locomo.json, examples/answer_quality.rs, examples/locomo.rs, examples/longmemeval.rs, examples/select_fusion.rs, plugin/skill… (823 chars total) |
| date | 2026-10-14 |
| command | `./target/release/examples/answer_quality --data eval/data/longmemeval_s_cleaned.json --n 500 --seed 42 --out-md /home/ishanp/Documents/github/my-projects/agentic-utility/memory/memory-wire/eval/baseline/2026-10-14-v0.7.0-dated-answer-quality/ANSWER_QUALITY_LME.md --out-json /home/ishanp/Documents/github/my-projects/agentic-utility/memory/memory-wire/eval/baseline/2026-10-14-v0.7.0-dated-answer-qua… (428 chars total)` |
| profile | `--release` |
| host | AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs |
| dataset | `eval/data/longmemeval_s_cleaned.json` |
| slice | 500 questions, seed 42 (LCG shuffle, as `examples/longmemeval.rs`) |
| recall budget | 8000 tokens (`DEFAULT_RECALL_BUDGET` when not passed) |
| answering model | `small-stack` |
| judge model | `small-stack` |
| temperature | 0 |
| answer token cap | 256 |
| api key | set (value not recorded here) |
| loadavg before | 20.91 / 20.08 / 18.31 |
| loadavg after | 3.04 / 3.66 / 6.64 |
| wall clock | 11041.2 s over 2000 LLM calls |

Load average is recorded because `AGENTS.md` §3 makes any timing number conditional on it. Note also what is *not* being timed: these calls are dominated by provider and network latency, so the retrieval the harness also performs is nowhere near the wall clock below.

## Result

| arm | accuracy | correct | scored | unscored | prompt tokens | total tokens |
|---|---|---|---|---|---|---|
| retrieval-conditioned | **18.6%** | 93 | 499 | 1 | 3,738,413 | 3,759,626 |
| closed-book control | **6.0%** | 30 | 500 | 0 | 192,621 | 253,277 |
| **delta (conditioned - closed-book)** | **+12.6pp** | | | | | |

Denominators are **scored** questions, not attempted: a question whose LLM call failed has no verdict and is excluded rather than counted wrong, and `unscored` says how many were excluded. **A cell reading `n/a` means nothing was scored at all** — the run produced no number, and no accuracy below may be quoted from it. A non-zero `unscored` on a cell that does have a percentage makes that percentage incomparable to a clean run: treat the whole artifact as provisional.

## Where recall and answer disagree

The failure this harness exists to catch: the gold session is retrieved, so `R@K` scores full marks, and the answer is still wrong. That is the third line.

| on this slice | count | share of n=500 |
|---|---|---|
| gold session present in the served context | 454 | 90.8% |
| … and the answer was judged correct (recall rewarded) | 89 | 17.8% |
| … and the answer was judged **wrong** (recall not rewarded) | 365 | 73.0% |
| gold session absent, answer judged correct anyway | 5 | 1.0% |

The third line is an answerer problem, not a retrieval problem: the evidence was served and the answer did not use it. The fourth is the closed-book floor, and it bounds how much of the first two lines is real memory use at all. A third line far below the second, together with a delta near zero, would say that the ranking work in `eval/RESULTS.md` is not buying a better answer — which is a result to record, not a bug in this harness.

## Per question type

| type | conditioned | closed-book | delta | gold in context | scored |
|---|---|---|---|---|---|
| knowledge-update | 17.9% | 7.7% | +10.3pp | 77 | 78 |
| multi-session | 9.0% | 9.0% | +0.0pp | 121 | 133 |
| single-session-assistant | 60.7% | 0.0% | +60.7pp | 56 | 56 |
| single-session-preference | 0.0% | 0.0% | +0.0pp | 17 | 30 |
| single-session-user | 31.4% | 8.6% | +22.9pp | 67 | 70 |
| temporal-reasoning | 8.3% | 4.5% | +3.8pp | 116 | 132 |

## The judge prompt, verbatim

What the judge was asked, once per question per arm. Reproduced here so the number above can be audited rather than trusted.

```text
You are grading one candidate answer against the reference answer for a question.

Question:
<question>

<|The Start of Reference Answer|>
<gold answer>
<|The End of Reference Answer|>

<|The Start of Candidate Answer|>
<candidate answer>
<|The End of Candidate Answer|>

The candidate is correct if it conveys the same information as the reference. Ignore wording, formatting, and extra detail that does not contradict the reference. A candidate that omits the key fact, or answers a different question, is incorrect. If the reference says the information is not known or not stated, then a candidate that says so is correct.

Reply with one JSON object and nothing else:
{"correct": true or false, "reason": "one short sentence"}
```

## The answering prompt, verbatim

Identical for both arms. The closed-book control is this same function with an empty context list, which is what makes the two arms differ in exactly one input.

```text
You are answering a question about a person using only the numbered memories below.

Memories:
[memory 0]
[<session date>]
<memory 0, as recall served it>

Question (asked on <question_date>): <question>

Answer with the shortest span that answers the question. If the memories do not contain the answer, reply exactly: I don't know. Do not use outside knowledge.
```

## Errors

2 of 500 questions lost at least one call, listed per question:

```text
bcbe585f: POST http://127.0.0.1:20199/v1/chat/completions failed
bcbe585f: no answer to grade
```

## Reproduce

```bash
export MEMORY_WIRE_LLM_URL='<openai-compatible base url>'
export MEMORY_WIRE_LLM_MODEL='<answering model id>'
export MEMORY_WIRE_LLM_KEY='<bearer token>'
# optional: export MEMORY_WIRE_LLM_JUDGE_MODEL='<judge model id>'
cargo run --release --example answer_quality -- \
  --data eval/data/longmemeval_s_cleaned.json --n 500 --seed 42
```

`--out-md` and `--out-json` write under `$TMPDIR` unless named, so a bare run cannot overwrite a committed `eval/` artifact. Updating one means naming it, the same rule as every other harness in this repo.
