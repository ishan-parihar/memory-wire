//! `daemon`: start the server in the background, stop it, and say whether it is
//! actually there.
//!
//! The reason this exists is a consequence, not a feature request. Our hooks are
//! never-fail by contract (`src/hooks.rs:3-7`): a down server prints local
//! framing and exits 0, because a memory server that is down must not put an
//! error in front of a model. That contract is right and it is not going to
//! change — but it turns "the server is not running" from a visible error into
//! **silent memory loss**, because a stopped server and a working server that
//! had nothing to say are indistinguishable from the model's side. A daemon plus
//! a `status` that can tell those apart is what makes the quiet hook safe rather
//! than merely quiet.
//!
//! Three decisions carry the design:
//!
//! - **The bind, not a lockfile, is the duplicate-daemon defence.** `start`
//!   attempts the bind in the foreground and refuses before spawning anything, so
//!   the error reaches the terminal instead of a log nobody is reading, and there
//!   is no lock to go stale and no lockfile to clean up. This is hindsight's
//!   arrangement (`main.py:322-332`).
//! - **Termination reuses the signal `serve` already handles.** `serve` drains
//!   in-flight requests on SIGTERM (`with_graceful_shutdown`), so `stop` sends
//!   SIGTERM and waits; there is no second signal path and no new shutdown mode.
//! - **A pid is not an identity.** The number is reused once the process it
//!   named exits, so the state file records the process's start time next to the
//!   pid and every command compares it. A recycled pid is reported as somebody
//!   else's process and is never signalled.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::doctor::{probe_server, ServerState};
use crate::paths;
use memory_wire::store::default_db_path;

/// Name of the state file, inside the store's own directory.
const STATE_FILE: &str = "serve.json";

/// Name of the file a detached server's stdout and stderr go to.
const LOG_FILE: &str = "serve.log";

/// How long `start` waits for the new server to answer `/health` before it
/// reports failure.
///
/// A spawn returning a pid proves the process exists, not that it is serving,
/// and the two come apart the moment the child cannot open the store. This is a
/// bounded confirmation, not a blocking start: the child is never waited on, the
/// deadline is fixed, and a server that binds in under a second — which is what
/// this one does — is unaffected. ponytail: fixed 5s ceiling; raise it if a
/// store on a slow network filesystem ever needs it to bind.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Gap between readiness probes while waiting out [`READY_TIMEOUT`].
const READY_POLL: Duration = Duration::from_millis(50);

/// How long `stop` waits for a signalled daemon to exit.
///
/// Every handler is one SQLite transaction or one bounded recall and the surface
/// has no long-poll, so a drain is short; ten seconds is generous rather than
/// tuned. `SIGKILL` remains available to an operator who wants the alternative,
/// and the refusal below says so rather than escalating on its own.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// Gap between liveness checks while waiting out [`STOP_TIMEOUT`].
const STOP_POLL: Duration = Duration::from_millis(25);

/// What `$XDG_DATA_HOME/memory-wire/serve.json` records about a running server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServeState {
    /// Pid of the detached `serve` child.
    pub pid: i32,
    /// The `--addr` the child was given, verbatim.
    pub addr: String,
    /// The store the child was given, or the default it resolved to.
    pub db: String,
    /// Unix epoch seconds at which `daemon start` spawned the child.
    pub started_at: i64,
    /// `/proc/<pid>/stat` field 22, read at spawn time.
    ///
    /// This is what makes a pid an identity. `None` where the kernel does not
    /// publish it — every platform but Linux — which weakens the recycled-pid
    /// check to the endpoint probe alone. It is `#[serde(default)]` so a state
    /// file written before this field existed still parses, and the weakened
    /// check is *said* rather than silently assumed.
    #[serde(default)]
    pub start_ticks: Option<u64>,
}

/// How sure we are that a live pid is the process `daemon start` spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Identity {
    /// The recorded start time is this process's start time.
    Matched,
    /// There is no start time on this platform to compare, so a recycled pid
    /// cannot be ruled out and the endpoint probe is all the evidence there is.
    Unavailable,
}

impl Identity {
    /// The sentence that goes next to the pid in a report.
    fn note(self) -> &'static str {
        match self {
            Identity::Matched => "recorded start time matches, so this is the process `daemon start` spawned",
            Identity::Unavailable => {
                "this platform publishes no process start time, so the pid could not be checked for reuse"
            }
        }
    }
}

/// May `stop` send SIGTERM to a pid in this state?
///
/// A live pid is not an identity — the number is reused once the process it
/// named exits. But where the kernel publishes a start time and it matches, the
/// pid *is* the process `daemon start` spawned, and a daemon that has wedged
/// with its listener gone is exactly the case where refusing to stop leaves the
/// user with nothing but a manual `kill`. Where no start time is published, the
/// endpoint probe is the only evidence available and a silent endpoint is not
/// evidence, so the conservative answer stands.
///
/// Split out from `stop` so it is testable: proving the rule needs a live pid
/// and a dead port, and signalling the one live pid available in a test would
/// end the test.
fn may_signal(identity: Identity) -> bool {
    matches!(identity, Identity::Matched)
}

