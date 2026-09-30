//! Codex's second file: `~/.codex/config.toml`, a TOML document we do not own.
//!
//! Codex is the one host with two install surfaces. `connect.rs` writes its five
//! lifecycle hooks into `~/.codex/hooks.json` and is finished; this module adds
//! the `[mcp_servers.memory-wire]` entry that gives a Codex session the four
//! tools at all. It is a separate file in a separate format, so it is a separate
//! module rather than a second branch inside the JSON pass.
//!
//! Why `url` and not `command`: Codex on this machine configures MCP by URL
//! (`~/.codex/config.toml`, `[mcp_servers.browseros-neo]`), so an entry carrying
//! `command`/`args` is silently ignored and the session gets context with no way
//! to call `memory_recall`. This points at the HTTP MCP endpoint `serve` already
//! exposes, which is the only transport Codex can reach.
//!
//! Four properties are inherited from `connect_plugin.rs`, which solves the
//! identical problem for YAML, and are the reason this file has that shape:
//!
//! - **Never corrupt.** There is no TOML dependency here and none is being added;
//!   a user's `config.toml` is not something to re-serialise through a second
//!   writer that would reorder every key in it. [`set_url`] is a surgical text
//!   edit that refuses every shape it does not fully understand and leaves the
//!   file byte-identical when it does.
//! - **Idempotent.** A file already in the requested state is not rewritten.
//! - **Reversible.** The pre-edit `config.toml` is copied under
//!   `backups/codex-<ts>/` before the first write, and `--uninstall` removes only
//!   the `url` line this install wrote, restoring a hand-written one from
//!   [`STATE_FILE`].
//! - **Own only what we wrote.** Keys in the table that this install never wrote
//!   keep their table, and are reported rather than deleted.

use std::path::{Path, PathBuf};

use crate::connect::{backup, timestamp, Host, Outcome, MARKER};
use crate::paths;

/// `~/.codex/config.toml`, home-relative. Codex reads hooks from `hooks.json`
/// and MCP servers from here.
fn config_rel() -> PathBuf {
    PathBuf::from(".codex").join("config.toml")
}

/// The install state, written beside the config it describes.
///
/// `[mcp_servers.memory-wire]` may have been the user's own before we touched it,
/// and the `url` in it may have been something they care about. Without this file
/// `--uninstall` would delete a hand-written entry, or leave ours behind.
const STATE_FILE: &str = ".memory-wire-install.json";

/// What to do to the `url` inside `[mcp_servers.memory-wire]`.
enum Url {
    /// Set it to this value.
    Set(String),
    /// Take out the line this install wrote. When `restore` is `Some`, that
    /// value goes back into the same line rather than the key disappearing —
    /// `--uninstall` owes the user whatever we displaced.
    Remove {
        /// The `url` that was there before us, when there was one.
        restore: Option<String>,
    },
}

