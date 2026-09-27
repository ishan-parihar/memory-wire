//! Agent wiring: install and remove memory-wire hook entries and MCP server
//! entries in host configs.
//!
//! Three properties decide every decision in this file:
//!
//! - **Never corrupt.** A config the user (and other tools) own is parsed
//!   before it is touched. A parse failure aborts that host, leaves the file
//!   byte-identical, and reports why. Writes go through
//!   [`paths::write_atomic`] (temp file + rename + re-read verify).
//! - **Idempotent.** Our entries are recognised by the `memory-wire` substring
//!   in their command, so a second install reports `already-wired` and a
//!   re-install from a moved binary replaces the stale path instead of stacking
//!   a second copy.
//! - **Reversible.** The pre-edit original is copied to
//!   `~/.local/share/memory-wire/backups/<host>-<ts>/` before the first write,
//!   and `--uninstall` removes only entries carrying the marker.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::paths;

/// Substring that identifies a config entry as ours.
pub const MARKER: &str = "memory-wire";

/// Seconds a host should allow the hook; the client also caps itself.
const HOOK_TIMEOUT: u64 = 5;

/// Where an MCP-only host keeps its server map, and the entry shape it wants.
type McpSpec = (&'static str, fn(&str) -> Value);

/// An agent host this installer can wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Host {
    /// Claude Code (`~/.claude/settings.json`).
    ClaudeCode,
    /// Codex (`~/.codex/hooks.json`).
    Codex,
    /// GitHub Copilot CLI (`~/.copilot/settings.json`).
    CopilotCli,
    /// Cursor (`~/.cursor/mcp.json`).
    Cursor,
    /// opencode (`~/.config/opencode/opencode.json`).
    Opencode,
}

/// Every host, in report order.
pub const ALL: &[Host] = &[
    Host::ClaudeCode,
    Host::Codex,
    Host::CopilotCli,
    Host::Cursor,
    Host::Opencode,
];

/// A host lifecycle event and the `hook` subcommand it runs.
struct Event {
    /// Key under the config's `hooks` object.
    name: &'static str,
    /// `memory-wire hook <lifecycle>` argument.
    lifecycle: &'static str,
}

const EVENTS: [Event; 3] = [
    Event { name: "SessionStart", lifecycle: "session-start" },
    Event { name: "UserPromptSubmit", lifecycle: "prompt" },
    Event { name: "Stop", lifecycle: "stop" },
];

/// The JSON shape a host expects for one hook entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    /// Claude Code / Codex: a matcher group wrapping `hooks`.
    Claude,
    /// Copilot CLI: a flat command entry with a `bash` field.
    Copilot,
    /// The host's only integration surface is an MCP server list — it fires no
    /// lifecycle command, so `memory-wire mcp` is what gets registered.
    McpOnly,
}

impl Host {
    /// Stable id, also the backup directory prefix.
    pub fn id(self) -> &'static str {
        match self {
            Host::ClaudeCode => "claude-code",
            Host::Codex => "codex",
            Host::CopilotCli => "copilot-cli",
            Host::Cursor => "cursor",
            Host::Opencode => "opencode",
        }
    }

    /// Config file this host reads, relative to the home directory.
    pub fn config_rel(self) -> PathBuf {
        let s = match self {
            Host::ClaudeCode => ".claude/settings.json",
            Host::Codex => ".codex/hooks.json",
            Host::CopilotCli => ".copilot/settings.json",
            Host::Cursor => ".cursor/mcp.json",
            Host::Opencode => ".config/opencode/opencode.json",
        };
        PathBuf::from(s)
    }

    /// Directory that proves the host is installed when no config exists yet.
    pub fn detect_rel(self) -> &'static str {
        match self {
            Host::ClaudeCode => ".claude",
            Host::Codex => ".codex",
            Host::CopilotCli => ".copilot",
            Host::Cursor => ".cursor",
            Host::Opencode => ".config/opencode",
        }
    }

    /// Executable that proves the host is installed.
    pub fn binary(self) -> &'static str {
        match self {
            Host::ClaudeCode => "claude",
            Host::Codex => "codex",
            Host::CopilotCli => "copilot",
            Host::Cursor => "cursor-agent",
            Host::Opencode => "opencode",
        }
    }

    /// Hook entry shape.
    fn style(self) -> Style {
        match self {
            Host::ClaudeCode | Host::Codex => Style::Claude,
            Host::CopilotCli => Style::Copilot,
            Host::Cursor | Host::Opencode => Style::McpOnly,
        }
    }

    /// Where this host keeps its MCP server map, and the entry shape it wants.
    ///
    /// `None` for hosts that expose no MCP registration. The two shapes mirror
    /// what each host's own config already uses: cursor's `mcpServers` takes
    /// `command`/`args`, opencode's `mcp` takes `type: local` plus a `command`
    /// array — an entry written in the other host's shape is silently ignored.
    fn mcp_spec(self) -> Option<McpSpec> {
        match self {
            Host::Cursor => Some((
                "mcpServers",
                |exe: &str| json!({ "command": exe, "args": ["mcp"] }),
            )),
            Host::Opencode => Some((
                "mcp",
                |exe: &str| json!({ "type": "local", "command": [exe, "mcp"], "enabled": true }),
            )),
            _ => None,
        }
    }
}

