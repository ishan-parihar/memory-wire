# CONTEXT PACK — context-pack-2026-11-19-round4

## READING CONTRACT

One message, one file: every input this round needs is below. Do not open repository files to re-read
what is already quoted here — that spends your tool calls and adds nothing.

- Cite by anchor: `# SECTION <NAME>` or `# FILE: <path> (lines a,b)`. A finding that rests on a figure
  must name its anchor and keep the figure's evidence class (`[LIVE]`, `[CERT]`, `[IN-SAMPLE]`,
  `[PRIOR]`, `[UNCONFIRMED]`). A figure with no anchor is an assumption: label it one.
- Lens families, where the pack carries them, are instruments, not vocabulary. Naming a lens earns
  nothing; the move the lens forces is the finding.
- If a number you need is absent, say which one and continue under a stated assumption. The report is
  judged on structure, arithmetic and falsifiers, not on filling gaps with plausible values.


# SECTION A-STATE

# FILE: AGENTS.md (lines 26,67)

## 1. What this project is

`memory-wire` is a single-binary agent memory service. A bank-isolated SQLite
store with FTS5 full-text search, exposed over HTTP and MCP, with hooks that
capture conversation prose into it.

It exists to be **the cheap option**. The competitive position is deliberate and
should not be eroded casually:

| | memory-wire | Hindsight | agentmemory |
|---|---|---|---|
| install | one static binary, no daemon required | Python image + Postgres/pg0, optional ONNX reranker | Node runtime, detached engine + worker |
| idle footprint | ~10.7 MiB RSS | 0.8–1.0 GB | ~6 MB heap at 1k, 316 MB at 50k |
| default build deps | 3 shared libraries | — | — |

Three properties are load-bearing and each has been paid for:

- **The default build must not gain a dependency or grow materially.** HEAD is
  **10.63 MiB** (11,141,240 B, `target/release/memory-wire`, measured 2026-11-19)
  with `DT_NEEDED` = 3. `v0.7.0` was 10.25 MiB
  (`eval/baseline/2026-10-13-v0.7.0/BENCH_FOOTPRINT.md`); the 9.7 MiB this line
  used to state was earlier still, and 10.25 was itself a year-old figure this
  line carried until a reviewer checked it against the binary. **Do not trust a
  size quoted here without re-running `stat`** — quote the command, not the
  number. Verify `ldd` after touching anything that links.
- **Retrieval reports are the record.** The competitor whose retrieval number we
  can actually compare against is agentmemory, on LongMemEval-S, and we lead it
  on the honest (unfitted) number. Do not publish a number you cannot trace to
  a committed artifact.
- **Retrieval is a reordering problem, not a coverage problem.** The candidate
  pool contains the gold memory in 500 of 500 LongMemEval questions. Work on
  ordering; do not spend effort on recall/coverage mechanisms. (Pool membership is
  not the same as the *top-k a reader is served*: on LoCoMo, questions with several
  gold sessions miss at k = 10 — `recall_all@10` 83.0% vs `recall_any@10` 94.2% —
  and that is still an ordering problem inside the pool. See `docs/handoff/RND_PLAN.md`.)

The parent projects are `rohitg00/agentmemory` and the Hindsight repositories,
audited read-only under `_audit/`. This is a deliberate Tier-C extraction: the
~3k LOC retrieval core is ported, the ~40k LOC surround is intentionally out.
**Full parent parity is not the goal** — pursuing it means rebuilding the
parents.


# SECTION A2-CLAIM

# FILE: AGENTS.md (lines 103,131)

## 3. Where things live

`src/`, 24.8k lines across 22 modules:

| module | lines | role |
|---|---|---|
| `store.rs` | 4353 | SQLite: schema, migrations, put/recall/delete, FTS5 |
| `api.rs` | 3080 | HTTP routes, the frozen error contract, the historian prompt |
| `recall.rs` | 2428 | RRF fusion, candidate pool, budget trim, scoring |
| `main.rs` | 1926 | clap surface, routing, wiring |
| `mcp.rs` | 1577 | MCP server: 4 tools + resources + prompts + completion |
| `connect.rs` | 1263 | agent-host detection and config writing |
| `daemon.rs` | 1132 | `daemon start/stop/status`, pidfile, start-time identity |
| `connect_plugin.rs` | 1100 | the hermes `MemoryProvider` plugin |
| `vector.rs` | 960 | dense arm (`embed` feature, OFF by default) |
| `cli_home.rs` | 948 | the content-first bare-invocation view |
| `hooks.rs` | 925 | the five lifecycle events |
| `connect_codex.rs` | 730 | codex TOML config writing |
| `render.rs` | 655 | human-readable report rendering |
| `seed.rs` | 649 | transcript reading for hook capture |
| `paths.rs` | 640 | bank identity, endpoint, data paths |
| `doctor.rs` | 590 | the health report |
| `mcp_http.rs` | 478 | stateless streamable-HTTP MCP mount |
| `embed.rs` | 177 | ONNX session, vendored MiniLM |

`tests/` — `backup.rs`, `daemon.rs`, `e2e.rs`, `recall_budget.rs`, `scale.rs`.
`examples/` — 15 benchmark and evaluation harnesses.
`eval/amb/` — our provider for vectorize-io's AMB harness, and its committed runs.


# SECTION B-DELTA

# FILE: research/audits/context-pack-2026-11-19-round4/B-DELTA.md (lines 1,119)

# B-DELTA — what landed since round 3, and what it changed about the shape

Round 3 closed at `98b1a36`. Six commits later, at `3ca8857`. Every claim here
names the command or the file that settles it.

---

## 1. The four-door enforcement problem was real, and it is now closed in all four doors

Round 3's strongest finding was that one policy — the `untrusted_external`
injection gate — existed in `src/hooks.rs` and the hermes plugin but in **neither
TypeScript plugin**. Both injected every row a recall returned, so an untrusted
mount's web content reached a model's system prompt as rules.

What round 3 did *not* notice: **hermes had the gate and no executable check.**
It filtered correctly (`__init__.py:516`), before the take, with the right
reasoning. Nothing could run it. The gate was enforced in three of four doors and
checked in two.

Closed across four batches:

| commit | what |
|---|---|
| `1f5ee17` (round 3) | gate added to both TS plugins, `trust` on the recall wire, `UntrustedExternal` added as a real tier |
| `db57850` | hermetic test for hermes (`trust_gate_test.py`, local HTTP stub, no hermes runtime) |
| `0b518de` | occurrence dates on all four doors |
| `1f93bf0` | capsule knob on the three plugin doors |

**The shape that matters for this round:** the failure was never a bug in one
file. It was **a policy with N implementations and N−1 checks**, plus a register
that stopped being maintained. The same shape produced:

- `measure_surface.sh` counting `src/` and stopping, so 1,622 lines of
  injection-path code were never in any surface number
- `TODO.md` row 4 listing P2.1/P4.1 open for a month after they shipped
- `INTEGRATION_GAPS.md` listing G6.8 open after hermes implemented it
- `README.md` describing a capsule knob that only the Rust door read

That is five instances of one defect class in one repo, found by one round of
external review. **It is the strongest evidence in this pack that the remaining
risk is not in any single module.**

---

## 2. The measured surface (re-derivable: `measure_surface.sh`)

| | claim | measured |
|---|---|---|
| `src/` size | AGENTS.md: **24.8k lines, 22 modules** | **35,298 lines, 32 modules** |
| AGENTS.md table | — | **18 rows** |
| modules absent from that table | — | **14**: `capture`, `connect_claude`, `connect_copilot`, `connect_ext`, `connect_opencode`, `grounding`, `guidelines`, `http`, `hygiene`, `lib`, `memory`, `skills`, `sweep`, `update` |
| `connect_*` family | 1 module in the table | **6,609 lines across 7 files** |
| TS plugins | not counted at all in round 3 | **741 lines, 98 identical** (59% of the smaller file) |
| inline `#[test]` blocks | — | **631** |

Line counts include inline test blocks, so a per-module count is a mix of code
and its own tests. That is deliberate and stated, not hidden: the repo keeps
tests in the same file (`AGENTS.md` §5), so "lines per module" is the unit a
reader of that module actually reads.

