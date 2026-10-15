# Baselines — the reference point every R&D change is measured against

A baseline is one run of `eval/baseline/run_baseline.sh` into a dated directory:
every harness in this repo, each artifact unedited, plus `PROVENANCE.md` (commit,
dirty paths, machine, loadavg before/after every step, exit codes). An R&D change
is judged by re-running the script into a new `TAG` and comparing directory to
directory — never by comparing against a number remembered from prose.

```bash
eval/baseline/run_baseline.sh                    # all three phases
eval/baseline/run_baseline.sh retrieval perf     # the free, deterministic part
TAG=2026-11-01-exp-foo eval/baseline/run_baseline.sh retrieval
TAG=<existing> eval/baseline/run_baseline.sh amb_rag   # one LLM step alone (`llm` = answer_quality + amb_rag)
```

`AGENTS.md` §6 still governs what a comparison may be used for: LoCoMo and the
relevance battery choose; LongMemEval-S (in `retrieval` and `llm`) is one test
consultation per run and is never the reason a change is kept;
PrecisionMemBench and the AMB runs are measure-only.

## What the battery contains, and what it deliberately leaves out

| phase | step | corpus / role | artifact |
|---|---|---|---|
| retrieval | `longmemeval` | LongMemEval-S, 500 q — **test** | `LONGMEMEVAL.md`, `longmemeval.json` |
| retrieval | `locomo` | LoCoMo, 1,531 q — **dev** | `LOCOMO.md`, `locomo.json` |
| retrieval | `coding_life`, `scale_sweep`, `recall_curve` | synthetic | `CODING_LIFE.md`, `SCALE_SWEEP.md`, `BENCH_RECALL_CURVE.md` |
| retrieval | `battery` | live bank `omp`, 10 labelled cases — **dev** (auto-injection) | `BATTERY.txt` |
| retrieval | `precisionmembench` | PMB via AMB, 77 cases — measure-only | `precisionmembench.json.gz` |
| perf | footprint, write, concurrency, coldstart, soak | synthetic | `BENCH_*.md`, `SOAK.md` |
| llm | `answer_quality` | LongMemEval-S, judged — **test** | `ANSWER_QUALITY_LME.md`, `answer_quality_lme.json` |
| llm | `amb_locomo_rag` | LoCoMo via AMB `rag`, judged — measure-only | `amb_locomo_rag.json.gz` |
| llm | (every call) | which model served it, retries, latency | `llm-calls.jsonl` |

**Left out on purpose, with the measurement that excludes each** (2026-10-13,
`docs/BENCHMARK_LANDSCAPE.md` §3). AMB `rag` hands the answerer the provider's
top 10 documents, and `eval/amb/memory_wire.py` stores each AMB document whole:

| corpus | docs/bank | top-10 tokens (median) | share of bank | why not in the baseline |
|---|---|---|---|---|
| LoCoMo | 29 | 15,870 | 34% | **included** — a real selection |
| BEAM 100k | 9 | 145,950 | 100% | top-10 *is* the whole history: a long-context test, not a memory test |
| PersonaMem 32k | 5 | 36,922 | 100% | same |
| BEAM 500k / 1m | 27 / 50 | ~248,000 | 37% / 20% | ~250k tokens of context per question on a free small model |
| PersonaMem 128k | 20 | 108,738 | 50% | ~109k tokens per question |
| LifeBench | 365 | 67,543 | 3% | meaningful selection, but ~135M input tokens for 2,003 q on a free tier |

They become measurable once the provider writes at the granularity the
product's own write path uses (the hooks retain turn pairs, not 25k-token
sessions) — `TODO.md`. Until then a number from them would describe the reader's
context window, not memory-wire.

## The LLM instrument, and why its numbers need their caveats

The `llm` phase routes through aroute's `small-stack` combo (free models:
`opencode-zen/space-bunny-free`, `poolside/laguna-s-2.1:free`,
`dots-studio/dots-3-note-preview:free` across cline / kilocode / openrouter, in
priority order) via `eval/llm_shim.py`. Measured while building it (2026-10-13,
aroute 0.1.12):