/// What the state file plus the live system add up to.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// Provably the daemon we started, and answering `ok` on its endpoint.
    Serving(ServeState),
    /// The recorded pid exists, and the endpoint does not answer.
    ///
    /// Both facts, always. This is the state that must never be reported as
    /// healthy (the process may be mid-start, wedged, or serving something else)
    /// and must never be killed to tidy up (the pid may have been reused).
    AliveNotServing {
        /// What the state file claims.
        state: ServeState,
        /// Whether the pid could be tied to that claim.
        identity: Identity,
        /// Why the probe failed, verbatim from the health check.
        why: String,
    },
    /// The recorded pid is gone: the state file is stale.
    Gone(ServeState),
    /// The recorded pid is alive but started at a different time, so the daemon
    /// this file describes has exited and the number now belongs to somebody
    /// else.
    Recycled(ServeState),
    /// No state file.
    Absent,
    /// A state file exists and does not parse.
    Corrupt(String),
}

impl Verdict {
    /// True only when the daemon is both provably ours and answering.
    fn serving(&self) -> bool {
        matches!(self, Verdict::Serving(_))
    }
}

/// `daemon`'s subcommand.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Action {
    /// Spawn a detached `serve` and prove it answers.
    Start {
        /// Bind address, passed to `serve --addr` unchanged. Loopback by
        /// default, because there is no authentication.
        #[arg(long, default_value = "127.0.0.1:8899")]
        addr: String,
        /// SQLite database path, passed to `serve --db` (default:
        /// $XDG_DATA_HOME/memory-wire/memory.db).
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// SIGTERM the recorded daemon, wait for it to drain, and drop the state file.
    Stop,
    /// Report the recorded daemon and whether its endpoint answers.
    Status,
}

/// Run one `daemon` subcommand; returns the process exit code.
pub fn run(action: Action) -> i32 {
    match action {
        Action::Start { addr, db } => start(&addr, db),
        Action::Stop => stop(),
        Action::Status => status(),
    }
}

// ---------------------------------------------------------------- state file

