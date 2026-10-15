#!/usr/bin/env bash
# The full baseline battery: every harness this repo has, one dated directory.
#
#   eval/baseline/run_baseline.sh [retrieval] [perf] [llm]     # default: all three
#   (`llm` = `answer_quality` + `amb_rag`; either can be named alone)
#
# Writes eval/baseline/$TAG/ (TAG defaults to <date>-v<version>): each harness's own
# artifact, unedited, plus PROVENANCE.md (commit, dirty state, loadavg around every
# step). An R&D change is judged by re-running this into a new TAG and diffing.
# What each phase costs and why each corpus is or is not in it:
# docs/BENCHMARK_LANDSCAPE.md §7 and eval/baseline/README.md.
#
# Phases:
#   retrieval  LongMemEval-S (test — counts as one consultation), LoCoMo (dev),
#              PrecisionMemBench via AMB, the relevance battery, coding_life,
#              scale_sweep, bench_recall_curve. Deterministic, free.
#   perf       footprint, write, concurrency, coldstart, soak. Free; wall-clock, so
#              read them with the loadavg recorded beside them.
#   llm        answer_quality on LongMemEval-S (500 q) and AMB `rag` on LoCoMo
#              (1,540 q), answer + judge through $LLM_UPSTREAM's $LLM_MODEL combo,
#              via eval/llm_shim.py, which logs the model that served every call.
#
# Env: AMB (AMB checkout, default ../_audit/amb), LLM_UPSTREAM (default aroute,
# http://127.0.0.1:20128), LLM_MODEL (default small-stack), LLM_JUDGE_MODEL
# (default $LLM_MODEL), N_LME (default 500), LOCOMO_LIMIT (AMB --query-limit,
# default all), BANK (relevance battery bank, default omp).
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
export PATH=/usr/bin:$PATH   # AGENTS §2: ~/.local/bin/ar shadows GNU ar

PHASES=("$@"); [ ${#PHASES[@]} -eq 0 ] && PHASES=(retrieval perf llm)
# `llm` is both LLM steps; name one to re-run it alone (each takes hours).
PHASES=($(printf '%s\n' "${PHASES[@]}" | sed 's/^llm$/answer_quality amb_rag/'))
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
TAG=${TAG:-$(date +%F)-v$VERSION}
OUT=$ROOT/eval/baseline/$TAG
AMB=${AMB:-$ROOT/../_audit/amb}
LLM_UPSTREAM=${LLM_UPSTREAM:-http://127.0.0.1:20128}
LLM_MODEL=${LLM_MODEL:-small-stack}
LLM_JUDGE_MODEL=${LLM_JUDGE_MODEL:-$LLM_MODEL}
N_LME=${N_LME:-500}
BANK=${BANK:-omp}
SCRATCH=$(mktemp -d)
# AMB's own output outlives the run so an LLM step that dies can resume (target/ is
# gitignored); the committed copy is the .json.gz in $OUT.
AMB_OUT=$ROOT/target/amb-out/$TAG
mkdir -p "$OUT"
PIDS=()
trap 'for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null || true; done; rm -rf "$SCRATCH"' EXIT

P="$OUT/PROVENANCE.md"
if [ ! -f "$P" ]; then
  {
    echo "# Baseline $TAG — provenance"
    echo
    echo "| | |"; echo "|---|---|"
    echo "| commit | \`$(git rev-parse HEAD)\` |"
    echo "| tree | $(git status --porcelain | grep -c . || true) uncommitted paths at start |"
    echo "| binary | $(./target/release/memory-wire --version 2>/dev/null || echo 'not built yet') |"
    echo "| machine | $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs), $(nproc) logical CPUs |"
    echo "| AMB | \`$(git -C "$AMB" rev-parse --short HEAD 2>/dev/null || echo absent)\`, $(git -C "$AMB" status --porcelain 2>/dev/null | grep -c . || true) locally modified paths (our provider) |"
    echo
    echo "| step | started (UTC) | loadavg before | loadavg after | exit |"
    echo "|---|---|---|---|---|"
  } > "$P"
fi

step() {  # step <name> <command...> — runs, logs, never aborts the battery
  local name=$1; shift
  local before; before=$(cut -d' ' -f1-3 /proc/loadavg)
  local start; start=$(date -u +%FT%TZ)
  echo "=== $name" >&2
  local rc=0
  "$@" > "$OUT/log-$name.txt" 2>&1 || rc=$?
  echo "| $name | $start | $before | $(cut -d' ' -f1-3 /proc/loadavg) | $rc |" >> "$P"
  [ $rc -eq 0 ] || echo "    $name exited $rc — see $OUT/log-$name.txt" >&2
}

wait_health() {
  for _ in $(seq 1 40); do
    python3 -c "import urllib.request,sys; urllib.request.urlopen(sys.argv[1], timeout=2)" "$1" 2>/dev/null && return 0
    sleep 0.5
  done
  echo "no health at $1" >&2; return 1
}

scratch_serve() {  # scratch_serve <port> — a fresh store, never the live one
  ./target/release/memory-wire serve --db "$SCRATCH/serve-$1.db" --addr "127.0.0.1:$1" \
    > "$SCRATCH/serve-$1.log" 2>&1 &
  PIDS+=($!)
  wait_health "http://127.0.0.1:$1/health"
}

amb_run() {  # amb_run <port> <amb args...>; LLM env comes from the caller
  (cd "$AMB" && MEMORY_WIRE_URL="http://127.0.0.1:$1" .venv/bin/amb run "${@:2}" \
     --memory memory-wire --output-dir "$AMB_OUT")
}

shim() {  # one LLM shim per run, shared by the LLM steps
  [ -n "${SHIM_UP:-}" ] && return
  python3 eval/llm_shim.py --upstream "$LLM_UPSTREAM" --port 20199 --log "$OUT/llm-calls.jsonl" \
    2> "$SCRATCH/shim.log" &
  PIDS+=($!); SHIM_UP=1
  sleep 1
}

cargo build --release --locked --examples --bins 2>&1 | tail -2
EX=./target/release/examples

for phase in "${PHASES[@]}"; do case $phase in
retrieval)
  step longmemeval $EX/longmemeval --data eval/data/longmemeval_s_cleaned.json --n 500 \
       --out-md "$OUT/LONGMEMEVAL.md" --out-json "$OUT/longmemeval.json"
  step locomo $EX/locomo --docs eval/data/locomo/documents.json --data eval/data/locomo/queries.json \
       --out-md "$OUT/LOCOMO.md" --out-json "$OUT/locomo.json"
  step coding_life $EX/coding_life --out-md "$OUT/CODING_LIFE.md"
  step scale_sweep $EX/scale_sweep --out-md "$OUT/SCALE_SWEEP.md"
  step recall_curve $EX/bench_recall_curve --out-md "$OUT/BENCH_RECALL_CURVE.md"
  step battery python3 eval/relevance/run_battery.py --bank "$BANK"
  cp "$OUT/log-battery.txt" "$OUT/BATTERY.txt"
  scratch_serve 8899
  # Placeholders: `amb run` builds an answer client even in retrieval mode.
  step precisionmembench env -u GOOGLE_API_KEY -u OPENAI_API_KEY GEMINI_API_KEY=unused GROQ_API_KEY=unused \
       bash -c "$(declare -f amb_run); AMB_OUT=$AMB_OUT AMB=$AMB; amb_run 8899 --dataset precisionmembench --split single-turn --mode retrieval"
  f=$AMB_OUT/precisionmembench/memory-wire/retrieval/single-turn.json
  [ -f "$f" ] && gzip -9c "$f" > "$OUT/precisionmembench.json.gz"
  ;;
perf)
  step footprint   $EX/bench_footprint   --out-md "$OUT/BENCH_FOOTPRINT.md"
  step write       $EX/bench_write       --out-md "$OUT/BENCH_WRITE.md"
  step concurrency $EX/bench_concurrency --out-md "$OUT/BENCH_CONCURRENCY.md"
  step coldstart   $EX/bench_coldstart   --out-md "$OUT/BENCH_COLDSTART.md"
  step soak        $EX/soak              --out-md "$OUT/SOAK.md"
  ;;
answer_quality)
  shim
  step answer_quality env MEMORY_WIRE_LLM_URL=http://127.0.0.1:20199/v1 \
       MEMORY_WIRE_LLM_MODEL="$LLM_MODEL" MEMORY_WIRE_LLM_JUDGE_MODEL="$LLM_JUDGE_MODEL" \
       MEMORY_WIRE_LLM_KEY=unused \
       $EX/answer_quality --data eval/data/longmemeval_s_cleaned.json --n "$N_LME" --seed 42 \
       --out-md "$OUT/ANSWER_QUALITY_LME.md" --out-json "$OUT/answer_quality_lme.json"
  ;;
