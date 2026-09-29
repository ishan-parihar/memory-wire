//! The `daemon` lifecycle, driven through the real binary.
//!
//! The unit tests in `src/daemon.rs` cover what the state file says; this covers
//! the thing that matters, which is whether a detached process actually comes up,
//! answers, and goes away. Nothing in-process can answer that, so every step here
//! runs `memory-wire` itself: spawn it, read the file it wrote, probe the port it
//! was given, signal it, and check the port is free afterwards.
//!
//! Isolation is the other half of it. `XDG_DATA_HOME` points at a scratch
//! directory, so the state file and the log are the fixture's, and `--db` names a
//! file inside it, so the developer's real store is never opened. The port comes
//! from the kernel and is released immediately. The fixture's `Drop` stops
//! whatever it started, so a failed assertion does not leave a server behind.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Duration;

/// The binary cargo just built for this test, so the test drives the shipped
/// entry point rather than an in-process stand-in for it.
const BIN: &str = env!("CARGO_BIN_EXE_memory-wire");

/// A scratch `XDG_DATA_HOME`, a free port, and a guaranteed stop.
struct Fixture {
    /// This fixture's `XDG_DATA_HOME`; `memory-wire/serve.json` lands under it.
    home: PathBuf,
    /// The port the server is told to bind.
    port: u16,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let home = std::env::temp_dir().join(format!("mw-daemon-{tag}-{}", std::process::id()));
        // `.ok()` because a leftover from a previous run is the normal case, and
        // a missing one is not an error either.
        std::fs::remove_dir_all(&home).ok();
        std::fs::create_dir_all(&home).expect("scratch XDG_DATA_HOME");
        // An ephemeral port from the kernel, released immediately. There is a
        // window between the release and the child's bind, and nothing closes it
        // without a lockfile the design deliberately does not have — a collision
        // here shows up as a clear "cannot bind" refusal rather than a silent
        // wrong result.
        let port = TcpListener::bind("127.0.0.1:0")
            .expect("reserve a port")
            .local_addr()
            .expect("local addr")
            .port();
        Self { home, port }
    }

    /// `--addr` for the server under test.
    fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// A store inside the fixture, so nothing real is opened.
    fn db(&self) -> PathBuf {
        self.home.join("agents.db")
    }

    /// `$XDG_DATA_HOME/memory-wire/serve.json`.
    fn state_file(&self) -> PathBuf {
        self.home.join("memory-wire").join("serve.json")
    }

    /// Run the real binary with this fixture's `XDG_DATA_HOME`.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(BIN)
            .args(args)
            .env("XDG_DATA_HOME", &self.home)
            .output()
            .unwrap_or_else(|e| panic!("spawn {BIN} {args:?}: {e}"))
    }

    /// `daemon start` against this fixture's port and store.
    fn start(&self) -> Output {
        self.run(&[
            "daemon",
            "start",
            "--addr",
            &self.addr(),
            "--db",
            &self.db().display().to_string(),
        ])
    }

    /// The state file as parsed JSON, so no assertion reads it through the code
    /// that wrote it.
    fn recorded_state(&self) -> serde_json::Value {
        let raw = std::fs::read_to_string(self.state_file())
            .unwrap_or_else(|e| panic!("read {}: {e}", self.state_file().display()));
        serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("{} is not json: {e}\n{raw}", self.state_file().display()))
    }
}

impl Drop for Fixture {
    /// Stops the daemon, then removes the scratch home.
    ///
    /// Runs on the panic path too, which is the whole reason the daemon is
    /// cleaned up from `Drop` rather than from a line at the end of the test.
    fn drop(&mut self) {
        let _ = self.run(&["daemon", "stop"]);
        std::fs::remove_dir_all(&self.home).ok();
    }
}

