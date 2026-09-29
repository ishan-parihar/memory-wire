//! Lifecycle hooks — the commands a host runs around a session.
//!
//! A hook is the one place where a bug becomes somebody else's outage, so the
//! contract is uniform: read JSON from stdin, print something useful, exit 0.
//! Every failure path here is silent. A memory server that is down must not put
//! an error in front of the model, and a hook that exits nonzero is one a host
//! will surface to the user as a broken integration.
//!
//! Output shape per lifecycle:
//!
//! - `session-start` — bank preamble (server config when the route exists,
//!   otherwise the historian framing from `REFLECT_SYSTEM_PROMPT`), plus a
//!   recall of the opening prompt when the host supplies one.
//! - `prompt` — the recall results, plain text, nothing else.
//! - `stop` — no output; retains one compacted line naming the transcript.
//! - `pre-compact`, `session-end` — no output; retain the conversation's own
//!   prose, read off the transcript, because both fire at the moment the words
//!   stop being available.

use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::guidelines::curl_examples;
use crate::{http, paths, seed};
use memory_wire::api::REFLECT_SYSTEM_PROMPT;

/// Recalled lines injected into a session; enough to orient, few enough to
/// leave room for the actual task.
const MAX_LINES: usize = 8;

/// Token budget for one recall.
const BUDGET: usize = 1200;

/// Wall-clock cap on the host's stdin.
///
/// A hook blocks the session it is attached to, so a host that opens the pipe
/// and then never writes — or dribbles forever — must not become a hang. Past
/// this cap an unfinished read is treated as no input at all, which is already a
/// supported input.
///
/// Two seconds, because the hook's total worst case is quoted: this cap plus
/// one `paths::IO_TIMEOUT` for the config probe plus one for the recall is the
/// ~6 seconds a `session-start` can spend before it must answer.
const STDIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Bytes of a transcript's end that `pre-compact` / `session-end` read.
///
/// The read is bounded by size rather than by a clock, which is the honest
/// shape for it: a 128 KiB tail is a sub-millisecond read from any page cache,
/// so the wall-clock claim the module makes stays the one `paths::IO_TIMEOUT`
/// already bounds — stdin (2s) plus one network call (2s), inside the ~6s
/// `session-start` ceiling. A size cap is also what keeps a long session from
/// turning the hook into a multi-megabyte read inside somebody else's turn.
///
/// ponytail: tail-only, 128 KiB. A turn larger than the whole window is dropped
/// rather than half-read; raise `TAIL_BYTES` if a session ever produces one.
const TAIL_BYTES: u64 = 128 * 1024;

/// Characters of transcript prose one flush retains — the same ceiling
/// `seed` applies to a whole transcript file, so a seeded and a flushed
/// conversation cost the same to store.
const FLUSH_CHARS: usize = 2000;

/// Session lifecycle events a host can fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::Subcommand)]
pub enum Lifecycle {
    /// Bank preamble (+ recall for the opening prompt).
    SessionStart,
    /// Recall for the submitted prompt.
    Prompt,
    /// Retain a compacted session-end line.
    Stop,
    /// Retain the conversation's prose before compaction discards it.
    PreCompact,
    /// Retain the conversation's prose as the session ends.
    SessionEnd,
}

/// Run one hook: never fails, always exits 0.
///
/// `bank` is the `--bank` override, when the host could pass one. It wins over
/// `MEMORY_WIRE_BANK` and over the repository the process is running in — see
/// [`paths::resolve_bank_with`] for the full order and `docs/BANK_IDENTITY.md`
/// for why the order is what it is.
pub fn run(lifecycle: Lifecycle, bank: Option<&str>) -> i32 {
    let input = read_input(std::io::stdin(), STDIN_TIMEOUT);
    let endpoint = paths::endpoint();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let bank = paths::resolve_bank_with(bank, &cwd);
    let out = match lifecycle {
        Lifecycle::SessionStart => session_start_at(&endpoint, &bank, &input),
        Lifecycle::Prompt => prompt_at(&endpoint, &bank, &input),
        Lifecycle::Stop => {
            stop_at(&endpoint, &bank, &input);
            String::new()
        }
        Lifecycle::PreCompact => {
            flush_at(&endpoint, &bank, &input, "pre-compact");
            String::new()
        }
        Lifecycle::SessionEnd => {
            flush_at(&endpoint, &bank, &input, "session-end");
            String::new()
        }
    };
    if !out.is_empty() {
        // A closed stdout (the host stopped reading) must not panic.
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(out.as_bytes());
        let _ = stdout.flush();
    }
    0
}