/// Directory holding the state file: the parent of the default store path.
///
/// Derived from [`default_db_path`] rather than from a second reading of
/// `XDG_DATA_HOME`, so the state file cannot drift from where the store lands
/// when that variable is unset, relative, or changed. The state file is
/// deliberately *not* derived from `--db`: there is one of it, and a server
/// started against a scratch store must still be stoppable from anywhere.
fn state_dir() -> PathBuf {
    default_db_path()
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// Where the state file lives.
fn state_path() -> PathBuf {
    state_dir().join(STATE_FILE)
}

/// Where a detached server's stdout and stderr go.
fn log_path() -> PathBuf {
    state_dir().join(LOG_FILE)
}

/// `http://` prefix for a `--addr` value, which is the endpoint the health
/// check is made against.
fn endpoint_of(addr: &str) -> String {
    format!("http://{addr}")
}

/// The recorded state, `None` when there is no state file, and the reason when
/// there is one that cannot be read as state.
///
/// Absent and unparseable are different answers and stay different all the way
/// out to the screen: absent means "never started", unparseable means "we cannot
/// tell", and the second one is never resolved by quietly overwriting the file.
fn read_state_at(path: &Path) -> Result<Option<ServeState>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Record the state atomically, so a crash mid-write cannot leave a state file
/// that parses as something else.
fn write_state_at(path: &Path, state: &ServeState) -> Result<(), String> {
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    paths::write_atomic(path, &format!("{json}\n"))
}

/// Remove the state file; `true` when one was there.
fn clear_state_at(path: &Path) -> bool {
    std::fs::remove_file(path).is_ok()
}

/// The state file in the ambient location.
fn inspect() -> Verdict {
    inspect_at(&state_path())
}

/// What the state file at `path` and the live system add up to.
///
/// The order is the one the design turns on: **pid, then identity, then the
/// endpoint.** A live pid proves nothing by itself — the number is recycled — so
/// a matching start time is what promotes it to "this is the process we
/// started", and only then does the endpoint decide whether it is serving.
/// Checking the endpoint first is how a stranger's process holding the
/// memory-wire port gets reported as our daemon, and it is how `stop` ends up
/// signalling somebody else's pid.
fn inspect_at(path: &Path) -> Verdict {
    let state = match read_state_at(path) {
        Ok(Some(s)) => s,
        Ok(None) => return Verdict::Absent,
        Err(why) => return Verdict::Corrupt(why),
    };
    if !alive(state.pid) {
        return Verdict::Gone(state);
    }
    let identity = match (state.start_ticks, start_ticks(state.pid)) {
        (Some(recorded), Some(live)) if recorded == live => Identity::Matched,
        (Some(_), Some(_)) => return Verdict::Recycled(state),
        _ => Identity::Unavailable,
    };
    match probe_server(&endpoint_of(&state.addr)) {
        ServerState::Up => Verdict::Serving(state),
        ServerState::Down(why) => Verdict::AliveNotServing { state, identity, why },
    }
}

// ------------------------------------------------------------------- process

/// Is there a process with this pid?
///
/// `kill(pid, 0)` is the check that separates "no such process" from "there, but
/// not ours to signal", and it is the only one of the three that means the same
/// thing on every platform this crate builds for. Reading `/proc` would answer
/// the first question and not the second, and only on Linux.
fn alive(pid: i32) -> bool {
    // Safety: signal 0 delivers nothing. It is the documented existence and
    // permission check, takes no pointer, and needs nothing preallocated.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    // EPERM means the process exists and belongs to somebody else. ESRCH is the
    // only answer that means gone.
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// When the kernel started this process, in clock ticks since boot —
/// `/proc/<pid>/stat` field 22 — or `None` where that is not published.
///
/// Field 2 is the executable name in parentheses and may itself contain spaces
/// and parentheses, so the fields are counted from the **last** `)` rather than
/// by splitting the whole line. `state` is field 3, so field 22 is the 20th
/// token after it.
#[cfg(target_os = "linux")]
fn start_ticks(pid: i32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?.1.split_whitespace().nth(19)?.parse().ok()
}

/// No process start time off Linux; see the `linux` arm.
#[cfg(not(target_os = "linux"))]
fn start_ticks(_pid: i32) -> Option<u64> {
    None
}

// --------------------------------------------------------------------- start

fn start(addr: &str, db: Option<PathBuf>) -> i32 {
    // Refuse on what is already recorded, before touching the port. Every
    // refusal here leaves the state file alone: `status` must still be able to
    // report what is actually there.
    match inspect() {
        Verdict::Serving(state) => {
            // A second daemon is refused whatever address it was asked for, and
            // not only because the port may be taken: there is one state file,
            // so a second server would be a process no command in this binary
            // could ever stop. The requested address is named anyway — a refusal
            // that talks only about a different port reads as a non-sequitur.
            return refuse(&format!(
                "already serving on http://{} as pid {}, so nothing was started on {addr}\n  \
                 one state file ({}), so a second daemon could not be stopped either\n  \
                 `memory-wire daemon status` reports it, `memory-wire daemon stop` ends it\n  \
                 to run on a different address, stop that one first",
                state.addr, state.pid, state_path().display(),
            ));
        }
        Verdict::AliveNotServing { state, identity, why } => {
            return refuse(&format!(
                "pid {} is alive but http://{} does not answer ({why}), so it is not being \
                 reported as a running daemon and it is not being killed\n  \
                 {}\n  \
                 there is one state file ({}), so a second daemon could not be stopped either.\n  \
                 if it really is the one `daemon start` spawned, stop it by hand: kill {}",
                state.pid,
                state.addr,
                identity.note(),
                state_path().display(),
                state.pid
            ));
        }
        Verdict::Gone(_) | Verdict::Recycled(_) => {
            // Stale: the recorded process is gone, or the number now belongs to
            // somebody else. Either way the file is describing nothing, so it is
            // removed rather than left to be read as a live daemon.
            println!(
                "memory-wire daemon: removing stale state at {}",
                state_path().display()
            );
            clear_state_at(&state_path());
        }
        Verdict::Corrupt(why) => {
            // Said out loud, and before the write, because overwriting is the
            // only way forward from here and a silent overwrite would destroy
            // the only evidence of whatever wrote it.
            eprintln!("memory-wire daemon: {why} does not parse; replacing it");
        }
        Verdict::Absent => {}
    }

    // The bind is attempted **here**, in the foreground, and the listener is
    // dropped before anything is spawned. That is the whole duplicate-daemon
    // defence: port binding prevents a second daemon from ever existing, and
    // attempting it in the parent is what puts the error on the terminal instead
    // of in a log nobody is reading. No lockfile, so there is nothing to go
    // stale and nothing to clean up.
    if let Err(e) = std::net::TcpListener::bind(addr) {
        let why = match probe_server(&endpoint_of(addr)) {
            ServerState::Up => format!(
                "cannot bind {addr}: {e}\n  \
                 GET http://{addr}/health answered `ok`, so a memory-wire server is already \
                 serving there\n  \
                 stop it with `memory-wire daemon stop` if this command started it"
            ),
            ServerState::Down(detail) => {
                format!("cannot bind {addr}: {e}\n  and it is not a memory-wire server: {detail}")
            }
        };
        return refuse(&why);
    }

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return refuse(&format!("cannot find this executable to re-exec: {e}")),
    };
    let log = log_path();
    // The log is opened before anything else writes, and `File::create` does not
    // make parents, so a first run on a clean machine would otherwise fail on the
    // one directory `daemon` exists to fill.
    if let Err(e) = std::fs::create_dir_all(state_dir()) {
        return refuse(&format!("cannot create {}: {e}", state_dir().display()));
    }
    let log_file = match std::fs::File::create(&log) {
        Ok(f) => f,
        // No log means a detached server with nowhere to say why it died, which
        // is the failure mode this whole command exists to prevent.
        Err(e) => return refuse(&format!("cannot open the server log {}: {e}", log.display())),
    };

    // Two descriptors into one log, so an interleaved panic message and a
    // shutdown line land in the same file in the order they happened.
    let log_out = match log_file.try_clone() {
        Ok(f) => f,
        Err(e) => return refuse(&format!("cannot open the server log {}: {e}", log.display())),
    };

    let mut cmd = Command::new(exe);
    cmd.arg("serve").arg("--addr").arg(addr);
    if let Some(db) = &db {
        cmd.arg("--db").arg(db);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(log_out))
        .stderr(Stdio::from(log_file));
    detach(&mut cmd);

    let child = match cmd.spawn() {
        Ok(c) => c,
        // A `pre_exec` failure lands here too, so a child that could not detach
        // is never reported as a running daemon.
        Err(e) => return refuse(&format!("could not spawn the detached server: {e}")),
    };
    let pid = match i32::try_from(child.id()) {
        Ok(p) => p,
        Err(_) => return refuse(&format!("child pid {} does not fit a pid", child.id())),
    };
    // Not waited on, and that is the point: dropping a `Child` sends no signal,
    // the pid is recorded below, and the server outlives this process.
    drop(child);

    let state = ServeState {
        pid,
        addr: addr.to_string(),
        db: db.unwrap_or_else(default_db_path).display().to_string(),
        started_at: now_epoch(),
        start_ticks: start_ticks(pid),
    };
    if let Err(why) = write_state_at(&state_path(), &state) {
        return refuse(&format!(
            "the server was started as pid {pid} but its state could not be recorded: {why}\n  \
             it is running and `memory-wire daemon stop` will not be able to reach it; \
             kill {pid} by hand"
        ));
    }

    // Prove it before claiming it.
    if let Err(why) = wait_until_serving(&state) {
        return refuse(&format!(
            "the server was started as pid {pid} but http://{} did not answer within \
             {READY_TIMEOUT:?}: {why}\n  \
             {}\n  \
             the state at {} was left in place — `memory-wire daemon status` reports what it \
             is now, and a later `daemon start` clears it if the process is gone",
            state.addr,
            log_tail(&log, 5),
            state_path().display()
        ));
    }

    println!(
        "memory-wire daemon started\n  \
         pid        {pid}\n  \
         endpoint   http://{}\n  \
         db         {}\n  \
         log        {}\n  \
         state      {}\n  \
         stop with  memory-wire daemon stop",
        state.addr,
        state.db,
        log.display(),
        state_path().display()
    );
    0
}

/// Put the child in a session of its own, so closing the terminal it was started
/// from cannot reach it.
///
/// `setsid` is the call that matters: the standard library has
/// `Command::process_group`, which is `setpgid(0, 0)` and does **not** create a
/// session, so the child stays in a process group that a terminal hangup can
/// still signal. `libc` was already in the tree through tokio, so this costs
/// nothing at runtime and adds no shared library.
fn detach(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // Safety: `pre_exec` runs between fork and exec, where the only permitted
        // work is async-signal-safe. `setsid` is one syscall, allocates nothing,
        // and touches no memory this process reads again; returning the errno
        // aborts the spawn, which is the fail-loudly behaviour the detach needs.
        unsafe {
            cmd.pre_exec(|| match libc::setsid() {
                // Failure is -1, and *only* -1. Testing for 0 instead looks
                // equivalent and is not: on this platform glibc's `setsid`
                // returns the new session id on success, so a `== 0` test
                // reports a successful detach as a failed spawn. `-1` is the
                // documented error return and is what the standard library
                // itself compares against, so it holds either way.
                -1 => Err(std::io::Error::last_os_error()),
                _ => Ok(()),
            });
        }
    }
    // Everywhere else `setsid` does not exist. `process_group(0)` is the closest
    // the standard library offers and it is strictly weaker — the child leaves
    // this terminal's foreground group but stays in the session, so a hangup can
    // still find it. Documented here rather than pretended away.
    #[cfg(not(unix))]
    cmd.process_group(0);
}

