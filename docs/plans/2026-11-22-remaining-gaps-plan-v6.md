# Outline plan — remaining gaps, redundancy, and upgrades (fifth pass)

**Status:** executed 2026-11-22 (see §9). Written from `327af57`, one commit past `v0.11.0`
(the tagged commit is `51b3133`).

Four previous plans were written and executed on 2026-11-21 and 2026-11-22. This
pass was written by running the things the last one shipped, which is the only
method that has produced a *new* defect each time. It found three, and one of
them was found by using the release rather than by reading it.

**Headline: `update` is broken in a way that matters, and the release gate would
not have caught it. Two smaller items are register hygiene. One is a test seam
that cannot fail.**

---

## 0. What is not left

Recorded so the next pass does not repeat it:

- **`v0.11.0` is shipped** with three Linux assets, all sha256-verified, musl
  `static-pie` with `DT_NEEDED=0`.
- **Plan v5 is executed** (`46ceba3`, `bcf7f56`, `d7e2ef4`, `07afba4`, `78b11fd`).
- **Seven of eight hosts are wired to `omp`**; the eighth (hermes) is excluded by
  design, and the `omp` *host*'s refusal is correct — see §2.
- **The doctor's `server up` / `not running` contradiction is gone**, because the
  live server was restarted on the new binary (pid 1768060).
- **No unused dependencies**, no stale module table, all three doc gates green.

---

## 1. `update` leaves the old server running, and its own module doc says otherwise

**This is the one with code in it. It shipped in `v0.11.0`.**

Run on this host, 2026-11-22, from 0.9.0 to 0.11.0. The output was:

```
memory-wire daemon: nothing running; pid 1371296 is gone, and the stale
  state at .../serve.json was removed
memory-wire: .../update-v0.11.0/memory-wire: No such file or directory
```

**What actually happened.** `orchestrate_into` (`src/update.rs:101-112`) calls:

1. `ops.extract(archive, &dir)?` → `dir/memory-wire`, exists
2. `ops.daemon_live()` → **false**
3. `ops.swap(&new_bin)?` → `std::fs::rename(new_bin, exe)` (`:379`) — **moves
   the file out of `dir`**
4. `ops.start_daemon(&new_bin)?` — skipped, because `was_live` was false
5. `ops.reconnect(&new_bin)?` — **execs `dir/memory-wire`, which step 3 moved**

So the binary was 0.11.0 while **pid 1295 kept serving 8888 from
`memory-wire.old`**. The stale-serve case, live, one minute after shipping the
fix that names it.

**Why the stop said 'nothing running' while a server was genuinely up.**
`GhOps::daemon_live` (`src/update.rs:346`) does probe the port — it reads the
address out of `serve.json` and calls `probe_server`. That returned true, so
`stop_daemon` ran. What went wrong is one level down: `daemon::stop`
(`src/daemon.rs:578`) branches on `inspect()`, and `inspect_at` (`:271`) checks
the pidfile *first* and returns `Verdict::Gone` the moment the pid is not alive
(`:278`) — before it ever looks at the endpoint. That is **deliberate**: the
doc comment at `:265-269` says checking the endpoint first is how `stop` ends up
signalling a stranger's pid.

So the design is right and the *reporting* is not. The state file named dead pid
1371296, so `stop` took the `Gone` branch, cleared the state file, printed
"nothing running; pid 1371296 is gone", and returned 0 — while pid 1295 held
8888. It never claimed pid 1295 was ours. It simply did not mention that the port
was occupied, and `orchestrate` read its exit 0 as "the port is free".

That is the shape of the `doctor` defect fixed in §78, one layer out: a surface
that is correct about the pidfile and silent about the port.

**Four defects, then, and they are not one fix:**

| # | defect | fix |
|---|---|---|
| 1 | `new_bin` used after `swap` moves it (`:109`, `:112`, `:413`) | exec `self.exe` after the swap, or copy rather than rename |
| 2 | `daemon::stop` prints "nothing running" and exits 0 while a foreign process holds the port | name the port in `Gone`/`Absent`; `orchestrate` must not read exit 0 as "the port is free" |
| 3 | `reconnect`'s failure leaves a swapped binary and a stale daemon | the swap already happened, so the failure is reported after a partial success — say so plainly rather than as a bare `No such file` |
| 4 | `orchestrate` does not re-check that the port is actually free after `stop` | after `stop`, assert the port is closed; if not, say so instead of proceeding |