impl Style {
    /// The entry to append for one lifecycle, or `None` for MCP-only hosts.
    fn entry(self, exe: &str, lifecycle: &str) -> Option<Value> {
        match self {
            // Quoted like the Copilot `bash` field: an install path can contain
            // a space, and both are shell command strings.
            Style::Claude => Some(json!({
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} hook {lifecycle}", shell_quote(exe)),
                    "timeout": HOOK_TIMEOUT,
                }],
            })),
            Style::Copilot => Some(json!({
                "type": "command",
                "bash": format!("{} hook {lifecycle}", shell_quote(exe)),
                "timeoutSec": HOOK_TIMEOUT,
            })),
            Style::McpOnly => None,
        }
    }
}

/// Why a host takes no wiring at all.
///
/// No host among [`ALL`] lands here any more — cursor and opencode both accept
/// an MCP server entry, and the other three take hooks. The case is kept so a
/// future MCP-only host with no known writable key reports why instead of
/// silently doing nothing.
const MCP_ONLY_REASON: &str =
    "mcp-config-only host: no writable MCP key is known for it";

/// Quote an executable path for a shell-run field.
///
/// A `bash` field is interpreted by a shell, so an install path containing a
/// space has to be quoted. The arguments are appended separately by the
/// caller, never split back out of a command string — an install path may
/// itself contain a space, and splitting would corrupt it.
fn shell_quote(exe: &str) -> String {
    if exe.contains(' ') || exe.contains('\'') {
        format!("'{exe}'")
    } else {
        exe.to_string()
    }
}

/// True when a config entry is one of ours.
fn is_ours(v: &Value) -> bool {
    serde_json::to_string(v).is_ok_and(|s| s.contains(MARKER))
}

/// Does `dir` exist under `home`?
fn exists(home: &Path, rel: &str) -> bool {
    home.join(rel).exists()
}

/// Is `name` on `PATH`?
pub fn binary_on_path(name: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| {
        let candidate = dir.join(name);
        candidate.is_file() && is_executable(&candidate)
    })
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// Is this host installed? Config dir first, then its binary on `PATH`.
fn detected_with(host: Host, home: &Path, on_path: fn(&str) -> bool) -> bool {
    exists(home, host.detect_rel()) || on_path(host.binary())
}

/// What one edit pass did to a config.
struct Edit {
    /// Slots that changed, in report order.
    events: Vec<String>,
    /// Slots deliberately left alone.
    notes: Vec<String>,
    /// How many slots were already in the requested state.
    already: usize,
}

/// A refusal that leaves the file untouched.
type Refusal = String;

/// Result of touching one host.
#[derive(Debug)]
pub enum Outcome {
    /// Entries written.
    Wired {
        /// Backup directory, when the file existed beforehand.
        backup: Option<PathBuf>,
        /// Events that changed (`SessionStart`, `UserPromptSubmit (replaced stale)`).
        events: Vec<String>,
        /// Events deliberately left alone.
        notes: Vec<String>,
    },
    /// Our entries were removed.
    Unwired {
        /// Backup directory, when the file existed beforehand.
        backup: Option<PathBuf>,
        /// Events that changed.
        events: Vec<String>,
        /// Events deliberately left alone.
        notes: Vec<String>,
    },
    /// Already in the requested state; file untouched.
    Already {
        /// Events deliberately left alone.
        notes: Vec<String>,
    },
    /// Nothing to do, with a reason.
    Skipped(String),
    /// Refused; file untouched.
    Failed(String),
}

