//! Hermes: the one host memory-wire installs as a **directory** rather than as
//! a line of JSON.
//!
//! Every other host in [`crate::connect::Host`] is one file with a known key.
//! Hermes is
//! a plugin loader: `~/.hermes/plugins/<name>/__init__.py` with a class on it,
//! activated by one scalar — `memory.provider` — in `~/.hermes/config.yaml`. So
//! this module does the two things that host needs and the JSON path cannot
//! express: copy a tree of embedded files, and move one key in a YAML document.
//!
//! Three properties are inherited from `connect.rs` rather than reinvented:
//!
//! - **Never corrupt.** [`set_provider`] is a surgical text edit that *refuses*
//!   every shape it does not fully understand and leaves the file byte-identical
//!   when it does. memory-wire has no YAML dependency and this does not add one;
//!   a 31 KB user config is not something to re-serialise through a second
//!   writer that would reorder every key in it.
//! - **Idempotent.** A file whose content already matches is not rewritten, and a
//!   second install reports `already-wired` rather than churning.
//! - **Reversible.** The pre-edit `config.yaml` and any pre-existing plugin
//!   directory are copied to `backups/hermes-<ts>/` before the first write, and
//!   `--uninstall` restores the *previous* provider — including removing the key
//!   when there was none — rather than leaving `provider: memory-wire` pointing at
//!   a directory that is gone.

use std::path::{Path, PathBuf};

use crate::connect::{backup, timestamp, Host, Outcome, MARKER};
use crate::paths;

/// The plugin tree, embedded at build time.
///
/// The competitor's own README answers "how do I install this" with
/// `cp -r integrations/hermes ~/.hermes/plugins/agentmemory` — a step that needs
/// a source checkout. `connect` has no such precondition: it must succeed on a
/// machine where nothing is running and no repository is present, so the files
/// travel inside the binary and this is a copy.
///
/// Source of truth is `plugin/integrations/hermes/memory-wire/`. A file added
/// there but not here would install an incomplete tree and pass every other test
/// in this module, so `the_template_directory_is_the_one_we_install` asserts the
/// two agree.
const FILES: &[(&str, &str)] = &[
    (
        "__init__.py",
        include_str!("../plugin/integrations/hermes/memory-wire/__init__.py"),
    ),
    (
        "plugin.yaml",
        include_str!("../plugin/integrations/hermes/memory-wire/plugin.yaml"),
    ),
    (
        "README.md",
        include_str!("../plugin/integrations/hermes/memory-wire/README.md"),
    ),
];

/// The install state, written beside the plugin it describes.
///
/// `memory.provider` holds one value and we are about to overwrite whatever is
/// there. Hermes enforces a one-external-provider limit, so on this box that
/// value is very likely `agentmemory` and the user will want it back. Without
/// this file `--uninstall` would leave `provider: memory-wire` naming a directory
/// that no longer exists, which Hermes reads as "no memory at all" and reports
/// nothing about.
const STATE_FILE: &str = ".memory-wire-install.json";

/// What to do to `memory.provider`.
///
/// Both the ownership rule and the no-op live here rather than in the caller: an
/// empty [`Provider::Set`] and a [`Provider::Remove`] that does not own the
/// current value are both "change nothing", and `set_provider` is the only place
/// that decides. A caller cannot forget the check by forgetting to call it.
enum Provider {
    /// Set the key to this value. Empty is a no-op.
    Set(String),
    /// Remove the key — but only if it currently holds our own marker — and the
    /// `memory:` block if this key was all it held.
    Remove,
}