Defect 1 alone is a one-line fix. Defect 2 is the one that made it invisible: the
port was serving the whole time.

**Why no test caught it.** The `Ops` fake (`:505-531`) implements `swap` as
`self.log.push(format!("swap {}", ...))` — it records a string and **does not move
any file**. So `new_bin` always exists in tests, and every one of the 8 tests
passes. The seam is fake in exactly the way that hides the defect it exists to
catch.

**The fix is a seam that can fail the same way production does:** `swap` on a
temp directory that renames the file, then assert `start_daemon` and `reconnect`
still work. That is a real-filesystem test for a real-filesystem bug.

**Also: the module doc is wrong.** `src/update.rs:6-10` says "the restart closes
the stale-serve window the same moment the new bytes land." It did not. The
restart was skipped. A module doc that states an outcome the code does not
guarantee is the same class of defect as the OPEN_HOOK correction block.

---

## 2. A refusal that was correct, and should be recorded as such

`memory-wire connect omp` failed:

```
FAILED  /home/ishanp/.omp/agent/mcp.json: a server named `memory-wire` is
already registered under `mcpServers` and is not ours; left untouched
```

I initially wrote this up as a `connect` bug. **It is not.** The entry on disk is:

```json
"memory-wire": { "type": "http", "url": "http://127.0.0.1:8888/mcp/omp" }
```

`Host::Omp`'s `mcp_spec` (`src/connect.rs:366`) writes a **stdio** entry
(`type: stdio`, `command`, `args`) because omp's schema wants one. An http entry
in omp's `mcpServers` is not ours and would likely be ignored by omp. Refusing
was right.

