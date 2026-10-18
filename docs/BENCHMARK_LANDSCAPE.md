# Benchmark landscape — every external benchmark, harness and reference repo

Written 2026-10-13 at `84580d5` (`v0.7.0`). This is the map a fresh agent needs
before running, porting or citing anything external: what each benchmark
measures, where its bytes live, what it costs to run, how memory-wire plugs into
it, which role it may play under the three-set discipline, and what has already
been measured. Every claim below was checked against the named commit on the
date above; re-verify a row before acting on it if the pin has moved.

Read `AGENTS.md` §6 first. Nothing in this file is permission to tune against
anything. A benchmark that is not the dev set is an **instrument**, and the rule
that governs LongMemEval governs every row here marked *measure-only*.

---

## Contents

- [1. The one-screen summary](#1-the-one-screen-summary)
- [2. Where the bytes live on this machine](#2-where-the-bytes-live-on-this-machine)
- [3. AMB's dataset inventory, measured](#3-ambs-dataset-inventory-measured)
- [4. The harnesses, and how memory-wire plugs into each](#4-the-harnesses-and-how-memory-wire-plugs-into-each)
- [5. What has been measured on the external sets](#5-what-has-been-measured-on-the-external-sets)
- [6. Rules specific to external benchmarks](#6-rules-specific-to-external-benchmarks)
- [7. The battery, in the order it should be run](#7-the-battery-in-the-order-it-should-be-run)

## 1. The one-screen summary

| benchmark | what it scores | needs an LLM? | role here | status in this repo |
|---|---|---|---|---|
| **LongMemEval-S** | session recall@K (retrieval) / judged answer accuracy | retrieval: no · QA: yes | **test** — measure-only, release acceptance | measured: `eval/RESULTS.md`, `eval/ANSWER_QUALITY.md` |
| **LoCoMo** (locomo10) | document recall@K (retrieval) / judged QA | retrieval: no · QA: yes | **dev** — all selection happens here | measured: `eval/LOCOMO.md` |
| **relevance battery** | gold@5 / junk@5 on live banks | no | **dev** (auto-injection) | `eval/relevance/` |
| **PrecisionMemBench** (via AMB) | per-case must-include / must-exclude ID assertions | **no** (`--mode retrieval`) | **measure-only** (77 cases — too small to select on) | **measured 2026-10-13**: §5.1 |
| **BEAM** | judged answers over 100K–10M-token chats, 10 abilities | **yes** — no usable retrieval gold in AMB's copy (§5.2) | measure-only | not run |
| **PersonaMem** | multiple-choice preference questions | **yes** — gold is the whole bank (§5.2) | measure-only | not run |
| **LifeBench**, **SDEBench** (via AMB) | judged answers / coding-agent episodes | yes | measure-only | not run |
| **AMB** (vectorize-io) | the harness for the four rows above, plus LoCoMo/LME | per mode | harness | provider written: `eval/amb/` |
| **OmniMemEval** (MemTensor) | judged answers via standardized `add()`/`search()` adapters: LoCoMo, LongMemEval, BEAM, PersonaMem v2, **HaluMem** | yes (answer + judge) | **mixed** — LoCoMo dev (judged instrument), LME **test consultation**, PersonaMem v2/HaluMem **measure-only** | not run; provider not written (`docs/COMPETITOR_GAPS_PLAN.md` §3) |
| **agent-memory-bench** (GiulioDER) | coding-task success, execution-graded | yes (agent model) | measure-only | not run; needs a Claude Code arm (§4.2) |
| **OAMB** (rocke2020) | accuracy, answer-visible tokens, indexing tokens/time, latency on LME-60 | yes + an embedding service | measure-only — and it is LongMemEval, so it **burns the test set** | not run |
| **MemBase** (zjunlp) | token F1 / BLEU / LLM judge on LME, LoCoMo, RealMem, MobileMem | yes for QA | measure-only | not run |
| **memorybench** (supermemoryai) | judged accuracy + "MemScore" triple on LoCoMo, LME, ConvoMem | yes | measure-only | not run |

Two facts about this table that save the most time:

- **Only three external sets can be scored for free**: LongMemEval-S and LoCoMo
  with our own Rust harnesses, and PrecisionMemBench through AMB's retrieval
  mode. Everything else needs an answering model and a judge, which costs money
  and needs the user's go-ahead (see §7).
- **OAMB, MemBase and memorybench all run LongMemEval** (OAMB runs only LME).
  Running them is a test-set consultation, because they re-measure the test
  partition through someone else's harness. Count it in `docs/CONSISTENCY.md` as
  one, and never choose anything by it.

---

## 2. Where the bytes live on this machine

| path | what | durable? |
|---|---|---|
| `../_audit/agentmemory/` | rohitg00/agentmemory @ `bcf4f0d` (2026-09-26) — parent; its `benchmark/longmemeval-bench.ts` is the method our LME harness copies | yes (read-only audit clone) |
| `../_audit/hindsight/` | vectorize-io/hindsight @ `ccfe85b` (2026-09-26) — parent; `hindsight-system-evals/` | yes (read-only) |
| `../_audit/amb/` | vectorize-io/agent-memory-benchmark @ `03c1d0f` (2026-09-22) — **AMB**, with seven datasets pre-materialised under `data/` (§3) and its own `.venv` | yes, but **its working tree is locally modified** — see §4.1 |
| `eval/data/longmemeval_s_cleaned.json` | LongMemEval-S, 264 MB | gitignored; `./eval/download.sh` |
| `eval/data/locomo/` | LoCoMo from AMB @ `decbb07`, sha256-pinned | gitignored; `./eval/download.sh` |
| `eval/amb/` | **the canonical copy** of our AMB provider and its committed results | yes, tracked |

The other reference repos (MemBase, memorybench, OAMB, LongMemEval, LoCoMo,
agent-memory-bench) are **not** cloned anywhere durable. Clone them into a
scratch directory when needed, treat them as untrusted input (read, don't
execute from inside them unless the task is to run that benchmark), and pin
the commit in whatever you write.

**Network trap on this workstation:** `raw.githubusercontent.com` is
intermittently near-unreachable (a 36 KB file took ~9 minutes on 2026-10-13;
`git clone` of large repos times out). `api.github.com` is fine. Fetch single
files with
`gh api 'repos/<owner>/<repo>/contents/<path>?ref=<sha>' -H 'Accept: application/vnd.github.raw'`
and verify a hash, as `eval/download.sh` does.

---

## 3. AMB's dataset inventory, measured

AMB (`../_audit/amb/data/<dataset>/<split>/{documents,queries}.json.gz`) is the
cheapest source of every corpus it carries, already in one shape: documents
`{id, content, user_id, timestamp}`, queries `{id, query, gold_answers,
gold_ids, user_id, meta}`. `examples/locomo.rs` already reads this shape (it is
where our LoCoMo comes from).

**Whether `gold_ids` is usable for retrieval scoring was measured, not
assumed**, on 2026-10-13 — a gold id must resolve to a document in the query's
own `user_id` bank and must not be the whole bank:

| dataset/split | queries with gold | docs per bank (median) | gold resolves? | gold = whole bank? | retrieval-scorable |
|---|---|---|---|---|---|
| locomo/locomo10 | 1531 / 1540 | 29 | yes | 0% | **yes** (our dev set) |
| longmemeval/s | 479 / 500 | ~48 | yes | — | yes (we use upstream's file instead) |
| beam/100k | 400 / 400 | 9 | **no — 0 of 400** | — | **no** |
| beam/500k | 700 / 700 | 27 | **no — 0 of 700** | — | **no** |
| beam/1m | 700 / 700 | 50 | **no — 0 of 700** | — | **no** |
| beam/10m | 200 / 200 | 1 | yes | **100%** | **no** (trivial) |
| personamem/32k · 128k · 1M | 589 · 2710 · 2674 | 5 · 20 · 60 | yes | **100%** | **no** (trivial) |
| lifebench/en | 0 / 2003 | — | — | — | no |
| sdebench/boltons | 0 / 61 | — | — | — | no |

BEAM's gold id is the *conversation* id (`"1"`) while its documents are chunks
(`1_s0_0`, `1_s0_1`, …); PersonaMem lists every chunk of the user's history. So
neither can extend the dev set without deriving turn-level evidence labels from
the upstream release — that is a labelling project, recorded as TODO, not a
harness flag. Do not build a "retrieval-only BEAM" on AMB's gold ids: it will
score 0 or 100 by construction.

---

## 4. The harnesses, and how memory-wire plugs into each

Every harness needs the same two things from us: **write a document under a
caller id** and **return ranked hits that map back to that id**. memory-wire
serves its own memory uuid, not the caller's `document_id`, so every adapter
keeps a uuid→document map from the `retain` response (see `eval/amb/memory_wire.py`
docstring — without it a perfect ranking scores 0.0).

### 4.1 AMB — vectorize-io/agent-memory-benchmark (MIT)

- Pipeline: ingest → retrieve → generate (Gemini/Groq/OpenAI) → judge. Modes:
  `rag`, `agentic_rag`, `agent`, `coding`, `retrieval` (`src/memory_bench/modes/`).
  `retrieval` is LLM-free and only PrecisionMemBench uses it.
- Provider contract: `src/memory_bench/memory/base.py` — `ingest(documents)`,
  `retrieve(query, k, user_id, query_timestamp, filters?) -> (docs, raw)`,
  optional `prepare/initialize/cleanup`, `supports_filters`, `direct_answer`
  (for `agent` mode; memory-wire's `reflect` could back it).
- **Our provider:** `eval/amb/memory_wire.py`, HTTP client over a separately
  started `memory-wire serve`. Install instructions and the exact run command
  are in `eval/amb/README.md`. It was previously only an *untracked file inside
  the audit clone* (`../_audit/amb/src/memory_bench/memory/memory_wire.py` plus a
  two-line registry edit) — one `git clean` from being lost. `eval/amb/` is now
  the source of truth; the audit-clone copy is a deployment of it.
- Known CLI trap: `amb run` constructs the answer-LLM client **eagerly**, even in
  `--mode retrieval`, so it exits on a missing `GEMINI_API_KEY`/`GROQ_API_KEY`.
  Set both to a placeholder and unset any real key for a retrieval run —
  `eval/amb/README.md` has the exact `env` line. Placeholders cannot spend money;
  a real key in the environment can.
- Leaderboard: agentmemorybenchmark.ai. Its numbers are judged-accuracy with
  Gemini in the loop; never place a recall@K beside them (`eval/LOCOMO.md`
  "Comparability").

### 4.2 agent-memory-bench — GiulioDER/agent-memory-bench (Apache-2.0, `c1b1572`, 2026-10-06)

The benchmark the user's list calls "execution-graded benchmark for
coding-agent memory". **Not** to be confused with
`hamza-dev-tech/agent-memory-bench` (MIT, retrieval-graded, Mem0/Letta/Zep —
a different project with the same name).

- Claude Code runs a real coding task in a sandboxed fixture repo; an executable
  checker grades the artifact against oracles the sandbox never sees. Each of
  the 34 tasks ships a `naive/` reference that must fail and an `informed/` one
  that must pass, asserted in CI. No LLM judge.
- Corpus: 195 verbatim session transcripts, ingested once per condition through
  each product's own write path, then frozen. A harm suite varies the corpus —
  `present`, `absent`, `superseded`, `contradictory`, `adjacent` — and defines
  damage as failing a cell the memory-free `bare` arm solved.
- Arms: `bare`, `placebo`, `claude_md` (the baseline), `fs_grep`, and one arm per
  product **entering through its shipped Claude Code integration** (plugin, MCP
  server or hooks), hash-pinned in `adapters/<name>/config.frozen.json`, behind an
  admission gate (MCP tools listed at init, hooks demonstrably fired).
- Official run `official-003`: retrieval over a bulk-ingested corpus (not the full
  lifecycle), model `deepseek/deepseek-v4-flash`, 26 tasks, 317 paired cells. **The
  headline is a null**: `placebo` 0.672, `recall` and `bare` 0.659, `claude_md`
  0.577, no arm's 95% interval excludes zero. Lifecycle probes (learning from the
  agent's own work across sessions) are separate and optional.
- Disclosure: built by the authors of `recall`, which competes in it.
- **memory-wire fit:** this is the benchmark that measures what the product is for.
  An arm would be `memory-wire connect claude-code` (hooks + MCP) — exactly the
  surface `docs/INTEGRATION_AUDIT.md` covers — with `adapters/memory_wire/` written
  against their `MemoryAdapter` contract (`docs/INTEGRATION-CONTRACT.md` upstream).
  Needs an agent-model key and Docker. Not started.

### 4.3 OAMB — rocke2020/open-agent-memory-benchmark (Apache-2.0, `9117145`, 2026-10-08)

- Compares systems through their **native REST APIs** on a fixed, balanced
  **LME-60** (10 questions × 6 LongMemEval types), with a frozen plan (questions,
  model roles, answer/judge policy) and question-level public results under
  `eval_results/`. Five metrics, never combined: answer accuracy,
  answer-visible context tokens, indexing tokens, retrieval latency, indexing time.
- v0.1.0 (DeepSeek V4.1 Flash for everything): Hindsight 57/60, Mem0 52/60 with
  the least context (392.6k tokens), OpenViking 51/60 and slowest (0.79 s median).
- Needs an OpenAI-compatible LLM endpoint **and** an embedding service
  (`qwen3-embedding:0.6b`, 1024-d, via Ollama on Linux). Providers run as
  services under `provider-services/` (compose); adapters under
  `src/oamb/memory_systems/<name>/adapter.py`.
- **memory-wire fit:** a natural one — "answer-visible context tokens" is exactly
  the axis where a budget-trimmed recall should win. But the data is LongMemEval:
  one run = one test-set consultation. Not started.

### 4.4 MemBase — zjunlp/MemBase (MIT, `a9ba949`, 2026-09-03)

- Three stages: `memory_construction.py` → `memory_search.py` →
  `memory_evaluation.py` (token F1, BLEU, LLM judge by default).
- Layer contract: `membase/layers/base.py::MemBaseLayer` — `add_message`,
  `add_messages`, `retrieve(query, k)`; register in the `membase` package.
- Datasets: LongMemEval, LoCoMo, **RealMem**, **MobileMem** — the last two are
  corpora we have nothing on and are the reason to care.
- Baselines: A-MEM, LangMem, Mem0, MemOS, EverMemOS, HippoRAG2, long-context,
  naive RAG. Non-invasive token-cost monitoring and execution-graph tracing.

### 4.5 memorybench — supermemoryai/memorybench (MIT, `94e2af5`, 2026-08-24)

- Bun/TypeScript. Phases ingest → index → search → answer → evaluate → report,
  checkpointed. Benchmarks: LoCoMo, LongMemEval, ConvoMem. Judges are swappable.
- Provider contract: `src/types/provider.ts` — `initialize`, `ingest(sessions,
  {containerTag})`, `awaitIndexing`, `search(query, {containerTag, limit,
  threshold})`, `clear(containerTag)`. `containerTag` maps onto a bank.
- Reports **MemScore** = `accuracy% / latencyMs / contextTokens` as a triple, on
  purpose not collapsed.
- **ConvoMem** is the only corpus here we have nothing on.

### 4.6 OmniMemEval — MemTensor/OmniMemEval (Apache-2.0, checked 2026-10-15)

- Two tracks. **User Memory**: a standardized `add()`/`search()` adapter layer
  (`scripts/client_factory/`, backend selected with `--lib`), one pipeline
  (ingest → retrieve → answer generation → LLM-as-judge → metrics) over LoCoMo,
  LongMemEval, BEAM, PersonaMem v2, HaluMem; results under
  `results/<benchmark>/<LIB>-<VERSION>/`. **Agent Memory**: AgentBench task
  domains through an OpenClaw memory plugin — needs Docker, LiveCodeBench,
  BrowseComp-Plus, an agent-model budget; same tier as agent-memory-bench
  (§4.2): parked until the user funds it.
- **Roles, classified before any run** (§6.1): LoCoMo = dev *judged instrument*
  (cross-check for the AMB rag number, never a ranking-selection signal);
  LongMemEval = **test consultation**, counted like any other LME run;
  PersonaMem v2 and HaluMem = measure-only, and the only two corpora here we
  have nothing on — HaluMem (hallucination resistance) is the first-run
  candidate per `docs/COMPETITOR_GAPS_PLAN.md` §3.
- Cost: answer+eval LLM credentials per run. Whether the free aroute tier can
  stand in (their client factory accepting an OpenAI-compatible base URL) is a
  fact to verify at adapter time from their `env_examples/`; do not assume it.
- Provider shape: a thin HTTP client with the uuid→document mapping §4.0
  requires — the AMB provider (`eval/amb/memory_wire.py`) is the template.
- License: Apache-2.0, with dataset licenses retained upstream
  (`THIRD_PARTY_NOTICES.md` in their repo) — vendoring their data is out; a
  pinned clone and their preparation scripts are not.

### 4.7 The datasets' own repos

| repo | license | what it adds beyond the copy we use |
|---|---|---|
| xiaowu0162/LongMemEval (`9e0b455`) | MIT | official retrieval eval (`src/retrieval/`, `src/evaluation/print_retrieval_metrics.py`), turn-level `has_answer` labels, LongMemEval-M (~500 sessions per history) and the oracle split |
| snap-research/locomo (`3eb6f2c`) | **CC BY-NC 4.0** | QA, event summarisation and multimodal-dialogue tasks; dialog-id `evidence` per QA; non-commercial licence — do not vendor it |
| mohammadtavakoli78/BEAM (`b2da22e`) | MIT code; dataset **CC BY-SA 4.0** | 100 conversations, 2,000 probing questions, 10 abilities (abstention, contradiction resolution, event ordering, information extraction, instruction following, knowledge update, multi-session reasoning, preference following, summarisation, temporal reasoning); 10M split on HF `Mohammadta/BEAM-10M`; LLM-judged |
| tenurehq/precisionmembench (HEAD `61a712a`; AMB pins `b95d6ab`) | MIT | 35 beliefs, 77 cases, 13 categories; paper arXiv 2605.11325 |

---

## 5. What has been measured on the external sets

### 5.1 PrecisionMemBench, 2026-10-13 — memory-wire 0.7.0 @ `84580d5`

Run once, through AMB @ `03c1d0f`, `--mode retrieval`, provider
`eval/amb/memory_wire.py` (no filters, `k` = AMB's `retrieval_limit` of 20),
against a scratch `memory-wire serve` on a fresh database. No LLM anywhere.
Load average 10.7 → 12.2 on 24 cores (latency below is a range, retrieval
numbers are load-independent). Artifact:
`eval/amb/PRECISIONMEMBENCH-2026-10-13-v0.7.0.json.gz` (AMB's own output, gzipped,
unedited).

| metric | memory-wire 0.7.0 | hindsight-cloud 0.10.0 (AMB's committed run) |
|---|---|---|
| total passes | **14 / 77** | 66 / 77 |
| active passes (provider-dependent cases) | **3 / 43** | 33 / 43 |
| mean precision | **0.17** | 0.85 |
| mean recall | **0.95** | 1.00 |
| avg context tokens | 535.6 | 150.5 |

The hindsight row is `../_audit/amb/outputs/precisionmembench/hindsight/retrieval/single-turn.json.gz`
and is **not a like-for-like comparison**: it ran with `supports_filters` on
(server-side scope and identity tag filters the dataset supplies) and with
identity labels derived by an extraction LLM at ingest. Ours ran with neither.

**What it shows.** memory-wire finds the right belief (recall 0.95) and returns
far too much around it: 563 of the 567 assertion failures are
`unexpected belief in relevantBeliefs`; 2 are missing beliefs and 2 are
forbidden beliefs surfaced. Median 5 beliefs returned per case, max 20. This is
the same mechanism `docs/OPEN_HOOK_RECALL_RELEVANCE.md` diagnosed on the live
banks — BM25 ORs the query tokens and the overlap stream keeps any `score > 0`,
so on a 35-document bank nearly everything that shares a word comes back — now
measured on an independent public set.

**What it does not license.** It does not reopen score thresholds (`TODO.md`
"Deliberately not doing"): 77 cases is the size `eval/relevance/README.md` calls
anecdote, and choosing a cut-off by this table would be fitting it. The levers
that can be argued mechanistically are recorded as TODO items, not acted on.

### 5.2 Comparability finding — LongMemEval abstention questions

LongMemEval-S marks its 30 abstention questions in **`question_id`** (suffix
`_abs`), not in `question_type`. The official retrieval evaluation skips them
("we always skip the 30 abstention instances", upstream README), scoring 470.

- `examples/longmemeval.rs` scores all 500.
- agentmemory's `benchmark/longmemeval-bench.ts:127-133` *tries* to skip them by
  filtering `question_type` against `*_abs` — which never matches in the cleaned
  file, so it also scores all 500 (verified 2026-10-13: every `_abs` question's
  `question_type` is a plain type; 30 of 500; their `answer_session_ids` are
  non-empty, 1–4 sessions each).

So **our number and agentmemory's are comparable to each other** (same 500), and
**neither is comparable to the paper's retrieval tables** (470). The 470-question
figure was added to the generator the same day and first read **84.0% R@1 /
97.2% R@5, n 470** (`eval/RESULTS.md`, last row).

---

## 6. Rules specific to external benchmarks

1. **Classify before you run.** Every new set gets a role — dev, test, or
   measure-only — written in this file *before* its first number. Measure-only
   sets follow the LongMemEval rule: report, never select.
2. **A harness wrapping LongMemEval is LongMemEval.** OAMB's LME-60, MemBase's and
   memorybench's LME runs, AMB's `longmemeval/s` — each run is a test-set
   consultation and is counted.
3. **Never place a retrieval number beside a judged-accuracy number**, and never
   beside a number produced with a different answering model or judge.
4. **Commit the harness's own output**, unedited (gzip it if large), next to the
   prose that quotes it. AGENTS §7: the artifact wins.
5. **Pin the harness commit and the dataset commit** in the artifact or its
   README, and record whether the harness tree was locally modified.
6. **Placeholders, never real keys, for LLM-free modes.** A paid run needs the
   user's explicit go-ahead and a stated budget.
7. **Name collisions are real**: "AMB" is both vectorize-io's Agent Memory
   Benchmark and the abbreviation GiulioDER's agent-memory-bench uses for itself.
   Always cite the GitHub owner.

---

## 7. The battery, in the order it should be run

Ordered by information per dollar. Rows 2–6 and the free-model LLM rows are
scripted as one battery: **`eval/baseline/run_baseline.sh`**, which writes a dated
directory under `eval/baseline/` (see its README for the first baseline and the
instrument caveats). Rows 7+ need an agent-model budget agreed with the user.
The LLM rows run free through aroute's `small-stack` combo via `eval/llm_shim.py`;
a paid frontier reader is a separate decision.

| # | run | cost | command / pointer | gates |
|---|---|---|---|---|
| 1 | build/test/clippy/doc gate | free | `AGENTS.md` §2 | must pass |
| 2 | LoCoMo dev set | free, ~20 s | `cargo run --release --example locomo` | selection happens here |
| 3 | relevance battery | free | `python3 eval/relevance/run_battery.py --bank omp` | auto-injection junk/gold |
| 4 | PrecisionMemBench | free, ~10 s | `eval/amb/README.md` | measure-only |
| 5 | LongMemEval-S retrieval | free, minutes | `cargo run --release --example longmemeval -- --n 500` | release acceptance only; counts as a consultation |
| 6 | synthetic scale / footprint / latency | free, quiet box | `eval/README.md` "benchmark harnesses" | `loadavg` recorded |
| 7 | agent-memory-bench Claude Code arm | agent-model tokens + Docker | §4.2 | measure-only; the product question |
| 8 | AMB `rag` on BEAM / PersonaMem / LifeBench | answer+judge tokens | `amb run --dataset beam --split 100k --memory memory-wire` (AMB's README says `--domain`; the CLI at `03c1d0f` takes `--split`) | measure-only |
| 9 | OAMB LME-60 | LLM + embedding service | §4.3 | **test consultation** |
| 10 | MemBase RealMem / MobileMem; memorybench ConvoMem | LLM | §4.4, §4.5 | measure-only; new corpora |
| 11 | OmniMemEval HaluMem / PersonaMem v2 | LLM (answer + judge; free tier only if their client factory takes an OpenAI-compatible base URL — verify) | §4.6, `docs/COMPETITOR_GAPS_PLAN.md` §3 | measure-only; **first run pre-registered: HaluMem, once, whatever it shows**; their LoCoMo = dev-judged cross-check; their LongMemEval = **test consultation** |
