//! Shared plumbing for the `bench_*` measurement harnesses.
//!
//! Not a harness itself — a directory module, so cargo does not auto-discover it
//! as a sixth example. Every helper here exists because at least two harnesses
//! need it; anything one harness needs alone stays in that harness.
//!
//! Copied from `examples/soak.rs` (the percentile, the `/proc/self/status` VmRSS
//! reader, the flag shape, the db-bytes accounting) and `examples/longmemeval.rs`
//! (the deterministic LCG) rather than re-invented, so a fix to one of them
//! cannot drift from the others. Two additions `soak.rs` has no use for: reading
//! another *process's* RSS (the README's headline number is a `serve` process,
//! not a harness) and a ~30-line HTTP/1.1 probe, which is what lets
//! `bench_footprint` and `bench_coldstart` measure the real binary instead of
//! a stand-in. `std::net` only; no dependency is added for either.
//!
//! RSS readers return `None` off Linux rather than failing, the same degradation
//! `soak.rs` already has: the harnesses stay runnable on a machine that has no
//! `/proc`, they just print `unavailable` for the numbers they cannot see.

// Cargo builds each `examples/*.rs` as its own crate root, so a helper only one
// harness uses is dead code from every other harness's compiler's point of view.
// That is the price of one shared copy instead of five, and it is cheaper than
// the drift five copies would carry.
#![allow(dead_code)]

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use memory_wire::recall::FusionWeights;

/// The value of `--name`, or `default` when the flag is absent.
pub fn arg(args: &[String], name: &str, default: &str) -> String {
    flag(args, name).unwrap_or_else(|| default.to_string())
}

/// The value of `--name`, or `None` when the flag is absent. Windowed rather than
/// parsed with a crate: the existing harnesses take `--flag value` positionally
/// and adding a parser would be the only argument library in the tree.
pub fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

/// Nearest-rank percentile of an already-sorted slice, `soak.rs`'s exact
/// shape: index `n*p`, clamped to the last element.
pub fn pct(sorted: &[u64], p: f64) -> u64 {
    match sorted.len() {
        0 => 0,
        n => sorted[((n as f64 * p) as usize).min(n - 1)],
    }
}

/// `VmRSS` of this process in kB, or `None` where `/proc` does not exist.
pub fn rss_kb() -> Option<u64> {
    rss_kb_from("/proc/self/status")
}

/// `VmRSS` of another process in kB, or `None`. The README's idle-RSS figure is
/// a `serve` process measured from outside, so measuring it means reading
/// somebody else's status file.
pub fn rss_kb_of(pid: u32) -> Option<u64> {
    rss_kb_from(&format!("/proc/{pid}/status"))
}

fn rss_kb_from(path: &str) -> Option<u64> {
    fs::read_to_string(path)
        .ok()?
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

/// Where a harness writes its markdown artifact, given the process arguments.
///
/// **A bare run cannot touch `eval/`.** The markdown files under `eval/` are the
/// reviewed record of what was measured; replacing one with a single run's numbers
/// is a documentation-destroying edit, and it has already happened twice in this
/// project (the curated LongMemEval methodology note, and a soak/benchmark
/// artifact). So the committed path is reachable only by naming it: with
/// `--out-md` absent the artifact goes to a scratch file under `$TMPDIR` and the
/// path is announced, and the caller has to pass `--out-md eval/<file>` to
/// deliberately update a committed artifact.
///
/// One definition for every harness, deliberately. Five copies of this rule is how
/// the rule drifts.
pub fn out_md(args: &[String], name: &str) -> String {
    match flag(args, "--out-md") {
        Some(explicit) => explicit,
        None => {
            let path = std::env::temp_dir().join(name);
            eprintln!(
                "no --out-md given: writing the scratch artifact to {} (pass `--out-md eval/{name}` to update the committed one)",
                path.display()
            );
            path.display().to_string()
        }
    }
}

/// Which cargo profile this binary was built with. Every artifact has to say so:
/// a default-profile run reports the same correctness and roughly 2-5x the
/// latency, so an unlabelled number is not comparable to anything.
pub fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug (default profile — NOT the shipping profile; expect 2-5x the latency)"
    } else {
        "release"
    }
}

/// CPU model and usable parallelism, for the provenance line. Falls back to the
/// target triple off Linux rather than printing nothing.
pub fn host() -> String {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let model = fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("model name"))
            .and_then(|l| l.split(':').nth(1))
            .map(|v| v.trim().to_string())
    });
    match model {
        Some(m) => format!("{m}, {cpus} logical CPUs"),
        None => format!("{} ({cpus} logical CPUs)", std::env::consts::OS),
    }
}

