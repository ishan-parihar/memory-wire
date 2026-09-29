# memory-wire

**Agent memory in one 8.4 MiB binary, three shared libraries, 10.7 MiB of RSS and
zero daemons** — roughly 1% of Hindsight's documented idle floor. No language
runtime, no database server, no install step: retain/recall/reflect with bank
isolation, FTS5 BM25 + RRF fusion and PII redaction on write, over HTTP or as an
MCP stdio server.

- **10.7 MiB** idle RSS · **8.5 MiB** binary · **3.4 MiB** download · **0** background processes
- **97.2% R@5** / **83.8% R@1** LongMemEval-S · **60.0%** answer accuracy vs an
  **8.3%** closed-book floor · 100% hit-rate coding-life
- On retrieval we are **roughly level to slightly behind** agentmemory's hybrid once
  the fitted weight is accounted for. Both numbers are in
  [Benchmarks](#benchmarks); the reason is one sweep on one dataset.
- Docs: `docs/AUDIT.md` (competitor teardowns) · `PLAN.md` · `docs/VERSIONS.md` (pins) · [`INSTALL_FOR_AGENTS.md`](INSTALL_FOR_AGENTS.md) (curl runbook)
- Proof: `eval/RESULTS.md` · `eval/CODING_LIFE.md` · `eval/SCALE_SWEEP.md` · `eval/BENCH_*.md` · `eval/SOAK.md` · `docs/BENCHMARK.md`

**Contents** — [Benchmarks](#benchmarks) · [Footprint](#footprint-vs-competitors) · [Quick start](#quick-start) · [CLI](#cli) · [Install](#install) · [How it works](#how-it-works) · [Reproduce](#reproduce) · [Roadmap](#roadmap)

## Benchmarks

Official LongMemEval-S, retrieval-only (no LLM judge), same methodology as the
competitors' harnesses — per-question fresh index, session-as-document, question-text
query. Regenerate: `cargo run --example longmemeval` → `eval/RESULTS.md`.

| System | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR |
|---|---|---|---|---|---|---|
| **memory-wire — shipped config** | **83.8%** | **97.2%** | **98.6%** | **99.6%** | **88.2%** | **89.2%** |
| memory-wire — unfitted (equal weight) | not measured | 93.0% | 97.4% | 99.6% | 83.5% | 83.9% |
| agentmemory BM25-only | not reported | 86.2% | 94.6% | 98.6% | 73.0% | 71.5% |
| agentmemory BM25+Vector | not reported | 95.2% | 98.6% | 99.4% | 87.9% | 88.2% |

**The first row is measured at a weight that was fitted to this set; the second is
the one to trust.** `overlap: 0.25` was chosen by sweeping 46 configurations
against these same 500 questions. Re-measured on 1,531 LoCoMo queries that never
chose it, the weight is worth **+1.4pp R@5, not +4.2pp**
([`eval/LOCOMO.md`](eval/LOCOMO.md)) — the direction replicated, the magnitude did
not, which is what the maximum of 46 noisy draws looks like. Adding that measured
delta to the clean baseline puts the honest out-of-sample estimate at **≈94.4%
R@5: behind agentmemory's 95.2% hybrid, not 2.0pp ahead of it.**

So the honest position is **decisively ahead on footprint, roughly level to
slightly behind on retrieval.** Both rows ship because publishing only the first
would mean publishing only the fitted number. `eval/LOCOMO.md` and
`docs/CONSISTENCY.md` §14 have the working.

**R@1 is the column that decides whether the answer is right, so it is the one to
read first.** An agent that reads the first memory it gets back is correct 83.8%
of the time; one that reads five is correct 97.2% of the time. The two figures are
the same 500 questions, the same build and the same harness method
(`eval/ORACLE_RERANK.md`, which re-derives the pool `recall` builds and verifies it
row-for-row against what `recall` serves). R@1 is **not** a coverage claim: it is
the shipped ranking's own first-hit rate, and the 100.0% quoted beside it in that
artifact is an *oracle* — a perfect reranker over the pool this build already
assembles, computed with the gold labels. That oracle is 500/500, so the entire
+16.2pp at R@1 is ordering, not matching, and it is the largest single number in
the system. Neither competitor publishes an R@1, so the column carries one number
rather than a comparison — that is the honest shape of it.

**R@1 is also the clearest statement of where the remaining work is.** Overall
83.8%, but `single-session-preference` ranks the right session first **43.3%** of
the time while its R@20 is **100.0%** — an ordering collapse, invisible in every
K≥5 column. Four mechanisms aimed at it (a BM25-magnitude tie-breaker the fusion
was discarding, a recency stream, sentence-level overlap, and a temporal-query
classifier) are built, unit-tested and **deliberately inert**: every weight sits at
a default that leaves the shipped ranking byte-identical. They are not a feature
until a weight is chosen on the dev set — see `docs/PERFORMANCE_PLAN.md`.

**The two streams are not equally weighted, and that is a measured result.** At
equal weight the fusion scored 93.0 / 97.4 / 99.6 / 83.5 / 83.9 — behind
agentmemory's hybrid on R@5 and 4.4pp behind on NDCG@10. A 500-question sweep of
the weights (`eval/SWEEP_FUSION.md`, `docs/NEXT_ITERATION.md` Phase E1) found the
token-overlap stream was a *net negative* as a co-equal voter, and moved its
weight to 0.25: **+4.2pp R@5, +4.8pp NDCG@10, R@20 unchanged.** The gain is
concentrated exactly where the deficit was — `single-session-preference` R@5
56.7% → 86.7% and `single-session-assistant` 87.5% → 100.0%, with no category
regressing. The cost is stated in full below rather than buried. **The +4.2pp is a
fitted number and did not hold its size off this dataset** — on LoCoMo it is
+1.4pp. The table above reports both the clean and the fitted configuration
rather than only the flattering one.

**The weight has not been shown to earn its place, and the comparison that matters
was not run until now.** `eval/SWEEP_FUSION.md` section D measures each stream
alone: **BM25 on its own scores 97.0 / 99.0 / 99.6 / 89.9 / 91.4** at
R@5 / R@10 / R@20 / NDCG@10 / MRR. Against the shipped fusion that is
**R@10 +0.4, NDCG@10 +1.7, MRR +2.2, and R@5 −0.2** — three metrics better and
one worse. So the token-overlap stream buys 0.2pp of R@5 and costs NDCG@10 and
MRR against deleting it, and the independent LoCoMo set separately found
`overlap: 0.00` ties `0.25` on R@1 and NDCG@10. Two datasets point the same way.
The counter-argument is real and is why the stream is still here: LoCoMo banks
hold 19–32 documents against LongMemEval's ~48 and `tests/scale.rs`'s 5,000, so
the dev set under-measures the pool size at which a second lexical voter pays
(`docs/CONSISTENCY.md` 14.2). **This is a live, unmade decision, not a settled
result** — and the honest reading is that the burden of proof has moved.

**Two later attempts to make that weight unnecessary both failed, and one of them
failed for an instructive reason** (`docs/NEXT_ITERATION.md` Phases E3–E4).
Weighting the overlap stream by IDF did exactly what E1's diagnosis predicted —
at equal weight it scores 96.4 / 88.6 where the raw stream scores 93.0 / 83.5 —
but at the shipped 0.25 it costs 0.4pp R@5 for +1.7pp NDCG@10, so it is not
shiped. A third distinct-term-coverage stream is *algebraically identical* to
raising the overlap weight: for any query with no repeated token the two rank
identically, and RRF is linear in the weights, so `coverage: w` ≡
`overlap: 0.25 + w`. Both levers were measured, rejected, and then **removed from
the tree** — they were dormant config nobody would ever turn on. The findings live
in `docs/NEXT_ITERATION.md`; the sweep artifact that produced them is
`eval/SWEEP_FUSION.md`.

## What an answer actually costs to get right

`recall_any@K` is a proxy. The number a user feels is whether the agent answers
correctly, so that is measured directly (`eval/ANSWER_QUALITY.md`): the same 25
LongMemEval questions, answered by an LLM from the retrieved context and graded
by an LLM, against a **closed-book control** that gets the question and no memory
at all.

| arm | accuracy | scored | prompt tokens |
|---|---|---|---|
| retrieval-conditioned | **60.0%** | 25 | 315,343 |
| closed-book control | **8.3%** | 24 | 150,372 |
| **delta** | **+51.7pp** | | |

Memory is doing real work, and the control is what makes that claim falsifiable —
if the conditioned arm were at parity with the closed-book one, every ranking
number above would be measuring nothing.

**The single largest win in this project was a budget, not a ranking change.**
`DEFAULT_RECALL_BUDGET` was 2,000 tokens against a **2,626-token median
LongMemEval session** — so the budget could not return a median memory. The trim
truncates the top hit and skips everything below it. Raising it to 8,000
(3× the median, so three sessions fit whole) moved answer accuracy
**40.0% → 60.0%**.

The mechanism is visible in the artifact, and it is the reason this is a
mechanism and not a coincidence:

| on this slice | before (2,000) | after (8,000) |
|---|---|---|
| gold session present in the served context | 92.0% | **92.0%** — unchanged |
| … answer judged **wrong** despite gold served | **52.0%** | **32.0%** |
| … answer judged correct | 40.0% | 60.0% |

Retrieval did not improve; the answerer stopped being starved. Note that
`eval/RESULTS.md` **cannot see this defect at all** — its harness passes an
explicit 100,000-token budget, so the default is structurally invisible to it.
That is why weeks of retrieval work left a 2× gap between what recall found and
what the consumer received. `tests/recall_budget.rs` now asserts a gold row
survives the trim.

Caveats, stated rather than buried: n=25 with 2 row errors, so the closed-book
cell is provisional under the harness's own rule; the judge is a local free-tier
model, not LongMemEval's official grader; and this is a 25-question sample, not
a benchmark. It sizes an effect. It does not establish a rate.

Competitor rows: agentmemory's `benchmark/LONGMEMEVAL.md` (same metric, same 500
questions, verified in-audit). Hindsight has **no comparable retrieval number**:
its one published LoCoMo result, 92.0%, is LLM-judged *answer* accuracy from a
`rag` run with two Gemini calls in the loop, and `git grep -iE "ndcg|recall@|
pool_size"` finds nothing in the benchmark repo — so the per-pool-size recall
table in their reranker blog is not reproducible from either repo. Their retrieval
stack is 4-stream TEMPR + cross-encoder rerank behind a 0.8–1.0 GB idle RSS
(see below).

| Suite | memory-wire | Competitors |
|---|---|---|
| coding-life, 15 labeled queries (`eval/CODING_LIFE.md`) | hit-rate **100%**, R@5 96.7%, p50 **260–620 µs** across release runs of this binary | grep baseline 96.7% (same harness); agentmemory publishes no score |
| Scale sweep 240→10k (`eval/SCALE_SWEEP.md`) | search p50 0.99→6.03 ms, token savings ≥95.7% | agentmemory: 0.1→22.8ms BM25, heap 6→316MB, savings to 100% |

Both latency ranges move 2–3x with the machine's load. `eval/CODING_LIFE.md` and
`eval/SCALE_SWEEP.md` each say so in their own text, and each artifact's
provenance line carries the load-relevant date, profile and machine. Quote them
as ranges, never as a single run.

## Footprint vs competitors

| Dimension | memory-wire (measured) | Hindsight (their install docs) | agentmemory (their SCALE.md) |
|---|---|---|---|
| Ship artifact | **8.5 MiB** binary — 8,872,000 B (LTO, incl. the MCP SDK); **3.4 MiB** gzipped, which is what you actually download | Python API image + PG/pg0 | Node 20 + iii-engine binary |
| Shared libraries it needs | **3**, on `linux-x86_64` — `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, all part of any glibc system. Dynamically linked, not static: `readelf -d` on the release binary lists exactly those three `NEEDED` entries and nothing else (a macOS build links `libSystem` instead, so the list is per-platform). The optional `--features embed` build needs **4** — it adds `ld-linux-x86-64.so.2` — `docs/CONSISTENCY.md` §16.6 | whole Python + `psycopg`/PG stack inside the image | Node's `libnode`, `libc`, `libstdc++`, `libm`, `libgcc_s`, `libdl`, `libpthread` |
| Idle RSS | **10.7 MiB** post-retain (**10,716–11,080 kB** over 8 reads; 7-round mean 10,911 kB); 9.1 MiB before the first retain | **0.8–1.0 GB** full / ~100s MB slim | heap **6 MB** @1k obs |
| RSS under load | **12.9 MiB** (13,084–13,360 kB, 3 rounds) — 5k retains + 200 recalls over HTTP, one process, sequential | **1.2–1.5 GB** full (models + ONNX arenas) | heap **316 MB** @50k obs |
| Minimum box | any glibc box — one file to copy. SQLite is **compiled in** (`rusqlite` with `features = ["bundled"]`), so there is no database to install and nothing to run | **1.5 GB** full / 512 MB slim + external providers + separate DB | Node + engine + 4 ports |
| 10k-memories storage | **2.9 MiB** (SQLite+FTS5, 3,022,848 B settled main file — 7,237,496 B while the WAL is unflushed) | Postgres + pgvector (+ reranker) | **35.7 MB** (BM25+vector) |
| Background processes | **0** | API + worker + UI + DB | REST + streams + viewer + worker WS |

Hindsight rows: their `docs/developer/installation.md` (RAM table). agentmemory rows:
their `benchmark/SCALE.md` (§1 heap + storage tables).

**The RAM comparison is only true against one of the two, and the basis is named
rather than left implicit.** Against Hindsight's 0.8–1.0 GB documented idle RSS,
10.7 MiB is **1.1–1.4%** — roughly 75–95x smaller. Against agentmemory the
answer depends entirely on which of their figures you take, because it is a
*heap* at an observation count and memory-wire is an RSS at a memory count: at
their **50k** figure (316 MB) memory-wire is **3.4%**, and at their **1k** figure
(6 MB heap) memory-wire is **larger**, not smaller. Neither row is a
like-for-like measurement and the table does not pretend otherwise; the honest
summary is that memory-wire is far below both on any large-bank comparison and
above agentmemory's smallest published number.

**How the memory-wire rows were measured, 2026-09-28, on this tree.** Binary by
`stat -c %s` after `cargo build --release --locked`, cross-checked by
`eval/BENCH_FOOTPRINT.md` (which reads the binary next to the harness's own path,
so it cannot read a debug build or a stale artifact). Both RSS rows by `ps -o
rss` — really `VmRSS` from `/proc/<pid>/status` — against a `serve` on a scratch
`--db`, read after `/health` answers `ok` and then again after one HTTP retain
(idle: 8 reads, two independent procedures; under load: 3 rounds of 5,000
retains + 200 recalls, sequential). Storage is the 10,000-memory row of
`eval/BENCH_FOOTPRINT.md`, which measures both halves — WAL-unflushed and
post-`wal_checkpoint(TRUNCATE)` — on every run. `eval/SCALE_SWEEP.md` reads the
same corpus as **3,014,656 B** for the main file; it samples mid-run while the
store holds the file open, so the ~8 KB difference is auto-checkpoint position,
not a disagreement.

**These are ranges because RSS on this box is a range.** Every RSS figure above
was taken on a machine under sustained load from *unrelated* work
(`loadavg` 21–48 on 24 cores), and a single reading on a loaded box is an
anecdote. Nothing here is a one-run point value except the two byte counts, which
are exact.

The deltas are interesting, and three of them moved in opposite directions.
**The binary grew, not shrank**: 8,836,032 B against the 8,593,368 B measured at
`0.1.0`, **+278,864 B (+3.2%)**. `fastembed` and its ONNX runtime were removed at
`0.3.0` and are **back in the tree now** as an optional, default-off `embed`
feature, so `fastembed`, `ort`, `ort-sys` and `tokenizers` *are* in `Cargo.lock`
again. Every footprint number in this section is still the **default** build and
is unaffected: `default = []`, so `src/vector.rs` and the weights compile only
under `--features embed`. The growth itself is the MCP SDK — `rmcp` + `schemars`
landed between those two measurements and cost more than `fastembed` ever did. The
binary was 6.4 MB before MCP; MCP is the reason the artifact is 8.5 MiB and not
smaller, and it is code, not a resident dependency, so idle RSS does not move
with it.

**Idle RSS is not flat any more, and the honest form is a range.** The previous
pin was 10,704 kB post-retain. Measured today over 8 reads — a 7-round shell
loop plus the committed harness's own single read, at `loadavg` 21–25 — it is
**10,716–11,080 kB** (7-round mean 10,911 kB, 10.65 MiB). The old pin sits at the
very bottom of today's band, so the claim has moved up by roughly 200 kB. That is
the read pool: four more SQLite connections' worth of page cache on a store that
already had two. Pre-retain moves the same way, 9,123 kB → **9,196–9,536 kB,
mean 9,351 kB (9.13 MiB)**. The pre/post gap is still the FTS5 index and the
write path's first pages, which is why the two must never be quoted
interchangeably. This is not an interleaved before/after — the pre-pool binary was
not rebuilt for this measurement — so treat the ~200 kB as "the range moved", not
as a precisely attributed delta. `docs/CONSISTENCY.md` §12.4 has the same record
and the honest caveat, and §11.3 has the page-cache settings that were tried and
rejected.

**The 10k store shrank, for the first time.** 3,022,848 B (2.88 MiB) settled main
file, **−172,032 B (−5.4%)** against the 3,194,880 B this table used to publish.
The cause is exact and structural: FTS5 now runs `detail=none`, so the index
stores no positional data. `memories_fts_data` went 331,776 → 159,744 B and the
whole index 434,176 → 262,144 B, **−39.6%**; `docsize` is unchanged at 94,208 B
because `bm25()` needs it. The migration is gated by a one-shot
`fts_detail_none` marker in the existing `schema_markers` table, so it runs once
per database and never re-runs. Separately, the store runs in **WAL mode** and
the settled figure is the settled one: while the process holds the file open,
10,000 memories occupy **7,237,496 B** (main 3,014,656 + `-wal` 4,190,072 +
`-shm` 32,768), collapsing back to 3,022,848 B at a clean
`wal_checkpoint(TRUNCATE)`. A server that has just been fed 10k memories and not
yet checkpointed is using 2.4x the settled figure. The main file reads *smaller*
in the first state than in the second because a checkpoint copies the committed
WAL frames back into it — the same reason `eval/SCALE_SWEEP.md` samples
3,014,656 B mid-run.

**Under load.** 13,084–13,360 kB (mean 13,249 kB, 12.94 MiB) for 5,000 retains +
200 recalls over HTTP, sequential, one process. The previous claim also carried
"recall p50 6 ms / p95 10 ms"; that number came from a different harness and is
**not** re-pinned here, because today's equivalent — 200 sequential `curl`
requests end to end, each a fresh process — measured **13.8–32.2 ms per
request**, which is the client and the load, not recall. The store's own recall
figures are in `eval/`: **849 µs** p50 under the 16-client soak (`eval/SOAK.md`),
**3.1 ms** at a 2k bank with 1 client (`eval/BENCH_CONCURRENCY.md`), and **11 ms**
p50 on LongMemEval-S sessions (`eval/RESULTS.md`).

**The quiet-machine recall-curve figures live in `docs/CONSISTENCY.md` §13, not in
`eval/`.** Three `bench_recall_curve` rounds at load 8.36–8.61 — the only
low-load recall-curve measurement in the repo — read p50 **1,015 µs** at 1k,
**6,129 µs** at 10k, **34,999 µs** at 50k and **71,331 µs** at 100k, with the
100k round spread 70.5–73.8 ms against 80–172 ms on a loaded box. §13 records all
four sizes, and it records that the session was *not* written to `eval/`, so
nothing under version control carries those numbers. The committed
`eval/BENCH_RECALL_CURVE.md` is a different, busier run — loadavg
**24.64/33.47/31.95**, p50 2,164/15,172/54,500/102,450 µs — and is **not**
comparable to the §13 band; its retrieval columns are. Everything else above is
wall-clock and load-confounded, and is quoted as a range for that reason.

**The recall-curve R@5 is no longer 100%, and that is E1's cost, not a bug.**
Reweighting the fusion (below) took R@5 on that suite from 100% at every size to
**96.9 / 84.4 / 90.6 / 90.6%** at 1k / 10k / 50k / 100k, and coding-life R@5 from
100% to 96.7% (its hit rate is still 100%, and it now ties the grep baseline
instead of beating it). All 32 gold rows still reach BM25's top-50 at every size,
so the 200-row fusion pool is still not the binding constraint — what changed is
how the *ordering* of near-tied candidates is decided. Both suites measure finding
a needle whose distinguishing token is one rare word among many common ones, which
is exactly the case a raw token-count voter is good at and exactly what cutting
that voter's weight to 0.25 gives up. LongMemEval-S measures the opposite case —
multi-facet questions where BM25's own term-rarity ordering is better and a raw
count double-counts common words — and gains 4.2pp there. The measured trade-off,
including the weight that holds both (`overlap: 0.75`, which keeps these two
suites at 100% and still gains 1.0pp on LongMemEval), is in
`docs/NEXT_ITERATION.md` Phase E1. The 0.25 default is the one the project's
designated decision instrument picks; the alternative is named so the choice is
revisitable rather than buried.

## Quick start

```bash
cargo build --release
./target/release/memory-wire serve --addr 127.0.0.1:8888 --db ~/.local/share/memory-wire/agents.db
curl -sS -X POST localhost:8888/banks/demo/retain -H 'Content-Type: application/json' -d '{"content":"auth uses jose"}'
curl -sS -X POST localhost:8888/banks/demo/recall -H 'Content-Type: application/json' -d '{"query":"how does auth work","budget":2000}'

# or, for an MCP client, over stdio:
./target/release/memory-wire mcp --bank my-project
```

`--db` is optional: without it the store lands at `$XDG_DATA_HOME/memory-wire/memory.db`,
falling back to `~/.local/share/memory-wire/memory.db` when `XDG_DATA_HOME` is unset
or relative. Point it at a scratch file for throwaway work —
`--db /tmp/scratch.db` — and nothing touches your real bank.

`GET /health` → `ok` (plain text) · `POST /banks/:id/{retain,recall,reflect}` ·
`GET`/`PUT /banks/:id/config` · `GET /banks/:id/memories[?limit=&offset=]` ·
`GET`/`DELETE /banks/:id/memories/:mid` · `GET /banks/:id/stats`.
Banks are isolated namespaces; any bank id is created implicitly by its first
`retain`, and a `default` bank is seeded at boot. Recall returns a bare JSON array
of content strings by default; `{"format":"full"}` returns `{id, score, content}`
objects instead, where `score` is the fused RRF value the ranking actually used
(`1·1/(60 + rank_bm25) + 0.25·1/(60 + rank_overlap)`, higher is better) rather than
a token count — so it is a fraction, and only comparable against other scores from
the same recall. Those two weights are the swept default
(`docs/NEXT_ITERATION.md`, Phase E1); they are compile-time constants, not
per-request options. A third coverage stream and an IDF-weighted overlap scorer
were both built, measured, rejected and removed (Phases E3–E4), so the formula
above is the whole of the shipped score.
`reflect` returns the top hit prefixed with its id — there is no
LLM in the loop yet.

**Bounds, and the error contract.** `budget` is a hard token cap, not a hint:
`budget: 0` returns `[]`, and a top hit longer than the budget is cut to exactly
the cap (4 chars per token) rather than dropped — with a literal `…[truncated]`
tail when the marker's own 12 characters fit inside the remaining room, and
unlabelled when they do not (budgets 1–3 return the top hit cut to 4–12 chars, not
`[]`). Lower hits that do not fit are skipped so smaller ones further down still
fill what is left. One recall never returns more than **100** memories. A blank
or whitespace bank id is a `400 invalid bank id`; a blank or whitespace
`content` is a `400 invalid content` (a memory of nothing matches no query and
cites nothing); an unknown memory id is `404 unknown memory`; and every storage
failure — including raw driver text that would echo SQL fragments and on-disk
paths — collapses to an opaque `500 storage error`.

Secrets are redacted before the write, on both `content` and `context`:
`<private>` blocks, PEM private keys, JWTs, `Bearer` headers, Slack tokens, Google
API keys, emails, and phone numbers.

## CLI

Eight subcommands in a default build, exactly as `--help` reports them (`help` is
clap's own and is not one of them). An `--features embed` build has a ninth,
`embed <text>`, which prints one 384-d vector as JSON; it is the reason the
weights survive LTO:

| Command | What it does |
|---|---|
| `info` | Audit + plan pointers (the default when no command is given) |
| `serve` | Start the HTTP server — `--addr` (default `127.0.0.1:8899`), `--db`. Stops accepting and drains in-flight requests on SIGINT/SIGTERM |
| `connect` | Wire the hooks into agent hosts — optional `<agent>`, `--uninstall`, `--guidelines` |
| `hook` | Lifecycle hook the hosts invoke — `session-start`, `prompt`, `stop`, `pre-compact`, `session-end` |
| `doctor` | Endpoint, bank, store, and server health — `--db`, `--strict` |
| `mcp` | Serve MCP over stdio — `--bank`, `--db` |
| `sweep` | Delete memories past their bank's `ttl_days` — `--dry-run`, `--db` |
| `seed` | One-shot bank seeding from this repo's git history — `--commits <N>` (default 100, capped at 500), `--transcripts`, `--bank`, `--db` |

`--addr` defaults to loopback because there is no authentication: anything that
can reach the port can read and delete every bank. A non-loopback address still
binds, and says so once on stderr before it does.

If 8899 is already taken, `serve --addr 127.0.0.1:<free port>` and point your
client at that port; `GET /health` on it must return exactly `ok`. There is no
`backup` subcommand — a SQLite store is backed up with `sqlite3 "$DB" ".backup
'$DB.bak'"`, recipe and caveats in `INSTALL_FOR_AGENTS.md`.

## Connect, hooks, doctor

`memory-wire connect` installs the five lifecycle hooks into every agent host it
detects (claude-code, codex, copilot-cli) and the MCP server entry into the two
that speak it (cursor, opencode); pass a host to wire just that one, `--uninstall`
to prune, `--guidelines` to write the rules block into this project's agent files
instead. It is idempotent and never touches a hook it did not write — malformed
config is refused untouched rather than rewritten. (The file it writes is
re-serialised with sorted keys and two-space indent; foreign entries survive
intact, and whenever there was a file to copy, a timestamped backup of it is kept
under `$XDG_DATA_HOME/memory-wire/backups/` and its path is printed.)

```bash
memory-wire connect claude-code     # -> claude-code  wired         SessionStart, UserPromptSubmit, Stop, PreCompact, SessionEnd
memory-wire doctor --db /path/to/agents.db
```

`hook session-start|prompt|stop|pre-compact|session-end` is what those host
entries invoke. The first two print: a bank preamble plus a recall, and the
recall lines for a submitted prompt. The last three print **nothing** — their
value is entirely in what they retain. `stop` retains one compacted line naming
the transcript; `pre-compact` and `session-end` fire at the two moments a
conversation stops being available (`PreCompact` runs immediately before Claude
Code discards it to context compaction), so both read the transcript at
`transcript_path` and retain the conversation's own prose, capped at 2,000
characters, rather than a pointer to the file. All five are best-effort by
contract — a down server prints the local framing and exits 0, never an error in
front of the model. The preamble
reads the bank's `background`/`preamble`/`system_prompt` from
`GET /banks/:id/config` when that key exists — for the bank the hook resolves
from its own working directory. That bank is resolved in a fixed order:
`--bank <id>`, then `MEMORY_WIRE_BANK`, then `owner/repo` from the repository's
`origin` remote, then the git work tree's top-level name, then `memory-wire`.
The remote is read straight out of `.git/config` with no `git` process spawned;
`https://github.com/acme/api.git`, `git@github.com:acme/api.git` and
`ssh://git@host/acme/api.git` all resolve to bank `acme-api`. A repository with
no remote, or one whose remote names no `owner/repo`, falls back to the old
basename rule. Full order, and the `.git`-as-a-file handling for worktrees and
submodules: [`docs/BANK_IDENTITY.md`](docs/BANK_IDENTITY.md).

**Upgrading moves your bank name, and nothing migrates for you.** A remote-derived
id is a different id from the basename that produced the same memories before,
so an existing bank stays where it is and new sessions write to the new name.
`doctor` warns when it sees that case — the derived name differs from the old
one *and* a bank of the old name exists — and names both, but it stays
read-only and moves nothing. Keep reaching the old memories with
`memory-wire hook session-start --bank <old>` or `MEMORY_WIRE_BANK=<old>`. There
is deliberately no automatic migration and no alias: a wrong guess moves
memories between namespaces on the strength of a directory name, silently, and
that is worse than a name you have to type.

`doctor` is one screen: endpoint, bank, store size and health, bank/memory row
counts, server state. `server up` means `/health` answered 2xx **and** the body
was exactly `ok`, so a foreign process on the port cannot pass for the server.
`--strict` exits nonzero when the server or store is unusable, and a store whose
pages fail `PRAGMA integrity_check` is reported `unreadable` rather than counted.
It never creates the store it reports on — pointing `--db` at a nonexistent path
reports `missing` without creating the file — and on an *existing* store it folds
the WAL into the main file on the way past (best effort, `PASSIVE`, never
blocking), which is what makes a `cp` taken straight afterwards a usable backup.
Point it at the same file you gave `serve --db`, or the report describes a store
the server never touches.

## Bank config

`GET`/`PUT /banks/:id/config` is a whole-object store-and-serve: unknown keys
(`retain_mission`, `recallPromptPreamble`, anything a future version adds) come
back byte-for-byte, and a malformed value in a key this build *does* act on falls
back to the default rather than taking recall down.

```bash
curl -sS -X PUT localhost:8888/banks/demo/config -H 'Content-Type: application/json' \
  -d '{"recallMaxTokens":128,"retainTags":["ops"],"retain_mission":"own the release"}'
```

Two keys change behavior. `recallMaxTokens` is the bank's default recall budget —
a recall that omits `budget` uses it, an explicit `budget` still wins.
`retainTags` is added to every later retain in that bank, after the request's own
tags, so a bank-wide default can never displace a tag you named.

Tags work without any config: `retain` accepts `{"content", "tags"}` and `recall`
accepts `{"query", "tags"}` to restrict the search to memories carrying **any** of
them (normalized, deduped, capped at 20; the cap keeps the head, preserving your
ordering).

## Forgetting (opt-in TTL)

Memory-wire has no background process, so nothing is ever deleted on a timer.
A bank that wants to forget says so in its own config, and
`memory-wire sweep` is what carries that out:

```bash
curl -sS -X PUT localhost:8899/banks/scratch/config -H 'Content-Type: application/json' \
  -d '{"ttl_days":30}'

memory-wire sweep --dry-run   # what would go, and how much
memory-wire sweep             # actually forget it
```

`ttl_days` is the retention window in days; **absent means nothing ever
expires**, so forgetting is off until a bank asks for it and every other bank is
listed as `skipped` and never touched. There is no way to switch forgetting on
globally, and no way to make it happen without running the command — a policy
nobody asked for is a policy that must not fire.

A sweep deletes every memory in a bank whose `created_at` is *older* than
`now - ttl_days`; one created exactly at the cutoff survives. Deleting the row is
all it takes: the `memories_ad` trigger retires its FTS entry and the tags
cascade, so the index can never cite a memory that is gone. Each bank is one
transaction, so a bank that fails leaves every other bank swept and nothing half
done. Output is per-bank, then a total, and the exit code is 0:

```text
memory-wire sweep
  demo        ttl 7d  cutoff 2026-09-20T13:52:46.190Z  would delete 3
  scratch     skipped (no ttl_days)
total 3 would be deleted
```

`--dry-run` is the same arithmetic with the delete left out. A value this build
cannot use — a string, a negative, a window no date can reach — reads as *no
policy* rather than failing the write, because a typo must not be able to make
forgetting happen (or stop a config from being stored at all). The column that
carries the policy is added by migration, so a store written by an earlier
version opens, reads, and sweeps exactly as it was.

## MCP

`memory-wire mcp` serves MCP over stdio — JSON-RPC 2.0, newline-delimited, stdout
only for protocol, logs to stderr. `--bank` sets the default bank, falling back to
`$MEMORY_WIRE_BANK` and then to `memory-wire`; a per-call `bank` argument
overrides both.

| Tool | Writes? | Arguments |
|---|---|---|
| `memory_retain` | yes | `content` (required), `bank?`, `context?`, `tags?` |
| `memory_recall` | no | `query` (required), `bank?`, `budget?`, `tags?` |
| `memory_reflect` | no | `query` (required), `bank?` |
| `memory_bank_config_get` | no | `bank?` |

Every tool declares `destructiveHint: false`; `memory_retain` is the only one
without `readOnlyHint: true` — a repeated retain of the same content in the same
bank writes nothing and returns the id that already holds it, but a repeated
retain of *different* content is a second memory. Each tool is a thin adapter onto
the same `MemoryService` methods
the HTTP routes call — retrieval, redaction, and budgeting are not reimplemented
for MCP, so the two surfaces cannot drift. An unknown tool name is a JSON-RPC
`MethodNotFound`; a tool that runs and fails returns `isError: true` with the same
opaque message the HTTP surface returns, never a crash.

```json
{ "mcpServers": { "memory-wire": {
  "command": "memory-wire", "args": ["mcp", "--bank", "my-project"] } } }
```

Bank-in-path scoping (`/mcp/:bank`) belongs to a future HTTP transport and is not
built; the argument-resolution rule above is already the one that mode needs.

## How it works

```text
serve        → SQLite store (bank-isolated) + seeded `default` bank
retain       → ensure bank exists → redact_pii(content, context) → put
               (+ tags ∪ bank retainTags); byte-identical content already in the
               bank returns that row's id, no second copy
               document_id set → supersede that document's prior row in the same tx
recall       → candidate pool: newest 200 rows ∪ the BM25 hits (bounded, so a
               recall never reads the whole bank) → FTS5 BM25 (LIMIT 50 in SQL)
               + overlap rank (cap 200) → weighted RRF k=60 (bm25 1.0,
                 overlap 0.25 — swept, `eval/SWEEP_FUSION.md`)
               → token-budget trim (truncate, never drop) → take 100
               budget precedence: request > bank recallMaxTokens > 2000
               format=full → {id, score, content} instead of bare strings,
               score = the fused RRF value
reflect      → recall(2000).first() cited by id
memories     → SELECT … LIMIT ? OFFSET ? (50/0 default, 500 cap) → {id, content, created_at}
stats        → COUNT(*) + COUNT(DISTINCT tag) + MIN/MAX(created_at) for the bank
mcp          → same four calls, over JSON-RPC on stdio
```

`document_id` turns retain into an upsert: with the default
`"update_mode":"replace"`, each call deletes that document's previous row and
inserts the new one **inside one transaction**, so a reader never sees the new
revision beside the old tag set and a failure never leaves the document with no
revision at all. Three replaces under one `document_id` leave exactly one row,
holding the last content. `tags` are replaced wholesale on each write, so a
revision does not inherit the previous revision's tags. The third field,
`"update_mode":"append"`, only ever *adds* a memory — it never extends a row — so
it is **not usable for repeated writes**: a second append under a `document_id`
that already holds a row is refused with `409 document already exists; use
update_mode=replace` and the first row survives, unchanged. Treat `append` as
single-use per document id; if you are accumulating revisions, `replace` is the
mode that works today. Neither field appears in any response — a document id is a
write-side handle only, so keep it yourself.

Library in `0.3.0`: `src/{api,store,recall,capture,embed,lib,memory}.rs` —
`capture.rs` supplies the redaction filter. Binary-side modules (`connect`,
`doctor`, `guidelines`, `hooks`, `http`, `mcp`, `paths`, `seed`, `sweep`) live in
`src/main.rs`; the HTTP surface is eight route groups (`/health`, and
`/{retain,recall,reflect,config,memories,memories/:mid,stats}` under `/banks/:id`)
plus four MCP tools.

There is no consolidation ladder in this build, and no vector stream in a
**default** build. What a default build ships is FTS5 BM25 plus token-overlap,
fused by RRF, and nothing else is reachable from a request.

The `embed` cargo feature — **off by default**, `default = []` — does add a third
stream: `src/vector.rs` brings an embedder, the `memory_vectors` column and a
dense branch in the query path, over ~23 MB of vendored int8 MiniLM weights in
`models/`. It changes no ranking, because `FusionWeights::vector` ships at `0.0`
in every build, so none of it is compiled unless you ask for it. Every footprint
and latency figure in this README is a default-build figure and is unaffected.
`src/embed.rs` remains the dependency-free cosine/rank kernel.

**Why it ships off, which is a measurement and not a preference.** A coordinate
descent over the vector weight on the 1,531-query LoCoMo dev set
(`eval/SELECTION_VECTOR_AXIS_FIXED.md`) swept `vector` across 0.00 → 1.50:

| `vector` | R@1 | R@5 | R@20 | NDCG@10 | up/down |
|---|---|---|---|---|---|
| 0.00 | 60.4% | 86.0% | 98.0% | 73.6% | — |
| 0.10 | **60.8%** | 86.7% | 98.0% | **74.2%** | 18/6 |
| 0.25 | 57.2% | **87.4%** | 98.3% | 73.0% | 34/12 |
| 0.50 | 53.4% | 87.1% | 98.5% | 71.1% | 58/40 |
| 1.50 | 45.4% | 78.2% | **99.2%** | 64.2% | **97/216** |

The arm is **reordering-only**: `R@20` rises monotonically while `R@1` falls,
which is the signature of ranking, not retrieval — and the dense arm alone
reaches *less* than the lexical fusion (R@pool 100%, R@5 62.4%). Its best
honest contribution is **+1.4pp R@5 at 0.25, with NDCG@10 and MRR moving
backward**; at 1.50 the up/down ratio inverts as deep results survive and
shallow ones die. That does not justify +53.8 MB, a fourth shared library and
a cold start, so the weight stays at `0.0`.

There is a real second reading, and it is not the one that flatters the table:
**NDCG@10 and MRR both peak at 0.10**, where R@1, NDCG@10 and MRR improve
*together* (60.8 / 74.2 / 72.3) for 18 up and 6 down. If a caller reads the top
two results — which is roughly what a token budget allows — 0.10 beats 0.25 on
every rank metric. Whether to enable it is a product decision about how many
results a caller reads, and it is left unmade rather than made here.
`0.00` and `0.25` help *disjoint* question sets (Jaccard 0.43), so this is a
genuine choice, not a stronger-vs-weaker version of one setting.

**What the `embed` build costs, measured rather than estimated.** Building it
(`cargo build --release --features embed`) produces a **65,007,536 B** binary
against the default build's **8,874,128 B** — the vendored weights plus ONNX
Runtime, and nothing else. Two facts about it are worth stating because both were
assumed the other way:

- **It needs no C++ shared library.** ONNX Runtime normally drags in
  `libstdc++.so.6`, which would add a fourth dependency the default build does not
  have. A `build.rs` resolves `libstdc++.a` with `cc -print-file-name` and links it
  into rustc's existing `-Wl,-Bstatic` group, gated on `#[cfg(feature = "embed")]`
  and a `*-linux-gnu` target; `--as-needed` then declines to record a
  `DT_NEEDED` for it at all. A plain `cargo build --release --features embed` needs
  **no env var, no `RUSTFLAGS` and no `.cargo/config.toml`**.
- **It runs on the same minimal root as the default build.** A `bwrap` root holding
  only `ld-linux-x86-64.so.2`, `libc.so.6`, `libm.so.6` and `libgcc_s.so.1` — and
  deliberately *no* `libstdc++.so.6` — runs `--version` and performs real ONNX
  inference there, not just the version print.

The full record, including the two rejected alternatives (`crt-static`, which works
but cannot be scoped to one feature because cargo has no per-feature rustflags, and
`-static-libstdc++`, which is a 0-byte no-op because rustc links through
`gcc-ld`/`lld`), is `docs/CONSISTENCY.md` §16.

**Lifecycle ops are routes now — custody, not relevance.** `recall` answers "what is
relevant"; these answer "what did I actually store, and how do I take it back":
`GET /banks/:id/memories` pages a bank (`?limit=` default 50, capped at 500;
`?offset=` default 0) and `GET`/`DELETE /banks/:id/memories/:mid` fetch or drop one
by id. Every memory those routes serve carries `created_at` (RFC 3339 UTC,
stamped at insert), and `context` appears only when the memory actually has one —
so a memory stored without capture context does not claim an empty one. A missing
id is `404 unknown memory`; deleting an already-deleted one is `200` with
`{"deleted":false}`, which makes the delete idempotent rather than an error.
`GET /banks/:id/stats` returns `{memories, tags, oldest, newest}` for the bank.

**The unknown-bank rule.** An operation that *names one resource* answers `404`
when that resource is absent, so a typo cannot read back as a plausible empty
answer: `GET`/`PUT /banks/:id/config` on a bank that was never created is
`404 unknown bank`, and `GET /banks/:id/memories/:mid` is `404 unknown memory`. An
operation that reads a *collection or an aggregate* answers `200` with the empty
answer, because banks are created implicitly by their first `retain` and a
pre-retain recall is normal, not exceptional: `GET .../memories` → `[]`,
`GET .../stats` → zeros and `null`s, `POST .../recall` → `[]`, `POST .../reflect` →
`"no relevant memories"`. `POST .../retain` creates the bank it names, and
`DELETE .../memories/:mid` stays idempotent — `200` with `{"deleted":false}`.
One corollary: **`PUT /banks/:id/config` requires an existing bank** and will not
create one, so configure a bank after its first retain.

`tests/{e2e,scale,backup}.rs` · `examples/{longmemeval,coding_life,scale_sweep,soak,bench_footprint,bench_write,bench_recall_curve,bench_concurrency,bench_coldstart,sweep_fusion,oracle_rerank}.rs`.

## Reproduce

```bash
./eval/download.sh            # official LongMemEval-S, 264 MB, once
cargo run --release --example longmemeval -- --data eval/data/longmemeval_s_cleaned.json --n 500 --out-md eval/RESULTS.md
cargo run --release --example coding_life  --out-md eval/CODING_LIFE.md
cargo run --release --example scale_sweep  --out-md eval/SCALE_SWEEP.md
cargo test && cargo clippy --all-targets --all-features --locked -- -D warnings
```

**`--out-md` has no `eval/` default, on purpose.** A bare
`cargo run --release --example …` writes its markdown artifact to a scratch file
under `$TMPDIR` and prints where; updating a committed `eval/*.md` means naming
it. Those files are the reviewed record of what was measured, and a single run
silently replacing one has already destroyed real content here twice — a curated
methodology note in `RESULTS.md`, and a benchmark artifact whose header then
disagreed with the build. One rule, shared by every harness.

The five `bench_*` harnesses, `soak`, and the two analysis harnesses are
measurement, not CI:

```bash
cargo run --release --example bench_footprint    --out-md eval/BENCH_FOOTPRINT.md
cargo run --release --example bench_write        --out-md eval/BENCH_WRITE.md
cargo run --release --example bench_recall_curve --out-md eval/BENCH_RECALL_CURVE.md
cargo run --release --example bench_concurrency  --out-md eval/BENCH_CONCURRENCY.md
cargo run --release --example bench_coldstart    --out-md eval/BENCH_COLDSTART.md
cargo run --release --example soak               --out-md eval/SOAK.md
# 500-question weight grid; ~7 min index build per full run
cargo run --release --example sweep_fusion       --out-md eval/SWEEP_FUSION.md
# oracle-rerank ceiling: where the first gold row actually sits in the pool recall builds
cargo run --release --example oracle_rerank \
  --data eval/data/longmemeval_s_cleaned.json --out-md eval/ORACLE_RERANK.md
```

**`eval/SWEEP_FUSION.md` is read, not regenerated.** It carries the E3/E4 `idf`
and `coverage` grid rows (its sections F and G) and those arms were removed from
`src/` in 3db88b4 — `examples/sweep_fusion.rs` no longer contains them — so a
re-run would silently drop the evidence for the removal. Leave it alone.

The eval artifacts are generated from `--release`, the profile that ships; a
default-profile run reports the same retrieval metrics but roughly 2–5× the
latency, so quote the profile with any latency number. Each artifact's own
header carries its date, profile and machine, and wall-clock latency is only as
good as the load the box was under when it ran — read that header before
quoting a number.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/ishan-parihar/memory-wire/main/install/get-memory-wire.sh | sh
# sh get-memory-wire.sh --db <path>     # echoes the serve line with that --db
# sh get-memory-wire.sh --uninstall     # removes the binary only, keeps the db
```

Detects `linux`/`macos` × `x86_64`/`aarch64` and refuses anything else;
`MW_REPO`, `MW_VERSION`, `MW_INSTALL_DIR`, and `MW_LOCAL_ASSET` override the
defaults, which resolve to this repo and its latest published release
(`v0.3.0`, asset `memory-wire-linux-x86_64.tar.gz`).

**The published release is `v0.3.0`; the working tree is ahead of it.** `v0.3.0`
is a real, published GitHub release (published 2026-09-28) and `Cargo.toml` still
says `0.3.0`, so the version string describes the release and *not* the tree: the
branch is **19 commits past the `v0.3.0` tag** with a large uncommitted wave on
top. Nothing described in this README's `embed` paragraphs is in `v0.3.0` — that
release shipped the lexical arm only, with the `embed` feature absent. So: what
`curl … | sh` gives you is `v0.3.0`; what this file describes is later work. The
tree-versus-release position, and the record that retired the older framing and
then found it true again, are in `docs/CONSISTENCY.md` §15.2 and
`docs/VERSIONS.md` §0.
Agent-facing runbook with per-step assertions: `INSTALL_FOR_AGENTS.md`.

## Roadmap

Shipped: `GET`/`PUT /banks/:id/config` (merged into the `serve` router) · the
`connect <agent>` / `hook` / `doctor` / `seed` CLI subcommands · the MCP stdio
server (`rmcp`, 4 tools, +1.6 MB binary) · tag filtering on retain and recall ·
the lifecycle routes (`GET .../memories`, `GET`/`DELETE .../memories/:mid`,
`GET .../stats`) with `created_at` on every served memory · `format: "full"` on
recall · `document_id` upsert on retain (replace works; a repeated `append` is a
`409`) · dedup-on-retain on the content hash (a repeated retain of the same
content returns the existing id; document-scoped retains are exempt so the
append/replace split is untouched) · a bounded recall candidate window, so recall
cost no longer grows with the bank · per-bank `ttl_days` plus the explicit
`memory-wire sweep` that enforces it (off by default, no scheduler) · a loopback
`--addr` default for `serve`, with a one-line stderr warning on any other bind ·
graceful shutdown on SIGINT/SIGTERM, draining in-flight requests.

Not built yet: a 4-tier consolidation ladder (promote/evict/retention had no
scheduler and no caller; the code was removed rather than left as a model of
behaviour that does not run) · vectors at retain + stored embeddings +
cross-encoder rerank, which would close the −2.2pp to agentmemory hybrid (the
`embed` feature and its ONNX dependency are back in the tree, default-off and
inert — `FusionWeights::vector` ships at `0.0`, so none of it is reachable from a
request in a default build; the cosine/rank kernel is likewise unused) · a `DELETE` for a whole bank (one memory at a time only) · `tags` in
the lifecycle responses (they serve `{content, created_at, id[, context]}`; tag
filtering stays on `recall`) · `document_id` visible in any response · a `backup`
subcommand (the `sqlite3 .backup` recipe is the interface) · Postgres/
pgvector backend · LLM-backed `reflect` synthesis · bank-in-path scoping for MCP
over HTTP (`/mcp/:bank`; stdio only today) · `connect` for hosts outside the five
detected (claude-code, codex, copilot-cli, cursor, opencode). Gaps that need live
LLMs/providers (Hindsight system-evals, agentmemory quality/real-embeddings) are
tracked in `docs/BENCHMARK.md`.

## Subagents

Project rule: delegate only with the `space-bunny` model.