/// Poll `state`'s endpoint until it answers `ok` or the deadline passes.
fn wait_until_serving(state: &ServeState) -> Result<(), String> {
    let endpoint = endpoint_of(&state.addr);
    let deadline = Instant::now() + READY_TIMEOUT;
    let mut last = ServerState::Down("no probe completed".to_string());
    while Instant::now() < deadline {
        last = probe_server(&endpoint);
        if last == ServerState::Up {
            return Ok(());
        }
        std::thread::sleep(READY_POLL);
    }
    Err(last.label())
}

// ---------------------------------------------------------------------- stop

fn stop() -> i32 {
    match inspect() {
        Verdict::Absent => {
            println!(
                "memory-wire daemon: nothing running; no state file at {}",
                state_path().display()
            );
            0
        }
        // Stopping a stopped daemon is not an error: both of these are the
        // "already stopped" answer with the extra fact that says which.
        Verdict::Gone(state) => {
            clear_state_at(&state_path());
            println!(
                "memory-wire daemon: nothing running; pid {} is gone, and the stale state at {} \
                 was removed",
                state.pid,
                state_path().display()
            );
            0
        }
        Verdict::Recycled(state) => {
            clear_state_at(&state_path());
            println!(
                "memory-wire daemon: nothing to stop; pid {} is alive but was started at a \
                 different time, so it is somebody else's process and was not signalled. \
                 The stale state at {} was removed",
                state.pid,
                state_path().display()
            );
            0
        }
        Verdict::Corrupt(why) => {
            // Exit 0: a state file that does not parse names no process, so
            // nothing was provably running. It is deliberately not deleted here —
            // `daemon start` is where replacing it is said out loud.
            println!("memory-wire daemon: nothing to stop; {why} does not parse");
            0
        }
        // A silent endpoint is not evidence of a stranger when the start time
        // says otherwise. `Identity::Matched` means the recorded start time is
        // this process's start time, so it is the process `daemon start`
        // spawned and nothing else — a daemon that has wedged with its listener
        // gone is exactly the case where refusing to stop leaves the user with
        // no way out but a manual `kill`. Only when identity is unavailable
        // (no start time on this platform) does the endpoint remain the sole
        // evidence, and there the conservative refusal stands.
        Verdict::AliveNotServing {
            state,
            identity,
            ..
        } if may_signal(identity) => terminate(&state),
        Verdict::AliveNotServing { state, identity, why } => refuse(&format!(
            "pid {} is alive but http://{} does not answer ({why}), and this platform publishes no \
             process start time, so the pid cannot be proven ours and will not be signalled\n  \
             {}\n  \
             if it really is the one `daemon start` spawned, stop it by hand: kill {}",
            state.pid,
            state.addr,
            identity.note(),
            state.pid
        )),
        Verdict::Serving(state) => terminate(&state),
    }
}