- **A combo is a fallback chain**, so the answering model is not fixed. aroute
  only routes combo names (a `provider/model` id is refused with
  `model_not_found`), so the model cannot be pinned; `llm-calls.jsonl` records the
  model that served each call, and a comparison between two baselines is only
  clean where the served-model mix matches.
- **aroute returned HTTP 200 with an `{"error": …502…}` body** (logged
  `outcome=ok`, no fallback) and **forwards every cline reply wrapped as
  `{"success": …, "data": {...}}`**. The shim retries the first and unwraps the
  second; both are aroute bugs, handed off in `docs/AROUTE_BUGS.md` (with two more:
  cline token headers read 0, and `opencode-zen` 403s on every call with no
  cooldown, so `space-bunny-free` served none of this baseline).
- **The free models ignore `response_format: json_schema`** and AMB's client sends
  no temperature; the shim states the schema in a system message and defaults
  `temperature` to 0.
- **A free tier has bad hours.** On 2026-10-13, 21:00–22:10, every provider hit
  transport failures. One shim call retried for about 20 minutes, AMB's OpenAI client
  timed out at 600 s, and AMB does not retry a timeout, so the whole run died after 384
  of 1,540 queries. The shim now stops at `--deadline 480` and returns a 502, which AMB
  does retry. `amb_rag` resumes from AMB's per-conversation saves
  (`target/amb-out/$TAG`, seeded from the committed `.json.gz`), so a dead step
  costs a restart, not the hours already spent.
- **The reader, not retrieval, dominates the judged number.** On the first 6
  LongMemEval questions (seed 42) memory-wire ranked a gold session **first in all
  6** and it was in the context every time; the small reader still answered
  `I don't know` on 4 of the 5 answerable ones. Read `answer_quality` beside
  `gold_in_context` / `first_gold_rank` in its JSON, never alone — and never
  beside a published accuracy produced with a frontier reader or the official
  judge.

## Baselines

### `2026-10-13-v0.7.0` — memory-wire 0.7.0 @ `84580d5` (+ the uncommitted harness edits listed in its PROVENANCE)

Load average 11–22 on 24 cores throughout (other agents' builds and the LLM
phase shared the box): **retrieval metrics are load-independent; every latency,
throughput and RSS figure is a range-quality reading** (AGENTS §7).

Retrieval — reproduced the committed artifacts value-for-value:

| | R@1 | R@5 | R@10 | NDCG@10 | MRR | n |
|---|---|---|---|---|---|---|
| LongMemEval-S, all | 83.8% | 97.2% | 98.6% | 88.2% | 89.2% | 500 |
| LongMemEval-S, abstention excluded (upstream protocol) — **new** | 84.0% | 97.2% | 98.5% | 88.5% | 89.3% | 470 |
| LoCoMo (dev), shipped `overlap 0.25` | 60.4% | 86.0% | 94.2% | 73.6% | 71.8% | 1531 |

| | result |
|---|---|
| PrecisionMemBench | 14/77 total, 3/43 active, precision 0.17, recall 0.95 |
| relevance battery (auto shape) | gold 5/6, junk-injected **0/9** (was 1/9 on 2026-10-08 — the live bank moved, the code did not; a live-bank number is not a pure code comparison) |
| coding_life (shipped) | P@k 22.7%, R@k 96.7%, hit 100.0%, n 15 |
| recall curve, 10k memories (shipped) | R@1 75.0%, R@5 84.4% |

Footprint and speed (range-quality, see load above):