/// Install or remove the Codex MCP entry. See the module docs.
pub fn apply(home: &Path, uninstall: bool, data_root: &Path) -> Outcome {
    let path = home.join(config_rel());
    let raw = match paths::read_or_empty(&path) {
        Ok(s) => s,
        Err(e) => return Outcome::Failed(format!("{}: {e}", path.display())),
    };

    // Read before writing. The value we displace is what `--uninstall` owes back,
    // and a file we cannot understand is a file we must not have edited.
    let found = match find_entry(&raw) {
        Ok(f) => f,
        Err(why) => return Outcome::Failed(format!("{}: {why}", path.display())),
    };
    let previous = state_previous_url(&path).unwrap_or(None);

    let ts = timestamp();
    let saved = match backup(data_root, home, Host::Codex, &ts, &config_rel()) {
        Ok(b) => b,
        Err(e) => return Outcome::Failed(e),
    };

    let mut events = Vec::new();
    let mut notes = Vec::new();

    if uninstall {
        // Only ever undo our own write: a `url` that is not the one we installed
        // was changed by the user or another tool since, and removing it would
        // be the clobber this installer exists to avoid.
        if !owned_by_us(found.as_ref()) {
            return Outcome::Skipped(if found.is_some() {
                "config.toml: mcp_servers.memory-wire.url is not ours; left it alone".to_string()
            } else {
                "no memory-wire MCP entry in config.toml".to_string()
            });
        }
        if let Err(e) =
            write_url(&raw, &path, Url::Remove { restore: previous.clone() }, &mut events)
        {
            return Outcome::Failed(e);
        }
        if events.is_empty() {
            return Outcome::Skipped("no memory-wire MCP entry in config.toml".to_string());
        }
        std::fs::remove_file(path.with_file_name(STATE_FILE)).ok();
        if previous.is_some() {
            notes.push("config.toml: the previous url is back in place".to_string());
        }
        return Outcome::Unwired { backup: saved, events, notes };
    }

    let url = mcp_url();
    if found.as_ref().and_then(Entry::url) == Some(url.as_str()) {
        notes.push("config.toml: mcp_servers.memory-wire.url already current".to_string());
        return Outcome::Already { notes };
    }
    // Record what we are about to displace, so `--uninstall` can put it back. A
    // reinstall keeps the *first* recorded value: the thing we displaced is
    // still displaced, and overwriting the record with a value that came after
    // us would strand whatever was there before.
    if let Some(was) = found.as_ref().and_then(|e| e.url()) {
        let was = was.to_string();
        if let Err(e) = write_state(&path, was) {
            return Outcome::Failed(e);
        }
    }
    if let Err(e) = write_url(&raw, &path, Url::Set(url), &mut events) {
        return Outcome::Failed(e);
    }
    if previous.is_some() {
        notes.push("config.toml: the previous url is recorded; --uninstall restores it".to_string());
    }
    Outcome::Wired { backup: saved, events, notes }
}

/// The URL Codex should dial: the endpoint the rest of the tree already resolves,
/// plus the MCP mount.
///
/// `paths::endpoint()` is `MEMORY_WIRE_URL` when set, else a running daemon's
/// recorded address, else `http://127.0.0.1:8888`
/// otherwise, so this is the one port literal in the tree again, and a user who
/// already points their hooks at a non-default server gets the MCP entry for that
/// same server. A trailing slash is trimmed so the join cannot produce `//mcp`.
fn mcp_url() -> String {
    format!("{}/mcp", paths::endpoint().trim_end_matches('/'))
}

/// Whether the entry in the file is still the one we installed.
///
/// Deliberately the *URL* and not the table name. The table is named after us, so
/// its name proves nothing; the value we wrote is what identifies our write. A
/// user who edits the `url` afterwards has made it theirs, and an uninstall that
/// removed it would be deleting their change.
fn owned_by_us(found: Option<&Entry>) -> bool {
    found.and_then(Entry::url) == Some(mcp_url().as_str())
}

/// The edit `op` asks for, applied to the document text and written back.
///
/// `events` grows only when the file really changes, so a config already in the
/// requested state falls through to `Already` rather than being rewritten — the
/// same "a note is not a change" rule `connect.rs` applies to hook slots.
fn write_url(raw: &str, path: &Path, op: Url, events: &mut Vec<String>) -> Result<(), String> {
    let next = set_url(raw, &op).map_err(|e| format!("{}: {e}", path.display()))?;
    if next == raw {
        return Ok(());
    }
    paths::write_atomic(path, &next).map_err(|e| format!("{}: {e}", path.display()))?;
    events.push("config.toml: mcp_servers.memory-wire".to_string());
    Ok(())
}

/// Where `[mcp_servers.memory-wire]` is, how far it runs, and what `url` holds.
#[derive(Debug)]
struct Entry {
    /// Index of the `[mcp_servers.memory-wire]` line.
    head: usize,
    /// One past the last line of the table.
    end: usize,
    /// `(index within the table body, value)` of `url`, when it is there.
    url: Option<(usize, String)>,
}

impl Entry {
    /// The `url` this table holds, if it holds one.
    fn url(&self) -> Option<&str> {
        self.url.as_ref().map(|(_, v)| v.as_str())
    }
}