impl Outcome {
    /// One aligned report line.
    pub fn render(self, host: Host) -> String {
        let joined = |v: &[String]| if v.is_empty() { String::new() } else { v.join(", ") };
        let (verb, mut detail) = match self {
            Outcome::Wired { backup, events, notes } => {
                let mut d = format!("{}{}", joined(&events), backup_suffix(backup.as_deref()));
                if !notes.is_empty() {
                    d.push_str(&format!("  | left alone: {}", joined(&notes)));
                }
                ("wired", d)
            }
            Outcome::Unwired { backup, events, notes } => {
                let mut d = format!("{}{}", joined(&events), backup_suffix(backup.as_deref()));
                if !notes.is_empty() {
                    d.push_str(&format!("  | left alone: {}", joined(&notes)));
                }
                ("unwired", d)
            }
            Outcome::Already { notes } => ("already-wired", joined(&notes)),
            Outcome::Skipped(why) => ("skipped", why),
            Outcome::Failed(why) => ("FAILED", why),
        };
        if detail.is_empty() {
            detail.push('-');
        }
        format!("{:<12} {:<13} {}", host.id(), verb, detail)
    }

    /// True for a refusal: the caller should exit nonzero.
    pub fn is_failure(&self) -> bool {
        matches!(self, Outcome::Failed(_))
    }
}

fn backup_suffix(backup: Option<&Path>) -> String {
    match backup {
        Some(d) => format!("  (backup: {})", d.display()),
        None => String::new(),
    }
}

/// Wall-clock stamp used in backup directory names.
pub fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Copy `home/rel` to `<data_root>/backups/<host>-<ts>/rel`.
///
/// Returns the backup directory, or `None` when there was no file to preserve
/// (a first install that creates the config).
pub fn backup(
    data_root: &Path,
    home: &Path,
    host: Host,
    ts: &str,
    rel: &Path,
) -> Result<Option<PathBuf>, String> {
    let src = home.join(rel);
    if !src.is_file() {
        return Ok(None);
    }
    let dir = data_root
        .join("backups")
        .join(format!("{}-{ts}", host.id()));
    let dst = dir.join(rel);
    let parent = dst
        .parent()
        .ok_or_else(|| format!("{} has no parent", dst.display()))?;
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    Ok(Some(dir))
}

/// Wire or unwire one host against the real environment.
pub fn run(host: Host, exe: &str, home: &Path, uninstall: bool) -> Outcome {
    apply(host, exe, home, uninstall, binary_on_path, &paths::data_dir(home))
}

// Six parameters, kept flat: `on_path` and `data_root` are the two injected
// probes (PATH lookup + data root) that let the wire/unwire path be tested for
// both a detected and an undetected machine without touching the real PATH or
// XDG dir. Bundling them into a context struct would not reduce the fields, only
// add a type every call site has to unpack again.
//
// No `clippy::too_many_arguments` suppression: 6 params is under clippy's default
// threshold of 7, so the lint does not fire. It carried a dead `#[allow]`; an
// `#[expect]` cannot replace one, because an unfulfilled expectation is itself a
// warning under `-D warnings`. Raise `too-many-arguments-threshold` before
// re-adding either, or the parameter list will need to actually grow past 7.
fn apply(
    host: Host,
    exe: &str,
    home: &Path,
    uninstall: bool,
    on_path: fn(&str) -> bool,
    data_root: &Path,
) -> Outcome {
    if !detected_with(host, home, on_path) {
        return Outcome::Skipped("not detected on this machine".to_string());
    }
    let style = host.style();

    let rel = host.config_rel();
    let path = home.join(&rel);
    let raw = match paths::read_or_empty(&path) {
        Ok(s) => s,
        Err(e) => return Outcome::Failed(format!("{}: {e}", path.display())),
    };
    let mut root: Value = if raw.trim().is_empty() {
        json!({})
    } else {
        match serde_json::from_str::<Value>(&raw) {
            Ok(v) if v.is_object() => v,
            Ok(_) => {
                return Outcome::Failed(format!(
                    "{}: not a JSON object; left untouched",
                    path.display()
                ))
            }
            Err(e) => {
                return Outcome::Failed(format!(
                    "{}: malformed JSON ({e}); left untouched",
                    path.display()
                ))
            }
        }
    };

    // One edit pass, whichever surface this host exposes: hook entries for the
    // three hosts that fire commands, an MCP server entry for the two that
    // only know how to launch a server.
    let edit = if style == Style::McpOnly {
        let Some((key, entry)) = host.mcp_spec() else {
            return Outcome::Skipped(MCP_ONLY_REASON.to_string());
        };
        edit_mcp(&mut root, key, (!uninstall).then(|| entry(exe)))
    } else {
        edit_hooks(&mut root, style, exe, uninstall)
    };
    let Edit { events, notes, already } = match edit {
        Ok(e) => e,
        Err(why) => return Outcome::Failed(format!("{}: {why}", path.display())),
    };

    // A note is not a change: re-serialising an untouched config would churn
    // the user's file and take a pointless backup on every run.
    if events.is_empty() {
        if already == 0 {
            return Outcome::Skipped(if notes.is_empty() {
                "no writable hook slot in this config".to_string()
            } else {
                notes.join(", ")
            });
        }
        return Outcome::Already { notes };
    }

    let ts = timestamp();
    let backup = match backup(data_root, home, host, &ts, &rel) {
        Ok(b) => b,
        Err(e) => return Outcome::Failed(e),
    };
    // An uninstall that leaves nothing behind removes the file rather than
    // writing `{}` into it: on a first install we created this file, so an
    // empty object is our own residue, not the user's config. Any foreign
    // content keeps the file alive, untouched by this branch.
    if uninstall && root.as_object().is_some_and(|o| o.is_empty()) {
        return match std::fs::remove_file(&path) {
            Ok(()) => Outcome::Unwired { backup, events, notes },
            Err(e) => Outcome::Failed(format!("{}: {e}", path.display())),
        };
    }
    let mut text = serde_json::to_string_pretty(&root).unwrap_or_default();
    text.push('\n');
    if let Err(e) = paths::write_atomic(&path, &text) {
        return Outcome::Failed(e);
    }
    if uninstall {
        Outcome::Unwired { backup, events, notes }
    } else {
        Outcome::Wired { backup, events, notes }
    }
}