/// SIGTERM the daemon, wait for it to leave, and drop the state file.
///
/// SIGTERM is not a new signal path: it is the one `serve` already handles, and
/// `with_graceful_shutdown` drains the requests in flight before the process
/// ends — exactly as it does for a Ctrl-C. What is added here is the proof that
/// the pid is ours, and the wait, so `stop` returning means stopped.
fn terminate(state: &ServeState) -> i32 {
    // Safety: SIGTERM to a pid this function has just proven is the server this
    // command started — `Verdict::Serving` means the pid is live, the recorded
    // start time matched wherever the kernel publishes one, and the endpoint
    // answered `ok`.
    if unsafe { libc::kill(state.pid, libc::SIGTERM) } != 0 {
        return refuse(&format!(
            "could not signal pid {}: {}",
            state.pid,
            std::io::Error::last_os_error()
        ));
    }
    let deadline = Instant::now() + STOP_TIMEOUT;
    while alive(state.pid) {
        if Instant::now() >= deadline {
            return refuse(&format!(
                "sent SIGTERM to pid {} but it was still alive after {STOP_TIMEOUT:?}\n  \
                 its in-flight requests may still be draining — `memory-wire daemon status` \
                 says whether it is there, and kill -9 {} is the operator's alternative",
                state.pid, state.pid
            ));
        }
        std::thread::sleep(STOP_POLL);
    }
    clear_state_at(&state_path());
    println!(
        "memory-wire daemon stopped: pid {} exited and the state at {} was removed",
        state.pid,
        state_path().display()
    );
    0
}

// -------------------------------------------------------------------- status

fn status() -> i32 {
    let verdict = inspect();
    println!("{}", render(&verdict));
    // Scriptable: `daemon status` is the check, so it is a nonzero exit exactly
    // when there is no server to talk to. Same convention as `doctor --strict`.
    i32::from(!verdict.serving())
}

/// The status screen.
fn render(verdict: &Verdict) -> String {
    let mut out = String::from("memory-wire daemon\n");
    match verdict {
        Verdict::Serving(state) => out.push_str(&format!(
            "  state      serving\n  \
             pid        {}\n  \
             uptime     {}  (started {})\n  \
             endpoint   http://{}\n  \
             db         {}\n  \
             state file {}\n  \
             log        {}\n  \
             stop with  memory-wire daemon stop",
            state.pid,
            human_duration(now_epoch() - state.started_at),
            stamp(state.started_at),
            state.addr,
            state.db,
            state_path().display(),
            log_path().display(),
        )),
        // Both facts, on adjacent lines, because either one alone is the thing
        // that misleads: the pid alone says "running" and the endpoint alone
        // says "not running", and the truth is that we know one and not the
        // other.
        Verdict::AliveNotServing { state, identity, why } => out.push_str(&format!(
            "  state      pid alive, not serving\n  \
             pid        {}  ({})\n  \
             endpoint   http://{}\n  \
             probe      {why}\n  \
             db         {}\n  \
             state file {}\n  \
             log        {}\n  \
             this is not a running daemon: `daemon stop` will not signal it, and nothing here \
             kills it for you",
            state.pid,
            identity.note(),
            state.addr,
            state.db,
            state_path().display(),
            log_path().display(),
        )),
        Verdict::Gone(state) => out.push_str(&format!(
            "  state      not running\n  \
             pid        {}  (no such process)\n  \
             endpoint   http://{}\n  \
             db         {}\n  \
             state file {}  (stale)\n  \
             `daemon start` clears the stale file and starts a server",
            state.pid, state.addr, state.db, state_path().display(),
        )),
        Verdict::Recycled(state) => out.push_str(&format!(
            "  state      not running\n  \
             pid        {}  (alive, but started at a different time — this is not this daemon)\n  \
             endpoint   http://{}\n  \
             db         {}\n  \
             state file {}  (stale)\n  \
             `daemon start` clears the stale file and starts a server",
            state.pid, state.addr, state.db, state_path().display(),
        )),
        Verdict::Absent => out.push_str(&format!(
            "  state      not running\n  \
             endpoint   {}\n  \
             state file {}  (absent)\n  \
             start one with `memory-wire daemon start`",
            paths::endpoint(),
            state_path().display(),
        )),
        Verdict::Corrupt(why) => out.push_str(&format!(
            "  state      corrupt\n  \
             {why}\n  \
             it was not overwritten. `memory-wire daemon start` says so and then replaces it; \
             read it first if you want to know what wrote it",
        )),
    }
    out
}