/// The one provenance line every artifact carries: date, build profile, machine.
///
/// A harness that emits latency without these three is not a measurement, it is
/// an anecdote — the profile alone moves the number by 2-5x, and a benchmark
/// that does not name its machine cannot be compared to another one.
pub fn provenance() -> String {
    format!(
        "Run {} from `--profile={}` on {}.",
        chrono::Utc::now().format("%Y-%m-%d"),
        profile(),
        host()
    )
}

/// Deterministic LCG shuffle (no extra deps; seed-stable slice like `--seed`).
/// Copied from `examples/longmemeval.rs:40-48`: the corpus layouts here depend on
/// where a signal row lands in insertion order, so the order has to be
/// reproducible across runs and machines.
pub fn shuffle(len: usize, seed: u64) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..len).collect();
    let mut s = seed.wrapping_add(0x9E3779B97F4A7C15);
    for i in (1..len).rev() {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        idx.swap(i, (s >> 33) as usize % (i + 1));
    }
    idx
}

/// `(main file, -wal, -shm)` bytes. Both halves matter: the store runs in WAL
/// mode, so while the process holds the database open most pages are still in
/// `-wal` and quoting the main file alone understates the real footprint.
pub fn db_bytes(db: &Path) -> (u64, u64, u64) {
    let main = fs::metadata(db).map(|m| m.len()).unwrap_or(0);
    let side = |suffix: &str| {
        fs::metadata(PathBuf::from(format!("{}-{suffix}", db.display())))
            .map(|m| m.len())
            .unwrap_or(0)
    };
    (main, side("wal"), side("shm"))
}

/// Remove the database and its two sidecars. `soak.rs` builds these names with
/// an explicit match because appending `"-"` unconditionally to name the main
/// file would produce a path nothing ever wrote; the same asymmetry here.
pub fn rm_db(db: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let path = match suffix.is_empty() {
            true => db.to_path_buf(),
            false => PathBuf::from(format!("{}-{}", db.display(), suffix)),
        };
        let _ = fs::remove_file(path);
    }
}

/// A port nothing is listening on right now: bind 0, read what the kernel gave,
/// drop it. In use for the ~1ms between the drop and the server binding.
pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap_or(0)
}

/// One HTTP/1.1 request, returning `(status, body)`.
///
/// Deliberately minimal and deliberately partial: the harnesses need the status
/// line, never a parsed body, so there is no chunked decoding, no redirect
/// handling and no keep-alive. `Connection: close` makes the server close after
/// one response, which is what lets a plain `read_to_string` terminate.
pub fn http(
    addr: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> std::io::Result<(u16, String)> {
    let mut sock = TcpStream::connect(addr)?;
    let head = match body {
        Some(b) => format!(
            "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            b.len()
        ),
        None => format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    };
    sock.write_all(head.as_bytes())?;
    if let Some(b) = body {
        sock.write_all(b.as_bytes())?;
    }
    let mut raw = String::new();
    sock.read_to_string(&mut raw)?;
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| std::io::Error::other(format!("no status line in {raw:?}")))?;
    let body = raw
        .split_once("\r\n\r\n")
        .map_or(String::new(), |(_, b)| b.to_string());
    Ok((status, body))
}

