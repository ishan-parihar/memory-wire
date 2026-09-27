//! One-shot self-seeding: a work tree's git history, and the host transcripts
//! under `~/.claude/projects`, into a bank.
//!
//! Every source is best-effort by contract. A directory that is not a work
//! tree, a transcript that is not JSON, a row the store refuses — each is a note
//! on stdout and the run still exits 0. The only failure worth a nonzero exit
//! is a store that would not open, and that happens before any of this runs.
//!
//! One run, then done: nothing here watches, schedules, or re-runs.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use memory_wire::api::MemoryService;
use memory_wire::capture::hash_content;
use memory_wire::memory::Bank;
use memory_wire::store::{Store, UpdateMode};
use serde_json::Value;

use crate::paths;

/// Commits read when `--commits` is absent.
const DEFAULT_COMMITS: usize = 100;
/// Ceiling on `--commits`, so one flag cannot ask for a whole history.
const MAX_COMMITS: usize = 500;
/// Body lines kept per commit: the subject is the headline, the body is the
/// decision behind it, and the rest is trailers and boilerplate.
const BODY_LINES: usize = 10;
/// Transcript files read per run, newest first.
const MAX_TRANSCRIPT_FILES: usize = 50;
/// Characters of a transcript retained per file.
const TRANSCRIPT_CHARS: usize = 2000;
/// Memories one run may write across both sources.
const MAX_MEMORIES: usize = 1000;

/// What one run wrote, and what it left out.
pub struct Seeded {
    /// Memories written from git history.
    pub git: usize,
    /// Memories written from host transcripts.
    pub transcripts: usize,
    /// Per-source lines naming what was skipped and why.
    pub notes: Vec<String>,
}

impl Seeded {
    /// The line an operator reads, plus every skip note under it.
    pub fn render(&self, bank: &str) -> String {
        let mut out = format!(
            "seeded {} git + {} transcripts into bank '{bank}'",
            self.git, self.transcripts
        );
        for note in &self.notes {
            out.push('\n');
            out.push_str(note);
        }
        out
    }
}

/// One memory a source wants written.
struct Seed {
    content: String,
    context: String,
    tags: Vec<String>,
    document_id: String,
}

/// Retain one seed row; `false` means the store refused it.
///
/// A refused row is a note, not a failure: one bad document must not abandon
/// the rows behind it. `replace` with a stable document id is what makes a
/// second run an upsert instead of a duplicate.
fn put<S: Store>(svc: &MemoryService<S>, bank: &str, seed: Seed, notes: &mut Vec<String>) -> bool {
    match svc.retain_doc(
        bank,
        &seed.content,
        Some(seed.context),
        &seed.tags,
        Some(&seed.document_id),
        UpdateMode::Replace,
    ) {
        Ok(_) => true,
        Err(e) => {
            notes.push(format!("skipped {}: {e}", seed.document_id));
            false
        }
    }
}

/// Seed from the current working directory and the real home.
pub fn run<S: Store>(
    svc: &MemoryService<S>,
    bank: &str,
    commits: Option<usize>,
    transcripts: bool,
) -> Seeded {
    let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let projects = paths::home()
        .ok()
        .map(|h| h.join(".claude").join("projects"));
    seed(svc, bank, &dir, commits, transcripts, projects.as_deref())
}

/// Seed both sources, best-effort. Never returns an error.
fn seed<S: Store>(
    svc: &MemoryService<S>,
    bank: &str,
    dir: &Path,
    commits: Option<usize>,
    transcripts: bool,
    projects: Option<&Path>,
) -> Seeded {
    let mut out = Seeded {
        git: 0,
        transcripts: 0,
        notes: Vec::new(),
    };
    // A bank that does not exist has no row to hang memories off, and the
    // store's foreign key would refuse every write under it.
    if let Err(e) = svc.store.put_bank(&Bank {
        id: bank.to_string(),
        name: bank.to_string(),
    }) {
        out.notes.push(format!("skipped both sources: bank '{bank}': {e}"));
        return out;
    }
    let mut budget = MAX_MEMORIES;
    match seed_git(svc, bank, dir, commits.unwrap_or(DEFAULT_COMMITS).min(MAX_COMMITS), &mut budget, &mut out.notes) {
        Ok(n) => out.git = n,
        Err(why) => out.notes.push(why),
    }
    if transcripts {
        match projects {
            // A home that cannot be read is not an error, it is one fewer source.
            None => out.notes.push("skipped transcripts: no home directory".to_string()),
            Some(root) => match seed_transcripts(svc, bank, root, &mut budget, &mut out.notes) {
                Ok(n) => out.transcripts = n,
                Err(why) => out.notes.push(why),
            },
        }
    }
    out
}