---

## 3. The open surface is five rows, and four of them are blocked on you

Verified from `TODO.md` on 2026-11-19:

| row | status | blocked on |
|---|---|---|
| 10 — agent-memory-bench Claude Code arm | open | agent-model budget |
| 11 — LLM-judged runs (AMB rag, OAMB, memorybench, MemBase) | open | budget **and** a consultation-budget conflict |
| 12 — AMB provider at turn granularity | open | **nothing — already decided by data** |
| 13 — aroute B1–B4 | open | another repo, another owner |
| 31 — door parity (redact/hygiene/supersede) | open | a surface decision |

Row 12 is worth naming precisely because it is the inverse of the round-3
finding: it is marked open, and its own row text says the decision was made.
Turn-level writes lose to shipped whole sessions on every aggregate
(`LOCOMO_TURNS.md`: R@1 54.6% vs 60.4%, `recall_all@10` 92.2% vs 94.2%, n
1,531), and windows breach the pool ceiling. **The register is stale in both
directions — it carries done work as open and open work as undecided.**

---

## 4. What the last pass exposed that nobody had recorded

Three findings from writing the *previous* plan, all verified, all new:

- **PersonaMem v2 ran to completion on 2026-10-17 and was never scored.**
  4,954 answers, 4,954 judged, 4,989 successful searches, 0 failures. The
  answers carry `is_correct`, so no judge re-run is needed. Scored now:
  **37.8% (1,872 / 4,954)**. `TODO.md` row 18 has said "named-not-scheduled"
  for two months. It is neither — it is run and unread. The directory is
  gitignored on purpose (the run wrote memory rows verbatim, one carrying a
  pasted bot token), so the raw rows cannot be committed under AGENTS §14.
- **`../_audit/` and `../_audit_scratch/` are both gone.** The pinned
  OmniMemEval clone (`a5c8b4b`) no longer exists on this box. H10 and H12 are
  reproducible in principle; re-running either means **re-cloning and
  re-pinning**, which is a different cost than DECISIONS §2 assumes.
- **`RND_PLAN.md` §6 still says "the upcoming cycle, outlined"** while W1–W3
  are all done (§47, §48, §46).

---

## 5. The provenance of every number here

| figure | class | where |
|---|---|---|
| 35,298 lines / 32 modules / 631 tests | `[LIVE]` | `measure_surface.sh`, run 2026-11-19 |
| 98 / 741 TS lines identical | `[LIVE]` | same script |
| 24.8k / 22 modules / 18 rows | `[CERT]` | `AGENTS.md`, quoted as the claim under audit |
| 37.8% PersonaMem v2 | `[LIVE]`-computed, **unrecorded** | `eval/omnimevaleval/memory-wire-mw_pmv2_dated/`, computed 2026-11-19 from committed run outputs; the directory is gitignored |
| 60.4% / 54.6% / 94.2% / 92.2% | `[CERT]` | `eval/LOCOMO.md`, `eval/LOCOMO_TURNS.md` |
| 659 tests, clippy clean, 0 doc warnings | `[LIVE]` | gate run 2026-11-19 at `3ca8857` |
| 500/500 gold in pool | `[CERT]` | `AGENTS.md` §1, from `eval/RESULTS.md` |
| 42 commits since v0.10.0 | `[LIVE]` | `git log v0.10.0..HEAD` |

**Not in this pack, deliberately:** any LongMemEval retrieval number as a
selection signal. Five consultations are spent; AGENTS §6 reserves the sixth for
release acceptance.

# SECTION C-ASK

# FILE: research/audits/context-pack-2026-11-19-round4/C-ASK.md (lines 1,82)

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

# SECTION D-SURFACE

# FILE: research/audits/context-pack-2026-11-19-round4/measure_surface.sh (lines 1,85)

#!/bin/sh
# Round-4 surface measurement. Every number in the pack comes from here, so the
# reviewer can re-derive any of them instead of trusting a claim.
#
# Run from the repo root:  sh research/audits/context-pack-2026-11-19-round4/measure_surface.sh
set -eu
cd "$(dirname "$0")/../../.."

echo "## src/ — authored Rust, per module (line counts include inline #[test] blocks)"
wc -l src/*.rs | sort -rn

echo
echo "## src/ — totals"
printf 'files: %s\n' "$(ls src/*.rs | wc -l | tr -d ' ')"
printf 'lines: %s\n' "$(cat src/*.rs | wc -l | tr -d ' ')"
printf 'inline #[test] blocks: %s\n' "$(grep -h '#\[test\]' src/*.rs | wc -l | tr -d ' ')"

echo
echo "## connect_* — the per-host adapters"
wc -l src/connect*.rs | sort -rn

echo
echo "## plugin/ — the injection path (runs inside a model's context)"
find plugin -name '*.ts' -not -path '*/node_modules/*' | xargs wc -l 2>/dev/null | tail -1
find plugin -name '*.py' -not -path '*/node_modules/*' | xargs wc -l 2>/dev/null | tail -1
echo "  files:"
find plugin -name '*.ts' -not -path '*/node_modules/*' -o -name '*.py' -not -path '*/node_modules/*' | sed 's/^/    /'

echo
echo "## TS plugin duplication — lines present verbatim in BOTH files"
A=plugin/integrations/extension/memory-wire.ts
B=plugin/integrations/opencode-plugin/memory-wire.ts
shared=$(comm -12 <(sort "$A") <(sort "$B") | grep -vE '^[[:space:]]*$|^[[:space:]]*//|^[[:space:]]*\*' | wc -l | tr -d ' ')
a_code=$(grep -vE '^[[:space:]]*$|^[[:space:]]*//|^[[:space:]]*\*' "$A" | wc -l | tr -d ' ')
b_code=$(grep -vE '^[[:space:]]*$|^[[:space:]]*//|^[[:space:]]*\*' "$B" | wc -l | tr -d ' ')
echo "  shared verbatim: $shared"
echo "  extension non-comment lines: $a_code"
echo "  opencode  non-comment lines: $b_code"