/// Poll `GET /health` until it answers 2xx, or give up. Returns how long the
/// server took to become answerable, measured from *after* `spawn` returned, so
/// this is the post-fork cost and the caller decides what to add to it.
pub fn wait_healthy(addr: &str, timeout: Duration) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok((code, body)) = http(addr, "GET", "/health", None) {
            // `/health` answers 2xx with the body `ok`; anything else on this
            // port is a foreign process, which must not read as "up".
            if (200..300).contains(&code) && body.trim() == "ok" {
                return Some(start.elapsed());
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

/// The `overlap` weight that `docs/EVALUATION_HYGIENE.md` §2.1 audits: the
/// winner of 46 configurations scored on LongMemEval-S's own 500 questions
/// (`eval/SWEEP_FUSION.md`, run 2026-09-28), so it was selected on the test set
/// (`AGENTS.md` §1). That history is unchanged — it is why the caveat below
/// travels with the weight — and the weight itself is unchanged by what follows.
///
/// **The `provisional:` marker on it is discharged, as a replication and not as
/// an endorsement** (`docs/CONSISTENCY.md` §14.1): `eval/LOCOMO.md` measured it
/// on 1,531 LoCoMo queries that never chose it, and it beat equal weight on
/// every ordering metric, by +5.6pp R@1, +1.4pp R@5, +2.9pp NDCG@10 and +3.7pp
/// MRR with R@20 flat. So the *direction* is confirmed on independent data and
/// the *magnitude* is not. The +4.2pp is still the maximum of 46 draws on the
/// test set, and §14.1's out-of-sample estimate is ≈94.4% R@5 rather than the
/// R@5 this weight measures here.
///
/// Named once, here, because a marker is worth nothing when it is emitted from
/// anything other than the weight the build actually used: a second hard-coded
/// `0.25` in a harness is how the marker and the weight drift apart, which is
/// the same failure five copies of [`out_md`] would be.
pub const FITTED_OVERLAP: f64 = 0.25;

/// The caveat paragraph a generated artifact carries when it reports retrieval
/// accuracy measured at [`FITTED_OVERLAP`], and `None` for every other weight.
///
/// The weight it describes is still fitted — see [`FITTED_OVERLAP`] — and the
/// paragraph says so. What changed is the *status*: an independent set has since
/// confirmed it, so `AGENTS.md` §1's `provisional:` marker is discharged, as a
/// replication rather than as an endorsement (`docs/CONSISTENCY.md` §14.1). The
/// paragraph therefore states the replication and its provenance, the part the
/// discharge does not cover — that the magnitude is not thereby earned — and the
/// null result recorded next to it in §14.2, so a reader meeting the word
/// "replicated" cannot over-read it in either direction.
///
/// `AGENTS.md` §2 requires a number in prose to exist in a committed artifact,
/// and a generated artifact quoting a fitted number is exactly where a reader is
/// most likely to take it for a generalisation estimate. Which is why the
/// paragraph belongs to the generator rather than to the file: one typed into
/// the artifact by hand is deleted by the next regeneration, and a caveat that
/// silently disappeared is worse than no caveat at all. The same rule that
/// carries the discharge is the rule that would have carried the marker, so the
/// two cannot disagree by construction.
///
/// `None` rather than a generic caveat for an unfitted weight, so a harness
/// re-based onto a value that was never chosen by looking at the test set drops
/// the paragraph by itself and nobody has to remember to delete it.
pub fn fitted_weight_note(weights: &FusionWeights) -> Option<String> {
    if (weights.overlap - FITTED_OVERLAP).abs() > 1e-9 {
        return None;
    }
    Some(format!(
        "> **replicated on independent data, magnitude not earned:** every retrieval \
         number in this artifact was measured at `overlap: {overlap:.2}` with `k: {k:.0}`. \
         That weight is fitted — it won 46 configurations scored on this same test set on \
         2026-09-28 (`AGENTS.md` §1, `eval/SWEEP_FUSION.md`) — and an independent set has \
         since confirmed the direction: over 1,531 LoCoMo queries that never chose it, \
         `0.25` beat the unfitted equal weight by +5.6pp R@1, +1.4pp R@5, +2.9pp NDCG@10 \
         and +3.7pp MRR, with R@20 unchanged (`eval/LOCOMO.md`; the decision in \
         `docs/CONSISTENCY.md` §14.1). `AGENTS.md` §1 holds a fitted value `provisional:` \
         until an independent set confirms it, so the marker is **discharged as a \
         replication, not as an endorsement** — and the size of the gain is not discharged \
         with it. The margin this weight shows over the unfitted configuration is still the \
         maximum of 46 draws on these same 500 questions, while the independently measured \
         value of that delta is +1.4pp; per `docs/CONSISTENCY.md` §14.1 the clean unfitted \
         LongMemEval number is 93.0% R@5 and the best out-of-sample estimate of the shipped \
         configuration is **≈94.4% R@5**. Read every column below as a measurement at a \
         weight whose direction is replicated and whose magnitude is not: it is not a \
         generalisation estimate, and the difference from an unfitted configuration must not \
         be quoted as earned. One null result stands beside it and is recorded, not acted \
         on: on LoCoMo, `overlap: 0.00` — the token-overlap stream deleted — ties `0.25` on \
         R@1 and NDCG@10, edges it by 0.3pp on R@5 and loses it by 0.2pp on MRR, so what \
         replicated is a direction, not a demonstrated need for this weight. LoCoMo banks \
         hold 19–32 documents against LongMemEval's ~50 and `tests/scale.rs`'s 5,000, so the \
         dev set under-measures the pool size at which a second lexical voter would pay, \
         which is why `docs/CONSISTENCY.md` §14.2 leaves the stream in place pending a \
         pool-scale re-test. No configuration is recommended here. See \
         `docs/EVALUATION_HYGIENE.md` §2.1 for the audit, §3 for the three-set protocol, \
         and `eval/SWEEP_FUSION.md` for the grid that chose the value.",
        overlap = weights.overlap,
        k = weights.k,
    ))
}
