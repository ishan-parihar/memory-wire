# Outline plan: remaining gaps, cleanup, and upgrades — third pass

**Status:** proposal, 2026-11-21 (third pass). Written from `2155adf`, 87 commits
past `v0.10.0`.

Two previous plans were executed in the same day (`...-plan-v2.md`, `...-plan-v3.md`;
both now carry supersession headers). This pass was written the way the last two
were not: **by looking for defects rather than by re-reading the register.** The
register is nearly empty — six rows, four blocked on you or the world — and it is
nearly empty because I keep closing it. The material below is what re-reading the
*tree* produced.

**Headline: one real tooling defect, one real doc contradiction, and one set of
audit findings that have gone stale without a status column.** Nothing else.

---

## 0. What is not left, and why saying so is the useful part

These were on earlier plans and are **done**, so they are not repeated here:

- **P0.1 / P0.2 / P1.1 of the integration plan are all landed.** P0.1 is exactly
  what `paths::endpoint()` does today (env → daemon state file → `DEFAULT_ADDR`),
  P0.2's recommendation became the shipped behaviour, and codex now gets an HTTP
  MCP entry (`connect_codex.rs` writes `[mcp_servers.memory-wire] url = .../mcp`).
  The plan says so itself in a re-verification block. **The two "decisions I need
  from you" it still lists are therefore answered by the code**, not open.
- Door parity is closed for everything except hygiene's MCP tool, which is a
  recorded deliberate deferral, not an oversight.
- The count-claim gate now covers `src/` as well as the prose.

---

## 1. A real defect: `doctor` looks away when the daemon state file is stale

**Measured on this machine, just now:**

```
$ memory-wire doctor
  server     up
  finding    bank `ishan-parihar-memory-wire` holds no memories; `omp` holds 4246.

$ memory-wire daemon status
  state      not running
  pid        1371296  (no such process)
  state file /home/ishanp/.local/share/memory-wire/serve.json  (stale)
```

Two surfaces, one contradiction. Both are individually correct:

- The server **is** up — pid 1295 has been serving 8888 for four days.
- The daemon **is not** running — pid 1371296, recorded on Oct 17, is gone.

`serve.json` describes a server that no longer exists, and because the live
server was started outside `daemon start`, nothing ever rewrote it. `daemon`
has Start/Stop/Status and **no `adopt`**, so that server can never be managed.

**Where the doctor goes quiet.** `stale_serve_at` is designed for the
swap-the-binary case: it reads the state file, looks up the pid, and compares the
running executable against this one. Line 397 is `let running = exe_of(pid)?;` —
when the pid is **dead**, `?` returns `None`, the whole function gives up, and
doctor reports nothing. So the exact case where the state file is maximally
wrong is the case where the doctor is silent.

**The fix is small and is the honest version of what the doctor already tries to
say.** When the endpoint answers, the daemon state file names a pid that is
gone, and that pid is not the process holding the port: report that the state
file is stale and that the serving process is an unadopted one. Not "restart it" —
that would kill a four-day-old server holding 6,200 memories' worth of live
traffic — but *name the situation*, which is what `doctor` is for and what AGENTS
§10 says its output should be.

Cost: a `daemon` verdict branch or a `doctor` helper, plus tests. **Not** a
supervisor, not an `adopt` command, not touching the running server.

---

## 2. A real doc contradiction: the correction block sits above text it contradicts

`docs/OPEN_HOOK_RECALL_RELEVANCE.md` is marked **OPEN** in AGENTS §4 and is
unchanged since 2026-09-30.

At line 27 there is a correction block:

> **CORRECTION 2026-11-18 — the code block below is STALE. Do not file a bug
> against it.** ... The score is **no longer discarded** ... **The open defect is
> narrower and still real:** `recall_section` ... never reads `score`.

Eight lines below, §"Mechanism" item 2 still says:

> The recall response is a scored structure; `src/api.rs:1903`
> (`recall_should_score_with_the_fused_rrf_value`) pins that the fused RRF score
> is on the wire. `recall_at` deserialises to `Vec<String>` — content only. **The
> score is dropped inside memory-wire's own code**, not by an external caller.

**The correction block is right and the paragraph under it is wrong.** I checked
the code: `recall_at` returns `Option<Vec<ScoredHit>>`, `ScoredHit` carries
`score`, and the only non-test consumer is a `tracing::debug!` at `src/hooks.rs:701`
whose own comment says nothing branches on it.