/// `git log` over the work tree at `dir`, parsed into one memory per commit.
fn seed_git<S: Store>(
    svc: &MemoryService<S>,
    bank: &str,
    dir: &Path,
    commits: usize,
    budget: &mut usize,
    notes: &mut Vec<String>,
) -> Result<usize, String> {
    let raw = git_log(dir, commits)?;
    let mut written = 0;
    for commit in parse_log(&raw) {
        if *budget == 0 {
            notes.push(format!("stopped after {written} commits: the {MAX_MEMORIES}-memory cap"));
            break;
        }
        // Merges stay: their bodies carry the decision that produced the branch.
        let body = commit.body.join("\n");
        let content = if body.is_empty() {
            commit.subject.clone()
        } else {
            format!("{}\n{body}", commit.subject)
        };
        let written_row = put(
            svc,
            bank,
            Seed {
                content,
                context: "seed:git".to_string(),
                tags: vec!["git".to_string(), "commit".to_string()],
                document_id: format!("git:{}", commit.sha),
            },
            notes,
        );
        if written_row {
            *budget -= 1;
            written += 1;
        }
    }
    Ok(written)
}

/// The raw `git log`, or why there is no history to read.
///
/// `%H %s` puts a full sha in front of every subject, which is what makes the
/// parse below able to tell a new record from a body line without trusting git's
/// record separator.
fn git_log(dir: &Path, commits: usize) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args([
            "log",
            &format!("--max-count={commits}"),
            "--format=%H %s%n%b",
        ])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("skipped git: git is not runnable here: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "skipped git: {} is not a git work tree ({} {})",
            dir.display(),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// One commit as the log reported it.
#[derive(Debug)]
struct Commit {
    sha: String,
    subject: String,
    body: Vec<String>,
}

/// Split `git log` output into commits.
///
/// A new record starts at any line whose first token is a full sha, so a body
/// containing blank lines — or a trailer that looks like a header — cannot
/// desynchronise the split the way a fixed separator would.
fn parse_log(raw: &str) -> Vec<Commit> {
    let mut commits: Vec<Commit> = Vec::new();
    for line in raw.lines() {
        if let Some((sha, subject)) = line.split_once(' ') {
            if sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()) {
                commits.push(Commit {
                    sha: sha.to_string(),
                    subject: subject.trim().to_string(),
                    body: Vec::new(),
                });
                continue;
            }
        }
        if let Some(commit) = commits.last_mut() {
            commit.body.push(line.to_string());
        }
    }
    for commit in &mut commits {
        while commit.body.last().is_some_and(|l| l.trim().is_empty()) {
            commit.body.pop();
        }
        commit.body.truncate(BODY_LINES);
    }
    commits
}