/// Locate `[mcp_servers.memory-wire]`, or `None` when there is no such table.
///
/// Refuses rather than guesses, on the same reasoning as the YAML editor: getting
/// a table's extent wrong rewrites a line the user wrote, and `config.toml` is
/// the file that decides which tools a coding agent can call at all.
///
/// - a line with an odd number of `"`, an unterminated string, which makes every
///   column after it a guess;
/// - a `[` that never closes, so no line-delimited extent can be trusted;
/// - two `[mcp_servers.memory-wire]` headers, or two `url` keys in one;
/// - `mcp_servers` defined as a scalar rather than a table;
/// - the entry present as an inline `memory-wire = {…}` or a dotted
///   `memory-wire.url = "…"`, neither of which is the shape this edit owns and
///   either of which would be a rewrite rather than an edit;
/// - a line inside the table that is not `key = value`.
fn find_entry(text: &str) -> Result<Option<Entry>, String> {
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.matches('"').count() % 2 == 1 {
            return Err(format!("line {} has an unterminated string; left untouched", i + 1));
        }
        let t = line.trim();
        if t.starts_with('[') && !t.ends_with(']') {
            return Err(format!(
                "line {} opens a table header that never closes; left untouched",
                i + 1
            ));
        }
    }

    let entry_header = format!("[mcp_servers.{MARKER}]");
    let mut heads: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim() == entry_header)
        .map(|(i, _)| i)
        .collect();
    if heads.len() > 1 {
        return Err(format!("more than one `{entry_header}` table; left untouched"));
    }
    // `mcp_servers` as a scalar has no table to extend.
    if let Some(i) = lines
        .iter()
        .position(|l| l.trim_start().starts_with("mcp_servers") && l.contains(" = ") && !l.contains('['))
    {
        return Err(format!("`mcp_servers` on line {} is not a table; left untouched", i + 1));
    }

    let Some(head) = heads.pop() else {
        // No table of our own. Refuse if the entry is present in a shape we do
        // not own, rather than appending a second definition of it.
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with(&format!("{MARKER} =")) || t.starts_with(&format!("{MARKER}.")) {
                return Err(format!(
                    "mcp_servers.{MARKER} on line {} is an inline or dotted key, not a table; \
                     left untouched",
                    i + 1
                ));
            }
        }
        return Ok(None);
    };

    // A table runs until the next line that opens one at column 0.
    let end = lines[head + 1..]
        .iter()
        .position(|l| l.starts_with('['))
        .map_or(lines.len(), |p| head + 1 + p);

    let mut url: Option<(usize, String)> = None;
    for (i, line) in lines.iter().enumerate().take(end).skip(head + 1) {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!(
                "line {} inside {entry_header} is not a `key = value`; left untouched",
                i + 1
            ));
        };
        if k.trim() != "url" {
            continue;
        }
        if url.is_some() {
            return Err(format!("more than one `url` in {entry_header}; left untouched"));
        }
        // Only a plain double-quoted scalar is a shape we may replace. A bare
        // token, a single-quoted string or a block scalar would come back as a
        // "value" and be replaced by a quoted one, changing what the file means.
        let value = v.split('#').next().unwrap_or("").trim();
        let inner = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or_else(|| {
                format!("`url` on line {} is not a plain quoted string; left untouched", i + 1)
            })?;
        url = Some((i - head - 1, inner.to_string()));
    }
    Ok(Some(Entry { head, end, url }))
}