And the citation itself rots twice over:

| citation | claim | reality |
|---|---|---|
| `src/api.rs:1903` | pins the fused score | the test is at **3132**; 1903 is a bare `}` |
| `src/api.rs:1903` at its own pinned commit `36bb947` | — | **2168**. Wrong even when written. |

The `src/hooks.rs:660` / `recall_at` citations are correct *at their stated
commit* and drifted since, which is the normal kind. `src/api.rs:1903` was never
right, which is the kind that matters.

**The fix.** Repair item 2 to match the correction block, and re-point the
citations. This is a live document a future agent will act on; the "Where the fix
belongs" section says "Stop discarding the score in `recall_at`" — which has
already happened — so the document currently recommends work that is done.

---

## 3. Stale audit findings with no status column

`docs/INTEGRATION_AUDIT.md` (2026-09-30) ranks seven findings F1–F7. Every
heading carries `[V]` or `[C]`, and I initially read those as status. They are
not — the legend at line 29 says **`[V]` verified by a command, `[C]` verified by
reading**. They are *evidence strength*, not resolution state. **The document has
no status column at all.**

So several findings read as live defects when they are fixed:

| finding | as written | today |
|---|---|---|
| F1 | "8888 is not memory-wire" / hook misroute hazard | `ss` shows `memory-wire` pid 1295 holding 8888; `doctor` reports `server up` |
| F4 | "our port is a flag, not a constant ... no single place to fix it" | `paths::DEFAULT_ADDR` exists and is the single place, and §11's rule that every clap default reads from it holds |
| F2 | daemon and hooks disagree on the endpoint | P0.1 landed: `endpoint()` resolves env → state file → default |

**The fix is not to delete the findings.** They are the audit of record, dated,
and AGENTS §7 says a historical record "deliberately describes a specific dated
build, stays and keeps its date." The fix is a **status line** at the top saying
which findings are resolved by commits since 2026-09-30 and which still stand,
with the evidence-strength legend clarified so nobody reads `[V]` as "open" again.

---

## 4. What I checked and found clean

Recorded so the next pass does not repeat it:

- **70 in-tree `file:line` citations**, all within range. Only two name a
  function and point at the wrong one (§2). Citations into other projects' trees
  are frozen audit evidence and must never be checked — those repos are not here.
- **Module table and every count** in AGENTS match the tree; the count gate now
  covers `src/` too.
- **`Host::ALL` = 8, `IMPLICIT` = 7**, matching the prose.
- Both free harnesses were re-run earlier today and are unchanged.
- Footprint: 10.73 MiB, `DT_NEEDED` 3.

---

## 5. Order of work

| # | item | tier | gates | notes |
|---|---|---|---|---|
| 1 | §1 doctor names a stale state file + an unadopted server | 1 | tests, clippy, doc, trust gate, module table | the only real code |
| 2 | §2 repair the OPEN_HOOK item-2 contradiction and its citations | 1 | docs-check | cite by name, not just line |
| 3 | §3 audit status line, `[V]`/`[C]` legend clarified | 1 | docs-check | additive, nothing deleted |
| 4 | §1 optional: a gate that flags a doc contradicting its own correction block | 3 | its own battery | cheap version: assert every `file:line` citation naming a function actually holds it |

**Tier 1 is three commits and well under an hour.** #4 is the only one that
needs a mutation battery, because a check that does not fire on the defect it was
written for is worse than none.

---

## 6. What I need from you

1. **Whether to touch the running server.** §1's finding names it and offers
   nothing. If you want `daemon adopt` — a command that makes an unadopted server
   manageable — that is a proposal, not a bug fix, and it should not be smuggled
   into a doctor fix.
2. **Whether `agent-memory-bench` is worth a model budget.** Still the only
   external benchmark that grades the product's actual job; still needs an
   answering model and Docker.
3. **Whether to cut a release.** 0.9.0 is installed, 87 commits behind, and
   §11 leaves it to you.
4. **The bank decision**, unchanged: `connect --bank omp` would move hooks off the
   empty `ishan-parihar-memory-wire` onto the bank with 4,246 memories. It rewrites
   five live host configs and I have not run it.