/// One memory per transcript file, newest first.
fn seed_transcripts<S: Store>(
    svc: &MemoryService<S>,
    bank: &str,
    root: &Path,
    budget: &mut usize,
    notes: &mut Vec<String>,
) -> Result<usize, String> {
    if !root.is_dir() {
        return Err(format!(
            "skipped transcripts: {} is not a directory",
            root.display()
        ));
    }
    // Every path here is built from entries found under `root`, and a symlink is
    // never followed (see `transcript_files`), so nothing outside it is read.
    let all = transcript_files(root);
    if all.len() > MAX_TRANSCRIPT_FILES {
        notes.push(format!(
            "transcripts: read the {MAX_TRANSCRIPT_FILES} newest of {} files",
            all.len()
        ));
    }
    let mut written = 0;
    for (_, path) in all.into_iter().take(MAX_TRANSCRIPT_FILES) {
        if *budget == 0 {
            notes.push(format!(
                "stopped after {written} transcripts: the {MAX_MEMORIES}-memory cap"
            ));
            break;
        }
        // A file that is not readable UTF-8, or not JSONL at all, is skipped
        // without a note: a corpus with one bad file in it is the normal case.
        let Some(seed) = seed_from_transcript(&path) else {
            continue;
        };
        if put(svc, bank, seed, notes) {
            *budget -= 1;
            written += 1;
        }
    }
    Ok(written)
}

/// `.jsonl` files under `root`, newest first.
fn transcript_files(root: &Path) -> Vec<(SystemTime, PathBuf)> {
    let mut files = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            // `file_type` does not follow symlinks, which is both the loop guard
            // for a self-referential link and the reason no path can resolve
            // outside the transcript directory.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                dirs.push(path);
            } else if kind.is_file() && path.extension().is_some_and(|e| e == "jsonl") {
                let mtime = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                files.push((mtime, path));
            }
        }
    }
    files.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));
    files
}

/// The seed row for one transcript file, or `None` when it carries no text.
fn seed_from_transcript(path: &Path) -> Option<Seed> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut text = String::new();
    for line in raw.lines() {
        // A line that is not JSON, or is not a user/assistant turn, contributes
        // nothing; the rest of the file is still worth keeping.
        if let Some(part) = turn_text(line) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&part);
        }
    }
    let content: String = text.chars().take(TRANSCRIPT_CHARS).collect();
    if content.trim().is_empty() {
        return None;
    }
    let digest = hash_content(&path.to_string_lossy());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Some(Seed {
        content,
        context: format!(
            "seed:transcript:{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        ),
        tags: vec!["transcript".to_string()],
        document_id: format!("transcript:{hex}"),
    })
}

/// The prose one JSONL line carries, or `None` when it carries none.
fn turn_text(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    let role = value
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/message/role").and_then(Value::as_str))?;
    if role != "user" && role != "assistant" {
        return None;
    }
    text_of(
        value
            .pointer("/message/content")
            .or_else(|| value.get("content"))
            .or_else(|| value.get("text"))?,
    )
}