/// Install or uninstall the Hermes plugin. See the module docs.
pub fn apply(host: Host, home: &Path, uninstall: bool, data_root: &Path) -> Outcome {
    let plugin_dir = home.join(plugin_rel());
    let config = home.join(host.config_rel());
    let raw = match paths::read_or_empty(&config) {
        Ok(s) => s,
        Err(e) => return Outcome::Failed(format!("{}: {e}", config.display())),
    };

    // Read before writing: the value we displace is the thing `--uninstall` owes
    // the user back, and a config we refuse to parse is a config we must not
    // have installed a plugin for.
    let current = match provider_value(&raw) {
        Ok(v) => v,
        Err(e) => return Outcome::Failed(format!("{}: {e}", config.display())),
    };
    let previous = state_previous_provider(&plugin_dir).unwrap_or(None);

    let ts = timestamp();
    let mut saved = match backup(data_root, home, host, &ts, &host.config_rel()) {
        Ok(b) => b,
        Err(e) => return Outcome::Failed(e),
    };
    // Both land in the same `hermes-<ts>/` directory, so the second call adds to
    // the first rather than replacing it.
    match backup(data_root, home, host, &ts, &plugin_rel()) {
        Ok(Some(d)) => saved = Some(d),
        Ok(None) => {}
        Err(e) => return Outcome::Failed(e),
    }

    let mut events = Vec::new();
    let mut notes = Vec::new();

    if uninstall {
        if plugin_dir.is_dir() {
            match remove_plugin(&plugin_dir) {
                Ok(foreign) => {
                    events.push(plugin_rel().display().to_string());
                    if !foreign.is_empty() {
                        notes.push(format!(
                            "left {} file(s) in {} that this install did not write",
                            foreign.len(),
                            plugin_dir.display()
                        ));
                    }
                }
                Err(e) => return Outcome::Failed(e),
            }
        }
        if let Err(e) = write_provider(
            &raw,
            &config,
            restore(current.as_deref(), previous.as_deref()),
            &mut events,
        ) {
            return Outcome::Failed(e);
        }
        if events.is_empty() {
            return Outcome::Skipped("no memory-wire plugin to remove".to_string());
        }
        // The same rule `connect.rs` applies to a config it created and then
        // emptied: on a first install this directory is our own residue, and a
        // Hermes with no plugins under it is better off without it. A directory
        // holding anyone else's plugin is not ours to remove.
        prune_empty_plugins_dir(&plugin_dir);
        return Outcome::Unwired {
            backup: saved,
            events,
            notes,
        };
    }

    // The tree first, then the key that activates it. The other order would
    // leave `provider: memory-wire` naming a directory that was never written,
    // which is the one state Hermes cannot report.
    match write_plugin(&plugin_dir) {
        Ok(written) => {
            if written.is_empty() {
                notes.push("plugin directory already current".to_string());
            } else {
                events.push(plugin_rel().display().to_string());
            }
        }
        Err(e) => return Outcome::Failed(e),
    }
    if current.as_deref() != Some(MARKER) {
        if let Err(e) = write_state(&plugin_dir, current.as_deref()) {
            return Outcome::Failed(e);
        }
    }
    if let Err(e) = write_provider(
        &raw,
        &config,
        Provider::Set(MARKER.to_string()),
        &mut events,
    ) {
        return Outcome::Failed(e);
    }
    if let Some(was) = current.as_deref().filter(|c| *c != MARKER) {
        notes.push(format!(
            "memory.provider was {was}; --uninstall restores it"
        ));
    }
    if events.is_empty() {
        return Outcome::Already { notes };
    }
    Outcome::Wired {
        backup: saved,
        events,
        notes,
    }
}

/// `~/.hermes/plugins/memory-wire`, home-relative.
///
/// The directory name *is* the provider name: `plugins/memory/__init__.py`
/// resolves `memory.provider` by directory name, so anything else here would
/// install a plugin the loader never looks for.
fn plugin_rel() -> PathBuf {
    PathBuf::from(".hermes").join("plugins").join(MARKER)
}

/// The op an uninstall should perform, given what is installed now.
fn restore(current: Option<&str>, previous: Option<&str>) -> Provider {
    // Only ever undo our own write. A `provider` naming something else was set
    // by the user or another tool since, and overwriting it would be the one
    // clobber this installer exists to avoid.
    if current != Some(MARKER) {
        return Provider::Set(String::new());
    }
    match previous {
        Some(p) => Provider::Set(p.to_string()),
        None => Provider::Remove,
    }
}

/// Write the plugin tree, or report which files changed.
///
/// Returns the names actually rewritten; an empty vector means the installed tree
/// already matches the template byte for byte, which is what makes a second
/// install a no-op rather than a mtime churn.
fn write_plugin(dir: &Path) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for (name, body) in FILES {
        let dst = dir.join(name);
        if std::fs::read_to_string(&dst).is_ok_and(|have| have == *body) {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        paths::write_atomic(&dst, body).map_err(|e| format!("{}: {e}", dst.display()))?;
        written.push((*name).to_string());
    }
    Ok(written)
}