/// Add or remove our three hook entries under `hooks`.
fn edit_hooks(root: &mut Value, style: Style, exe: &str, uninstall: bool) -> Result<Edit, Refusal> {
    match root.get("hooks") {
        Some(v) if v.is_object() => {}
        Some(_) => return Err("`hooks` is not an object; left untouched".to_string()),
        None => {
            if let Some(obj) = root.as_object_mut() {
                obj.insert("hooks".to_string(), json!({}));
            }
        }
    }

    let mut events = Vec::new();
    let mut notes = Vec::new();
    let mut already = 0usize;
    {
        let hooks = root
            .get_mut("hooks")
            .and_then(Value::as_object_mut)
            .expect("hooks inserted or validated above");
        for ev in EVENTS.iter() {
            let desired = if uninstall {
                None
            } else {
                style.entry(exe, ev.lifecycle)
            };
            match hooks.get_mut(ev.name) {
                None => {
                    if let Some(d) = desired {
                        hooks.insert(ev.name.to_string(), Value::Array(vec![d]));
                        events.push(ev.name.to_string());
                    }
                }
                Some(Value::Array(arr)) => {
                    let mut next: Vec<Value> =
                        arr.iter().filter(|v| !is_ours(v)).cloned().collect();
                    if let Some(d) = &desired {
                        next.push(d.clone());
                    }
                    if next.is_empty() {
                        hooks.remove(ev.name);
                        events.push(format!("{} (pruned)", ev.name));
                    } else if *next == *arr {
                        already += 1;
                    } else if desired.is_some() {
                        let replaced = arr.iter().any(is_ours);
                        hooks.insert(ev.name.to_string(), Value::Array(next));
                        events.push(if replaced {
                            format!("{} (replaced stale)", ev.name)
                        } else {
                            ev.name.to_string()
                        });
                    } else {
                        hooks.insert(ev.name.to_string(), Value::Array(next));
                        events.push(ev.name.to_string());
                    }
                }
                Some(_) => notes.push(format!(
                    "{} (existing value is not a hook list)",
                    ev.name
                )),
            }
        }
        // A host we emptied out entirely should not keep an empty `hooks` key.
        if uninstall && hooks.is_empty() {
            if let Some(obj) = root.as_object_mut() {
                obj.remove("hooks");
            }
        }
    }
    Ok(Edit { events, notes, already })
}