echo
echo "## HTTP routes (every .route() registration, nested routers included)"
grep -rhoE '\.route\("[^"]+"' src/*.rs | sed 's/.*route("//;s/"//' | sort -u

echo
echo "## MCP tools and prompts"
# The declared tool-name constants, not every string literal that looks like one:
# a bare grep also catches `memory_nope`, which is a test's unknown-tool probe.
grep -ohE 'pub const [A-Z_]+: &str = "memory_[a-z_]+"' src/mcp.rs \
  | sed 's/.*= "//;s/"//' | sort -u
echo "  (memory_historian is declared as a prompt, not a tool — see the table)"

echo
echo "## CLI subcommands"
sed -n '/^enum Cmd {/,/^}/p' src/main.rs | grep -E '^[[:space:]]{4}[A-Z][A-Za-z]+' | sed 's/[[:space:]]//g' | tr -d '{,'

echo
echo "## Agent hosts detected by connect"
grep -ohE '"(claude-code|codex|copilot-cli|cursor|opencode|hermes)"' src/connect*.rs | sort -u | tr -d '"'

echo
echo "## Authored docs"
printf 'docs/*.md count: %s\n' "$(ls docs/*.md 2>/dev/null | wc -l | tr -d ' ')"
wc -l AGENTS.md README.md TODO.md | tail -1

echo
echo "## AGENTS.md module table vs tree"
printf 'AGENTS.md claims: %s\n' "$(grep -oE '`src/`, [0-9.]+k lines across [0-9]+ modules' AGENTS.md)"
printf 'AGENTS.md table rows: %s\n' "$(grep -cE '^\| `[a-z_]+\.rs` \| [0-9]+ \|' AGENTS.md)"
printf 'actual modules: %s\n' "$(ls src/*.rs | wc -l | tr -d ' ')"
echo "  modules absent from the AGENTS.md table:"
for f in $(ls src/*.rs | xargs -n1 basename); do
  grep -q "| \`$f\` |" AGENTS.md || echo "    $f"
done

echo
echo "## Gate status"
printf 'cargo test (release-mode harness): %s\n' "see docs/CONSISTENCY.md"

echo
echo "## Dependency count (the default build must not gain one)"
# Count only the [dependencies] block's own entries: the range above runs to the
# next `[` header, which on this manifest is a feature table full of `x = [...]`
# lines that would otherwise be counted as crates.
awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f && /^[a-zA-Z0-9_-]+ *=/' Cargo.toml | wc -l | tr -d ' '
echo "  (the [features] block below is not dependencies)"

# SECTION E-ENFORCE

# FILE: plugin/integrations/hermes/memory-wire/__init__.py (lines 86,569)

# Entries and characters one prefetch may inject — the same ceilings every
# auto-injection surface enforces (docs/RETRIEVAL_EFFICACY_AUDIT.md GAP-1),
# so a Hermes block and an extension block cost the same context.
MAX_LINES = 5
ENTRY_CAP_CHARS = 400
SECTION_CAP_CHARS = 2000

# The one trust tier the auto-injection path refuses.
#
# A memory ingested from a web page, a PR comment or an email body lands as
# ``untrusted_external``. Ranking it is not the problem: fetched text can be
# shaped to rank first, which is what makes it a prompt-injection vector. The
# problem is that this function's output reaches a model *without anyone asking
# for it*, so this is the boundary where the row must be stopped.
#
# This is not a relevance threshold and must not become one. A cut on the score
# is a fitted constant (docs/OPEN_HOOK_RECALL_RELEVANCE.md, and CONSISTENCY
# §32/§48: no score threshold, ever); this is a deterministic allow/deny on a
# stored tier, with no number to fit. The string is the server's canonical form
# for ``Trust.UntrustedExternal`` (``src/api.rs``).
#
# Only a positive match drops a row. A missing ``trust`` is the bare wire shape
# or a server too old to send it, and dropping on unknown would turn a missing
# field into a silent recall outage.
UNTRUSTED_TIER = "untrusted_external"

# An answer shorter than this is an "OK", not a turn worth remembering — the
# same rule the omp/pi extension applies (audit GAP-6, and the 943
# prompt-pair rows a bank filled with before it).
MIN_ANSWER_CHARS = 32

# Characters of the user half one sync_turn retain keeps. The question is the
# retrieval key; the tail-cut this replaces could drop it whole (audit GAP-6).
USER_CAP_CHARS = 600

# Tags whose rows are never auto-injected: imported raw transcripts and bare
# session markers are corpus, not context for a turn in flight
# (docs/RETRIEVAL_EFFICACY_AUDIT.md GAP-3/7). Optional on the server too, so a
# plugin newer than its server degrades to today's behavior.
EXCLUDE_TAGS = ("transcript", "marker")

# Characters of prose one retain may carry. Same literal as `FLUSH_CHARS` in
# `src/hooks.rs`, for the same reason: a seeded and a flushed conversation
# should cost the same to store.
FLUSH_CHARS = 2000

# The historian preamble, verbatim from `REFLECT_SYSTEM_PROMPT` in `src/api.rs`.
# Static by contract -- `MemoryProvider.system_prompt_block` is documented as
# "STATIC provider info (instructions, status)" -- so this costs no network call
# on the system-prompt assembly path.
HISTORIAN_PREAMBLE = (
    "You are a historian reading a memory bank's record of past decisions.\n"
    "\n"
    "Report decisions and rationale, never the current implementation.\n"
    "\n"
    "- Write declarative past-tense prose: what was decided, and on what reasoning.\n"
    "- Never issue instructions or recommendations. Do not write \"you should\",\n"
    '  "remove", "update", "switch to", "add", or any other imperative.\n'
    "- Reproduce literal tables, identifiers, and numbers verbatim. Do not\n"
    "  summarize, round, or paraphrase a recorded figure.\n"
    "- Treat every entry as history. If a decision was later superseded, say it was\n"
    "  superseded — do not restate it as what the code does now.\n"
)

_memory_write_warned = False


# ---------------------------------------------------------------------------
# HTTP
# ---------------------------------------------------------------------------


def _endpoint() -> str:
    """Server URL: ``MEMORY_WIRE_URL``, then the saved config, then the default."""
    url = os.environ.get("MEMORY_WIRE_URL", "").strip()
    if url:
        return url
    cfg = _config()
    url = str(cfg.get("url", "") or "").strip()
    return url or DEFAULT_ENDPOINT


def _bank() -> str:
    """Bank id: ``MEMORY_WIRE_BANK``, then the saved config, then the default."""
    bank = os.environ.get("MEMORY_WIRE_BANK", "").strip()
    if bank:
        return bank
    bank = str(_config().get("bank", "") or "").strip()
    return bank or DEFAULT_BANK


def _config() -> Dict[str, Any]:
    """The plugin's saved config, or ``{}``.

    Read per call rather than cached at import: ``save_config`` runs in a
    different process from the agent loop, and a cached copy would make
    ``hermes memory setup`` look like it had done nothing.
    """
    home = os.environ.get("HERMES_HOME", "").strip() or str(Path.home() / ".hermes")
    try:
        raw = (Path(home) / "memory-wire.json").read_text(encoding="utf-8")
        doc = json.loads(raw)
    except (OSError, ValueError):
        return {}
    return doc if isinstance(doc, dict) else {}


def _valid_url(base: str) -> bool:
    """A URL we can build request paths from."""
    if not base:
        return False
    try:
        parsed = urlparse(base)
        # ``.port`` raises ValueError on a non-numeric or out-of-range port.
        _ = parsed.port
    except ValueError:
        return False
    return parsed.scheme in ("http", "https") and bool(parsed.hostname)


def _request(
    base: str,
    path: str,
    body: Optional[Dict[str, Any]] = None,
    method: str = "GET",
) -> Optional[Any]:
    """One JSON call, or ``None`` on any failure whatsoever.

    Every caller treats ``None`` as "say nothing". That is deliberate and it is
    the whole error contract: a memory server that is down must never put an
    error in front of a model, and ``MemoryManager`` catches provider exceptions
    anyway -- but a caught exception still costs a traceback in the log on every
    turn, and a silent ``None`` costs nothing.
    """
    if not _valid_url(base):
        return None
    data = json.dumps(body).encode("utf-8") if body is not None else None
    headers = {"Content-Type": "application/json"} if data else {}
    req = urllib.request.Request(
        f"{base}{path}", data=data, headers=headers, method=method
    )
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except (urllib.error.URLError, OSError, ValueError, TimeoutError):
        return None


def _retain(base: str, bank: str, content: str, tag: str, context: str = "", document_id: str = "") -> bool:
    """Store one memory. Returns whether the server accepted it.

    ``document_id`` is the turn-dedupe key: a repeated ask replaces its row
    instead of piling up twin questions (audit GAP-6).
    """
    if not content.strip():
        return False
    body: Dict[str, Any] = {"content": content, "context": context, "tags": [tag]}
    if document_id:
        body["document_id"] = document_id
    out = _request(
        base,
        f"/banks/{bank}/retain",
        body,
        method="POST",
    )
    return isinstance(out, dict)


def _recall(
    base: str,
    bank: str,
    query: str,
    exclude: Sequence[str] = (),
) -> Optional[List[Dict[str, Any]]]:
    """Memories for *query* as ``{"content", "score", "trust", "created_at"}``
    dicts, or ``None`` when the
    server is unreachable.

    ``None`` and ``[]`` mean different things on purpose: the first is "no
    answer available", the second is "the answer is that there is nothing".
    ``prefetch`` collapses them, because a model cannot act on that difference.

    ``format: "full"`` is what puts ``score`` on the wire; without it the route
    serves a bare array of content strings and there is no score to carry. The
    bare shape is still accepted, because ``format`` is opt-in and a plugin can
    outlive the server build it was installed against.

    ``score`` is the fused RRF value the ranking used, and it is *relative*: the
    top hit of a set that has nothing to do with the query scores well by
    construction, because BM25 is min-max normalised per query. It is carried so
    a relevance decision is possible; nothing branches on it, because a cut on
    this number is a fitted constant and no dev set exists to choose one. See
    ``docs/OPEN_HOOK_RECALL_RELEVANCE.md``.

    ``trust`` rides along for the opposite reason: it is not a ranking input, it
    is what :attr:`MemoryProvider.prefetch` uses to refuse a row. A missing
    ``trust`` stays ``None`` rather than becoming a guess. Same rule for
    ``created_at``: present only when the wire carried it, ``None`` rather than
    a guess, and prefetch renders it as a prefix only when it is there.
    """
    if not query.strip():
        return None
    # `MEMORY_WIRE_FORMAT=summary` asks for H8 capsules instead of whole
    # memories. It is read from the environment rather than from the bank's
    # `injectionFormat` config key because the plugins already take
    # `MEMORY_WIRE_URL` and `MEMORY_WIRE_BANK` from there, so this costs no
    # extra request -- whereas reading the config key would put a second HTTP
    # call on every turn, which `src/hooks.rs` documents as already costing one.
    # The Rust hook reads both; the bank wins there. Anything other than
    # "summary" reads as the default, so an unknown value is not an error.
    fmt = os.environ.get("MEMORY_WIRE_FORMAT", "").strip().lower()
    body: Dict[str, Any] = {
        "query": query,
        "budget": BUDGET,
        "format": "summary" if fmt == "summary" else "full",
    }
    if exclude:
        # Optional on the server: a plugin newer than its server degrades to
        # the unfiltered ranking rather than erroring.
        body["exclude_tags"] = list(exclude)
    out = _request(
        base,
        f"/banks/{bank}/recall",
        body,
        method="POST",
    )
    if not isinstance(out, list):
        return None
    hits = []
    for hit in out:
        # A scored entry is an object; a bare one is the content itself.
        content = hit if isinstance(hit, str) else (hit.get("content") if isinstance(hit, dict) else None)
        if not isinstance(content, str) or not content.strip():
            continue
        score = hit.get("score") if isinstance(hit, dict) else None
        trust = hit.get("trust") if isinstance(hit, dict) else None
        # The occurrence time, when the wire carried one. `prefetch` prefixes
        # it, the way `src/hooks.rs::recall_section` does: H12 measured the
        # date prefix as the single largest serving-shape lever this project
        # has (≈+13pp on the dated HaluMem instrument), and a reader resolving
        # "what did we decide last week" is blind without it. It is carried
        # here and not in the content because the content is what the store
        # stored; the date is when it happened.
        created_at = hit.get("created_at") if isinstance(hit, dict) else None
        hits.append({
            "content": content,
            "score": score if isinstance(score, float) else None,
            "trust": trust if isinstance(trust, str) else None,
            "created_at": created_at if isinstance(created_at, str) else None,
        })
    return hits


# ---------------------------------------------------------------------------
# Prompt hygiene
# ---------------------------------------------------------------------------


_REMINDER_RE = re.compile(
    r"<system-(?:reminder|notice)>.*?</system-(?:reminder|notice)>", re.DOTALL
)


def _strip_reminders(text: str) -> str:
    """Drop harness reminder/notice envelopes — the host talking to the model,
    not the user's words. Retained as prompt text they outranked real
    questions; in a query they are lexical noise (audit GAP-2)."""
    return _REMINDER_RE.sub("", text or "").strip()


def _clamp(text: str, max_chars: int) -> str:
    """Collapse to one line and cap — the extension's ``clamp``, so every
    surface shapes an entry identically."""
    flat = " ".join((text or "").split())
    if len(flat) <= max_chars:
        return flat
    return flat[: max_chars - 1] + "…"


def _djb36(text: str) -> str:
    """djb2 over code points, base-36 — the same digest the extension's
    ``hash`` produces, so a turn retained here and one retained there key
    identically."""
    h = 5381
    for ch in text or "":
        h = ((h << 5) + h + ord(ch)) & 0xFFFFFFFF
    digits = ""
    while True:
        digits = "0123456789abcdefghijklmnopqrstuvwxyz"[h % 36] + digits
        h //= 36
        if h == 0:
            break
    return digits


# ---------------------------------------------------------------------------
# Transcript prose
# ---------------------------------------------------------------------------


def _text_of(content: Any) -> str:
    """The text of one message's ``content``, whatever shape it arrived in.

    A Hermes turn is usually a plain string. It can also be the OpenAI block
    list -- ``[{"type": "text", "text": ...}, {"type": "tool_use", ...}]`` -- and
    the tool blocks are not prose worth storing, so they are skipped rather
    than stringified.
    """
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = [
            block.get("text", "")
            for block in content
            if isinstance(block, dict) and block.get("type") == "text"
        ]
        return "\n".join(p for p in parts if isinstance(p, str))
    return ""


def _prose(messages: List[Dict[str, Any]]) -> str:
    """A conversation's own words, from a Hermes message list.

    This is the plugin's whole answer to "where is the transcript?": there isn't
    one to read. ``on_pre_compress`` and ``on_session_end`` are handed the
    message list in-process, so the conversation is already in memory and the
    words never have to survive a round trip through a file. That is strictly
    better than the file-based hooks, and it is why the 128 KiB tail window in
    `src/hooks.rs` has no equivalent here.

    The last :data:`FLUSH_CHARS` characters are kept, matching
    ``transcript_tail``: the recent turns are the ones a later session needs.
    """
    lines = []
    for message in messages or []:
        if not isinstance(message, dict):
            continue
        text = _strip_reminders(_text_of(message.get("content"))).strip()
        if not text:
            continue
        role = str(message.get("role", "") or "message")
        lines.append(f"{role}: {text}")
    joined = "\n".join(lines)
    return joined[-FLUSH_CHARS:]


# ---------------------------------------------------------------------------
# Provider
# ---------------------------------------------------------------------------


class MemoryWireProvider(MemoryProvider):
    """memory-wire as a Hermes memory provider."""

    @property
    def name(self) -> str:
        return "memory-wire"

    def is_available(self) -> bool:
        """Whether this provider could serve a session. No network call.

        Hermes calls this during agent init and inside
        ``discover_memory_providers()``, which runs for *every* candidate
        plugin on the box -- a probe here would put a 2s timeout in front of
        starting the agent. So this answers the question it can answer without
        the network: is there a URL to talk to, and is it one we can build a
        path from. A server that is down still reports available, and the hooks
        then say nothing, which is the documented never-fail contract.
        """
        return _valid_url(_endpoint())

    def initialize(self, session_id: str, **kwargs: Any) -> None:
        """Bind per-session state. No network call, and no failure mode.

        Hermes passes ``hermes_home`` and ``platform`` in every case, and may
        pass ``agent_context`` (``primary`` / ``subagent`` / ``cron`` / ``flush``)
        for some. The context matters: a cron employee replaying a system prompt
        would otherwise write the system prompt into a user representation, which
        the base class warns about by name, so writes are skipped for anything
        that is not the primary agent.
        """
        self._session_id = session_id
        self._base = _endpoint()
        self._bank = _bank()
        self._home = kwargs.get("hermes_home") or _hermes_home()
        self._platform = kwargs.get("platform", "")
        self._context = kwargs.get("agent_context", "primary")
        self._identity = kwargs.get("agent_identity", "")
        self._cwd = kwargs.get("cwd", "")
        self._started = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    def _writable(self) -> bool:
        """Whether this session should write, per Hermes' own context rule."""
        return self._context in ("primary", "")

    # -- lifecycle -----------------------------------------------------------

    def system_prompt_block(self) -> str:
        """The historian framing, plus which bank this session is bound to.

        Static, per the base class: "This is for STATIC provider info
        (instructions, status)." ``MemoryManager.build_system_prompt`` calls
        this inline while assembling the system prompt, so a network call here
        would sit on the critical path of every session start.

        Note the base class also says prefetched recall is injected *separately*
        -- through ``prefetch`` -- so this block is instructions, not content,
        and must not try to smuggle recalled memories in through it.
        """
        if not self._base:
            self._base = _endpoint()
        return (
            f"## memory-wire — bank `{self._bank}`\n\n"
            f"{HISTORIAN_PREAMBLE}\n"
            "Memories in this bank are read with `memory_recall`, written with\n"
            "`memory_retain`, and cited with `memory_reflect`. The store is local\n"
            f"and unauthenticated; it is served at `{self._base}`.\n"
        )

    def prefetch(self, query: str, *, session_id: str = "") -> str:
        """Recall for the upcoming turn, as injected context.

        The signature is keyword-only on ``session_id`` because that is what
        ``MemoryManager._prefetch_provider`` calls
        (``provider.prefetch(query, session_id=session_id)``). A provider that
        took it positionally would be called correctly and then be wrong about
        which argument it received.

        Returns "" for a blank query, an unreachable server, an empty bank, and a
        server that errored. Hermes wraps this in its own thread with an 8s
        deadline and logs a warning if we exceed it, so one TIMEOUT is the whole
        cost of a down server.

        The top ``MAX_LINES`` are injected. Each carries its fused RRF ``score``
        — see :func:`_recall` for what it is and why nothing branches on it yet
        — and the ranking is *relative*, so this emits the best of whatever set
        the bank offered, relevant or not. That is the defect
        ``docs/OPEN_HOOK_RECALL_RELEVANCE.md`` records; the score now reaches
        this function so the policy half is possible, and the policy half needs a
        labelled dev set this repo does not have.

        One thing *is* filtered, and it is not relevance: rows marked
        :data:`UNTRUSTED_TIER` are dropped, because this function's output is
        how a recalled memory reaches a model without anyone asking for it. An
        explicit ``memory_recall`` still returns those rows. See
        :data:`UNTRUSTED_TIER` for why this is a boundary and not a ranking rule.
        """
        if not _valid_url(self._base):
            return ""
        hits = _recall(self._base, self._bank, _strip_reminders(query), EXCLUDE_TAGS)
        if not hits:
            return ""
        # Before the take, not after: a poisoned row at rank 0 must not consume
        # a slot a trusted row would have filled. Ranking untrusted text first
        # is exactly the adversarial case this exists for.
        hits = [h for h in hits if h.get("trust") != UNTRUSTED_TIER]
        if not hits:
            return ""
        # The shape every auto-injection surface emits (audit GAP-1): the bank
        # header, the provenance label — a best-of-irrelevant set must not read
        # as a verified one — and at most MAX_LINES entries, each clamped, the
        # whole block under SECTION_CAP_CHARS.
        out = (
            f"### memory-wire recall — `{self._bank}`\n"
            "(recalled by relevance, unverified)\n\n"
        )
        left = SECTION_CAP_CHARS
        for hit in hits[:MAX_LINES]:
            # The date prefix renders only when the wire carried one, so a
            # server older than the field degrades to the exact pre-2026-11-19
            # rendering, byte for byte. Same shape as `src/hooks.rs`:
            # `[<YYYY-MM-DD>] ` before the clamped content, day precision
            # because that is what an occurrence time is for.
            stamp = hit.get("created_at") or ""
            prefix = f"[{stamp[:10]}] " if len(stamp) >= 10 else ""
            entry = f"- {prefix}{_clamp(hit['content'], ENTRY_CAP_CHARS)}\n"
            if len(entry) > left:
                break
            out += entry
            left -= len(entry)
        if len(hits) > MAX_LINES:
            out += f"- …and {len(hits) - MAX_LINES} more\n"
        return out


# FILE: plugin/integrations/extension/memory-wire.ts (lines 205,351)

var UNTRUSTED_TIER = "untrusted_external";

/** A row a mount may not read back as instructions. Absent means keep. */
function isUntrusted(item) {
  if (!item || typeof item !== "object") return false;
  return item.trust === UNTRUSTED_TIER;
}

