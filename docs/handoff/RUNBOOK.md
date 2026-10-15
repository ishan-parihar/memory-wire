# Runbook — exact commands and the traps around them

Run from the repo root. Scripts are bash (the login shell on this machine is fish; use
`bash -c` or the shebang, and `env VAR=… cmd` rather than `VAR=… cmd` in fish).

## 1. Always

```bash
export PATH=/usr/bin:$PATH        # ~/.local/bin/ar shadows GNU ar and breaks libsqlite3-sys
```

Gate (AGENTS §2). Must be green before you report anything as verified:

```bash
cargo build --release --locked
cargo test --locked                                              # 9 suites / 557 tests at handoff
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo doc --no-deps --all-features --locked 2>&1 | grep -c '^warning'   # must print 0
```

Trap: `grep -c` printing `0` makes a `set -e` script exit 1 — that is a false failure,
not a doc warning. Benchmarks and latency artifacts must be `--release`; never trust debug
numbers. Record `loadavg` before and after anything timed (AGENTS §7).

## 2. The inner loop (free, deterministic, ~25 s)

```bash
cargo run --release --example locomo -- \
  --out-md <scratch>/LOCOMO.md --out-json <scratch>/locomo.json
cargo run --release --example locomo -- --turns --out-md … --out-json …     # turn arm
```

A **bare run writes to `$TMPDIR`, never to `eval/`**. To update a committed artifact you
must name it (`--out-md eval/LOCOMO.md --out-json eval/results_locomo.json`), and do so
only when the change is intended to become the new record. Use the session scratch
directory for exploration. Output JSON is a list of configs; each has `overlap` and
`queries[]` with `recall_any_at_{1,5,10,20}`, `recall_all_at_10`, `ndcg_at_10`, `mrr`,
`gold_in_pool`, `category`, `query_id`.

The grid in `examples/locomo.rs` (`GRID`) builds `FusionWeights { overlap: w,
..FusionWeights::SHIPPED }`. To test another field, change the struct update there (a
local experiment), run, then **revert** unless it is the deliverable.

Walk-style selection over several axes (slow, minutes to ~30 min; dev set only):

```bash
cargo run --release --example select_fusion -- --axes overlap,k,bm25_magnitude \
  --rounds 2 --budget-seconds 1800 --out-md <scratch>/SEL.md --out-json <scratch>/sel.json
```

Axes that exist: `overlap k bm25_magnitude agreement recency recency_half_life_days
vector`. **Not** axes: `keyword_scope`, `overlap_scope`, `recency_policy` (RND_PLAN H1a,
H1b, H3). `select_fusion` does not yet print `recall_all@10`.

## 3. Test-set consultations (count every one)

```bash
cargo run --release --example longmemeval -- --n 500       # retrieval; free, minutes
```

Each run goes in `docs/CONSISTENCY.md` as a consultation. Run it only after the dev-set
gate passed, once per change, and never to choose between candidates.

## 4. The full baseline battery

```bash
eval/baseline/run_baseline.sh retrieval perf            # free part, into eval/baseline/<TAG>/
TAG=2026-11-01-exp-foo eval/baseline/run_baseline.sh retrieval
TAG=<existing> eval/baseline/run_baseline.sh amb_rag    # one LLM step alone
TAG=<new>      eval/baseline/run_baseline.sh answer_quality
```

`TAG` defaults to `<date>-v<version>`. **One dated directory per run; compare directory to
directory; never overwrite an old baseline.** Each artifact is the harness's own output,
unedited, plus `PROVENANCE.md` (commit, dirty paths, loadavg around each step, exit codes).
Phase `llm` = `answer_quality` + `amb_rag` (hours each). Always run LLM steps with:

```bash
env -u GOOGLE_API_KEY TAG=<tag> setsid nohup eval/baseline/run_baseline.sh amb_rag \
  > <scratch>/run.log 2>&1 < /dev/null & disown
```

`-u GOOGLE_API_KEY` guarantees a mistake cannot spend a real key. Do not run two LLM
jobs at once.

### Resuming after a crash or reboot (this happened three times)