/// Set or remove the `url` inside `[mcp_servers.memory-wire]`.
///
/// Adding the table is an append separated by one blank line, which is the shape
/// the host's own file already uses. Removing is deliberately narrower than
/// deleting the table: a key this install did not write keeps its table, because
/// a directory of someone else's configuration is not ours to empty.
fn set_url(text: &str, op: &Url) -> Result<String, String> {
    let Some(entry) = find_entry(text)? else {
        let Url::Set(v) = op else { return Ok(text.to_string()) };
        let mut out = text.to_string();
        if !out.is_empty() {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(&format!("[mcp_servers.{MARKER}]\nurl = \"{v}\"\n"));
        return Ok(out);
    };

    let all: Vec<String> = text.lines().map(str::to_string).collect();
    // Edits happen on the table's own slice. Doing them in place in `all` would
    // shift every index after a removal, and `end` was computed before the edit.
    let mut body: Vec<String> = all[entry.head + 1..entry.end].to_vec();
    let rel = entry.url.as_ref().map(|(i, _)| *i);
    match (rel, op) {
        (Some(i), Url::Set(v)) => {
            body[i] = preserve_comment(&body[i], &format!("url = \"{v}\""));
        }
        (Some(i), Url::Remove { restore: Some(previous) }) if body_is_ours(&body[i]) => {
            body[i] = preserve_comment(&body[i], &format!("url = \"{previous}\""));
        }
        (Some(i), Url::Remove { restore: None }) if body_is_ours(&body[i]) => {
            body.remove(i);
        }
        // A value we did not write, or a key to remove that is not there: change
        // nothing.
        (Some(_), _) | (None, Url::Remove { .. }) => return Ok(text.to_string()),
        (None, Url::Set(v)) => body.push(format!("url = \"{v}\"")),
    }

    // A table emptied of everything, including anything a user added, goes with
    // its header. A table that still holds a real key is theirs and stays.
    let keep_head =
        !body.iter().all(|l| l.trim().is_empty() || l.trim().starts_with('#'));
    let mut out: Vec<String> = all[..entry.head].to_vec();
    if keep_head {
        out.push(all[entry.head].clone());
    } else if out.last().is_some_and(|l| l.trim().is_empty()) {
        // Take the blank line that separated this table from the one above it,
        // so removing an install leaves the file as it was found.
        out.pop();
    }
    out.extend(body);
    out.extend_from_slice(&all[entry.end..]);
    let mut joined = out.join("\n");
    if !joined.is_empty() {
        joined.push('\n');
    }
    Ok(joined)
}

/// Whether this `url` line holds the value [`mcp_url`] produces.
fn body_is_ours(line: &str) -> bool {
    line.split('#').next().unwrap_or("").trim() == format!("url = \"{}\"", mcp_url())
}

/// Rewrite a `key = value` line, keeping any trailing `# comment`.
///
/// A comment on a line the user annotated is theirs; dropping it because we
/// changed the value beside it would be an edit they did not ask for.
fn preserve_comment(line: &str, replacement: &str) -> String {
    match line.find(" #") {
        Some(at) => format!("{} {}", replacement, &line[at..]),
        None => replacement.to_string(),
    }
}

/// The `previous_url` this install recorded, if it ever recorded one.
fn state_previous_url(config: &Path) -> Result<Option<String>, String> {
    let Ok(text) = std::fs::read_to_string(config.with_file_name(STATE_FILE)) else {
        return Ok(None);
    };
    Ok(serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("previous_url").and_then(|p| p.as_str()).map(str::to_string)))
}