/** The block appended to the system prompt, or `""` when there is nothing worth
 *  saying. Empty is the signal the caller uses to inject nothing at all.
 *
 *  `items` are the server's `{id, score, content}` entries, so each carries the
 *  fused RRF score the ranking used. It is *relative* — the top hit of an
 *  irrelevant set scores well by construction, because BM25 is min-max
 *  normalised per query — and nothing branches on it here, because a cut on that
 *  number is a fitted constant and no dev set exists to choose one. The top
 *  `RECALL_ENTRIES` are injected unconditionally either way; see
 *  `docs/OPEN_HOOK_RECALL_RELEVANCE.md`. */
function formatRecall(items) {
  var lines = [];
  for (var i = 0; i < items.length && i < RECALL_ENTRIES; i++) {
    var item = items[i];
    // The default recall response is a bare array of content strings; a
    // `format: "full"` server answers with `{id, score, content}` instead. Both
    // are accepted so this file does not depend on which one it is talking to.
    var content = typeof item === "string" ? item : item && item.content;
    if (typeof content !== "string" || !content.trim()) continue;
    // Filtered **before** the take: keeping the slot and dropping a trusted row
    // would inject four memories where five were earned. It happens inside the
    // loop rather than as a separate pass because `RECALL_ENTRIES` counts
    // emitted lines, and a pre-pass that dropped rows would leave the count
    // short.
    if (isUntrusted(item)) continue;
    // The occurrence time, when the wire carried one. `src/hooks.rs` prefixes
    // it for the same reason H12 measured: a reader resolving "what did we
    // decide last week" is blind without it. It renders only when the field is
    // there, so a server older than it degrades to the exact pre-2026-11-19
    // rendering, byte for byte. Day precision -- that is what an occurrence
    // time is for.
    var stamp = (item && item.created_at) || "";
    var datePrefix = stamp.length >= 10 ? "[" + stamp.slice(0, 10) + "] " : "";
    lines.push(lines.length + 1 + ". " + datePrefix + clamp(content, ENTRY_CAP_CHARS));
  }
  if (!lines.length) return "";
  var block =
    "## memory-wire long-term memory\n" +
    "(recalled by relevance from bank `" + bank() + "`, unverified)\n" +
    lines.join("\n");
  return block.length <= INJECT_CAP_CHARS ? block : block.slice(0, INJECT_CAP_CHARS - 1) + "…";
}