**What is actually worth doing:** say so in `connect`'s refusal. The message
says "is not ours" and leaves it there; a reader cannot tell whether that means
*malicious* or *a different shape*. Naming the shape difference ("this is an
`http` entry; omp takes `stdio`") turns a refusal into a diagnosis.

**Not a bug. A diagnosability improvement, tier 3.**

---

## 3. A register row that says "Closed" in its text and `open` in its status

TODO.md #31 reads, in the status column, `open`, and in the text,
"**Closed 2026-11-21**". Both of its claims verified true against the tree:

- `POST /banks/:id/memories/:mid/redact` exists (`api.rs`, 3 references)
- `memory-wire hygiene --bank <id>` exists and runs

TODO.md is **hand-written** (no `genome:` zones), so this is a stale hand edit,
not generated drift. It matters because `genome emit` reads TODO.md to build
`STATE.md`'s status table — a wrong `open` there propagates.

**Fix:** flip #31 to `done`, or fold it into #38 if it is genuinely the same
work. One line.

---

## 4. The release gate does not run what it builds

`scripts/build-release.sh` verifies the git state, the tag, and the sha256 of
each tarball. It **never executes a built binary.** On 2026-11-22 I ran
`dist/` binaries by hand to check they printed `0.11.0` — that check is not in
the gate, so a binary that builds and immediately fails would have been
published with a valid checksum.

**Fix:** after the build loop, extract each tarball to a temp dir and run
`<bin> --version`, expecting the tag's version. Three lines of shell per target,
and it is the difference between "the archive hashes" and "the archive works".

This is cheap and it is the last thing standing between a green build and a
broken release. Tier 1.

---

## 5. What I checked and found clean

- **Footprint holds:** 10.73 MiB, `DT_NEEDED` 3 (gnu), 0 (musl).
- **The musl misdetection is a documented ceiling**, not an unknown gap:
  `asset_slug_for`'s `ponytail:` comment says a musl host is offered the gnu
  asset and `MW_ASSET` overrides. Recorded so it is not re-filed.
- **`omp`'s refusal is correct** (§2).
- **TODO 38 is correctly `open`** — half done, blocked on a real drive that
  would rewrite live host config.
- **The `citation_name_check` gate** is green and its battery is 5/5.

---

## 6. Order of work

| # | item | tier | gates | notes |
|---|---|---|---|---|
| 1 | §4 release script runs each binary | 1 | `--check` on a local build | cheapest, highest value |
| 2 | §1 fix `new_bin` after swap + `daemon_live` + a seam that can fail | 1 | cargo test, clippy, doc, trust | the real defect |
| 3 | §3 flip TODO 31 | 1 | docs-check | one line |
| 4 | §2 name the shape in `connect`'s refusal | 3 | cargo test | diagnosability |
| 5 | §1 correct the `update` module doc | 1 | docs-check | it states an outcome the code does not guarantee |

**Sizing.** #1 is minutes. #2 is the substantial one: a real-filesystem `Ops`
fake, two one-line fixes, and a test that fails on the old code. #3 and #5 are
doc lines. #4 is optional.

---

## 7. What I need from you

1. **Whether to cut v0.11.1 for §1 and §4.** `update` is broken in the field and
   the release gate would not catch a recurrence. Both fixes are small. The
   alternative is to leave 0.11.0 as-is and fix in 0.12.0 with whatever else
   lands — which means anyone who runs `update` before then gets a swapped binary
   and a stale server.

2. **The `omp` host entry.** §2 is not a bug, but the http entry in its
   `mcpServers` is dead weight — omp ignores it. Removing it is a one-line edit to
   a live config, so it is yours.

---

## 8. Definition of done

- `scripts/build-release.sh` runs `<bin> --version` on each built asset and fails
  if it does not print the tag's version.
- `orchestrate_into` no longer execs a path that `swap` moved; `daemon_live`
  probes the port rather than trusting the pidfile; a test exercises a real
  rename.
- The `update` module doc's stale-serve claim is corrected to what the code does.
- TODO #31's status agrees with its text.
- All gates green; `CONSISTENCY.md` gets the section, including the correction
  that `omp`'s refusal was right.
---

## 9. Execution record — all five items done, 2026-11-22

| § | item | commit | evidence |
|---|---|---|---|
| 4 | release-script smoke test | `57066c9` | each built binary is extracted and `--version`-checked; mutation-verified by pointing the expected version at a wrong value |
| 1 | `update` fix | `3c08e5f`, `a6e82f7` | `Ops::swap` returns the installed path and `orchestrate` uses it; `Ops::port_holder` lets it refuse before the swap; `daemon stop` names the port; `reconnect` says what its failure cost; module doc corrected |
| 5 | module doc | `a6e82f7` | the "restart closes the stale-serve window" claim is replaced with the correction and its evidence |
| 3 | TODO #31 → done | `92b2ebe` | status column said `open`, text said `Closed 2026-11-21`; all three claims re-verified (`api.rs:1498`, `api.rs:1428`, `main.rs:285`) |
| 2 | connect shape mismatch | `92b2ebe` | the refusal now names both shapes; mutation-verified against the old message |

**Four defects in §1, not one.** The plan named the exec-after-swap as *the*
defect. Fixing it exposed three more in the same path, all found by the same
question — *what does this assume that is not true?*:

- **(a) exec after move** — `swap` renames, so `new_bin` is gone. Fixed by
  returning the installed path.
- **(b) `daemon stop` silence** — exits 0 for "state file names a dead pid"
  while a foreign process holds the port. `inspect_at`'s ordering is
  *deliberate* and unchanged; the missing piece was the sentence, not the logic.
- **(c) bare `No such file`** — a reconnect failure after a successful swap read
  like a connect bug. Now says the binary is installed and the hosts were not
  rewired.
- **(d) no port re-check after stop** — `orchestrate` read exit 0 as "the port
  is free". Now re-checks and refuses before swapping.

**The DoD's `daemon_live` clause was superseded, not met.** The plan proposed
`daemon_live` should "probe the port rather than trusting the pidfile". That is
wrong and was not done: the pidfile-first ordering exists precisely so a wedged
daemon whose listener died can still be stopped, and `inspect_at` documents why.
Probing the port there would make a wedged daemon unkillable. The real gap was
(b), and (b) is fixed.

**Mutations run, not asserted.** Three separate reversions, each confirmed to
fail a named test before the fix was restored:

| mutation | test that failed |
|---|---|
| `start_daemon`/`reconnect` on the moved path | `orchestrate_runs_the_full_order_when_live` |
| remove the post-stop port check | `an_occupied_port_refuses_before_the_swap` **and** the ordering test |
| restore the terse connect refusal | `a_foreign_server_under_our_name_is_refused_in_both_directions` |

**Gate:** 681 tests, clippy 0, doc 0, three doc gates green, module table updated
for `update.rs`, `daemon.rs` and `connect.rs`. `CONSISTENCY.md` §80 records the
round.

**Not done, and it is a limit not an oversight.** The fix is verified against a
real rename in a unit test, not by running `update` on this host. The live path
gets exercised at the next release cut. §80 states this plainly.