/// Best-effort parse of the host's payload: absent, malformed, or unfinished
/// input is all "no input", and the hook carries on.
///
/// The read runs on its own thread so a producer that never finishes costs at
/// most `cap` instead of the whole session. A timeout deliberately leaves that
/// thread behind, parked on a read nobody will ever satisfy. Not joining it is
/// the decision, not an oversight: a hook process answers, exits, and is gone
/// within milliseconds of a timeout, so the parked reader is bounded at one per
/// process and dies with it. Joining instead would buy a tidier shutdown in
/// exchange for waiting on a producer that has already proven it will not
/// speak — which is the exact failure this cap exists to prevent.
fn read_input(mut src: impl Read + Send + 'static, cap: Duration) -> Value {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut raw = String::new();
        let _ = src.read_to_string(&mut raw);
        let _ = tx.send(raw);
    });
    rx.recv_timeout(cap)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .unwrap_or(Value::Null)
}

/// First non-empty string field among `keys`.
fn field(input: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(s) = input.get(*k).and_then(Value::as_str) {
            if !s.trim().is_empty() {
                return s.to_string();
            }
        }
    }
    String::new()
}

/// `session-start` output for one endpoint/bank.
pub fn session_start_at(endpoint: &str, bank: &str, input: &Value) -> String {
    let mut out = preamble(endpoint, bank);
    let prompt = field(input, &["prompt", "user_prompt"]);
    if let Some(hits) = recall_at(endpoint, bank, &prompt) {
        if !hits.is_empty() {
            out.push_str(&recall_section(bank, &hits));
        }
    }
    out
}

/// `prompt` output for one endpoint/bank.
pub fn prompt_at(endpoint: &str, bank: &str, input: &Value) -> String {
    let query = field(input, &["prompt", "user_prompt"]);
    match recall_at(endpoint, bank, &query) {
        Some(hits) if !hits.is_empty() => recall_section(bank, &hits),
        _ => String::new(),
    }
}

/// `stop`: retain one marker per finished session.
///
/// This used to retain a **pointer** — the transcript's path and its size on
/// disk. That was worse than useless in three ways, all of which are now gone:
///
/// - Nothing in this crate can read it back. `recall` returned the string
///   "session ended; transcript /home/…/abc.jsonl (1.2 MB)" and no way to reach
///   what the session actually said.
/// - `stop` fires on **every turn**, so it was the highest-churn writer in the
///   crate: fifty rows of the same useless sentence in a fifty-turn session.
/// - The path injected filesystem tokens — `/tmp`, `home`, `jsonl`, and a
///   random per-transcript basename — into the FTS index that recall searches.
///   That is real retrieval damage, not just clutter: those rows compete for
///   "session" and "ended" in every BM25 query about sessions.
///
/// What is left is a marker whose content is **stable for the life of a
/// session**, which means the store's existing per-bank content dedup collapses
/// the per-turn repeats into one row with no extra state here: the second and
/// subsequent `stop` firings in a session are byte-identical writes that dedup
/// already absorbs. `pre-compact` and `session-end` below capture the prose at
/// the two moments it stops being available. See `docs/INTEGRATION_GAPS.md` §G2.
pub fn stop_at(endpoint: &str, bank: &str, input: &Value) {
    // A payload with no transcript path is malformed for this event: without it
    // there is no session to mark as finished. Say nothing, as every other hook
    // does on a payload it cannot use.
    if field(input, &["transcript_path", "transcriptPath"]).is_empty() {
        return;
    }
    let session = field(input, &["session_id", "sessionId"]);
    let line = if session.is_empty() {
        "session ended".to_string()
    } else {
        format!("session {session} ended")
    };
    retain(endpoint, bank, &line, "hook:stop");
}

/// `pre-compact` and `session-end`: the two moments a session's words stop being
/// available, so the words themselves are what gets stored.
///
/// The host's transcript *is* readable here — it is a file on the same machine,
/// owned by the same user, whose path the payload hands over — so unlike `stop`
/// this reads it and retains the prose. That is the whole difference: a
/// conversation that survives a compaction is worth far more than a filename
/// that points at it.
///
/// The payload shape is read per event rather than assumed. `PreCompact` carries
/// `trigger` (`manual` or `auto`) and `SessionEnd` carries `reason` (`clear`,
/// `logout`, `prompt_input_exit`, `other`); both carry `session_id` and
/// `transcript_path`. A payload missing any of them, or missing all of them,
/// collapses to a no-op the same way malformed stdin does.
pub fn flush_at(endpoint: &str, bank: &str, input: &Value, event: &str) {
    let transcript = field(input, &["transcript_path", "transcriptPath"]);
    if transcript.is_empty() {
        return;
    }
    let Some(digest) = transcript_tail(&transcript) else {
        return;
    };
    let mut head = event.to_string();
    let session = field(input, &["session_id", "sessionId"]);
    if !session.is_empty() {
        // The session id is the half of the path that is actually a memory:
        // Claude Code names the transcript after it, so a later `recall` of
        // this row can still be traced back to the file it came from.
        head.push_str(&format!(" session {session}"));
    }
    let why = field(input, &["trigger", "reason"]);
    if !why.is_empty() {
        head.push_str(&format!(" ({why})"));
    }
    retain(endpoint, bank, &format!("{head}\n{digest}"), &format!("hook:{event}"));
}

