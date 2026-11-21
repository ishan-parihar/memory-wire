# Ruling sheet — five decisions waiting on the operator

Each row: the ruling id, the options (yours marked), what deciding unblocks, and
what leaving it undecided costs. **No row carries my recommendation** — the
reviewer supplies that.

Operator-only facts are marked **[OPERATOR]**: the reviewer can answer "not in
evidence" about them and should.

---

## R1 — The release

**Question.** Cut a new binary release from `main`, or keep waiting?

**State.** `Cargo.toml` is 0.10.0. The newest tag is `v0.10.0`, cut 2026-10-27.
`main` is **89 commits** past that tag. The *installed* binary on the operator's
machine is **0.9.0**, dated Oct 20, and provably lacks `memory_redact`
(`git show v0.9.0:src/mcp.rs | grep -c memory_redact` → 0; the tree → 3).

**Options.**
- (a) Cut now from `main` — three-Linux set via `scripts/build-release.sh`.
  **[MINE: I lean here but have not been told to]**
- (b) Keep waiting until the remaining work is final.
- (c) Cut a patch release of only the fixes, leaving features unreleased.

**Deciding unblocks:** the hosts already wired to the installed binary can use
`memory_redact` and the doctor bank block; it also makes "newest release" and
"newest code" the same thing again.

**Deferral costs:** the drift between installed and tree keeps growing (89 → N),
and every host wired to 0.9.0 silently lacks capabilities. It is **not**
irreversible either way — a later cut supersedes an earlier one.

**Owner: operator.** AGENTS §11 reserves this to the user.

---

## R2 — The bank

**Question.** Move this host's hooks onto the bank that holds the memories?

**State.** `memory-wire doctor` on the operator's machine:

```
finding  bank `ishan-parihar-memory-wire` holds no memories; `omp` holds 4246.
         A hook here resolves to `ishan-parihar-memory-wire` and will recall nothing.
```

Every hook firing in that directory recalls nothing. `omp` holds 4,246 memories.
There are 9 banks on the store.

**Options.**
- (a) `memory-wire connect --bank omp` — forwards the pin to every `connect` it
  runs, including the `--uninstall` that unwires hosts. **Rewrites five live host
  configs** (`~/.claude.json`, `~/.copilot/`, `~/.cursor/`,
  `~/.config/opencode/`, `~/.hermes/config.yaml`). **[MINE: I lean here]**
- (b) Leave it, and set `MEMORY_WIRE_BANK=omp` per shell.
- (c) Migrate the memories into the resolved bank instead, and keep the current pin.

**Deciding unblocks:** hooks on this host actually recall, which is the product's
whole job.

**Deferral costs:** it is **silent** — a hook that recalls nothing looks exactly
like a hook that found nothing. It has been true since at least `bde4b6a`.

**Owner: operator**, because (a) edits five files outside the repo.

---

## R3 — agent-memory-bench

**Question.** Spend an answering-model budget on the Claude Code arm?

**State.** `docs/BENCHMARK_LANDSCAPE.md` calls it "the product question": the only
external benchmark that grades **task success with hooks+MCP wired** rather than
retrieval quality. It is **not cloned**, and needs an answering model plus
Docker. It is measure-only.

**Options.**
- (a) Fund it and run it. **[MINE: I lean here — it is the only thing that
  grades the actual product]**
- (b) Defer until the release and bank decisions land.
- (c) Drop it; the retrieval harnesses are enough.

**Deciding unblocks:** the first external evidence about the product's job rather
than its retriever.

**Deferral costs:** none technically. It is the largest remaining *evidence*
gap, not a correctness one.

**Owner: operator** — it spends money.

---

## R4 — The unadopted server

**Question.** Add a `daemon adopt` command for a server started outside
`daemon start`?

**State.** Measured today:

```
$ memory-wire doctor
  server     up
$ memory-wire daemon status
  state      not running
  pid        1371296  (no such process)
  state file /home/ishanp/.local/share/memory-wire/serve.json  (stale)
```

Both are correct. A server has been up four days (pid 1295, started outside
`daemon start`); the daemon's recorded pid (1371296, Oct 17) is gone, so
`serve.json` describes a server that no longer exists and nothing rewrote it.
`daemon` has Start/Stop/Status and **no `adopt`**, so that server can never be
managed. `doctor`'s `stale_serve_at` returns `None` when the recorded pid is
dead (`let running = exe_of(pid)?`), so the doctor stays quiet in exactly the
case where the state file is maximally wrong.

**Options.**
- (a) `daemon adopt` — make an unadopted server manageable. **[MINE: I lean
  against, as a proposal rather than a bug fix]**
- (b) Fix only the doctor's silence (name the stale state file + the unadopted
  server; offer nothing).
- (c) Do nothing; the operator restarts by hand.

**Deciding unblocks:** the tooling stops contradicting itself.

**Deferral costs:** `doctor` and `daemon status` keep disagreeing, and a reader
trusting either one is misled about whether memory-wire is running.

**Owner: split.** (b) is agent work; (a) and (c) are operator.

---

## R5 — The benchmark battery

**Question.** Run LongMemEval-S now as a regression check?

**State.** The dataset is present (277 MB, `eval/data/longmemeval_s_cleaned.json`)
and the command works. Running it is a **counted consultation** — the count is at
**5** (`docs/CONSISTENCY.md` §74). The last two batches changed doors, a doctor
line, docs and a count gate; **nothing touched ranking**, and both free harnesses
(LoCoMo R@10 94.2%, relevance battery H8 gate held, 42% less auto-injected) are
unchanged within rounding.

**Options.**
- (a) Run it now, as a release-boundary regression check. **[MINE: I lean
  against]**
- (b) Run it at the release, not before.
- (c) Never re-run; the retrieval core is closed.

**Deciding unblocks:** a test-set number on the current build.

**Deferral costs:** low — the two free harnesses already cover ranking, and §6's
rule is that a consultation is for measurement or release acceptance.

**Owner: operator** decides whether to spend a consultation.

---

## Operator-only facts, so the reviewer can say "not in evidence"

- The money available for R3.
- Whether the five host configs in R2 may be rewritten.
- Whether a release is wanted at all before the remaining work is final (R1).
- Whether the four-day-old server in R4 is load-bearing (it serves live traffic).

Do not infer these from the pack.