amb_rag)
  shim
  scratch_serve 8898
  # Resume: --skip-ingested skips conversations already in AMB's output (it saves one
  # per finished conversation). Seed that output from the committed artifact so a
  # step that died, here or on another checkout, continues instead of re-spending
  # hours. Delete $OUT/amb_locomo_rag.json.gz for a fresh run.
  f=$AMB_OUT/locomo/memory-wire/rag/locomo10.json
  if [ ! -f "$f" ] && [ -f "$OUT/amb_locomo_rag.json.gz" ]; then
    mkdir -p "$(dirname "$f")"; gunzip -c "$OUT/amb_locomo_rag.json.gz" > "$f"
  fi
  limit=(); [ -n "${LOCOMO_LIMIT:-}" ] && limit=(--query-limit "$LOCOMO_LIMIT")
  step amb_locomo_rag env -u GOOGLE_API_KEY GEMINI_API_KEY=unused GROQ_API_KEY=unused \
       OPENAI_BASE_URL=http://127.0.0.1:20199/v1 OPENAI_API_KEY=unused \
       LOCOMO_DATA_PATH="$ROOT/eval/data/locomo/locomo10.json" \
       OMB_ANSWER_LLM=openai OMB_ANSWER_MODEL="$LLM_MODEL" \
       OMB_JUDGE_LLM=openai OMB_JUDGE_MODEL="$LLM_JUDGE_MODEL" \
       bash -c "$(declare -f amb_run); AMB_OUT=$AMB_OUT AMB=$AMB; amb_run 8898 --dataset locomo --split locomo10 --mode rag --llm openai --skip-ingested ${limit[*]}"
  [ -f "$f" ] && gzip -9c "$f" > "$OUT/amb_locomo_rag.json.gz"
  ;;
*) echo "unknown phase '$phase'; valid: retrieval perf llm answer_quality amb_rag" >&2; exit 2 ;;
esac; done

echo "baseline written to $OUT" >&2