/** memory-wire's memory extension: recall injected before each turn, the turn
 *  retained after it.
 *  @param {{ on: (event: string, handler: Function) => void }} pi */
export default function memoryWireExtension(pi) {
  var lastPrompt = "";
  var lastAnswer = "";
  var lastRetained = "";

  pi.on("before_agent_start", async function (event) {
    var prompt = event && typeof event.prompt === "string" ? stripReminders(event.prompt) : "";
    if (!prompt) return undefined;
    // Recorded before the await, so a recall that fails still leaves the prompt
    // available to `agent_end`.
    lastPrompt = prompt;

    // A prompt this short cannot rank anything on its own: "Continue" matches
    // every memory equally and the top of that tie is noise. The query borrows
    // the previous turn's answer, so what ranks is the work being continued,
    // with the prompt still steering inside it. A mechanism, not a filter --
    // nothing is cut on a score (docs/OPEN_HOOK_RECALL_RELEVANCE.md).
    var query = prompt;
    if (prompt.length <= SHORT_PROMPT_CHARS && lastAnswer) {
      query = clamp(lastAnswer, QUERY_CONTEXT_CHARS) + "\n" + prompt;
    }

    var items = await post(
      "/banks/" + encodeURIComponent(bank()) + "/recall",
      // `format: "full"` is what puts `score` on the wire; the default shape is a
      // bare array of content strings. `formatRecall` reads both, so this stays
      // compatible with a server that does not know the field.
      // `MEMORY_WIRE_FORMAT=summary` asks for H8 capsules instead of whole
      // memories. It comes from the environment rather than the bank's
      // `injectionFormat` config key because this file already reads
      // `MEMORY_WIRE_URL` and `MEMORY_WIRE_BANK` from there, so it costs no
      // extra request; reading the config key would add one per turn, which
      // `src/hooks.rs` documents as already costing one. The Rust hook reads
      // both and the bank wins there. Anything but "summary" reads as default.
      {
        query: query,
        budget: RECALL_BUDGET,
        format:
          (process.env.MEMORY_WIRE_FORMAT || "").trim().toLowerCase() === "summary"
            ? "summary"
            : "full",
        exclude_tags: EXCLUDE_TAGS,
      },
      RECALL_TIMEOUT_MS
    );
    if (!Array.isArray(items)) return undefined;
    var block = formatRecall(items);
    if (!block) return undefined;

    // Returning `systemPrompt` REPLACES it, so the harness's own prompt has to be
    // carried through or its instructions are silently erased. A host may hand
    // it over as a string or as a list of segments; both are joined rather than
    // stringified, which would collapse a list into one comma-riddled line.
    var own = event.systemPrompt;
    if (Array.isArray(own)) own = own.join("\n\n");
    return { systemPrompt: [own, block].filter(Boolean).join("\n\n") };
  });

  pi.on("agent_end", async function (event) {
    var prompt = lastPrompt;
    // Cleared before the await: a turn that starts before this one finishes must
    // not have its prompt consumed by the earlier retain.
    lastPrompt = "";
    var answer = lastAssistantText(event && event.messages);
    // The next turn's short-prompt query composition borrows this answer.
    lastAnswer = answer;
    // An unanswered or bare-acknowledgement turn is a question with no knowledge
    // in it: retaining prompt-only rows is how the bank filled with duplicate
    // question texts ("asked: continue", asked three times over) that recall
    // then injected as if they were memory. The pair is the memory.
    if (!prompt || !answer || answer.length < MIN_ANSWER_CHARS) return undefined;

    var content = [
      "asked: " + clamp(prompt, RETAIN_PART_CAP_CHARS),
      "answered: " + clamp(answer, RETAIN_PART_CAP_CHARS),
    ].join("\n");
    // An auto-retry re-submits the same turn, and the server dedupes identical
    // content anyway -- but not before paying for the write, so it is checked here.
    if (!content || content === lastRetained) return undefined;

    // `document_id` keyed on the prompt makes a repeated ask replace its row
    // (the server's default update mode) rather than add a twin.
    var stored = await post(
      "/banks/" + encodeURIComponent(bank()) + "/retain",
      { content: content, document_id: "turn-" + hash(prompt) },
      RETAIN_TIMEOUT_MS
    );
    // Only a confirmed store updates the marker, so a retain that failed is
    // retried on the next turn rather than dropped for the rest of the session.
    if (stored) lastRetained = content;
    return undefined;
  });
}