/// Add or remove our MCP server entry under the host's `key`.
///
/// MCP maps key servers by name, so ours is recognised by its key as well as by
/// the [`MARKER`] substring — an entry registered under another name still gets
/// pruned, and a re-install from a moved binary replaces the stale command
/// instead of stacking a second server. The pass is judged by comparing the slot
/// before and after, which is what makes it idempotent for free.
fn edit_mcp(root: &mut Value, key: &str, entry: Option<Value>) -> Result<Edit, Refusal> {
    let before = root.get(key).cloned();
    {
        let obj = root
            .as_object_mut()
            .expect("root is a JSON object; the caller refuses anything else");
        match obj.get_mut(key) {
            Some(v) if v.is_object() => {
                let map = v.as_object_mut().expect("object checked above");
                map.retain(|name, v| name != MARKER && !is_ours(v));
                if let Some(e) = entry {
                    map.insert(MARKER.to_string(), e);
                }
                if map.is_empty() {
                    obj.remove(key);
                }
            }
            Some(v) => {
                return Err(format!(
                    "`{key}` is a {}, not an object; left untouched",
                    kind(v)
                ))
            }
            None => {
                if let Some(e) = entry {
                    obj.insert(key.to_string(), json!({ MARKER: e }));
                }
            }
        }
    }
    let after = root.get(key).cloned();
    Ok(if before == after {
        Edit { events: Vec::new(), notes: Vec::new(), already: 1 }
    } else {
        Edit { events: vec![key.to_string()], notes: Vec::new(), already: 0 }
    })
}