/// Text out of a `content` field: a bare string, or the text parts of a block list.
fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        // A block list carries tool calls and metadata beside the prose; only
        // the text parts are worth a memory.
        Value::Array(blocks) => {
            let parts: Vec<&str> = blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect();
            (!parts.is_empty()).then(|| parts.join("\n"))
        }
        Value::Object(_) => value
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_wire::store::SqliteStore;

    /// A scratch directory of its own, so parallel tests never share one.
    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-seed-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        dir
    }

    fn git(args: &[&str], cwd: &Path) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?} -> {}", String::from_utf8_lossy(&out.stderr));
    }

    /// A commit with an identity of its own, so the fixture never depends on the
    /// host's git config (and never waits on a gpg-agent).
    fn commit(dir: &Path, file: &str, subject: &str, body: Option<&str>) {
        std::fs::write(dir.join(file), format!("{file}\n")).expect("write");
        git(&["add", "-A"], dir);
        let mut args = vec!["-c", "commit.gpgsign=false", "commit", "-q", "-m", subject];
        if let Some(body) = body {
            args.extend(["-m", body]);
        }
        git(&args, dir);
    }

    fn repo_with_three_commits(dir: &Path) {
        git(&["init", "-q"], dir);
        commit(dir, "a.txt", "first commit", None);
        commit(dir, "b.txt", "second commit", None);
        commit(dir, "c.txt", "third commit", Some("body line one\nbody line two"));
    }

    /// The service over a scratch store, using the same opener `serve` uses.
    fn svc(db: &Path) -> MemoryService<SqliteStore> {
        MemoryService::new(crate::open_store(db).expect("store"))
    }

    /// What landed in the bank, read straight from SQLite: the lifecycle routes
    /// serve neither `document_id` nor tags, and both are the contract here.
    fn rows(db: &Path, bank: &str) -> Vec<(Option<String>, String, Vec<String>)> {
        let conn = rusqlite::Connection::open(db).expect("open");
        let mut stmt = conn
            .prepare("SELECT id, content, document_id FROM memories WHERE bank_id=?1 ORDER BY id")
            .expect("prepare");
        let memories: Vec<(String, String, Option<String>)> = stmt
            .query_map([bank], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");
        let mut tags = conn.prepare("SELECT tag FROM memory_tags WHERE memory_id=?1").expect("tags");
        memories
            .into_iter()
            .map(|(id, content, document_id)| {
                let mut row_tags: Vec<String> = tags
                    .query_map([&id], |r| r.get(0))
                    .expect("tag query")
                    .collect::<Result<_, _>>()
                    .expect("tags");
                row_tags.sort();
                (document_id, content, row_tags)
            })
            .collect()
    }

    // The git source is an upsert, not an append: a second run over the same
    // history must leave exactly the rows the first run left.
    #[test]
    fn git_seeding_writes_one_tagged_row_per_commit_and_is_idempotent() {
        let base = tmp("git");
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        repo_with_three_commits(&repo);
        let db = base.join("memory.db");
        let svc = svc(&db);

        let first = seed(&svc, "demo", &repo, None, false, None);
        assert_eq!(first.git, 3, "{:?}", first.notes);
        assert!(first.notes.is_empty(), "{:?}", first.notes);

        let landed = rows(&db, "demo");
        assert_eq!(landed.len(), 3, "{landed:?}");
        let mut docs: Vec<String> = landed.iter().map(|(d, _, _)| d.clone().expect("document id")).collect();
        docs.sort();
        assert!(docs.iter().all(|d| d.starts_with("git:")), "{docs:?}");
        assert_eq!(docs.len(), 3, "one distinct document id per commit: {docs:?}");
        for (_, _, tags) in &landed {
            assert_eq!(tags, &["commit".to_string(), "git".to_string()], "{landed:?}");
        }
        assert!(
            landed.iter().any(|(_, c, _)| c.contains("body line one")),
            "a commit body must reach the memory: {landed:?}"
        );

        // Rerun: the count is the same because each row replaces its own
        // revision, and the bank still holds three memories, not six.
        let second = seed(&svc, "demo", &repo, None, false, None);
        assert_eq!(second.git, 3, "{:?}", second.notes);
        assert_eq!(rows(&db, "demo").len(), 3);
        assert!(second.render("demo").starts_with("seeded 3 git + 0 transcripts"));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn transcripts_land_and_a_line_that_is_not_json_is_skipped() {
        let base = tmp("transcripts");
        let projects = base.join("projects");
        let alpha = projects.join("alpha");
        let beta = projects.join("beta");
        std::fs::create_dir_all(&alpha).expect("alpha");
        std::fs::create_dir_all(&beta).expect("beta");
        std::fs::write(
            alpha.join("session.jsonl"),
            "{\"type\":\"user\",\"message\":{\"content\":\"how does auth work\"}}\n\
             {\"type\":\"system\",\"message\":{\"content\":\"ignored role\"}}\n",
        )
        .expect("alpha");
        std::fs::write(
            beta.join("other.jsonl"),
            "not json at all\n\
             {\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"it uses jose\"},{\"type\":\"tool_use\",\"id\":\"t1\"}]}}\n",
        )
        .expect("beta");
        std::fs::write(beta.join("notes.md"), "not a transcript").expect("md");

        let db = base.join("memory.db");
        let svc = svc(&db);
        let out = seed(&svc, "demo", &base, Some(0), true, Some(&projects));

        assert_eq!(out.transcripts, 2, "{:?}", out.notes);
        assert_eq!(out.git, 0, "no history was requested: {:?}", out.notes);
        let landed = rows(&db, "demo");
        assert_eq!(landed.len(), 2, "{landed:?}");
        for (document_id, _, tags) in &landed {
            assert!(
                document_id.as_deref().is_some_and(|d| d.starts_with("transcript:")),
                "{landed:?}"
            );
            assert_eq!(tags, &["transcript".to_string()], "{landed:?}");
        }
        let all: String = landed.iter().map(|(_, c, _)| c.as_str()).collect();
        assert!(all.contains("how does auth work"), "{landed:?}");
        assert!(all.contains("it uses jose"), "{landed:?}");
        // The tool-call block and the non-user/assistant line are not prose.
        assert!(!all.contains("tool_use"), "{landed:?}");
        assert!(!all.contains("ignored role"), "{landed:?}");
        assert!(!all.contains("not json"), "{landed:?}");

        // A second run replaces rather than duplicating: the document id is a
        // hash of the path, so it is stable across runs.
        assert_eq!(seed(&svc, "demo", &base, Some(0), true, Some(&projects)).transcripts, 2);
        assert_eq!(rows(&db, "demo").len(), 2);
        std::fs::remove_dir_all(&base).ok();
    }

    // Neither source has anything to offer, and that is not a failure: the
    // run reports what it skipped and reports zero.
    #[test]
    fn a_plain_directory_and_a_missing_transcript_dir_seed_nothing() {
        let base = tmp("empty");
        let db = base.join("memory.db");
        let svc = svc(&db);
        let out = seed(&svc, "demo", &base, None, true, Some(&base.join("absent")));

        assert_eq!(out.git, 0, "{:?}", out.notes);
        assert_eq!(out.transcripts, 0, "{:?}", out.notes);
        assert!(rows(&db, "demo").is_empty());
        assert!(
            out.notes.iter().any(|n| n.contains("transcripts")),
            "a missing transcript dir must be named: {:?}",
            out.notes
        );
        assert_eq!(out.git + out.transcripts, 0);
        std::fs::remove_dir_all(&base).ok();
    }

    // The log split keys off the sha, not off blank lines, so a body with a
    // paragraph break stays one commit.
    #[test]
    fn parse_log_should_split_on_the_sha_not_on_blank_lines() {
        let raw = "\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa one subject
body one

body two after a break
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb two subject
";
        let commits = parse_log(raw);
        assert_eq!(commits.len(), 2, "{commits:?}");
        assert_eq!(commits[0].sha, "a".repeat(40));
        assert_eq!(commits[0].subject, "one subject");
        assert_eq!(commits[0].body, vec!["body one", "", "body two after a break"]);
        assert_eq!(commits[1].sha, "b".repeat(40));
        assert!(commits[1].body.is_empty(), "{commits:?}");
    }

    // The body cap, so one verbose commit cannot fill a memory on its own.
    #[test]
    fn parse_log_should_keep_ten_body_lines() {
        let body: String = (1..=25).map(|i| format!("line {i}\n")).collect();
        let raw = format!("{} subject\n{body}", "c".repeat(40));
        let commits = parse_log(&raw);
        assert_eq!(commits[0].body.len(), BODY_LINES, "{:?}", commits[0].body);
        assert_eq!(commits[0].body[9], "line 10");
    }

    // A run with nothing to seed still exits 0 and says so on stdout — the
    // contract a user scripting a first-run seed depends on.
    #[test]
    fn the_cli_exits_zero_when_there_is_nothing_to_seed() {
        let dir = tmp("cli");
        let out = std::process::Command::new(crate::bin())
            .args(["seed", "--transcripts", "--bank", "nightly-repo"])
            .arg("--db")
            .arg(dir.join("memory.db"))
            .current_dir(&dir)
            .env("HOME", &dir)
            .env_remove("XDG_DATA_HOME")
            .output()
            .expect("run memory-wire");
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(out.status.code(), Some(0), "stderr: {stderr}");
        assert!(
            stdout.contains("seeded 0 git + 0 transcripts into bank 'nightly-repo'"),
            "stdout: {stdout}\nstderr: {stderr}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