/// Remove the files this install wrote, then the directory if we emptied it.
///
/// Returns the names of foreign files left behind rather than deleting them: a
/// directory under `~/.hermes/plugins/` is the user's namespace and `rm -rf` on
/// it would take anything they added. `__pycache__` is the one exception — it is
/// this tree's own byte-compiled form of our own `__init__.py`.
fn remove_plugin(dir: &Path) -> Result<Vec<String>, String> {
    for (name, _) in FILES {
        let path = dir.join(name);
        if path.is_file() {
            std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    std::fs::remove_file(dir.join(STATE_FILE)).ok();
    std::fs::remove_dir_all(dir.join("__pycache__")).ok();
    if let Err(e) = std::fs::remove_dir(dir) {
        // `DirectoryNotEmpty` is the expected shape when the user added
        // something. Anything else is worth saying out loud.
        if e.kind() != std::io::ErrorKind::DirectoryNotEmpty {
            return Err(format!("{}: {e}", dir.display()));
        }
    }
    Ok(foreign_names(dir))
}

/// Names of anything still in the directory we did not put there.
fn foreign_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Drop `~/.hermes/plugins/` if our removal left it empty.
///
/// Best effort by design: a `DirectoryNotEmpty` or a permission error means
/// somebody else is using it, and neither is worth failing an uninstall over.
fn prune_empty_plugins_dir(plugin_dir: &Path) {
    let Some(plugins) = plugin_dir.parent() else {
        return;
    };
    let empty = std::fs::read_dir(plugins)
        .map(|mut e| e.next().is_none())
        .unwrap_or(false);
    if empty {
        std::fs::remove_dir(plugins).ok();
    }
}

/// The `previous_provider` this install recorded, if it ever recorded one.
fn state_previous_provider(dir: &Path) -> Result<Option<String>, String> {
    let Ok(raw) = std::fs::read_to_string(dir.join(STATE_FILE)) else {
        return Ok(None);
    };
    Ok(serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|v| {
            v.get("previous_provider")
                .and_then(|p| p.as_str())
                .map(str::to_string)
        }))
}

/// Record what `memory.provider` was, so `--uninstall` can put it back.
///
/// A reinstall keeps the *first* recorded value: the thing we displaced is still
/// displaced, and overwriting the record with a value that came after us would
/// strand whatever was there before.
fn write_state(dir: &Path, previous: Option<&str>) -> Result<(), String> {
    if let Some(first) = state_previous_provider(dir)? {
        if first != previous.unwrap_or_default() {
            return Ok(());
        }
    }
    let doc = serde_json::json!({ "previous_provider": previous });
    let mut text = serde_json::to_string_pretty(&doc).unwrap_or_default();
    text.push('\n');
    paths::write_atomic(&dir.join(STATE_FILE), &text)
        .map_err(|e| format!("{}: {e}", dir.join(STATE_FILE).display()))
}

/// The value of `memory.provider`, or `None` when the key or the block is absent.
fn provider_value(text: &str) -> Result<Option<String>, String> {
    Ok(find_block(text)?.and_then(|b| b.provider.map(|(_, v)| v)))
}

/// The edit `op` asks for, applied to the config text and written back.
///
/// `events` grows only when the file really changes, so a config already in the
/// requested state falls through to `Already` rather than being rewritten — the
/// same "a note is not a change" rule `connect.rs` applies to hook slots.
fn write_provider(
    raw: &str,
    path: &Path,
    op: Provider,
    events: &mut Vec<String>,
) -> Result<(), String> {
    if matches!(&op, Provider::Set(v) if v.is_empty()) {
        return Ok(());
    }
    let next = set_provider(raw, &op).map_err(|e| format!("{}: {e}", path.display()))?;
    if next == raw {
        return Ok(());
    }
    paths::write_atomic(path, &next).map_err(|e| format!("{}: {e}", path.display()))?;
    events.push("config.yaml: memory.provider".to_string());
    Ok(())
}

/// Where `memory:` is, how far its block runs, and what `provider` holds.
struct Block {
    /// Index of the `memory:` line.
    head: usize,
    /// One past the last line of the block.
    end: usize,
    /// Indent of the block's children.
    child: usize,
    /// `(line index, value)` of `memory.provider`, when it is there.
    provider: Option<(usize, String)>,
}

/// Locate the top-level `memory:` block, or `None` when there is no such key.
///
/// Refuses rather than guesses on the four shapes where a block's extent is
/// ambiguous, because getting it wrong rewrites a line the user wrote:
///
/// - a multi-document file (`---`), where the key may live in a different one;
/// - a `memory:` that is not a block mapping, which has no line-delimited extent;
/// - a second top-level `memory:`, a duplicate key and a parse error in strict
///   YAML;
/// - a `provider:` at column 0, which is some other section's key and not ours.
fn find_block(text: &str) -> Result<Option<Block>, String> {
    if text.starts_with("---") || text.contains("\n---") {
        return Err("config.yaml is a multi-document file; left untouched".to_string());
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut heads: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| is_bare_key(l, 0, "memory"))
        .map(|(i, _)| i)
        .collect();
    if heads.len() > 1 {
        return Err("more than one top-level `memory:` key; left untouched".to_string());
    }
    // `has_key_at` but not `is_bare_key`: a `memory:` that carries a value is a
    // scalar or a flow mapping, neither of which has a line-delimited extent.
    if let Some(i) = lines
        .iter()
        .position(|l| has_key_at(l, 0, "memory") && !is_bare_key(l, 0, "memory"))
    {
        return Err(format!(
            "`memory:` on line {} is not a block mapping; left untouched",
            i + 1
        ));
    }
    if lines.iter().any(|l| has_key_at(l, 0, "provider")) {
        return Err("a top-level `provider:` key exists; left untouched".to_string());
    }
    let Some(head) = heads.pop() else {
        return Ok(None);
    };

    let end = lines[head + 1..]
        .iter()
        .position(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#') && leading_spaces(l) == 0
        })
        .map(|p| head + 1 + p)
        .unwrap_or(lines.len());
    let child = lines[head + 1..end]
        .iter()
        .find(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .map_or(2, |l| leading_spaces(l));

    let mut found: Vec<(usize, String)> = Vec::new();
    for (i, line) in lines.iter().enumerate().take(end).skip(head + 1) {
        if !has_key_at(line, child, "provider") {
            continue;
        }
        let Some(value) = scalar_value(line) else {
            return Err(format!(
                "`memory.provider` on line {} is not a single scalar; left untouched",
                i + 1
            ));
        };
        found.push((i, value));
    }
    if found.len() > 1 {
        return Err("more than one `memory.provider` key; left untouched".to_string());
    }
    Ok(Some(Block {
        head,
        end,
        child,
        provider: found.into_iter().next(),
    }))
}

/// Set, restore, or remove `memory.provider`.
fn set_provider(text: &str, op: &Provider) -> Result<String, String> {
    let Some(block) = find_block(text)? else {
        let Provider::Set(v) = op else {
            return Ok(text.to_string());
        };
        let mut out = text.to_string();
        if !out.is_empty() {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(&format!("memory:\n  provider: {v}\n"));
        return Ok(out);
    };

    let indent = " ".repeat(block.child);
    let all: Vec<String> = text.lines().map(str::to_string).collect();
    // Edits happen on the block's own slice. Doing them in place in `all` would
    // shift every index after a removal, and `end` was computed before the edit.
    let mut body: Vec<String> = all[block.head + 1..block.end].to_vec();
    let ours = block_value_is_ours(&block);
    let rel = block.provider.as_ref().map(|(i, _)| i - block.head - 1);
    match (rel, op) {
        (Some(i), Provider::Set(v)) if !v.is_empty() => {
            body[i] = preserve_comment(&body[i], &format!("{indent}provider: {v}"));
        }
        (Some(i), Provider::Remove) if ours => {
            body.remove(i);
        }
        // Not ours, or an empty set: change nothing.
        (Some(_), _) | (None, Provider::Remove) => return Ok(text.to_string()),
        (None, Provider::Set(v)) => {
            // After the block's last real child, not after its trailing blanks —
            // a key orphaned below a blank line reads as a different section to
            // anyone opening the file.
            let at = body
                .iter()
                .rposition(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                .map_or(0, |i| i + 1);
            body.insert(at, format!("{indent}provider: {v}"));
        }
    }

    // A `memory:` block we emptied is our own residue, not the user's config.
    let drop_head = matches!(op, Provider::Remove)
        && body
            .iter()
            .all(|l| l.trim().is_empty() || l.trim().starts_with('#'));
    let mut out: Vec<String> = all[..block.head].to_vec();
    if !drop_head {
        out.push(all[block.head].clone());
    }
    out.extend(body);
    out.extend_from_slice(&all[block.end..]);
    let mut joined = out.join("\n");
    if !joined.is_empty() {
        joined.push('\n');
    }
    Ok(joined)
}

/// Rewrite a `key: value` line, keeping any trailing `# comment`.
///
/// A comment on a line the user annotated is theirs; dropping it because we
/// changed the value beside it would be an edit they did not ask for.
fn preserve_comment(line: &str, replacement: &str) -> String {
    match line.find(" #") {
        Some(at) => format!("{} {}", replacement, &line[at..]),
        None => replacement.to_string(),
    }
}

/// Whether `memory.provider` currently holds our own marker.
fn block_value_is_ours(block: &Block) -> bool {
    block
        .provider
        .as_ref()
        .is_some_and(|(_, v)| v == MARKER)
}

/// The value of a `key: value` line, or `None` when it is not a plain scalar.
///
/// The allow-list is deliberately narrow — word characters plus `.`, `-`, `_`
/// and `/`, which covers every provider name. It exists to refuse the shapes
/// where replacing the value would be a rewrite rather than an edit: a quoted
/// scalar, a block scalar (`>` / `|`), an anchor or alias, a flow mapping, a
/// comment-only line. Each of those would otherwise come back as a "value" and
/// be replaced by a bare token, silently changing what the file means.
fn scalar_value(line: &str) -> Option<String> {
    let value = line.split_once(':')?.1.split('#').next().unwrap_or("").trim();
    if value.is_empty()
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/'))
    {
        return None;
    }
    Some(value.to_string())
}

/// A line that is exactly `key:` (or `key: # comment`) at `indent` spaces — a
/// block head, with no value of its own.
fn is_bare_key(line: &str, indent: usize, key: &str) -> bool {
    has_key_at(line, indent, key) && {
        let rest = &line[indent + key.len() + 1..];
        rest.is_empty() || rest.starts_with('#')
    }
}

/// A line that *starts* `key:` at `indent` spaces, whatever follows it — a
/// `key: value` line, whose value may be anything at all.
fn has_key_at(line: &str, indent: usize, key: &str) -> bool {
    leading_spaces(line) == indent && line.trim_start().starts_with(&format!("{key}:"))
}

/// Spaces before the first non-space character.
fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::Outcome;

    fn tmp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-hermes-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp home");
        dir
    }

    fn write_config(home: &Path, body: &str) {
        let p = home.join(Host::Hermes.config_rel());
        std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        std::fs::write(&p, body).expect("write config");
    }

    fn read_config(home: &Path) -> String {
        std::fs::read_to_string(home.join(Host::Hermes.config_rel())).expect("read config")
    }

    fn install(home: &Path) -> Outcome {
        let data = home.join("data");
        std::fs::create_dir_all(&data).expect("data dir");
        crate::connect::apply(Host::Hermes, "memory-wire", home, false, no_path, &data)
    }

    fn uninstall(home: &Path) -> Outcome {
        let data = home.join("data");
        std::fs::create_dir_all(&data).expect("data dir");
        crate::connect::apply(Host::Hermes, "memory-wire", home, true, no_path, &data)
    }

    /// A PATH lookup that finds nothing, so detection cannot depend on what
    /// happens to be installed on the machine running the tests.
    fn no_path(_name: &str) -> bool {
        false
    }

    /// The shape on this box: a `memory:` block, eight keys, a provider set to
    /// the competitor, and sections either side of it.
    const REAL: &str = "\
gateway:
  enabled: true

memory:
  memory_enabled: true
  user_profile_enabled: true
  write_approval: false
  memory_char_limit: 8000
  user_char_limit: 5000
  nudge_interval: 8
  provider: agentmemory
  flush_min_turns: 4

telemetry:
  enabled: false
";

    // -- the YAML editor -------------------------------------------------

    #[test]
    fn the_real_config_shape_is_understood_and_its_provider_read() {
        assert_eq!(provider_value(REAL).expect("parseable"), Some("agentmemory".to_string()));
    }

    #[test]
    fn setting_the_provider_leaves_every_other_line_byte_identical() {
        let out = set_provider(REAL, &Provider::Set("memory-wire".into())).expect("set");
        assert_eq!(
            out,
            REAL.replace("  provider: agentmemory", "  provider: memory-wire")
        );
    }

    #[test]
    fn a_block_with_no_provider_gets_one_at_the_blocks_own_indent() {
        let text = "memory:\n    memory_enabled: true\n";
        let out = set_provider(text, &Provider::Set("memory-wire".into())).expect("set");
        assert_eq!(out, "memory:\n    memory_enabled: true\n    provider: memory-wire\n");
    }

    #[test]
    fn a_config_with_no_memory_block_gains_one_at_the_end() {
        let out = set_provider("gateway:\n  enabled: true\n", &Provider::Set("memory-wire".into()))
            .expect("set");
        assert_eq!(out, "gateway:\n  enabled: true\n\nmemory:\n  provider: memory-wire\n");
    }

    #[test]
    fn an_empty_config_becomes_exactly_the_block() {
        let out = set_provider("", &Provider::Set("memory-wire".into())).expect("set");
        assert_eq!(out, "memory:\n  provider: memory-wire\n");
    }

    #[test]
    fn a_config_with_no_trailing_newline_is_not_joined_to_our_append() {
        let out = set_provider("gateway:\n  enabled: true", &Provider::Set("memory-wire".into()))
            .expect("set");
        assert_eq!(out, "gateway:\n  enabled: true\n\nmemory:\n  provider: memory-wire\n");
    }

    #[test]
    fn a_trailing_comment_on_the_provider_line_survives_the_write() {
        let text = "memory:\n  provider: agentmemory  # chosen 2026-01\n";
        let out = set_provider(text, &Provider::Set("memory-wire".into())).expect("set");
        assert_eq!(out, "memory:\n  provider: memory-wire  # chosen 2026-01\n");
    }

    #[test]
    fn removing_the_only_key_removes_the_block_we_would_have_left_behind() {
        let out = set_provider("memory:\n  provider: memory-wire\n", &Provider::Remove).expect("rm");
        assert_eq!(out, "", "our own residue, not the user's config");
    }

    #[test]
    fn removing_the_key_keeps_a_block_that_holds_anything_else() {
        // The provider here has to be ours, or the ownership guard refuses the
        // whole operation — which is the behaviour the next test pins.
        let installed = REAL.replace("provider: agentmemory", "provider: memory-wire");
        let out = set_provider(&installed, &Provider::Remove).expect("rm");
        assert!(!out.contains("provider:"), "{out}");
        assert!(out.contains("  memory_enabled: true"), "{out}");
        assert!(out.contains("  flush_min_turns: 4"), "{out}");
        assert!(out.contains("telemetry:"), "{out}");
    }

    #[test]
    fn removing_a_provider_that_is_not_ours_is_a_no_op() {
        // The ownership rule lives in `set_provider`, not in the caller, so it
        // holds no matter which path asked for the removal.
        let text = "memory:\n  provider: agentmemory\n";
        assert_eq!(set_provider(text, &Provider::Remove).expect("rm"), text);
    }

    // The four shapes `find_block` must refuse rather than guess at. Each one is
    // a case where a wrong guess rewrites a line the user wrote.
    #[test]
    fn a_multi_document_config_is_refused() {
        for text in ["---\nmemory:\n  provider: x\n", "a: 1\n---\nb: 2\n"] {
            let err = provider_value(text).expect_err("must refuse");
            assert!(err.contains("multi-document"), "{err}");
        }
    }

    #[test]
    fn a_non_block_memory_key_is_refused() {
        for text in ["memory: {provider: agentmemory}\n", "memory: 7\n"] {
            let err = provider_value(text).expect_err("must refuse");
            assert!(err.contains("not a block mapping"), "{err}");
        }
    }

    #[test]
    fn a_duplicate_top_level_memory_key_is_refused() {
        let text = "memory:\n  provider: a\nmemory:\n  provider: b\n";
        let err = provider_value(text).expect_err("must refuse");
        assert!(err.contains("more than one top-level"), "{err}");
    }

    #[test]
    fn a_top_level_provider_key_is_refused_rather_than_read_as_ours() {
        let text = "provider: agentmemory\nmemory:\n  provider: agentmemory\n";
        let err = provider_value(text).expect_err("must refuse");
        assert!(err.contains("top-level `provider:`"), "{err}");
    }

    #[test]
    fn a_quoted_or_block_scalar_provider_is_refused_not_retyped() {
        for text in [
            "memory:\n  provider: \"agentmemory\"\n",
            "memory:\n  provider: >\n      agentmemory\n",
        ] {
            let err = provider_value(text).expect_err("must refuse");
            assert!(err.contains("not a single scalar"), "{err}");
        }
    }

    #[test]
    fn a_duplicate_provider_key_inside_the_block_is_refused() {
        let text = "memory:\n  provider: a\n  provider: b\n";
        let err = provider_value(text).expect_err("must refuse");
        assert!(err.contains("more than one `memory.provider`"), "{err}");
    }

    #[test]
    fn a_commented_out_provider_is_not_a_provider() {
        assert_eq!(provider_value("memory:\n  # provider: agentmemory\n").expect("parse"), None);
    }

    // -- the install ------------------------------------------------------

    #[test]
    fn the_template_directory_is_the_one_we_install() {
        for (name, body) in FILES {
            let on_disk = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/integrations/hermes/memory-wire")
                .join(name);
            assert_eq!(
                std::fs::read_to_string(&on_disk).unwrap_or_default(),
                *body,
                "{name} is embedded but the template on disk differs, or vice versa"
            );
        }
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugin/integrations/hermes/memory-wire");
        let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
            .expect("template dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.') && n != "__pycache__")
            .collect();
        let mut expected: Vec<String> = FILES.iter().map(|(n, _)| (*n).to_string()).collect();
        on_disk.sort();
        expected.sort();
        assert_eq!(on_disk, expected, "the template dir and FILES disagree");
    }

    #[test]
    fn the_installed_plugin_is_a_hermes_memory_provider_directory() {
        // The three things `plugins/memory/__init__.py` actually requires, as
        // opposed to what a manifest might suggest: the directory is named after
        // the provider, `__init__.py` exists, and its first 8 KiB mention one of
        // the two marker strings the loader greps for.
        let home = tmp_home("shape");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        assert!(matches!(install(&home), Outcome::Wired { .. }));
        let dir = home.join(plugin_rel());
        let init = std::fs::read_to_string(dir.join("__init__.py")).expect("__init__.py");
        let head: String = init.chars().take(8192).collect();
        assert!(
            head.contains("register_memory_provider") || head.contains("MemoryProvider"),
            "the loader's 8 KiB heuristic would skip this plugin"
        );
        assert_eq!(
            dir.file_name().and_then(|n| n.to_str()),
            Some(MARKER),
            "the directory name is the provider name"
        );
        assert!(dir.join("plugin.yaml").is_file());
    }

    #[test]
    fn the_manifest_claims_exactly_the_five_events_that_are_implemented() {
        let yaml = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/integrations/hermes/memory-wire/plugin.yaml"),
        )
        .expect("plugin.yaml");
        let init = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/integrations/hermes/memory-wire/__init__.py"),
        )
        .expect("__init__.py");
        let hooks: Vec<&str> = yaml
            .lines()
            .skip_while(|l| *l != "hooks:")
            .skip(1)
            .take_while(|l| l.starts_with("  - "))
            .map(|l| l.trim_start_matches("  - ").trim())
            .collect();
        assert_eq!(
            hooks,
            vec![
                "prefetch",
                "sync_turn",
                "on_session_end",
                "on_pre_compress",
                "system_prompt_block"
            ],
            "the manifest claims exactly the implemented events"
        );
        for event in &hooks {
            assert!(
                init.contains(&format!("def {event}(")),
                "{event} is claimed but not implemented"
            );
        }
        // The sixth: absent from the claimed list, and saying so out loud in the
        // code rather than inheriting a silent no-op.
        assert!(!hooks.contains(&"on_memory_write"), "not implemented, so not claimed");
        assert!(init.contains("def on_memory_write("), "still explicit about it");
    }

    #[test]
    fn the_tool_names_match_the_stdio_mcp_server_exactly() {
        let init = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/integrations/hermes/memory-wire/__init__.py"),
        )
        .expect("__init__.py");
        for name in [
            crate::mcp::RETAIN,
            crate::mcp::RECALL,
            crate::mcp::REFLECT,
            crate::mcp::CONFIG_GET,
        ] {
            assert!(init.contains(&format!("\"{name}\"")), "{name} is not the same tool");
        }
    }

    #[test]
    fn install_then_uninstall_leaves_the_home_as_it_found_it() {
        let home = tmp_home("roundtrip");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        let before = read_config(&home);

        assert!(matches!(install(&home), Outcome::Wired { .. }));
        assert!(read_config(&home).contains("  provider: memory-wire"));
        assert!(home.join(plugin_rel()).join("__init__.py").is_file());

        assert!(matches!(uninstall(&home), Outcome::Unwired { .. }));
        assert_eq!(read_config(&home), before, "the previous provider is put back");
        assert_eq!(
            std::fs::read_dir(home.join(".hermes"))
                .expect("hermes home")
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["config.yaml".to_string()],
            "nothing of ours is left: the plugins/ dir we created went too"
        );
    }

    #[test]
    fn a_plugins_directory_holding_someone_elses_plugin_survives() {
        let home = tmp_home("sibling");
        let other = home.join(".hermes/plugins/agentmemory");
        std::fs::create_dir_all(&other).expect("sibling plugin");
        std::fs::write(other.join("plugin.yaml"), "name: agentmemory\n").expect("write");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        assert!(matches!(install(&home), Outcome::Wired { .. }));
        assert!(matches!(uninstall(&home), Outcome::Unwired { .. }));
        assert!(other.is_dir(), "a dir we did not create is not removed");
        assert!(other.join("plugin.yaml").is_file());
    }

    #[test]
    fn an_install_over_agentmemory_records_it_rather_than_forgetting_it() {
        let home = tmp_home("restore");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        let Outcome::Wired { notes, .. } = install(&home) else {
            panic!("expected a wired outcome");
        };
        assert!(
            notes.iter().any(|n| n.contains("agentmemory") && n.contains("restores")),
            "the displaced provider is reported: {notes:?}"
        );
        let state = std::fs::read_to_string(home.join(plugin_rel()).join(STATE_FILE)).expect("state");
        assert!(state.contains("agentmemory"), "{state}");
    }

    #[test]
    fn uninstall_leaves_a_provider_it_did_not_write_alone() {
        let home = tmp_home("notours");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        assert!(matches!(install(&home), Outcome::Wired { .. }));
        // The user switches to another provider by hand while ours is installed.
        write_config(&home, &REAL.replace("provider: agentmemory", "provider: hindsight"));
        let Outcome::Unwired { .. } = uninstall(&home) else {
            panic!("expected an unwired outcome");
        };
        assert!(
            read_config(&home).contains("  provider: hindsight"),
            "a value this install did not write is not clobbered"
        );
    }

    #[test]
    fn a_reinstall_keeps_the_first_displaced_value_not_the_intermediate_one() {
        let home = tmp_home("reinstall");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        assert!(matches!(install(&home), Outcome::Wired { .. }));
        // The user uninstalls, picks a different provider, and reinstalls.
        assert!(matches!(uninstall(&home), Outcome::Unwired { .. }));
        write_config(&home, &REAL.replace("provider: agentmemory", "provider: hindsight"));
        assert!(matches!(install(&home), Outcome::Wired { .. }));
        let state = std::fs::read_to_string(home.join(plugin_rel()).join(STATE_FILE)).expect("state");
        assert!(state.contains("hindsight"), "{state}");
        // And uninstalling restores that, not the value before it.
        assert!(matches!(uninstall(&home), Outcome::Unwired { .. }));
        assert!(read_config(&home).contains("  provider: hindsight"));
    }

    #[test]
    fn a_second_install_reports_already_and_rewrites_nothing() {
        let home = tmp_home("idempotent");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        assert!(matches!(install(&home), Outcome::Wired { .. }));
        let after_first = read_config(&home);
        let stamp = std::fs::metadata(home.join(plugin_rel()).join("__init__.py"))
            .expect("stat")
            .modified()
            .expect("mtime");
        assert!(
            matches!(install(&home), Outcome::Already { .. }),
            "a second install is a no-op"
        );
        assert_eq!(read_config(&home), after_first);
        assert_eq!(
            std::fs::metadata(home.join(plugin_rel()).join("__init__.py"))
                .expect("stat")
                .modified()
                .expect("mtime"),
            stamp,
            "an unchanged file is not rewritten"
        );
    }

    #[test]
    fn a_stale_plugin_tree_is_replaced_not_appended_to() {
        let home = tmp_home("stale");
        let dir = home.join(plugin_rel());
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("__init__.py"), "# an older build\n").expect("write");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        assert!(matches!(install(&home), Outcome::Wired { .. }));
        let init = std::fs::read_to_string(dir.join("__init__.py")).expect("read");
        assert!(init.contains("class MemoryWireProvider"), "the stale file was replaced");
    }

    #[test]
    fn a_file_the_install_did_not_write_is_reported_not_deleted() {
        let home = tmp_home("foreign");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        assert!(matches!(install(&home), Outcome::Wired { .. }));
        let dir = home.join(plugin_rel());
        std::fs::write(dir.join("NOTES.md"), "mine\n").expect("write");
        let Outcome::Unwired { notes, .. } = uninstall(&home) else {
            panic!("expected an unwired outcome");
        };
        assert!(dir.join("NOTES.md").is_file(), "a user's file survives");
        assert!(notes.iter().any(|n| n.contains("did not write")), "{notes:?}");
    }

    #[test]
    fn a_config_we_cannot_parse_is_refused_untouched() {
        let home = tmp_home("refuse");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        let hostile = "provider: agentmemory\nmemory:\n  provider: agentmemory\n";
        write_config(&home, hostile);
        let Outcome::Failed(why) = install(&home) else {
            panic!("expected a refusal");
        };
        assert!(why.contains("left untouched"), "{why}");
        assert_eq!(read_config(&home), hostile, "byte-identical");
        assert!(
            !home.join(plugin_rel()).exists(),
            "nothing is installed for a config we could not read"
        );
    }

    #[test]
    fn a_machine_without_hermes_is_skipped_not_created() {
        let home = tmp_home("absent");
        assert!(matches!(install(&home), Outcome::Skipped(_)));
        assert!(!home.join(".hermes").exists(), "no hermes home is conjured up");
    }

    #[test]
    fn uninstalling_an_absent_plugin_says_so_rather_than_failing() {
        let home = tmp_home("gone");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        assert!(matches!(uninstall(&home), Outcome::Skipped(_)));
        assert_eq!(read_config(&home), REAL, "untouched");
    }

    #[test]
    fn a_pre_existing_config_and_tree_are_both_backed_up_before_the_first_write() {
        let home = tmp_home("backup");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        write_config(&home, REAL);
        let dir = home.join(plugin_rel());
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("__init__.py"), "# an older build\n").expect("write");
        let data = home.join("data");

        let Outcome::Wired { backup, .. } =
            crate::connect::apply(Host::Hermes, "memory-wire", &home, false, no_path, &data)
        else {
            panic!("expected a wired outcome");
        };
        let b = backup.expect("a backup, because both targets already existed");
        assert_eq!(
            std::fs::read_to_string(b.join(".hermes/config.yaml")).expect("config backup"),
            REAL
        );
        assert_eq!(
            std::fs::read_to_string(b.join(".hermes/plugins/memory-wire/__init__.py"))
                .expect("tree backup"),
            "# an older build\n"
        );
    }

    #[test]
    fn a_first_install_on_an_empty_home_backs_up_nothing() {
        let home = tmp_home("firstrun");
        std::fs::create_dir_all(home.join(Host::Hermes.detect_rel())).expect("hermes home");
        let Outcome::Wired { backup, .. } = install(&home) else {
            panic!("expected a wired outcome");
        };
        assert!(backup.is_none(), "there was nothing to preserve");
        assert!(home.join(".hermes/config.yaml").is_file());
    }

    #[test]
    fn hermes_is_one_of_the_hosts_a_bare_connect_wires() {
        assert!(
            crate::connect::ALL.contains(&Host::Hermes),
            "a bare `memory-wire connect` should reach it like every other host"
        );
    }

    #[test]
    fn the_report_line_names_the_host_the_way_every_other_one_does() {
        let line = Outcome::Wired {
            backup: None,
            events: vec!["config.yaml: memory.provider".into()],
            notes: vec![],
        }
        .render(Host::Hermes);
        assert!(line.starts_with("hermes "), "{line}");
        assert!(line.contains("wired"), "{line}");
    }
}