/// The tail of a transcript's prose, or `None` when it carries none.
///
/// Only the file's last [`TAIL_BYTES`] are read, and only the tail of the prose
/// those hold is kept: the last turns are the ones compaction is about to
/// discard. The prose extraction is `seed`'s transcript parser, reused rather
/// than reimplemented, so a seeded conversation and a flushed one are split the
/// same way.
fn transcript_tail(path: &str) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len == 0 {
        return None;
    }
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES))).ok()?;
    let mut buf = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut buf).ok()?;

    let window = String::from_utf8_lossy(&buf);
    let mut text = String::new();
    for line in window.lines() {
        // The window usually starts mid-record, so its first fragment is not a
        // whole JSONL line. It fails to parse and drops out here, which is the
        // same thing `seed` does with a line that is not JSON.
        if let Some(part) = seed::turn_text(line) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&part);
        }
    }
    let content: String = text
        .chars()
        .skip(text.chars().count().saturating_sub(FLUSH_CHARS))
        .collect();
    (!content.trim().is_empty()).then_some(content)
}

/// One best-effort retain. Every failure is silence, by the module's contract.
fn retain(endpoint: &str, bank: &str, content: &str, context: &str) {
    let body = json!({ "content": content, "context": context }).to_string();
    if let Ok(resp) = http::post_json(
        &format!("{endpoint}/banks/{bank}/retain"),
        &body,
        paths::IO_TIMEOUT,
    ) {
        let _ = resp.ok();
    }
}

/// The bank preamble, plus the three operations.
fn preamble(endpoint: &str, bank: &str) -> String {
    let framing = server_preamble_at(endpoint, bank)
        .unwrap_or_else(|| REFLECT_SYSTEM_PROMPT.trim().to_string());
    format!(
        "## memory-wire — bank `{bank}`\n\n\
         {framing}\n\n\
         {}\n",
        curl_examples(endpoint, bank)
    )
}