// ------------------------------------------------------------------- helpers

/// Print a refusal on stderr and hand back the exit code for it.
///
/// One helper so every failure path leaves the same shape and the same nonzero
/// status instead of each inventing its own — and so "never report success for a
/// daemon that is not running" is one decision rather than a habit to remember
/// at each return.
fn refuse(why: &str) -> i32 {
    eprintln!("memory-wire daemon: {why}");
    1
}

/// Seconds since the Unix epoch, or 0 if the clock is before it.
fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// `45s`, `3m 12s`, `2h 5m`, `3d 4h` — two units at most, because nobody reads
/// a daemon's uptime past that.
fn human_duration(secs: i64) -> String {
    let s = secs.max(0);
    match s {
        s if s < 60 => format!("{s}s"),
        s if s < 3_600 => format!("{}m {}s", s / 60, s % 60),
        s if s < 86_400 => format!("{}h {}m", s / 3600, (s % 3_600) / 60),
        s => format!("{}d {}h", s / 86_400, (s % 86_400) / 3_600),
    }
}

/// RFC 3339 UTC for a start timestamp, or `-` for one chrono will not render.
fn stamp(epoch: i64) -> String {
    chrono::DateTime::from_timestamp(epoch, 0).map_or_else(
        || "-".to_string(),
        |t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    )
}