`amb_rag` keeps AMB's output in `target/amb-out/$TAG` (gitignored) and passes
`--skip-ingested`: relaunch the **same command with the same TAG**; the log's first line
is `Skipping N already-ingested units: conv-…`. Before relaunching, rename the dead run's
log so the history is kept: `mv log-amb_locomo_rag.txt log-amb_locomo_rag-<reason>.txt`.
`answer_quality` has no resume: a crash costs the run.

## 5. The LLM instrument

```
harness ──► eval/llm_shim.py (127.0.0.1:20199) ──► aroute small-stack (127.0.0.1:20128)
```

The script starts the shim itself. Manual use:
`python3 eval/llm_shim.py --upstream http://127.0.0.1:20128 --port 20199 --log calls.jsonl`
(`--deadline 480` default; keep it below the OpenAI SDK's 600 s). It unwraps cline's
`{"success","data"}` envelope, retries 200-with-error bodies, restates `json_schema` in a
system message (free models ignore `response_format`), defaults `temperature` to 0, and
logs one JSONL line per call: `served_model, provider, attempts, enveloped, tokens_in/out,
ms, status`. A judged number is only interpretable next to that log.

- Model is `LLM_MODEL` (default `small-stack`); judge `LLM_JUDGE_MODEL` (default the same).
  aroute routes **combo names only** and the serving model inside a combo cannot be pinned.
  If the user wants a faster combo, they must name it; do not enumerate or read aroute's
  config or credentials, and do not restart `aroute.service`.
- Expect ~12 s median per call at load 20+ on ~15k-token prompts; hours per LoCoMo run.

## 6. AMB (vectorize-io/agent-memory-benchmark)

Checkout `../_audit/amb` @ `03c1d0f`. The provider's source of truth is
`eval/amb/memory_wire.py`; after editing it, **copy** it into the checkout:

```bash
cp eval/amb/memory_wire.py ../_audit/amb/src/memory_bench/memory/memory_wire.py
```

PrecisionMemBench (free, ~10 s) and the required placeholder keys: `eval/amb/README.md`.
A scratch server on a fresh database (never the live store at `:8888`):
`memory-wire serve --db "$TMPDIR/x.db" --addr 127.0.0.1:8899`. The adapter is a client of
`POST /banks/:bank/{retain,recall}`; ids round-trip through a client table because the
server serves its own uuid, not the caller's `document_id`.

Traps already paid for: the dataset prompt builders replace the context with
`json.dumps(raw_response)` if truthy → the adapter returns `None`; AMB does not retry
client timeouts (shim deadline); LoCoMo's dataset loader hangs on a network fetch unless
`LOCOMO_DATA_PATH` points at the pinned local copy (the baseline script sets it).

## 7. Re-deriving the judged-run diagnostics

```python
import json, gzip, re, ast, collections
d = json.load(gzip.open('eval/baseline/2026-10-13-v0.7.0/amb_locomo_rag.json.gz'))['results']
q = {x['id']: x for x in json.load(open('eval/data/locomo/queries.json'))}
agg = collections.Counter()
for r in d:
    g = q[r['query_id']]['gold_ids']; gold = set(ast.literal_eval(g) if isinstance(g, str) else g)
    served = {f"{c}_{s}" for s, c in re.findall(r'\((session_\d+) of (conv-\d+)\)', r['context'])}
    t = 'all' if gold <= served else 'some' if gold & served else 'none'
    agg[t, 'n'] += 1; agg[t, 'ok'] += bool(r['correct'])
print({k: v for k, v in agg.items()})   # expect all 1176/1289, some 88/166, none 31/85
```

Per-call model mix / retries: read `llm-calls.jsonl` in the baseline directory (AMB-era
rows have `ts > 1791905000`).

## 8. Relevance battery (dev, live bank)

`python3 eval/relevance/run_battery.py --bank omp` — read-only against the live store.
Drifts as the bank grows; compare before/after only within one sitting, or on a frozen
snapshot.

## 9. Things not to do

- Do not write to the live store `~/.local/share/memory-wire/memory.db` or port 8888 from
  a harness. Do not hand-edit installed agent-host files (`connect` owns them).
- Do not hand-edit any generated artifact under `eval/` — fix the generator and re-run.
- Do not edit `docs/CONSISTENCY.md` except to append.
- Do not enable CI, tag, push, or publish. Do not add a dependency (AGENTS §5/§8).