/// JSON type name, for a refusal message that says what is in the way.
fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PATH lookup that finds nothing, so detection cannot depend on what
    /// happens to be installed on the machine running the tests.
    fn no_path(_name: &str) -> bool {
        false
    }

    fn tmp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-connect-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp home");
        dir
    }

    fn write_config(home: &Path, rel: &Path, body: &str) {
        let p = home.join(rel);
        std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        std::fs::write(&p, body).expect("write");
    }

    fn read_config(home: &Path, rel: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(home.join(rel)).expect("read")).expect("json")
    }

    fn count_ours(home: &Path, rel: &Path) -> usize {
        std::fs::read_to_string(home.join(rel))
            .expect("read")
            .matches(MARKER)
            .count()
    }

    /// How many of our servers live in an MCP map. Keyed on the name rather than
    /// on the marker substring, which also appears in the installed path.
    fn our_entries(doc: &Value, key: &str) -> usize {
        doc.get(key)
            .and_then(Value::as_object)
            .map(|m| m.keys().filter(|k| k.as_str() == MARKER).count())
            .unwrap_or(0)
    }

    /// A pre-existing foreign hook that must survive every operation.
    const FOREIGN: &str = r#"{"hooks":{"SessionStart":[{"matcher":"","hooks":[{"type":"command","command":"/opt/other/hook.sh","timeout":9}]}]}}"#;

    #[test]
    fn install_should_be_idempotent_and_preserve_foreign_hooks() {
        let home = tmp_home("idem");
        let rel = Host::ClaudeCode.config_rel();
        write_config(&home, &rel, FOREIGN);

        let first = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(first, Outcome::Wired { .. }), "{:?}", first);
        assert_eq!(count_ours(&home, &rel), 3, "exactly one entry per event");
        let doc = read_config(&home, &rel);
        let entries = doc["hooks"]["SessionStart"].as_array().expect("array");
        assert_eq!(entries.len(), 2, "foreign hook kept, ours added");
        assert!(entries.iter().any(|e| e.to_string().contains("/opt/other/hook.sh")));

        let second = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(second, Outcome::Already { .. }), "{second:?}");
        assert_eq!(count_ours(&home, &rel), 3, "re-install must not stack entries");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn install_should_replace_a_stale_entry_when_the_binary_moves() {
        let home = tmp_home("stale");
        let rel = Host::ClaudeCode.config_rel();
        write_config(
            &home,
            &rel,
            r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"/old/dir/memory-wire hook stop","timeout":5}]}]}}"#,
        );
        let out = apply(Host::ClaudeCode, "/new/dir/memory-wire", &home, false, no_path, &home.join("data"));
        let Outcome::Wired { events, .. } = &out else {
            panic!("{out:?}");
        };
        assert!(events.iter().any(|e| e.contains("replaced stale")), "{events:?}");
        assert_eq!(count_ours(&home, &rel), 3);
        assert!(std::fs::read_to_string(home.join(&rel))
            .expect("read")
            .contains("/new/dir/memory-wire"));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn malformed_config_should_be_refused_untouched() {
        let home = tmp_home("bad");
        let rel = Host::ClaudeCode.config_rel();
        let broken = "{ \"hooks\": { oops";
        write_config(&home, &rel, broken);
        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
        assert!(out.is_failure());
        assert_eq!(
            std::fs::read_to_string(home.join(&rel)).expect("read"),
            broken,
            "refusal must leave the file byte-identical"
        );
        let backups = home.join("data/backups");
        assert!(
            !backups.exists(),
            "a refused host must not create a backup at {}",
            backups.display()
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn uninstall_should_remove_only_our_entries() {
        let home = tmp_home("unwire");
        let rel = Host::ClaudeCode.config_rel();
        write_config(&home, &rel, FOREIGN);
        apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"));

        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, true, no_path, &home.join("data"));
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        assert_eq!(count_ours(&home, &rel), 0, "all our entries removed");
        let doc = read_config(&home, &rel);
        assert_eq!(doc["hooks"]["SessionStart"].as_array().expect("array").len(), 1);
        assert!(doc["hooks"]["SessionStart"]
            .to_string()
            .contains("/opt/other/hook.sh"));
        // The event keys we created and emptied are pruned.
        assert!(doc["hooks"].get("Stop").is_none(), "{doc}");

        let again = apply(Host::ClaudeCode, "/bin/memory-wire", &home, true, no_path, &home.join("data"));
        assert!(matches!(again, Outcome::Skipped(_) | Outcome::Already { .. }), "{again:?}");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn copilot_style_should_use_a_flat_bash_entry() {
        let home = tmp_home("copilot");
        let rel = Host::CopilotCli.config_rel();
        write_config(&home, &rel, r#"{"hooks":{"SessionStart":[{"type":"command","bash":"bash '/x.sh'","timeoutSec":10}]}}"#);
        let out = apply(Host::CopilotCli, "/opt/my tools/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let doc = read_config(&home, &rel);
        let ours = doc["hooks"]["SessionStart"]
            .as_array()
            .expect("array")
            .iter()
            .find(|e| e.to_string().contains(MARKER))
            .expect("our entry")
            .clone();
        assert_eq!(ours["bash"], "'/opt/my tools/memory-wire' hook session-start");
        assert_eq!(ours["timeoutSec"], 5);
        std::fs::remove_dir_all(&home).ok();
    }

    // An install path with a space is quoted in every shell command field, not
    // just Copilot's: a bare `/opt/my tools/memory-wire` runs as
    // `/opt/my` and silently loses the hook.
    #[test]
    fn a_spaced_install_path_should_be_quoted_in_claude_style_commands() {
        let home = tmp_home("quote");
        let rel = Host::ClaudeCode.config_rel();
        write_config(&home, &rel, FOREIGN);
        let out = apply(Host::ClaudeCode, "/opt/my tools/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let doc = read_config(&home, &rel);
        for ev in EVENTS {
            let cmd = doc["hooks"][ev.name]
                .as_array()
                .expect("array")
                .iter()
                .flat_map(|g| g["hooks"].as_array().expect("hooks").iter())
                .filter_map(|h| h["command"].as_str())
                .find(|c| c.contains(MARKER))
                .unwrap_or_else(|| panic!("{} command missing in {doc}", ev.name));
            assert_eq!(cmd, format!("'/opt/my tools/memory-wire' hook {}", ev.lifecycle));
        }
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn backup_dir_should_be_named_host_and_timestamp() {
        let home = tmp_home("backup");
        let rel = Host::Codex.config_rel();
        write_config(&home, &rel, FOREIGN);
        let data = home.join("data");
        let dir = backup(&data, &home, Host::Codex, "20260102-030405", &rel)
            .expect("backup")
            .expect("a backup was made");
        let name = dir.file_name().expect("name").to_string_lossy().into_owned();
        assert_eq!(name, "codex-20260102-030405");
        assert!(name.starts_with(&format!("{}-", Host::Codex.id())));
        assert_eq!(
            std::fs::read_to_string(dir.join(&rel)).expect("backed up copy"),
            FOREIGN
        );
        // Nothing to preserve on a first install that creates the file.
        let missing = home.join(".codex/other.json");
        assert!(backup(&data, &home, Host::Codex, "20260102-030405", Path::new(".codex/other.json"))
            .expect("backup")
            .is_none());
        assert!(!missing.exists());
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn detection_should_see_a_config_dir_and_a_missing_host() {
        let home = tmp_home("detect");
        assert!(!exists(&home, Host::Cursor.detect_rel()));
        assert!(!binary_on_path("memory-wire-definitely-not-installed"));
        assert!(!detected_with(Host::Cursor, &home, no_path));

        let skipped = apply(Host::Cursor, "/bin/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(&skipped, Outcome::Skipped(w) if w.contains("not detected")), "{skipped:?}");

        std::fs::create_dir_all(home.join(Host::Cursor.detect_rel())).expect("mkdir");
        assert!(detected_with(Host::Cursor, &home, no_path));
        // Detected is enough now: an MCP-only host is wired, not skipped.
        let wired = apply(Host::Cursor, "/bin/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(wired, Outcome::Wired { .. }), "{wired:?}");
        assert!(!MCP_ONLY_REASON.contains("no MCP server"), "stale reason: {MCP_ONLY_REASON}");
        std::fs::remove_dir_all(&home).ok();
    }

    // Both MCP-only hosts, wired the way each host's own config spells a local
    // server: cursor takes `command`/`args` under `mcpServers`, opencode takes a
    // `command` array plus `type`/`enabled` under `mcp`. Install, re-install,
    // and uninstall are checked together because idempotence and reversibility
    // are the same code path read twice.
    #[test]
    fn mcp_hosts_should_install_idempotently_and_uninstall_cleanly() {
        for (host, key, entry) in [
            (Host::Cursor, "mcpServers", json!({ "command": "/bin/memory-wire", "args": ["mcp"] })),
            (
                Host::Opencode,
                "mcp",
                json!({ "type": "local", "command": ["/bin/memory-wire", "mcp"], "enabled": true }),
            ),
        ] {
            let home = tmp_home(&format!("mcp-{}", host.id()));
            let rel = host.config_rel();
            // A foreign server in the same map must survive both operations.
            std::fs::create_dir_all(home.join(host.detect_rel())).expect("mkdir");
            write_config(
                &home,
                &rel,
                &format!(r#"{{"{key}":{{"other":{{"url":"http://127.0.0.1:9/mcp"}}}}}}"#),
            );
            let data = home.join("data");

            let first = apply(host, "/bin/memory-wire", &home, false, no_path, &data);
            assert!(matches!(first, Outcome::Wired { .. }), "{}: {first:?}", host.id());
            let doc = read_config(&home, &rel);
            assert_eq!(doc[key][MARKER], entry, "{} entry shape", host.id());
            assert_eq!(doc[key].as_object().expect("map").len(), 2, "{} foreign server kept", host.id());
            assert_eq!(our_entries(&doc, key), 1, "{}: one server per file", host.id());

            let second = apply(host, "/bin/memory-wire", &home, false, no_path, &data);
            assert!(matches!(second, Outcome::Already { .. }), "{}: {second:?}", host.id());
            assert_eq!(our_entries(&doc, key), 1, "{}: a re-install must not stack", host.id());

            // A moved binary replaces the stale command rather than adding one.
            let moved = apply(host, "/new/dir/memory-wire", &home, false, no_path, &data);
            assert!(matches!(moved, Outcome::Wired { .. }), "{}: {moved:?}", host.id());
            let doc = read_config(&home, &rel);
            assert_eq!(our_entries(&doc, key), 1, "{}: {moved:?}", host.id());
            assert!(
                std::fs::read_to_string(home.join(&rel)).expect("read").contains("/new/dir/memory-wire"),
                "{}: stale command must be replaced",
                host.id()
            );

            let out = apply(host, "/new/dir/memory-wire", &home, true, no_path, &data);
            assert!(matches!(out, Outcome::Unwired { .. }), "{}: {out:?}", host.id());
            let doc = read_config(&home, &rel);
            assert_eq!(our_entries(&doc, key), 0, "{}: our entry removed", host.id());
            assert!(doc[key]["other"]["url"] == json!("http://127.0.0.1:9/mcp"), "{doc}");

            let again = apply(host, "/new/dir/memory-wire", &home, true, no_path, &data);
            assert!(
                matches!(again, Outcome::Skipped(_) | Outcome::Already { .. }),
                "{}: {again:?}",
                host.id()
            );
            std::fs::remove_dir_all(&home).ok();
        }
    }

    // A non-object map is a refusal, not something to reshape.
    #[test]
    fn a_non_object_mcp_key_should_be_refused_untouched() {
        let home = tmp_home("mcpbad");
        let rel = Host::Opencode.config_rel();
        let broken = r#"{"mcp": ["not","an","object"]}"#;
        write_config(&home, &rel, broken);
        std::fs::create_dir_all(home.join(Host::Opencode.detect_rel())).expect("mkdir");
        let out = apply(Host::Opencode, "/bin/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
        assert_eq!(
            std::fs::read_to_string(home.join(&rel)).expect("read"),
            broken,
            "refusal must leave the file byte-identical"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    // An uninstall that would leave an empty `{}` removes the file instead: on a
    // first install we created it, so the empty object is our own residue.
    #[test]
    fn uninstall_should_delete_a_config_it_created_and_leave_a_foreign_one() {
        for host in [Host::Cursor, Host::Opencode] {
            let home = tmp_home(&format!("empty-{}", host.id()));
            let rel = host.config_rel();
            let data = home.join("data");
            std::fs::create_dir_all(home.join(host.detect_rel())).expect("mkdir");

            apply(host, "/bin/memory-wire", &home, false, no_path, &data);
            assert!(home.join(&rel).is_file(), "{}: install created the config", host.id());
            let out = apply(host, "/bin/memory-wire", &home, true, no_path, &data);
            assert!(matches!(out, Outcome::Unwired { .. }), "{}: {out:?}", host.id());
            assert!(
                !home.join(&rel).exists(),
                "{}: an empty residue file must be deleted, not left as {{}}",
                host.id()
            );

            // The same host with a foreign server keeps its file, minus ours.
            write_config(
                &home,
                &rel,
                &format!(r#"{{"{}":{{"other":{{"url":"x"}}}}}}"#, host.mcp_spec().expect("spec").0),
            );
            apply(host, "/bin/memory-wire", &home, false, no_path, &data);
            apply(host, "/bin/memory-wire", &home, true, no_path, &data);
            let doc = read_config(&home, &rel);
            assert_eq!(
                our_entries(&doc, host.mcp_spec().expect("spec").0),
                0,
                "{}: foreign config must keep no trace of us: {doc}",
                host.id()
            );
            std::fs::remove_dir_all(&home).ok();
        }
    }

    // The hook hosts get the same empty-residue treatment: `hooks` is pruned,
    // so a config that held nothing else is ours to remove.
    #[test]
    fn uninstall_should_delete_a_hook_config_it_created() {
        let home = tmp_home("emptyhook");
        let rel = Host::ClaudeCode.config_rel();
        std::fs::create_dir_all(home.join(Host::ClaudeCode.detect_rel())).expect("mkdir");
        let data = home.join("data");
        apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &data);
        assert!(home.join(&rel).is_file());
        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, true, no_path, &data);
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        assert!(!home.join(&rel).exists(), "an empty residue file must be deleted");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn render_should_align_verbs() {
        let wired = Outcome::Wired {
            backup: None,
            events: vec!["UserPromptSubmit".into()],
            notes: vec!["SessionStart (existing value is not a hook list)".into()],
        }
        .render(Host::ClaudeCode);
        assert_eq!(
            wired,
            "claude-code  wired         UserPromptSubmit  | left alone: SessionStart (existing value is not a hook list)"
        );
        let already = Outcome::Already { notes: vec![] }.render(Host::Codex);
        assert_eq!(already, "codex        already-wired -");
    }

    // A note must not trigger a rewrite: re-running on an unchanged config
    // leaves it byte-identical and takes no second backup. This is the shape
    // of a real `~/.claude/settings.json`, where `SessionStart` is a legacy
    // string we refuse to reshape.
    #[test]
    fn an_untouchable_event_should_not_cause_a_rewrite_on_reinstall() {
        let home = tmp_home("noteonly");
        let rel = Host::ClaudeCode.config_rel();
        write_config(
            &home,
            &rel,
            r#"{"hooks":{"SessionStart":"legacy-string-hook"}}"#,
        );
        let data = home.join("data");
        let first = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &data);
        let Outcome::Wired { events, notes, .. } = &first else {
            panic!("{first:?}");
        };
        assert_eq!(events, &["UserPromptSubmit".to_string(), "Stop".to_string()]);
        assert_eq!(notes, &["SessionStart (existing value is not a hook list)".to_string()]);
        let after_first = std::fs::read_to_string(home.join(&rel)).expect("read");
        let backups = data.join("backups");
        let n_backups = std::fs::read_dir(&backups).expect("readdir").count();

        let second = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &data);
        let Outcome::Already { notes } = &second else {
            panic!("{second:?}");
        };
        assert_eq!(notes, &["SessionStart (existing value is not a hook list)".to_string()]);
        assert_eq!(
            std::fs::read_to_string(home.join(&rel)).expect("read"),
            after_first,
            "a no-op run must leave the file byte-identical"
        );
        assert_eq!(
            std::fs::read_dir(&backups).expect("readdir").count(),
            n_backups,
            "a no-op run must not take another backup"
        );
        std::fs::remove_dir_all(&home).ok();
    }
}