/// The last `n` lines of the server log, for a failure message.
///
/// A detached server's terminal is gone, so its log is the only place its own
/// complaint exists; a start that cannot prove the server came up has nothing
/// else to show.
fn log_tail(path: &Path, n: usize) -> String {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return format!("{} is unreadable: {e}", path.display()),
    };
    let lines: Vec<&str> = text.lines().rev().take(n).collect();
    if lines.is_empty() {
        return format!("{} wrote nothing", path.display());
    }
    let mut out = format!(
        "the last {} line(s) of {}:",
        lines.len(),
        path.display()
    );
    for line in lines.into_iter().rev() {
        out.push_str(&format!("\n  {line}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// A scratch directory holding a state file, removed when it drops.
    struct Tmp(PathBuf);

    impl Tmp {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("mw-daemon-{tag}-{}", self_pid()));
            std::fs::remove_dir_all(&p).ok();
            std::fs::create_dir_all(&p).expect("tmp dir");
            Self(p)
        }

        fn state_file(&self) -> PathBuf {
            self.0.join(STATE_FILE)
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    /// A loopback server answering `/health` with exactly the body memory-wire
    /// answers with, so `Verdict::Serving` can be reached without a real daemon.
    ///
    /// The responder outlives the test on purpose: it accepts in a loop rather
    /// than once, because a status render probes and a re-render would otherwise
    /// hang on the second call.
    fn answering_ok() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            while let Ok((mut sock, _)) = listener.accept() {
                let mut raw = [0u8; 1024];
                let _ = sock.read(&mut raw);
                let _ = sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            }
        });
        format!("{addr}")
    }

    /// A port nothing is listening on, taken from the kernel and released.
    fn free_addr() -> String {
        std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind")
            .local_addr()
            .expect("addr")
            .to_string()
    }

    /// This process's pid, as the `i32` every pid in this module is.
    fn self_pid() -> i32 {
        i32::try_from(std::process::id()).expect("a pid fits an i32")
    }

    /// A pid that is guaranteed not to exist: a child that has been waited for.
    fn dead_pid() -> i32 {
        let mut child = std::process::Command::new("true").spawn().expect("spawn true");
        child.wait().expect("wait");
        i32::try_from(child.id()).expect("pid fits an i32")
    }

    /// A state file claiming `pid` is serving `addr`.
    fn claiming(pid: i32, addr: &str) -> ServeState {
        ServeState {
            pid,
            addr: addr.to_string(),
            db: "/tmp/does-not-matter.db".to_string(),
            started_at: now_epoch(),
            start_ticks: start_ticks(pid),
        }
    }

    // The file is the only thing that survives the process, so it has to hold
    // everything the other two commands need: which pid, on what endpoint,
    // against which store, and since when.
    #[test]
    fn a_state_file_should_round_trip() {
        let t = Tmp::new("roundtrip");
        let state = ServeState {
            pid: 4242,
            addr: "127.0.0.1:18899".to_string(),
            db: "/tmp/agents.db".to_string(),
            started_at: 1_700_000_000,
            start_ticks: Some(987_654),
        };
        write_state_at(&t.state_file(), &state).expect("write");
        assert_eq!(
            read_state_at(&t.state_file()).expect("read"),
            Some(state.clone()),
            "every field the other commands read must survive the write"
        );

        // A state file written before `start_ticks` existed still parses, and
        // says so by being absent rather than by failing.
        std::fs::write(t.state_file(), r#"{"pid":1,"addr":"a:1","db":"b","started_at":2}"#)
            .expect("write legacy");
        let old = read_state_at(&t.state_file()).expect("read legacy").expect("some");
        assert_eq!(old.start_ticks, None);
        assert_eq!(old.pid, 1);

        assert!(clear_state_at(&t.state_file()));
        assert_eq!(read_state_at(&t.state_file()).expect("read after clear"), None);
        assert!(!clear_state_at(&t.state_file()), "removing twice is not an error");
    }

    // A state file we cannot read names no process, so it is never treated as a
    // daemon and never silently replaced.
    #[test]
    fn a_corrupt_state_file_should_be_reported_not_guessed() {
        let t = Tmp::new("corrupt");
        for (bytes, why) in [
            ("{ not json", "not json at all"),
            ("[]", "valid JSON, wrong shape"),
            ("", "empty"),
        ] {
            std::fs::write(t.state_file(), bytes).expect("write");
            let err = read_state_at(&t.state_file()).expect_err("must not parse");
            assert!(err.starts_with(&t.state_file().display().to_string()), "{err}");
            assert!(!err.contains(why), "sanity: {why}");

            let verdict = inspect_at(&t.state_file());
            let Verdict::Corrupt(reported) = &verdict else {
                panic!("{why} must read as corrupt, not as a daemon: {verdict:?}");
            };
            assert!(reported.contains(&t.state_file().display().to_string()), "{reported}");
            let screen = render(&verdict);
            assert!(screen.contains("corrupt"), "{screen}");
            assert!(screen.contains("not overwritten"), "{screen}");
            // Still on disk: reporting it is not the same as cleaning it up.
            assert!(t.state_file().exists(), "{why}");
        }
    }

    // The staleness rule: a pid that no longer exists is not a daemon, and the
    // file describing it must not block a later `start`.
    #[test]
    fn a_stale_pid_should_read_as_gone() {
        let t = Tmp::new("stale");
        let state = claiming(dead_pid(), &free_addr());
        write_state_at(&t.state_file(), &state).expect("write");

        let verdict = inspect_at(&t.state_file());
        assert_eq!(verdict, Verdict::Gone(state), "{verdict:?}");
        assert!(!verdict.serving(), "a dead pid is never a serving daemon");

        let screen = render(&verdict);
        assert!(screen.contains("not running"), "{screen}");
        assert!(screen.contains("stale"), "{screen}");
        assert!(screen.contains("no such process"), "{screen}");
    }

    // The distinction the whole design turns on: a live pid says the process
    // exists and says nothing about the server. Reporting this as healthy is how
    // a stopped daemon stays indistinguishable from a silent one.
    #[test]
    fn a_live_pid_on_a_silent_endpoint_should_not_read_as_serving() {
        let t = Tmp::new("alive-silent");
        let addr = free_addr();
        let state = claiming(self_pid(), &addr);
        write_state_at(&t.state_file(), &state).expect("write");

        let verdict = inspect_at(&t.state_file());
        let Verdict::AliveNotServing { state: seen, identity, why } = &verdict else {
            panic!("a live pid on a dead endpoint must not be a daemon: {verdict:?}");
        };
        assert_eq!(seen.pid, self_pid(), "the pid is reported, not dropped");
        assert!(!why.is_empty(), "the probe's own reason is carried, not summarised away");
        assert_eq!(
            *identity,
            match start_ticks(self_pid()) {
                Some(_) => Identity::Matched,
                None => Identity::Unavailable,
            }
        );

        let screen = render(&verdict);
        assert!(screen.contains("pid alive, not serving"), "{screen}");
        assert!(screen.contains(&format!("pid        {}", self_pid())), "{screen}");
        assert!(screen.contains(&addr), "{screen}");
        assert!(screen.contains(identity.note()), "{screen}");
        // The two things a reader must not do with this state.
        assert!(!screen.contains("state      serving"), "{screen}");
        assert!(screen.contains("will not signal it"), "{screen}");
    }

    // The other half of the same distinction: when the endpoint does answer, and
    // the pid is the one recorded, this is the only verdict that is healthy.
    #[test]
    fn a_live_pid_on_a_health_ok_endpoint_should_read_as_serving() {
        let t = Tmp::new("alive-serving");
        let state = claiming(self_pid(), &answering_ok());
        write_state_at(&t.state_file(), &state).expect("write");

        let verdict = inspect_at(&t.state_file());
        assert!(verdict.serving(), "{verdict:?}");
        let screen = render(&verdict);
        assert!(screen.contains("state      serving"), "{screen}");
        assert!(screen.contains("uptime"), "{screen}");
        assert!(screen.contains("http://"), "{screen}");
    }

    // A 2xx is not proof: the same live pid, on a port that answers something
    // else entirely, is still not our server. This is doctor's rule, reused
    // rather than reimplemented, so the two cannot disagree.
    #[test]
    fn a_foreign_two_hundred_should_not_read_as_serving() {
        let t = Tmp::new("foreign");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            while let Ok((mut sock, _)) = listener.accept() {
                let mut raw = [0u8; 1024];
                let _ = sock.read(&mut raw);
                let _ = sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 21\r\n\r\n<html>someone</html>\n");
            }
        });
        let state = claiming(self_pid(), &addr.to_string());
        write_state_at(&t.state_file(), &state).expect("write");

        let verdict = inspect_at(&t.state_file());
        let Verdict::AliveNotServing { why, .. } = &verdict else {
            panic!("a foreign 200 is not this daemon: {verdict:?}");
        };
        assert!(why.contains("unexpected health response"), "{why}");
    }

    // A live pid is not an identity, so the signal decision leans on the start
    // time where the kernel publishes one. Proven here rather than by signalling
    // a live process, which would end the test.
    #[test]
    fn only_a_start_time_match_authorises_signalling() {
        assert!(may_signal(Identity::Matched), "a matched start time is proof of identity");
        assert!(
            !may_signal(Identity::Unavailable),
            "without a start time the pid is only a hint, so the endpoint is all the evidence"
        );
    }

    // A pid that exists, started at a different time, is somebody else's
    // process. Reporting it as our daemon — or signalling it — is the one mistake
    // here that hurts a stranger, so it is tested on Linux where the start time
    // is published and the check is real.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_recycled_pid_should_not_read_as_our_daemon() {
        let t = Tmp::new("recycled");
        let live = start_ticks(self_pid()).expect("linux publishes a start time");
        let mut state = claiming(self_pid(), &free_addr());
        // The same pid, a different process: exactly what a reboot or a long
        // enough uptime produces.
        state.start_ticks = Some(live.wrapping_add(1));
        write_state_at(&t.state_file(), &state).expect("write");

        let verdict = inspect_at(&t.state_file());
        assert_eq!(verdict, Verdict::Recycled(state), "{verdict:?}");
        assert!(!verdict.serving(), "a recycled pid is never our daemon");
        let screen = render(&verdict);
        assert!(screen.contains("not running"), "{screen}");
        assert!(screen.contains("different time"), "{screen}");
        assert!(screen.contains("stale"), "{screen}");
    }

    #[test]
    fn no_state_file_should_read_as_not_running() {
        let t = Tmp::new("absent");
        let verdict = inspect_at(&t.state_file());
        assert_eq!(verdict, Verdict::Absent);
        assert!(!verdict.serving());
        let screen = render(&verdict);
        assert!(screen.contains("not running"), "{screen}");
        assert!(screen.contains("daemon start"), "{screen}");
    }

    #[test]
    fn uptime_should_scale_units_and_never_go_negative() {
        assert_eq!(human_duration(0), "0s");
        assert_eq!(human_duration(45), "45s");
        assert_eq!(human_duration(60), "1m 0s");
        assert_eq!(human_duration(192), "3m 12s");
        assert_eq!(human_duration(3_600), "1h 0m");
        assert_eq!(human_duration(7_500), "2h 5m");
        assert_eq!(human_duration(86_400), "1d 0h");
        assert_eq!(human_duration(274_800), "3d 4h");
        // A clock that moved backwards must not print a negative age.
        assert_eq!(human_duration(-5), "0s");
    }

    #[test]
    fn a_timestamp_should_render_as_utc_or_as_a_dash() {
        assert_eq!(stamp(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(stamp(i64::MIN), "-", "an unrenderable second is a dash, not a panic");
    }

    #[test]
    fn a_log_with_nothing_in_it_should_say_so_rather_than_pretend() {
        let t = Tmp::new("log");
        let log = t.0.join(LOG_FILE);
        std::fs::write(&log, "").expect("write");
        assert!(log_tail(&log, 5).contains("wrote nothing"), "{}", log_tail(&log, 5));

        std::fs::write(&log, "one\ntwo\nthree\nfour\n").expect("write");
        let tail = log_tail(&log, 2);
        assert!(tail.contains("three") && tail.contains("four"), "{tail}");
        assert!(!tail.contains("one"), "only the tail: {tail}");
    }

    #[test]
    fn liveness_should_agree_with_the_operating_system() {
        assert!(alive(self_pid()), "this process is running");
        assert!(!alive(dead_pid()), "a waited-for child is not");
        // Not asserted: `kill(0, 0)`. Signal 0 on pid 0 addresses the caller's
        // whole process group, so it answers about the group and not about a
        // pid, and it is a query nothing here should ever make — a state file
        // can only hold a pid a `spawn` returned, and a spawn never returns 0.
    }
}