/// Record the `url` we are about to displace, so `--uninstall` can put it back.
fn write_state(config: &Path, previous: String) -> Result<(), String> {
    let state = config.with_file_name(STATE_FILE);
    if let Some(first) = state_previous_url(config)? {
        if first != previous {
            return Ok(());
        }
    }
    let mut text =
        serde_json::to_string_pretty(&serde_json::json!({ "previous_url": previous }))
            .unwrap_or_default();
    text.push('\n');
    paths::write_atomic(&state, &text).map_err(|e| format!("{}: {e}", state.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-codex-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp home");
        dir
    }

    fn write_config(home: &Path, body: &str) {
        let p = home.join(config_rel());
        std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        std::fs::write(&p, body).expect("write config");
    }

    fn read_config(home: &Path) -> String {
        std::fs::read_to_string(home.join(config_rel())).expect("read config")
    }

    /// Drive the TOML pass on its own, so a test is about `config.toml` and not
    /// about whatever the hooks pass happened to do in the same call.
    fn toml_only(home: &Path, uninstall: bool) -> Outcome {
        let data = home.join("data");
        std::fs::create_dir_all(&data).expect("data dir");
        apply(home, uninstall, &data)
    }

    /// A PATH lookup that finds nothing, so detection cannot depend on what is
    /// installed on the machine running the tests.
    fn no_path(_name: &str) -> bool {
        false
    }

    /// The shape on this machine: a `url`-based foreign MCP entry, then unrelated
    /// tables either side of it.
    const REAL: &str = "\
model = \"gpt-5.6-sol\"

[mcp_servers.browseros-neo]
url = \"http://127.0.0.1:9012/mcp\"

[notice.model_migrations]
\"gpt-5.6-sol\" = \"free-stack\"
";

    fn ours() -> String {
        mcp_url()
    }

    // -- the TOML editor -------------------------------------------------

    #[test]
    fn the_real_config_shape_is_understood_and_our_entry_found_absent() {
        assert!(find_entry(REAL).expect("parseable").is_none());
    }

    #[test]
    fn setting_the_url_leaves_every_other_line_byte_identical() {
        let out = set_url(REAL, &Url::Set(ours())).expect("set");
        assert_eq!(out, format!("{REAL}\n[mcp_servers.memory-wire]\nurl = \"{}\"\n", ours()));
    }

    #[test]
    fn a_reinstall_replaces_our_own_url_in_place() {
        let first = set_url(REAL, &Url::Set(ours())).expect("install");
        let second = set_url(&first, &Url::Set(ours())).expect("reinstall");
        assert_eq!(second, first, "an identical write is a no-op");
    }

    #[test]
    fn removing_our_table_takes_the_separating_blank_line_with_it() {
        let installed = set_url(REAL, &Url::Set(ours())).expect("install");
        let gone = set_url(&installed, &Url::Remove { restore: None }).expect("remove");
        assert_eq!(gone, REAL);
    }

    #[test]
    fn removing_ours_puts_a_displaced_url_back_in_the_same_line() {
        let hand = format!("{REAL}\n[mcp_servers.memory-wire]\nurl = \"http://elsewhere/mcp\"\n");
        // We took it over…
        let taken = set_url(&hand, &Url::Set(ours())).expect("take over");
        assert!(taken.contains(&ours()), "{taken}");
        // …and uninstall hands the user's value back rather than dropping the key.
        let back = set_url(&taken, &Url::Remove {
            restore: Some("http://elsewhere/mcp".to_string()),
        })
        .expect("restore");
        assert_eq!(back, hand, "the file is exactly as the user left it");
    }

    #[test]
    fn removing_an_entry_that_is_not_ours_changes_nothing() {
        // A hand-written entry: the url is not the one we write, so Remove is a
        // no-op and every byte survives — including when a restore is offered.
        let hand = format!("{REAL}\n[mcp_servers.memory-wire]\nurl = \"http://elsewhere/mcp\"\n");
        assert_eq!(
            set_url(&hand, &Url::Remove { restore: None }).expect("remove"),
            hand
        );
        assert_eq!(
            set_url(&hand, &Url::Remove { restore: Some("x".to_string()) }).expect("remove"),
            hand
        );
    }

    #[test]
    fn a_trailing_comment_on_our_url_line_survives_a_reinstall() {
        let pre = format!("{REAL}\n[mcp_servers.memory-wire]\nurl = \"http://old/mcp\"  # pinned\n");
        let out = set_url(&pre, &Url::Set(ours())).expect("set");
        assert!(out.contains("# pinned"), "a user's comment was dropped: {out}");
    }

    // -- refusals: every one leaves the text exactly as it found it --------

    #[test]
    fn a_duplicate_table_header_is_refused() {
        let dup = format!(
            "{REAL}\n[mcp_servers.memory-wire]\nurl = \"http://x/mcp\"\n\
             [mcp_servers.memory-wire]\nurl = \"http://y/mcp\"\n"
        );
        let err = find_entry(&dup).expect_err("duplicate must refuse");
        assert!(err.contains("more than one"), "{err}");
        assert!(set_url(&dup, &Url::Set(ours())).is_err(), "and does not edit");
    }

    #[test]
    fn a_refusal_leaves_the_text_byte_identical() {
        // The property every refusal exists for: `set_url` propagates the error
        // rather than returning a rewritten document, so the caller writes
        // nothing. Proven here on the text, and end to end on a real file by
        // `a_malformed_config_is_refused_and_left_byte_identical`.
        let broken = "model = \"gpt\n\n[mcp_servers.memory-wire]\nurl = \"http://x/mcp\"\n";
        let before = broken.as_bytes().to_vec();
        assert!(set_url(broken, &Url::Set(ours())).is_err());
        assert_eq!(broken.as_bytes(), before, "a refusal is not an edit");
    }

    #[test]
    fn an_inline_entry_under_mcp_servers_is_refused_rather_than_rewritten() {
        for inline in [
            "mcp_servers = { memory-wire = { url = \"http://x/mcp\" } }\n",
            "memory-wire = { url = \"http://x/mcp\" }\n",
        ] {
            let err = find_entry(inline).expect_err("inline must refuse");
            assert!(err.contains("left untouched"), "{err}");
            // Refused, and the document is not rewritten on the way out.
            assert!(set_url(inline, &Url::Set(ours())).is_err());
        }
    }

    #[test]
    fn a_dotted_key_entry_is_refused() {
        let dotted = "memory-wire.url = \"http://x/mcp\"\n";
        let err = find_entry(dotted).expect_err("dotted must refuse");
        assert!(err.contains("not a table") || err.contains("not a shape"), "{err}");
        assert!(set_url(dotted, &Url::Set(ours())).is_err(), "and does not edit");
    }

    #[test]
    fn a_scalar_mcp_servers_is_refused() {
        let err = find_entry("mcp_servers = 3\n").expect_err("scalar must refuse");
        assert!(err.contains("not a table"), "{err}");
    }

    #[test]
    fn a_malformed_document_is_refused() {
        for (broken, needle) in [
            ("model = \"gpt\n\n[mcp_servers.memory-wire]\n", "unterminated string"),
            ("[mcp_servers.memory-wire\nurl = \"http://x/mcp\"\n", "never closes"),
        ] {
            let err = find_entry(broken).expect_err("broken must refuse");
            assert!(err.contains(needle), "{err}");
        }
    }

    #[test]
    fn a_url_that_is_not_a_plain_quoted_string_is_refused() {
        for odd in [
            "[mcp_servers.memory-wire]\nurl = 'http://x/mcp'\n",
            "[mcp_servers.memory-wire]\nurl = http://x/mcp\n",
        ] {
            let err = find_entry(odd).expect_err("odd url must refuse");
            assert!(err.contains("not a plain quoted string"), "{err}");
        }
    }

    #[test]
    fn a_line_inside_our_table_that_is_not_a_pair_is_refused() {
        let odd = "[mcp_servers.memory-wire]\nurl = \"http://x/mcp\"\nthis is not toml\n";
        let err = find_entry(odd).expect_err("stray line must refuse");
        assert!(err.contains("not a `key = value`"), "{err}");
    }

    // -- install / uninstall through the real entry point -----------------

    #[test]
    fn install_then_uninstall_round_trips_and_leaves_the_file_as_it_was() {
        let home = tmp_home("roundtrip");
        write_config(&home, REAL);
        let out = toml_only(&home, false);
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let installed = read_config(&home);
        assert!(installed.contains("[mcp_servers.memory-wire]"), "{installed}");
        assert!(installed.starts_with(REAL.trim()), "the foreign entry survives: {installed}");

        let out = toml_only(&home, true);
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        assert_eq!(read_config(&home), REAL, "uninstall restores the file exactly");
        std::fs::remove_dir_all(&home).ok();
    }

    // The one test that goes through `connect::apply`, because the thing worth
    // pinning is that Codex gets BOTH files: the hooks in `hooks.json` and the
    // tools in `config.toml`. A pass that only did the second would still pass
    // every test above.
    #[test]
    fn connect_codex_wires_the_hooks_and_the_mcp_entry_in_one_call() {
        let home = tmp_home("both-files");
        std::fs::create_dir_all(home.join(".codex")).expect("mkdir");
        let data = home.join("data");
        std::fs::create_dir_all(&data).expect("data dir");

        let out = crate::connect::apply(Host::Codex, "/bin/memory-wire", &home, false, no_path, &data, None);
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let hooks = std::fs::read_to_string(home.join(".codex").join("hooks.json"))
            .expect("hooks.json is wired");
        for ev in ["SessionStart", "UserPromptSubmit", "Stop", "PreCompact", "SessionEnd"] {
            assert!(hooks.contains(ev), "{ev} missing: {hooks}");
        }
        let toml = read_config(&home);
        assert!(toml.contains("[mcp_servers.memory-wire]"), "no MCP entry: {toml}");
        assert!(toml.contains(&ours()), "the MCP entry has no url: {toml}");

        // A second call changes neither file.
        let before = (read_config(&home), hooks);
        let out = crate::connect::apply(Host::Codex, "/bin/memory-wire", &home, false, no_path, &data, None);
        assert!(matches!(out, Outcome::Already { .. }), "a re-install must be Already: {out:?}");
        assert_eq!((read_config(&home), std::fs::read_to_string(home.join(".codex").join("hooks.json")).expect("read")), before);

        // Uninstall takes out the tools and reports the hooks it did not own —
        // `hooks.json` was created by this same call, so it goes too.
        let out = crate::connect::apply(Host::Codex, "/bin/memory-wire", &home, true, no_path, &data, None);
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        assert!(!home.join(".codex").join(STATE_FILE).exists(), "the state file survived");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_second_install_is_a_no_op_and_does_not_churn_the_file() {
        let home = tmp_home("idempotent");
        write_config(&home, REAL);
        toml_only(&home, false);
        let after_first = read_config(&home);
        assert!(matches!(toml_only(&home, false), Outcome::Already { .. }), "must be Already");
        assert_eq!(read_config(&home), after_first, "the file is byte-identical");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_malformed_config_is_refused_and_left_byte_identical() {
        let home = tmp_home("malformed");
        let broken = "model = \"gpt\n\n[mcp_servers.memory-wire]\nurl = \"http://x/mcp\"\n";
        write_config(&home, broken);
        let before = std::fs::read(home.join(config_rel())).expect("bytes");
        assert!(matches!(toml_only(&home, false), Outcome::Failed(_)), "must refuse");
        assert_eq!(std::fs::read(home.join(config_rel())).expect("bytes"), before, "a refused install writes nothing");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn install_onto_a_foreign_url_takes_it_over_and_uninstall_restores_it() {
        let home = tmp_home("takeover");
        let foreign = "http://127.0.0.1:7777/mcp";
        write_config(&home, &format!("{REAL}\n[mcp_servers.memory-wire]\nurl = \"{foreign}\"\n"));
        toml_only(&home, false);
        assert!(read_config(&home).contains(&ours()), "we did not take it over");

        // The displaced value is on record, so `--uninstall` owes it back.
        let state: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(home.join(".codex").join(STATE_FILE)).expect("state"),
        )
        .expect("json");
        assert_eq!(state["previous_url"], serde_json::json!(foreign));

        toml_only(&home, true);
        let after = read_config(&home);
        assert!(after.contains(foreign), "the user's url was not restored: {after}");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn uninstall_leaves_a_users_key_in_place_and_takes_only_our_url() {
        let home = tmp_home("foreign-key");
        // Our url *and* a key we never wrote, in the same table.
        let pre = format!(
            "{REAL}\n[mcp_servers.memory-wire]\nurl = \"{}\"\nenv = {{ FOO = \"bar\" }}\n",
            ours()
        );
        write_config(&home, &pre);
        assert!(matches!(toml_only(&home, false), Outcome::Already { .. }), "already ours");

        toml_only(&home, true);
        let after = read_config(&home);
        assert!(after.contains("env = { FOO = \"bar\" }"), "a user's key was deleted: {after}");
        assert!(after.contains("[mcp_servers.memory-wire]"), "the table was dropped: {after}");
        assert!(!after.contains(&ours()), "our url survived: {after}");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn uninstall_leaves_a_url_somebody_else_rewrote_entirely_alone() {
        let home = tmp_home("rewritten");
        let edited = "http://127.0.0.1:1234/mcp";
        write_config(&home, &format!("{REAL}\n[mcp_servers.memory-wire]\nurl = \"{}\"\n", ours()));
        toml_only(&home, false);
        // The user repoints the entry after we installed it.
        let path = home.join(config_rel());
        let text = std::fs::read_to_string(&path).expect("read");
        std::fs::write(&path, text.replace(&ours(), edited)).expect("rewrite");

        let before = std::fs::read_to_string(&path).expect("read");
        let out = toml_only(&home, true);
        assert!(matches!(out, Outcome::Skipped(_)), "a url that is not ours is not removed: {out:?}");
        assert_eq!(read_config(&home), before, "and the file is byte-identical");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_url_points_at_the_mcp_mount_on_the_one_default_endpoint() {
        assert_eq!(mcp_url(), format!("{}/mcp", paths::endpoint().trim_end_matches('/')));
    }
}
