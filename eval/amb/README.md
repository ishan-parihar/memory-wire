# memory-wire under AMB (vectorize-io/agent-memory-benchmark)

`memory_wire.py` is AMB's memory provider for memory-wire: a thin HTTP client
over `POST /banks/:bank/{retain,recall}`, one bank per AMB `user_id`. It tests
the shipped binary, not the library. This directory is its **source of truth**;
an AMB checkout gets a copy. Background and the full benchmark map:
`docs/BENCHMARK_LANDSCAPE.md` §4.1.

## Install into an AMB checkout

Tested against AMB @ `03c1d0f` (the clone at `../_audit/amb`).

```bash
AMB=../_audit/amb
cp eval/amb/memory_wire.py "$AMB/src/memory_bench/memory/memory_wire.py"
```

and add two lines to `$AMB/src/memory_bench/memory/__init__.py`:

```python
from .memory_wire import MemoryWireProvider          # with the other imports
    "memory-wire": MemoryWireProvider,               # inside REGISTRY
```

## Run PrecisionMemBench (LLM-free, ~10 s)

```bash
# a scratch server on a fresh database — never the live store
memory-wire serve --db "$TMPDIR/amb.db" --addr 127.0.0.1:8899 &

cd ../_audit/amb
env -u GOOGLE_API_KEY -u OPENAI_API_KEY \
    GEMINI_API_KEY=unused GROQ_API_KEY=unused \
    MEMORY_WIRE_URL=http://127.0.0.1:8899 \
  .venv/bin/amb run --dataset precisionmembench --split single-turn \
                    --memory memory-wire --mode retrieval
```

- **The placeholder keys are required.** `amb run` builds the answer-LLM client
  before it reads `--mode`, so a retrieval run exits on a missing key. Retrieval
  mode never calls it; unsetting the real keys guarantees a mistake can't spend.
- **First run downloads two fixtures** from `raw.githubusercontent.com` into
  `.datasets/precisionmembench/`. On this workstation that host can stall for
  minutes; fetch them through the API instead:
  ```bash
  for f in beliefs.seed.json retrieval.cases.json; do
    gh api "repos/tenurehq/precisionmembench/contents/fixtures/$f?ref=b95d6abb471c0d591c440172283dd74e7af000df" \
      -H 'Accept: application/vnd.github.raw' > ".datasets/precisionmembench/$f"
  done
  ```
  sha256 on 2026-10-13: `beliefs.seed.json` `35659b3b…de98`,
  `retrieval.cases.json` `09e99f08…709c`.
- Output lands in `outputs/precisionmembench/memory-wire/retrieval/single-turn.json`.
  To commit a run, gzip it here unedited as
  `PRECISIONMEMBENCH-<date>-<version>.json.gz` and quote it from
  `docs/CONSISTENCY.md`.

## Provider knobs

| env | default | meaning |
|---|---|---|
| `MEMORY_WIRE_URL` | `http://127.0.0.1:8899` | the scratch server (deliberately not `8888`, the live default) |
| `MEMORY_WIRE_BUDGET` | `200000` | recall token budget; large so the budget never truncates below `k` |
| `MEMORY_WIRE_TIMEOUT` | `60` | per-request seconds |

`supports_filters = False`: AMB's PrecisionMemBench filter carries exact/fuzzy
name-resolution clauses that memory-wire's tag filter cannot express. Turning
it on is a provider-capability change — decide it by mechanism, and record that
the PMB number moved because of it (`docs/BENCHMARK_LANDSCAPE.md` §5.1).

**What a `rag` reader sees.** `retrieve` returns `raw_response = None` on purpose.
AMB's LoCoMo, LongMemEval and LifeBench prompt builders replace the formatted
documents with `json.dumps(raw_response)` whenever one is returned. A provider that
returns metadata there (this one did until 2026-10-13) hands the reader **no
memories at all**. That first AMB LoCoMo run scored 0/29 and was aborted; its log
is kept as `eval/baseline/2026-10-13-v0.7.0/log-amb_locomo_rag-aborted.txt`. Each hit
is also prefixed with `[<context> — <created_at>]`. `created_at` is AMB's own document
timestamp, sent as `retain`'s `created_at` and read back from `format: full` recall (TODO
row 15, closed 2026-10-14), so the date round-trips through the product; `context` (AMB's
session label) is still held client-side. A document without a timestamp gets no date. The
committed baseline's LoCoMo `rag` run predates this and held both client-side: re-running
with the current adapter is a new instrument version. The index is untouched, so ranking
is exactly memory-wire's.

## Committed runs

| file | binary | AMB | result |
|---|---|---|---|
| `PRECISIONMEMBENCH-2026-10-13-v0.7.0.json.gz` | 0.7.0 @ `84580d5` | `03c1d0f` + this provider | 14/77 total, 3/43 active, precision 0.17, recall 0.95 |