# SECTION F-LAW

# FILE: AGENTS.md (lines 186,267)

## 5. Rust discipline

The `rust-best-practices` skill (`~/.agents/skills/rust-best-practices`, based on
Apollo's handbook) is the house style. **Read the relevant chapter before
reviewing or writing Rust** — the chapters are in `references/` and are worth
more than the quick-reference summary at the bottom of the skill file.

The rules this repo actually enforces beyond the compiler:

- `deny(missing_docs)` is on. Public items need `///` docs.
- No `unwrap()` in non-test code. `expect()` needs a reason naming the
  invariant. `unwrap()` under `#[cfg(test)]` is fine.
- No new dependency without first saying what the standard library or an
  already-present dependency cannot do. See §8.
- Blocking SQLite work must not run on a tokio worker. `src/store.rs` uses
  `spawn_blocking` for this; do not add a blocking call to an async path.
- Comments explain **why**; `///` docs explain **what**. A comment restating the
  code is a finding.
- A `ponytail:` comment marks a deliberate simplification with a named ceiling
  and an upgrade path. That is an accepted pattern, not debt to remove.

## 6. The evaluation set is not a training set

This is the rule that matters most here, and the one most easily broken by
accident.

`eval/RESULTS.md` holds LongMemEval-S results over 500 questions. Those 500
questions are an **instrument for measuring**, never an objective to optimise
against.

**Never do any of the following and then describe the result as an improvement:**

- sweep a parameter, weight, threshold, or feature configuration against the
  LongMemEval numbers and keep the winner
- add, remove, or reorder a retrieval signal because it moved R@5 / R@10 /
  NDCG@10 on LongMemEval
- re-run a change, observe a delta, and iterate until the delta is positive
- quote a LongMemEval number as evidence that a component is "better" when the
  component was chosen by looking at that number

**A change selected by looking at the evaluation set is fitted, not earned.**
When that has happened — and it has — say so in the commit message and in
`docs/CONSISTENCY.md`, mark the value provisional, and treat re-validation on
independent data as outstanding work.

| Allowed | Not allowed |
|---|---|
| Measuring on LongMemEval and reporting the number | Choosing a value by comparing LongMemEval numbers |
| Fixing a bug that LongMemEval exposed | Tuning until a metric stops complaining |
| Deciding a change on a mechanistic argument, then *observing* the metric | Deciding a change on the metric |
| Selecting against a **development** set that is not LongMemEval | Using the test partition as a dev set |
| Reporting that a change did not help | Reverting a change because a benchmark "regressed" while keeping an equal-or-worse alternative that scored better |

### The three-set discipline

- **dev** — a labelled set that is *not* LongMemEval. All selection happens here.
  `eval/data/locomo/` is the dev corpus (independent source, ~1530 usable
  queries, gold document ids). See `docs/EVALUATION_HYGIENE.md`.
- **test** — LongMemEval-S. Consulted for **measurement and release acceptance
  only**. Never for selection.
- **synthetic** — `examples/bench_recall_curve.rs` and `tests/scale.rs` generate
  their own corpora with fixed seeds. Useful for scaling and regression
  assertions, but we author the distractors, so it is weak evidence about
  ranking quality. Do not tune against it either.
- **measure-only** — every other external set (PrecisionMemBench, BEAM,
  agent-memory-bench, …). Same rule as test: report, never select. Any harness
  that runs LongMemEval (OAMB, MemBase, memorybench) *is* a test consultation.
  Roles are assigned in `docs/BENCHMARK_LANDSCAPE.md` before a set's first number.

**A constant changed in response to a bug is derived from mechanism, then
measured exactly once.** Never iterate until a metric improves — that yields a
better number and a worthless one. `DEFAULT_RECALL_BUDGET` is the worked example:
derived as 3× the measured median LongMemEval session, measured once, and the
result reported whether it moved or not.

### Count the consultations

Every change that is *selected* on the test set burns it. `docs/CONSISTENCY.md`
records the count. If a component's parameters were fitted on LongMemEval, it
carries a `provisional:` marker in the code and in the docs until an independent
set confirms it.


# FILE: AGENTS.md (lines 286,297)

## 8. Rejected work stays rejected

Levers already measured and removed, with the measurement that killed each, are
listed in `docs/RERANKING_PLAN.md` §3 and `docs/PERFORMANCE_PLAN.md`. Do not
re-propose one without new evidence. If you find yourself re-litigating a
rejected lever, the answer is in those documents, not in your intuition.

Currently closed, each with its measurement recorded: vectors as a default,
a cross-encoder, a graph stream, consolidation and extraction, Porter stemming,
IDF in the overlap stream, a distinct-term-coverage stream, page-cache and
mmap tuning, and the token-overlap stream's fitted weight.


# SECTION G-OPEN

# FILE: docs/handoff/REMAINING_WORK_PLAN.md (lines 70,232)

## 2. Blocked: the decisions that are yours

Each row: what it costs, what it buys, whether it is reversible, and what I
recommend. **Nothing below moves without you.**

### 2.1 Door parity (TODO row 31) — a design ruling, not a bug

**The actual asymmetry today**, verified against the route table and tool list:

| capability | HTTP | MCP | CLI | hook |
|---|---|---|---|---|
| redact | no route | no tool | yes | no |
| hygiene | `GET /banks/:id/hygiene` | no | no | no |
| supersede | no route | `memory_supersede` | no | no |
| config get | yes | `memory_bank_config_get` | yes | yes (probe) |

**Why it is not simply "add the missing doors."** `memory_redact` on MCP is
contested on its merits, not on effort: redaction is a *custody* operation over a
store, and exposing it to any agent that can reach the MCP port means any agent
that can reach the port can rewrite history. Today the same limitation applies to
HTTP `DELETE /banks/:id/memories/:mid`, which already exists — so this is a
consistency question, not a new capability. The honest options:

- **(a) Do nothing; document the asymmetry as deliberate.** Cost: one doc pass.
  Buys: nothing changes, and a reader stops assuming parity. Reversible.
- **(b) Add `memory_redact` to MCP.** Cost: a tool plus tests plus a doc entry.
  Buys: an agent that retained a secret through MCP can retract it through MCP.
  Reversible. **This is the one I'd pick** — the capability gap is real and the
  risk profile already exists on HTTP.
- **(c) Add hygiene to CLI/MCP, supersede to HTTP.** Cost: two more surfaces.
  Buys: completeness that nothing has asked for. Reversible. Lower value than (b).

**Recommendation: (a) + (b).** Document the ruling, close the widest gap.

### 2.2 aroute (TODO row 13) — not my repo, and the fix is yours

Four defects, all in `agentic-utility/proxy/artificial-route`, all handed off in
`docs/AROUTE_BUGS.md` (202 lines, with a verification recipe at the end):

| | defect | effect on us |
|---|---|---|
| B1 | upstream failure → HTTP 200 with `{"error"}` body, logged `outcome=ok`, no failover | every OpenAI client misreads a failure as an answer |
| B2 | cline replies forwarded inside `{"success","data"}` envelope | clients must unwrap; the shim does |
| B3 | cline token/cost headers read 0 | spend limits silently miss cline |
| B4 | permanent 403 gets `cooldown_ms=0` and is retried every request | wasted calls on a dead provider |

**What it costs to leave it:** `eval/llm_shim.py` exists solely to paper over
B1/B2. Every LLM-judged run routes through it. Fixing aroute deletes the shim and
speeds up all future judged runs.

**What it costs to fix:** their repo, their service, their call — AGENTS is
explicit that memory-wire agents do not edit aroute or restart its service.

**What you'd need to do:** hand `docs/AROUTE_BUGS.md` to whoever owns aroute, then
re-run one short shim job; the doc names the exact log fields that prove each fix
(`attempts > 1` = B1, `enveloped: true` = B2, zero `tokens_in` = B3).

**Recommendation: treat as a separate piece of work, not part of this plan.**
Blocking on it costs nothing today; it only gates *speed* of future runs.

### 2.3 agent-memory-bench (TODO row 10) — needs a model budget, and it is the highest-value one

**What it is:** GiulioDER's agent-memory-bench, the only external benchmark that
grades what this product actually does — hooks and MCP wired end-to-end, task
success, plus a harm suite (superseded / contradictory / adjacent).

**Why it is different from everything else in the battery:** every other number
we publish measures retrieval on a corpus we injected. This one measures whether
an agent using memory-wire through its real interface gets the task right. It is
the answer to "does any of this help a user".

**What it costs:** an adapter (`adapters/memory_wire/` in their repo, wiring
`memory-wire connect claude-code`), plus **agent-model tokens** — a real spend, on
your account, against a frontier model. No free tier for the agent arm.

**What it buys:** the product question, and a harm-suite read that no other
instrument provides.

**Reversible?** The measurement is repeatable; the spend is not recoverable, but
it is bounded by the number of tasks you choose to run.

**Recommendation: fund it, but only after §2.1(b).** DECISIONS §4 already says
"schedule after recommendation 1 lands so it measures the improved product, not
the old one" — and recommendation 1 (dates in the surfaces, capsules) shipped in
`0b518de` and `1f93bf0`. So the precondition is now met: the product it would
measure is the improved one.

**What I need from you:** the budget ceiling, and which agent model. Without a
number I cannot even scope the adapter.

### 2.4 LLM-judged runs (TODO row 11) — needs budget and touches the test set

Four corpora: AMB `rag` on BEAM/PersonaMem/LifeBench, OAMB LME-60,
memorybench ConvoMem, MemBase RealMem/MobileMem.

**The split that matters, and it is a law question rather than a preference:**

| run | role | consequence |
|---|---|---|
| AMB `rag` on BEAM / PersonaMem / LifeBench | measure-only | may report, never select |
| **OAMB LME-60** | **test consultation** | burns a LongMemEval consultation |
| MemBase / memorybench arms that embed LME | test consultation | same |

**The consultation counter is at 5.** AGENTS §6 makes the 6th release-acceptance
only, and DECISIONS §5 says release only when a ratified decision ships. So an
OAMB run now would consume the release-acceptance slot before there is a release
to accept. That is the real cost, and it is invisible in the row text.

**What it buys:** OAMB's answer-visible-token axis is where a budget-trimmed
recall should show up — the one axis none of our instruments measure. The others
are new corpora (MemBase, memorybench) with no other evidence.

**Recommendation: fund AMB `rag` + MemBase/memorybench (measure-only, no
consultation); hold OAMB until a release is actually being cut.** That keeps the
consultation budget intact for its stated purpose.

### 2.5 TODO row 12 — turn-granularity writes: measured, and I recommend closing it

The row is still `open`, but the decision it asks for has already been made by
data:

| arm | R@1 | R@5 | `recall_all@10` | n |
|---|---|---|---|---|
| whole sessions, `overlap 0.25` (shipped) | **60.4%** | 86.0% | 94.2% | 1,531 |
| turns pooled (`locomo --turns`) | 54.6% | 83.0% | 92.2% | 1,531 |
| windows W=2 | 59.3% | — | 82.6% | — |

Turn-level loses on every aggregate; windows lose on `recall_all@10` badly enough
to breach the pool ceiling. The row's own text says it "still buys measurability
of the long corpora at a ranking cost, and is not a ranking lever."

**Recommendation: strike it as rejected, keeping the row as the record.** That is
AGENTS §8's "rejected work stays rejected" applied to an unstruck row. No new
decision needed from you unless you want it kept open for the long-corpus
measurability reason.

### 2.6 Release (DECISIONS §5) — the one with a deadline shape

**42 commits since `v0.10.0`**, including four product-visible changes:
the trust gate reaching two doors that lacked it (`98b1a36`), recall failures
becoming tellable apart (`81a2212`), dates on all four doors (`0b518de`), the
capsule knob on the plugin surfaces (`1f93bf0`).

DECISIONS §5 said: "a release becomes worth cutting when you ratify recommendation
1 (dates in surfaces / capsule flag): that is the first product-visible change of
the next cycle, and it deserves its own version." Recommendation 1 has now
shipped twice over.

**Cost of a release:** AGENTS §11 — version bump first, one unlocked `cargo
check` to refresh `Cargo.lock`, then the `--locked` gates; two commits; a tag; the
three-Linux asset set via `scripts/build-release.sh`, which refuses a dirty tree
and bills no Actions minutes. CI must be disabled around the tag push
(`ci.yml` **and** `release.yml` both trigger on `v*`).

**Cost of not releasing:** the improved product is not what anyone installs. The
dates and capsules only exist in a tree.

**Recommendation: cut one release after §2.1(b), before §2.3.** It is the
cheapest way to make the last two batches real for users, and the aroute work
does not gate it.

---


# SECTION H-DECIDED

# FILE: docs/handoff/CLOSED.md (lines 1,45)

# Closed levers — measured and rejected (do not re-propose without new evidence)

AGENTS §8 forbids re-litigating these. Each row names the measurement that closed it;
if you think new evidence reopens one, state what is new **before** you spend anything.
Detail lives in the cited documents; this is the index a cheaper agent can scan.

| lever | verdict | closed by |
|---|---|---|
| dense vectors **as a fusion-weight lever** (H1d) | measured 2026-10-15, gate FAILED: `recall_all@10` +1.8pp best (vec 0.75) bought at −9.9pp R@1; deep-miss rescue 4/99; below-gate at low weights. Re-opening needs a new mechanism, not a re-run (`CONSISTENCY.md` §41) | `eval/SELECTION_VECTOR_AXIS_ALL10.md` |
| dense vectors **as a default** | declined: no coverage failure to fix on LongMemEval (R@20 99.6%, pool 500/500); costs ~3× artifact size, 4–7× idle RSS, 19–30× write path. The arm exists behind `--features embed`, weight 0.0. Reopening is the **user's** call — now informed by the H1d measurement above | `docs/PERFORMANCE_PLAN.md` "Explicitly not doing"; `docs/EXCEED_PLAN.md` Phase D |
| cross-encoder reranker | disqualified: breaks the single-binary position, unmeasured even by Hindsight who ship it optional | `docs/RERANKING_PLAN.md` §3.2 |
| graph stream; consolidation / extraction ladder | both competitors measure with it off; no benchmark win available | `docs/EXCEED_PLAN.md` §3 |
| Porter stemming (E2) | flat on top of E1; temporal-reasoning −2.26 pp; removed | `docs/NEXT_ITERATION.md` E2 |
| IDF-weighted overlap (E3) | the hypothesis it would fix the recall curve was disproved; removed | `docs/NEXT_ITERATION.md` E3 |
| distinct-term-coverage stream (E4) | algebraically identical to raising the overlap weight; removed | `docs/NEXT_ITERATION.md` E4 |
| `cache_size` / `mmap` tuning | no gain, +15–17% RSS; refused | `docs/PERFORMANCE_PLAN.md` |
| the token-overlap stream's *fitted weight* as evidence | `overlap 0.25` was chosen on LongMemEval (46 configs): fitted, not earned. Not "closed" as a value, but closed as a claim | `docs/EVALUATION_HYGIENE.md` §2.1 |
| a **hardcoded** synonym table | maintenance + licensing; the user-supplied `recallSynonyms` config is the supported form | `docs/EXCEED_PLAN.md` §3 |
| score thresholds on auto-injection | measured; the levers are composition, caps, labels, tags — not a cut-off | `docs/CONSISTENCY.md` §32; `docs/OPEN_HOOK_RECALL_RELEVANCE.md` |
| `agreement` prior (`FusionWeights::agreement`) | moved **0 of 1,531** LoCoMo questions at 0.50 (and 0 of 500 on LongMemEval) | `eval/SELECTION.md` reading 1 |
| `k` in RRF (`k = 60`, inherited from Hindsight) | flat: R@5 spans 86.0–86.1% across k ∈ {5…120} at the shipped overlap | `eval/SELECTION.md` reading 3 |
| `bm25_magnitude` as a big win | does not collapse (the competitor warning does not transfer) but R@5 only 86.0→86.3% at 4.00 — small. *`recall_all@10` was not measured then*; H1c reopens only that metric — **re-opened 2026-10-14 and closed again**: `recall_all@10` 83.0→82.8% (4 up / 7 down), multi-hop R@1 −3.4pp (n 1,531) | `eval/SELECTION.md` reading 2; H1c: `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| `keyword_scope: ExactAndPrefix` (H1a) | best single-lever R@1 gain on the dev set (R@1 +3.0pp, MRR +2.2pp, NDCG +1.7pp) but `recall_all@10` +0.5pp < the +1pp gate and single-hop R@5 −0.4pp — real, deterministic. Gate failed as pre-registered; no LongMemEval consultation | `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| `overlap_scope: BestSentence` (H1b) | `recall_all@10` −0.3pp, R@1 −3.4pp overall, multi-hop R@1 −11.3pp (n 1,531). Concentrating overlap on one dense segment loses more than it gains | `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| recency 1.0 `TemporalQueriesOnly` on the dated corpus (H3) | falsified: temporal `recall_all@10` −7.8pp, temporal R@1 −33.4pp; control exact (1,025/1,025 detector-negative lists bit-identical). LoCoMo temporal questions ask *when an event happened*, not *what is newest*. Recency stays 0.0; the dated corpus (H0) remains for date-*aware* mechanisms that are not a recency prior | `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| intermediate write granularity, windows of W=2 turns (H2) | R@1 −1.1pp, `recall_all@10` −0.4pp vs sessions, and R@pool 100.0→98.7% — windowing pushes banks past the candidate-pool ceiling, so some gold is unreachable at any K. With `Turns` (§35) also worse, granularity is closed: sessions stay | `eval/LOCOMO_WINDOWS.md`, `docs/CONSISTENCY.md` §37 |
| **turn-level writes, session scored at best turn** (this cycle) | worse on every aggregate on LoCoMo at the shipped config: `recall_all@10` 83.0→80.7%, R@1 60.4→54.6%, R@5 86.0→83.0%, NDCG@10 73.6→69.7% (n 1,531); per-query `recall_all@10` 42 up / 76 down. One run, not tuned | `eval/LOCOMO_TURNS.md`, `docs/CONSISTENCY.md` §35 |
| a recency stream *measured on the old LoCoMo ingest* | invalid, not rejected: every LoCoMo `created_at` was the harness clock (0.006 s spread), so R@1 24.8% at `recency 1.0` is an ingest-order artifact. Re-test only after H0 | `eval/SELECTION.md` "recency caveat" |
| cross-bank retrieval | banks are isolated by design; Hindsight's accuracy comes from ~1000 candidates *inside* one bank | `docs/EXCEED_PLAN.md` §3 |
| targeting Hindsight's 92.0% | a different instrument (LLM-judged accuracy, third-party judge, ~1000-deep pool) | `docs/EXCEED_PLAN.md` §0.2 |

## Things that look like findings but are instrument artifacts

Do not cite these as memory-wire properties:

- **17.0% LongMemEval `answer_quality`**: a small-reader floor, undated prompt (STATE §2).
  *The dated re-measure (2026-10-14, harness fixed per TODO #16) reads 18.6% conditioned /
  6.0% closed-book — a different instrument (dates in the prompt), recorded beside, never
  compared as better/worse (`CONSISTENCY.md` §39). Neither number is comparable to any
  published LongMemEval figure: judge, prompt and truncation are all this harness's own.*
- **84.1% AMB LoCoMo**: free `small-stack` reader and judge; not comparable to any
  published LoCoMo score; the date handling predates the `created_at` round-trip.
- **Latency / RSS / throughput in the baseline**: taken at load 11–28 on 24 cores.
- **Relevance-battery junk-injected 0/9**: the live bank `omp` grew; the code did not change.
- **PrecisionMemBench 14/77**: 77 cases, measure-only — *superseded 2026-10-14 by the H6 capability change: with scope-tag filters honoured the adapter measures 19/77, precision 0.24 (`eval/amb/PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz`). The pre-change 14/77 stays here as the historical no-filter reading.*

# FILE: docs/handoff/RND_PLAN.md (lines 679,698)

### 6.4 Decision-gated — waiting on the user (nothing moves until picked)

**Recommendations for every row below are filed in `docs/handoff/DECISIONS.md`
(2026-10-17), ranked by what the cycle's measurements say buys memory
performance** — the short version: adopt the serving-shape wins first (dates
in the surfaces, capsules per-bank), run the dated head-to-head, keep
`overlap 0.25`, fund Track 2, commit the cycle, release only when a ratified
decision ships.

| option | cost | what it buys |
|---|---|---|
| **Capsule adoption by the surfaces** (auto-injection → `format: "summary"` behind a per-bank flag) | small patch, battery + e2e | the one change that moves what agents see, not what benchmarks see; the numbers it cites are committed (`eval/relevance/BATTERY-H8-2026-10-15.txt`: 41% less served text, fidelity held, ranking identical) |
| **Second-system HaluMem head-to-head** (same harness, same answerer) | one more ~7 h run + their setup | the only thing that turns 0.6129 into a competitive claim (§44: comparability none until a second system runs this exact harness) — best run on the dated adapter, i.e. after H12 |
| **Track 2 agent-memory-bench** (TODO #10) | agent-model budget | the only external benchmark grading the product's actual job — hooks+MCP wired, task success, harm suite |
| **Answerer-side product work** (the 73% ceiling) | product iterations, battery-gated | the evidence-backed frontier: on LME 73.0% of served-and-gold contexts are answered wrong; H5 and H10 both name the answerer, not the ranking |
| **A paid/frontier reader or judge** | money | re-opens `answer_quality` as a meaningful instrument (17.0/18.6% are small-reader floors, not memory quality) |
| **Dense-arm promotion** (`--features embed` by default) | ~3× artifact size + model download | declined; H1d evidence says no (+1.8pp best at −9.9pp R@1, deep-miss rescue 4/99). The call is the user's and is now informed |
| **Next release** | the protocolized §45 chain | nothing in tree; v0.8.2 is current |
| **aroute fixed** | run the `docs/AROUTE_BUGS.md` recipe | deletes `eval/llm_shim.py`; condition-gated on the other repo, which is the user's |