/// Bank framing from the server, when the config route exists.
///
/// A sibling wave is adding `GET`/`PUT /banks/:id/config`; until it lands (or
/// when it answers 404) the historian framing is the right default anyway,
/// since that is what `reflect` synthesises with.
fn server_preamble_at(endpoint: &str, bank: &str) -> Option<String> {
    let resp = http::get(&format!("{endpoint}/banks/{bank}/config"), paths::IO_TIMEOUT).ok()?;
    if !resp.ok() {
        return None;
    }
    let doc: Value = serde_json::from_str(&resp.body).ok()?;
    ["background", "preamble", "system_prompt"]
        .iter()
        .find_map(|k| doc.get(*k).and_then(Value::as_str))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Recalled memories, or `None` when the server is unreachable.
fn recall_at(endpoint: &str, bank: &str, query: &str) -> Option<Vec<String>> {
    if query.trim().is_empty() {
        return None;
    }
    let body = json!({ "query": query, "budget": BUDGET }).to_string();
    let resp = http::post_json(
        &format!("{endpoint}/banks/{bank}/recall"),
        &body,
        paths::IO_TIMEOUT,
    )
    .ok()?;
    if !resp.ok() {
        return None;
    }
    serde_json::from_str::<Vec<String>>(&resp.body).ok()
}

/// Recalled memories as plain text.
fn recall_section(bank: &str, hits: &[String]) -> String {
    let mut out = format!("\n### memory-wire recall — `{bank}`\n\n");
    for hit in hits.iter().take(MAX_LINES) {
        out.push_str("- ");
        out.push_str(hit);
        out.push('\n');
    }
    if hits.len() > MAX_LINES {
        out.push_str(&format!("- …and {} more\n", hits.len() - MAX_LINES));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A canned loopback server: route on the path's suffix, record every
    /// request, and keep answering for the life of the test process.
    ///
    /// The route key is a suffix rather than a full path so a test need not name
    /// the bank — including the bank a spawned `hook` resolves from its own
    /// working directory. Returns the base URL plus a receiver of `path body`
    /// strings. One connection per request, matching the client's
    /// `Connection: close`.
    fn canned(routes: Vec<(&'static str, &'static str)>) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok((mut sock, _)) = listener.accept() {
                let mut raw = Vec::new();
                let mut buf = [0u8; 1024];
                loop {
                    let n = sock.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buf[..n]);
                    if let Some(h) = String::from_utf8_lossy(&raw).find("\r\n\r\n") {
                        let head = String::from_utf8_lossy(&raw[..h]).to_ascii_lowercase();
                        let want: usize = head
                            .split("content-length:")
                            .nth(1)
                            .and_then(|r| r.split("\r\n").next())
                            .and_then(|n| n.trim().parse().ok())
                            .unwrap_or(0);
                        if raw.len() >= h + 4 + want {
                            break;
                        }
                    }
                }
                let text = String::from_utf8_lossy(&raw).into_owned();
                let head = text.split("\r\n\r\n").next().unwrap_or_default().to_string();
                let req_body = text.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
                let path = head
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();
                let _ = tx.send(format!("{path} {req_body}"));
                let (status, payload) = match routes.iter().find(|(p, _)| path.ends_with(*p)) {
                    Some((_, body)) => ("200 OK", *body),
                    None => ("404 Not Found", "not found"),
                };
                let _ = sock.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
                        payload.len()
                    )
                    .as_bytes(),
                );
            }
        });
        (format!("http://{addr}"), rx)
    }

    /// `/banks/<bank>/recall` -> `<bank>`. A spawned hook resolves its bank from
    /// its own working directory, so the tests read the bank off the wire
    /// instead of assuming where the crate happens to be checked out.
    fn bank_of(path: &str) -> String {
        path.trim_start_matches("/banks/")
            .split_once('/')
            .map(|(bank, _)| bank.to_string())
            .unwrap_or_else(|| panic!("not a bank-scoped path: {path}"))
    }

    /// What one `hook` run of the compiled binary produced.
    struct HookRun {
        elapsed: std::time::Duration,
        stdout: String,
        stderr: String,
        code: Option<i32>,
    }

    /// One run of `memory-wire hook <lifecycle>`: a real pipe on stdin,
    /// `MEMORY_WIRE_URL` pointed at `endpoint`, `RUST_LOG` at `log`.
    ///
    /// `Some(text)` writes the payload and closes the pipe. `None` leaves it
    /// open and silent — the host that opens a pipe and never speaks, which is
    /// the case `STDIN_TIMEOUT` exists for.
    fn run_hook(lifecycle: &str, stdin: Option<&str>, endpoint: &str, log: &str) -> HookRun {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(crate::bin())
            .args(["hook", lifecycle])
            .env("RUST_LOG", log)
            .env("MEMORY_WIRE_URL", endpoint)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn memory-wire");
        let started = std::time::Instant::now();
        // Dropping this is what closes the pipe, so an absent payload leaves the
        // child reading a producer that never finishes.
        let mut pipe = child.stdin.take().expect("stdin pipe");
        if let Some(text) = stdin {
            pipe.write_all(text.as_bytes()).expect("write stdin");
            drop(pipe);
        }
        let out = child.wait_with_output().expect("wait for hook");
        HookRun {
            elapsed: started.elapsed(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code(),
        }
    }

    #[test]
    fn session_start_should_preamble_and_recall() {
        // Order matters: the preamble probes `/config` before the recall runs.
        let (ep, rx) = canned(vec![
            ("/config", "{}"),
            ("/recall", r#"["auth uses jose","rate limit is token bucket"]"#),
        ]);
        let out = session_start_at(&ep, "demo", &json!({ "prompt": "how does auth work" }));
        assert!(out.contains("## memory-wire — bank `demo`"));
        assert!(out.contains("historian"), "fallback framing");
        assert!(out.contains("/banks/demo/recall"));
        assert!(out.contains("- auth uses jose"));
        assert!(out.contains("- rate limit is token bucket"));

        let seen: Vec<String> = rx.try_iter().collect();
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert!(seen[0].starts_with("/banks/demo/config"), "{seen:?}");
        assert!(seen[1].starts_with("/banks/demo/recall"), "{seen:?}");
        assert!(seen[1].contains("\"budget\":1200"), "{:?}", seen[1]);
        assert!(seen[1].contains("how does auth work"), "{:?}", seen[1]);
    }

    #[test]
    fn session_start_without_a_prompt_should_print_the_preamble_only() {
        let (ep, rx) = canned(vec![("/config", "{}")]);
        let out = session_start_at(&ep, "demo", &json!({}));
        assert!(out.contains("## memory-wire"));
        assert!(!out.contains("### memory-wire recall"), "{out}");
        assert!(rx.try_iter().count() == 1, "only the config probe");
    }

    #[test]
    fn prompt_should_return_plain_recall_lines() {
        let (ep, _rx) = canned(vec![("/recall", r#"["one","two"]"#)]);
        let out = prompt_at(&ep, "demo", &json!({ "prompt": "what about auth" }));
        assert_eq!(out, "\n### memory-wire recall — `demo`\n\n- one\n- two\n");
        let empty = prompt_at(&ep, "demo", &json!({ "prompt": "   " }));
        assert!(empty.is_empty(), "blank prompt is not a search");
    }

    #[test]
    fn server_config_should_override_the_default_framing_when_present() {
        let (ep, _rx) = canned(vec![("/config", r#"{"background":"BANK-SPECIFIC FRAMING"}"#)]);
        let out = session_start_at(&ep, "demo", &json!({}));
        assert!(out.contains("BANK-SPECIFIC FRAMING"), "{out}");
        assert!(!out.contains("historian"), "{out}");
    }

    #[test]
    fn stop_should_retain_one_marker_carrying_no_path() {
        let (ep, rx) = canned(vec![("/retain", r#"{"id":"m1"}"#)]);
        stop_at(
            &ep,
            "demo",
            &json!({ "session_id": "s-7", "transcript_path": "/tmp/does-not-exist.jsonl" }),
        );
        let seen: Vec<String> = rx.try_iter().collect();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(seen[0].starts_with("/banks/demo/retain"), "{:?}", seen[0]);
        assert!(seen[0].contains("session s-7 ended"), "{:?}", seen[0]);
        // The regression this removed: a path in the body put /tmp, jsonl and a
        // random transcript basename into the FTS index recall searches.
        assert!(!seen[0].contains("does-not-exist"), "{:?}", seen[0]);
        assert!(!seen[0].contains("/tmp"), "{:?}", seen[0]);
    }

    /// `stop` fires on every turn. Its content is deliberately stable for the
    /// life of a session so the store's per-bank content dedup collapses the
    /// repeats into one row — asserted here by byte-equality, which is the
    /// property dedup actually keys on.
    #[test]
    fn stop_should_be_byte_identical_across_turns_of_one_session() {
        let payload = json!({
            "session_id": "s-7",
            "transcript_path": "/tmp/first.jsonl",
            "transcript_size": 100,
        });
        let mut bodies = Vec::new();
        for _ in 0..3 {
            let (ep, rx) = canned(vec![("/retain", r#"{"id":"m1"}"#)]);
            stop_at(&ep, "demo", &payload);
            let seen: Vec<String> = rx.try_iter().collect();
            assert_eq!(seen.len(), 1, "{seen:?}");
            bodies.push(seen[0].clone());
        }
        assert!(
            bodies.windows(2).all(|w| w[0] == w[1]),
            "three firings of one session must produce one identical write: {bodies:?}"
        );
    }

    /// Two sessions in one bank are two different events, so they must not
    /// collapse into each other.
    #[test]
    fn stop_should_distinguish_two_sessions() {
        let (ep, rx) = canned(vec![
            ("/retain", r#"{"id":"m1"}"#),
            ("/retain", r#"{"id":"m2"}"#),
        ]);
        stop_at(
            &ep,
            "demo",
            &json!({ "session_id": "s-7", "transcript_path": "/tmp/a.jsonl" }),
        );
        stop_at(
            &ep,
            "demo",
            &json!({ "session_id": "s-8", "transcript_path": "/tmp/b.jsonl" }),
        );
        let seen: Vec<String> = rx.try_iter().collect();
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert_ne!(seen[0], seen[1], "two sessions must not dedup into one row");
    }

    #[test]
    fn stop_without_a_transcript_should_do_nothing() {
        // Port 1 is closed: any request would be an error, so a silent no-op
        // and a silent failure are indistinguishable — which is the contract.
        stop_at("http://127.0.0.1:1", "demo", &json!({ "session_id": "s-7" }));
    }

    /// A host transcript in Claude Code's JSONL shape: a bare-string turn, a
    /// block-list turn, a line that is not JSON, and a non-turn record. Its
    /// path is returned because the payload hands the hook a path, not a file.
    fn transcript(tag: &str) -> String {
        let path = std::env::temp_dir().join(format!("mw-hook-{tag}-{}", std::process::id()));
        std::fs::write(
            &path,
            [
                r#"{"type":"user","message":{"role":"user","content":"how does auth work"},"sessionId":"s-7"}"#,
                r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"auth uses jose"}]}}"#,
                r#"not json at all"#,
                r#"{"type":"summary","summary":"ignored"}"#,
            ]
            .join("\n"),
        )
        .expect("write transcript");
        path.to_string_lossy().into_owned()
    }

    /// The four cases a never-fail lifecycle has to survive, run through the
    /// compiled binary with a real transcript on disk. `qualifier` is the
    /// payload field that distinguishes the event: `PreCompact` sends
    /// `trigger`, `SessionEnd` sends `reason`.
    fn flush_should_never_fail(lifecycle: &str, qualifier: &str) {
        let path = transcript(lifecycle);
        let payload = format!(
            r#"{{"session_id":"s-7","transcript_path":"{path}",{qualifier}}}"#
        );

        // Well-formed: the conversation's own prose is what lands, and the path
        // to it does not — the distinction `stop` gets wrong.
        let (ep, rx) = canned(vec![("/retain", r#"{"id":"m1"}"#)]);
        let got = run_hook(lifecycle, Some(&payload), &ep, "off");
        assert_eq!(got.code, Some(0), "{lifecycle}: {}", got.stderr);
        assert_eq!(got.stdout, "", "`{lifecycle}` speaks only to the server");
        let seen: Vec<String> = rx.try_iter().collect();
        assert_eq!(seen.len(), 1, "{lifecycle}: {seen:?}");
        assert!(seen[0].starts_with("/banks/"), "{lifecycle}: {:?}", seen[0]);
        assert!(
            seen[0].contains("auth uses jose"),
            "{lifecycle} must persist the conversation, not a pointer: {:?}",
            seen[0]
        );
        assert!(
            !seen[0].contains(".jsonl"),
            "{lifecycle} must not persist a path it cannot read back: {:?}",
            seen[0]
        );
        assert!(seen[0].contains("hook:"), "{lifecycle}: {:?}", seen[0]);

        // Missing payload fields: no transcript, so no work and no request.
        let (ep, rx) = canned(vec![("/retain", r#"{"id":"m1"}"#)]);
        for empty in [r#"{}"#, r#"{"session_id":"s-7"}"#, r#"{"transcript_path":""}"#] {
            let got = run_hook(lifecycle, Some(empty), &ep, "off");
            assert_eq!(got.code, Some(0), "{lifecycle} {empty}: {}", got.stderr);
            assert_eq!(got.stdout, "", "{lifecycle} {empty}");
        }
        assert_eq!(rx.try_iter().count(), 0, "{lifecycle}: nothing to retain");

        // Malformed stdin: the same collapse, by the path `read_input` already has.
        let (ep, rx) = canned(vec![("/retain", r#"{"id":"m1"}"#)]);
        let broken = ["not json", "", r##"{"transcript_path":"#}"##];
        for empty in broken {
            let got = run_hook(lifecycle, Some(empty), &ep, "off");
            assert_eq!(got.code, Some(0), "{lifecycle} {empty:?}: {}", got.stderr);
            assert_eq!(got.stdout, "", "{lifecycle} {empty:?}");
        }
        assert_eq!(rx.try_iter().count(), 0, "{lifecycle}: no panic, no retain");

        // A server that is down: exit 0, no stdout, and no budget spent.
        let got = run_hook(lifecycle, Some(&payload), "http://127.0.0.1:1", "off");
        assert_eq!(got.code, Some(0), "{lifecycle}: {}", got.stderr);
        assert_eq!(got.stdout, "", "{lifecycle}: an unreachable server says nothing");
        assert!(got.elapsed < Duration::from_secs(5), "{lifecycle}: {:?}", got.elapsed);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn pre_compact_should_retain_the_conversation_before_it_is_discarded() {
        flush_should_never_fail("pre-compact", r#""trigger":"auto""#);
    }

    #[test]
    fn session_end_should_retain_the_conversation_when_the_session_ends() {
        flush_should_never_fail("session-end", r#""reason":"clear""#);
    }

    #[test]
    fn unreachable_server_should_degrade_to_silence_not_a_crash() {
        let ep = "http://127.0.0.1:1";
        assert!(prompt_at(ep, "demo", &json!({ "prompt": "anything" })).is_empty());
        let out = session_start_at(ep, "demo", &json!({ "prompt": "anything" }));
        assert!(out.contains("## memory-wire"), "preamble still local");
        assert!(!out.contains("### memory-wire recall"), "no fabricated recall");
    }

    /// A producer that stalls must cost the hook `cap`, not the session.
    struct Slow {
        /// Delay before each chunk.
        delay: Duration,
        /// Bytes still to hand over.
        left: &'static str,
    }

    impl Read for Slow {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            std::thread::sleep(self.delay);
            let n = self.left.len().min(buf.len());
            buf[..n].copy_from_slice(&self.left.as_bytes()[..n]);
            self.left = &self.left[n..];
            Ok(n)
        }
    }

    #[test]
    fn a_slow_stdin_should_be_read_once_it_arrives() {
        let src = Slow {
            delay: Duration::from_millis(50),
            left: r#"{"prompt":"how does auth work"}"#,
        };
        let got = read_input(src, Duration::from_secs(5));
        assert_eq!(field(&got, &["prompt"]), "how does auth work", "{got}");
    }

    // The failure this cap exists for: a host that opens the pipe and never
    // closes it. The hook must proceed with no input rather than hang.
    #[test]
    fn an_unfinished_stdin_should_time_out_as_no_input() {
        let src = Slow {
            delay: Duration::from_secs(30),
            left: r#"{"prompt":"never arrives in time"}"#,
        };
        let started = std::time::Instant::now();
        let got = read_input(src, Duration::from_millis(200));
        assert!(got.is_null(), "an unfinished read is no input: {got}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the cap must not wait for the producer: {:?}",
            started.elapsed()
        );
    }

    // H4.3 — the compiled binary, real argv, a real pipe on stdin: the path a
    // host actually takes. Nothing here calls `session_start_at` directly, so
    // argument parsing, stdin parsing, output, and the exit code are all in
    // scope. `pre-compact` and `session-end` are covered by their own tests.
    #[test]
    fn the_hook_binary_should_answer_every_lifecycle_from_a_piped_stdin() {
        // session-start: the preamble, then the recall of the opening prompt.
        let (ep, rx) = canned(vec![
            ("/config", r#"{"background":"BANK-SPECIFIC FRAMING"}"#),
            ("/recall", r#"["auth uses jose"]"#),
        ]);
        let got = run_hook(
            "session-start",
            Some(r#"{"prompt":"how does auth work"}"#),
            &ep,
            "off",
        );
        assert_eq!(got.code, Some(0), "a hook always exits 0: {}", got.stderr);
        let seen: Vec<String> = rx.try_iter().collect();
        assert_eq!(seen.len(), 2, "config probe then recall: {seen:?}");
        let bank = bank_of(&seen[0]);
        assert_eq!(
            seen[0].split(' ').next().unwrap_or_default(),
            format!("/banks/{bank}/config"),
            "{seen:?}"
        );
        assert!(
            seen[1].starts_with(&format!("/banks/{bank}/recall ")),
            "both calls target the same bank: {seen:?}"
        );
        assert!(seen[1].contains("how does auth work"), "{:?}", seen[1]);
        assert!(
            got.stdout.starts_with(&format!("## memory-wire — bank `{bank}`\n")),
            "the preamble leads: {:?}",
            got.stdout
        );
        assert!(got.stdout.contains("BANK-SPECIFIC FRAMING"), "{:?}", got.stdout);
        assert!(
            got.stdout.ends_with("- auth uses jose\n"),
            "the recall trails: {:?}",
            got.stdout
        );

        // prompt: the recall lines and nothing else.
        let (ep, rx) = canned(vec![("/recall", r#"["one","two"]"#)]);
        let got = run_hook("prompt", Some(r#"{"prompt":"what about auth"}"#), &ep, "off");
        assert_eq!(got.code, Some(0), "{}", got.stderr);
        let bank = bank_of(&rx.try_iter().next().expect("one recall"));
        assert_eq!(
            got.stdout,
            format!("\n### memory-wire recall — `{bank}`\n\n- one\n- two\n"),
            "stdout is exactly the recall section"
        );

        // stop: no output at all, and one marker retain on the wire. The body
        // carries no transcript path: a path here is unreachable from the store
        // and its filesystem tokens land in the FTS index recall searches.
        let (ep, rx) = canned(vec![("/retain", r#"{"id":"m1"}"#)]);
        let got = run_hook(
            "stop",
            Some(r#"{"session_id":"s-7","transcript_path":"/tmp/does-not-exist.jsonl"}"#),
            &ep,
            "off",
        );
        assert_eq!(got.code, Some(0), "{}", got.stderr);
        assert_eq!(got.stdout, "", "`stop` speaks only to the server");
        let seen: Vec<String> = rx.try_iter().collect();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(seen[0].contains("session s-7 ended"), "{:?}", seen[0]);
        assert!(!seen[0].contains("does-not-exist"), "{:?}", seen[0]);
    }

    // H4.4 — the override is what a hook talks to. Neither run names a bank or
    // a port beyond `MEMORY_WIRE_URL`, so a child that ignored the override
    // would fall back to the default endpoint and never reach this listener:
    // the second half asserts exactly that by pointing it elsewhere.
    #[test]
    fn the_hook_should_talk_to_the_memory_wire_url_override() {
        let (ep, rx) = canned(vec![("/recall", r#"["from the override"]"#)]);
        let hit = run_hook("prompt", Some(r#"{"prompt":"auth"}"#), &ep, "off");
        assert_eq!(hit.code, Some(0), "{}", hit.stderr);
        assert!(hit.stdout.contains("- from the override"), "{:?}", hit.stdout);

        // Same binary, same stdin, an endpoint that is not this listener: a
        // refused connect, and nothing the canned server can see.
        let missed = run_hook("prompt", Some(r#"{"prompt":"auth"}"#), "http://127.0.0.1:1", "off");
        assert_eq!(missed.code, Some(0), "{}", missed.stderr);
        assert_eq!(missed.stdout, "", "an unreachable server says nothing");
        assert_eq!(rx.try_iter().count(), 1, "only the override run reached this server");
    }

    // H4.2, the claim itself. A host that never writes its stdin plus a server
    // that accepts and never answers spend every budget the docs quote: stdin
    // (2s) + config probe (2s) + recall (2s). It must still answer with the
    // local preamble and exit 0, because a hook that hangs is worse than one
    // that knows nothing.
    #[test]
    fn a_silent_stdin_and_a_stalling_server_should_stay_under_the_documented_budget() {
        let stalling = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = stalling.local_addr().expect("addr");
        std::thread::spawn(move || {
            while let Ok((sock, _)) = stalling.accept() {
                // Answer nothing, and hold the socket open far past the client's
                // deadline: closing it would make the read an instant EOF and the
                // budget would never be spent.
                std::thread::sleep(Duration::from_secs(30));
                drop(sock);
            }
        });

        let got = run_hook("session-start", None, &format!("http://{addr}"), "off");
        assert_eq!(got.code, Some(0), "{}", got.stderr);
        assert!(got.stdout.contains("## memory-wire"), "the preamble is local: {:?}", got.stdout);
        assert!(
            !got.stdout.contains("### memory-wire recall"),
            "no fabricated recall: {:?}",
            got.stdout
        );
        assert!(
            got.elapsed < Duration::from_secs(8),
            "stdin + probe + recall must stay inside the documented budget, took {:?}",
            got.elapsed
        );
    }

    // The other half of the same claim: a server that is simply not there is a
    // refused connection, and a hook must not spend the budget finding out.
    #[test]
    fn a_refused_server_should_cost_a_hook_almost_nothing() {
        let got = run_hook(
            "session-start",
            Some(r#"{"prompt":"auth"}"#),
            "http://127.0.0.1:1",
            "off",
        );
        assert_eq!(got.code, Some(0), "{}", got.stderr);
        assert!(got.elapsed < Duration::from_secs(1), "{:?}", got.elapsed);
    }

    // H5.2 — stdout belongs to the host, so the log level must not move a byte
    // of it, in any lifecycle, at any level. `off` is the baseline; the rest
    // have to reproduce it exactly, and the protocol output is unchanged.
    #[test]
    fn the_log_level_should_never_reach_a_hook_stdout() {
        let (ep, _rx) = canned(vec![
            ("/config", "{}"),
            ("/recall", r#"["one","two"]"#),
            ("/retain", r#"{"id":"m1"}"#),
        ]);
        let payload = r#"{"prompt":"auth","session_id":"s-1","transcript_path":"/tmp/nope.jsonl"}"#;
        for lifecycle in ["session-start", "prompt", "stop", "pre-compact", "session-end"] {
            let quiet = run_hook(lifecycle, Some(payload), &ep, "off");
            assert_eq!(quiet.code, Some(0), "{lifecycle}: {}", quiet.stderr);
            for level in ["error", "warn", "info", "debug", "trace"] {
                let got = run_hook(lifecycle, Some(payload), &ep, level);
                assert_eq!(got.code, Some(0), "hook {lifecycle} at {level}: {}", got.stderr);
                assert_eq!(
                    got.stdout, quiet.stdout,
                    "hook {lifecycle} at RUST_LOG={level} changed stdout — a log line reached the host's channel"
                );
            }
        }
    }

    // Same contract for the real stdin, and it is the whole reason the hook can
    // afford to read at all.
    #[test]
    fn empty_stdin_should_stay_an_empty_object() {
        assert!(read_input(&b""[..], Duration::from_secs(5)).is_null());
        assert!(read_input(&b"not json"[..], Duration::from_secs(5)).is_null());
    }

    #[test]
    fn recall_section_should_cap_lines() {
        let hits: Vec<String> = (0..MAX_LINES + 3).map(|i| format!("hit {i}")).collect();
        let out = recall_section("b", &hits);
        assert!(out.contains(&format!("- hit {}", MAX_LINES - 1)));
        assert!(out.contains("…and 3 more"), "{out}");
    }
}