| | value |
|---|---|
| release binary | 10,745,080 B (10.25 MiB), `DT_NEEDED` 3 |
| serve idle RSS | 9.88 MB |
| cold start to `/health` | 13.2–15.5 ms |
| retain (service), 10k | 2,018 ops/s, p50 288 µs |
| recall p50, 10k memories | 7.2 ms (`SCALE_SWEEP.md`) |
| soak recall p50 / p99 | 774 / 2,158 µs |
| concurrency | **gate FAILED** at 2 clients: 1.30× vs 1.50× floor — absolute ops/s *rose* against `eval/BENCH_CONCURRENCY.md` (2 clients 758.5 vs 708.0; 1 client 583.7 vs 408.7); the ratio is the noisy part at load 17→22. 64-client gate passed (1.85×). Re-measure on a quiet box before reading it as a regression |

LLM-judged, reader and judge both `small-stack` at temperature 0. 2,000 calls, all
served by `poolside/laguna-s-2.1:free` (1,931 via cline, 69 via kilocode), 0 retries,
p50 2.9 s per call. **Read both rows as a floor set by a small reader, not as memory
quality.**

| | conditioned | closed-book | delta | n |
|---|---|---|---|---|
| `answer_quality`, LongMemEval-S (test) | **17.0%** (85) | 6.0% (30) | +11.0pp | 500 |

What the 17.0% is made of (`answer_quality_lme.json`):

- **Retrieval is not what fails.** A gold session was in the served context for 454
  of 500, ranked first in 419 of those 454.
- **The reader refuses.** 374 of the 454 were judged wrong, and 368 of those 374
  answers are `I don't know`. The 30 abstention questions all scored correct, by
  refusing.
- **More served sessions make the reader refuse.** Measured directly on 10
  single-session-user refusals whose gold answer is a literal span of the gold
  session: the gold session **alone** was answered 6/10; gold + 2 other sessions,
  1/10. A needle probe rules out truncation (a planted fact was found at every
  position up to 7.8k prompt tokens, and token counts match end to end). That is
  `TODO.md` row 17.
- **The harness omits session dates** (`TODO.md` row 16), which caps
  temporal-reasoning (5.3%, n 133) and knowledge-update (14.1%, n 78) by
  construction.

| AMB LoCoMo `rag` (measure-only) | **84.1%** (1,295) | — | — | 1,540 |

`amb_locomo_rag.json.gz` (AMB's own output, unedited). The reader is served the provider's
top 10 sessions, median 14,313 context tokens. Same `small-stack` reader and judge as above,
so the two rows share an instrument but not a corpus: **do not read 84.1% against 17.0%** as
"LoCoMo is easier memory"; LoCoMo banks hold 19–32 sessions, so the top 10 is a third of the bank.

| `meta.category` | correct | n |
|---|---|---|
| open-domain | 92.7% (780) | 841 |
| temporal | 81.9% (263) | 321 |
| multi-hop | 68.8% (66) | 96 |
| single-hop | 66.0% (186) | 282 |

- **Accuracy follows whether *all* gold sessions were served.** All gold in the reader's
  top 10: 91.2% (1,176/1,289). Some: 53.0% (88/166). None: 36.5% (31/85).
- **The LME failure shape is absent here.** 12 of the 245 wrong answers are refusals
  (LME: 368 of 374). LoCoMo's misses are multi-session coverage, not a reader drowning in
  context: `recall_all@10` is 83.0% against `recall_any@10` 94.2% (`eval/LOCOMO.md`).
- Run in 4 segments across 3 interruptions (a 20-min network outage, two host resets at
  02:09 and 03:34 on 2026-10-14), resumed from AMB's per-conversation saves each time;
  the four logs are kept beside the artifact. 3,572 AMB-era LLM calls: 3,365 first try,
  207 needed more than one attempt (118 two, 89 three or more), 22 of them ending as a shim 502 that AMB then retried;
  2,050 of them were cline-enveloped replies the shim unwrapped (`docs/AROUTE_BUGS.md` B2).
  Served model: `poolside/laguna-s-2.1:free` for 3,542, `dots-3-note-preview:free` for 8.
- Every row was produced by the adapter that held session dates client-side (the header
  prefix); the product-side `created_at` round-trip landed afterwards (TODO row 15), so a
  re-run with it is a new instrument version, not a replay.