/// stdout and stderr of one run, for a failure message.
fn text(out: &Output) -> String {
    format!(
        "status {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// `GET /health` on `addr`, as the body. `None` means nothing answered, which is
/// a different answer from a body that is not `ok`.
fn health(addr: &str) -> Option<String> {
    let mut sock = TcpStream::connect(addr).ok()?;
    sock.set_read_timeout(Some(Duration::from_secs(5)))
        .ok()?;
    sock.write_all(
        format!("GET /health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .ok()?;
    let mut raw = Vec::new();
    sock.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    Some(text.split_once("\r\n\r\n")?.1.trim().to_string())
}

/// Can a server bind this address right now?
fn port_is_free(addr: &str) -> bool {
    TcpListener::bind(addr).is_ok()
}

// The lifecycle, end to end, in the order a user meets it. This is the test that
// proves the feature: a process is spawned that is not a child of anything the
// test waits on, it answers, it is stopped by a signal the server already
// handles, and the port it held is released.
#[test]
fn the_daemon_lifecycle_should_start_serve_stop_and_release_the_port() {
    let f = Fixture::new("lifecycle");

    // 1. Nothing is running, and the command says so with a failing exit.
    let before = f.run(&["daemon", "status"]);
    assert!(
        !before.status.success(),
        "status before start must not report a daemon\n{}",
        text(&before)
    );
    assert!(text(&before).contains("not running"), "{}", text(&before));
    assert!(!f.state_file().exists(), "status must not create a state file");

    // 2. Start. Returns immediately, having proved the server answers.
    let started = f.start();
    assert!(started.status.success(), "start failed\n{}", text(&started));
    let out = text(&started);
    assert!(out.contains("pid"), "{out}");
    assert!(out.contains(&f.addr()), "{out}");

    // 3. The state file is where the design says it is, beside the store, and it
    //    holds what `status` and `stop` both need.
    assert!(
        f.state_file().is_file(),
        "no state file at {}",
        f.state_file().display()
    );
    let recorded = f.recorded_state();
    assert_eq!(recorded["addr"], f.addr(), "{recorded}");
    assert_eq!(recorded["db"], f.db().display().to_string(), "{recorded}");
    let pid = recorded["pid"].as_i64().expect("a pid in the state file");
    assert!(pid > 0, "{recorded}");
    assert!(recorded["started_at"].as_i64().expect("a start time") > 0, "{recorded}");

    // 4. Status reports serving, with the facts, and the endpoint really answers
    //    — the body `ok`, which is the check a foreign 200 cannot pass.
    let serving = f.run(&["daemon", "status"]);
    assert!(serving.status.success(), "{}", text(&serving));
    let screen = text(&serving);
    assert!(screen.contains("serving"), "{screen}");
    assert!(screen.contains(&format!("pid        {pid}")), "{screen}");
    assert!(screen.contains("uptime"), "{screen}");
    assert!(screen.contains(&f.addr()), "{screen}");
    assert_eq!(health(&f.addr()).as_deref(), Some("ok"), "the server must answer `ok`");

    // 5. Stop. The server drains and exits, and the state file goes with it.
    let stopped = f.run(&["daemon", "stop"]);
    assert!(stopped.status.success(), "{}", text(&stopped));
    assert!(text(&stopped).contains("stopped"), "{}", text(&stopped));
    assert!(!f.state_file().exists(), "the state file must not outlive the daemon");
    assert_eq!(health(&f.addr()), None, "nothing may still answer /health");
    assert!(port_is_free(&f.addr()), "the port must be free again");

    // 6. And the last word is the machine's, not ours: nothing is listening.
    let after = f.run(&["daemon", "status"]);
    assert!(!after.status.success(), "{}", text(&after));
    assert!(text(&after).contains("not running"), "{}", text(&after));
}

// Starting twice is the failure the bind check exists for. The second `start`
// must refuse before spawning anything, so the user gets the reason on their
// terminal instead of a child that dies on bind and leaves a state file naming
// a corpse.
#[test]
fn a_second_start_should_refuse_rather_than_spawn_a_server_that_dies_on_bind() {
    let f = Fixture::new("twice");
    let first = f.start();
    assert!(first.status.success(), "{}", text(&first));

    let second = f.start();
    assert!(!second.status.success(), "start must refuse\n{}", text(&second));
    let err = text(&second);
    assert!(err.contains("already serving"), "{err}");
    assert!(err.contains(&f.addr()), "{err}");
    assert!(err.contains("daemon stop"), "{err}");

    // The first daemon is untouched and still the one on record.
    let status = f.run(&["daemon", "status"]);
    assert!(status.status.success(), "{}", text(&status));
    let pid = f.recorded_state()["pid"].as_i64().expect("a pid");
    assert!(text(&status).contains(&format!("pid        {pid}")), "{}", text(&status));
    assert_eq!(health(&f.addr()).as_deref(), Some("ok"));
}

// Stopping a stopped daemon is not an error — it is the answer to a question,
// and the question is asked by scripts on a machine that may not have one.
#[test]
fn stopping_nothing_should_succeed_and_say_so() {
    let f = Fixture::new("stopnothing");
    let stopped = f.run(&["daemon", "stop"]);
    assert!(stopped.status.success(), "{}", text(&stopped));
    assert!(text(&stopped).contains("nothing running"), "{}", text(&stopped));

    // Twice, because the state file removal is the part that could go wrong.
    let again = f.run(&["daemon", "stop"]);
    assert!(again.status.success(), "{}", text(&again));
    assert!(text(&again).contains("nothing running"), "{}", text(&again));
}

// A pid that no longer exists is stale, and stale must not block a start. The
// daemon is killed with a signal nothing waits for — SIGKILL, which `serve` does
// not handle — so the state file is left describing a process that is gone.
#[test]
fn a_stale_state_file_should_not_block_a_new_daemon() {
    let f = Fixture::new("stale");
    assert!(f.start().status.success());

    let pid = f.recorded_state()["pid"].as_i64().expect("a pid");
    // SIGKILL, which `serve` does not handle, so nothing drains and nothing
    // waits: the process is simply gone and the state file survives it, which is
    // the exact state the staleness rule is about.
    let killed = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .expect("spawn kill");
    assert!(killed.success(), "kill -9 {pid}");
    for _ in 0..200 {
        if TcpStream::connect(f.addr()).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(f.state_file().exists(), "SIGKILL leaves the state file behind");

    // Stale is reported as stale, and the pid is not signalled by `stop`.
    let stale = f.run(&["daemon", "status"]);
    assert!(!stale.status.success(), "{}", text(&stale));
    assert!(text(&stale).contains("not running"), "{}", text(&stale));
    assert!(text(&stale).contains("stale"), "{}", text(&stale));

    let stopped = f.run(&["daemon", "stop"]);
    assert!(stopped.status.success(), "{}", text(&stopped));
    assert!(text(&stopped).contains("nothing running"), "{}", text(&stopped));
    assert!(!f.state_file().exists(), "the stale file must be cleaned up");

    // And a fresh start is not blocked by any of it.
    let restarted = f.start();
    assert!(restarted.status.success(), "{}", text(&restarted));
    assert_eq!(health(&f.addr()).as_deref(), Some("ok"));
}
