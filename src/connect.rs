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
///
/// The names are [`Host::id`]s rather than variant names, so a host can only
/// ever be spelled one way: [`parse_hosts`] is the single place a name is
/// resolved, and there is no second list for it to disagree with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    /// hermes-agent (`~/.hermes/config.yaml` plus `~/.hermes/plugins/`).
    Hermes,
}

/// Every host, in report order.
pub const ALL: &[Host] = &[
    Host::ClaudeCode,
    Host::Codex,
    Host::CopilotCli,
    Host::Cursor,
    Host::Opencode,
    Host::Hermes,
];

/// The hosts a bare `memory-wire connect` wires without being asked.
///
/// Deliberately not [`ALL`]. The other five are additive: a hook entry or an MCP key
/// added to a JSON file, leaving whatever was already there working. Hermes activates
/// exactly one memory provider, so wiring it moves a single global slot — from
/// `agentmemory` to us, on this machine, unasked. A bare `connect` should not trade
/// one memory system for another; `connect hermes` is one word longer and says what
/// it does.
pub const IMPLICIT: &[Host] = &[
    Host::ClaudeCode,
    Host::Codex,
    Host::CopilotCli,
    Host::Cursor,
    Host::Opencode,
];

/// Resolve a host list: one name, several separated by commas or spaces, or the
/// same flag given twice. Duplicates collapse, so one call that names a host
/// twice wires it once.
///
/// Spaces separate as well as commas because a caller writing
/// `"claude-code, cursor"` in a script means the two hosts it wrote, and
/// refusing a call that is not wrong buys nothing. Names are otherwise matched
/// exactly, as the single-host argument was.
pub fn parse_hosts(raw: &[String]) -> Result<Vec<Host>, String> {
    let mut hosts: Vec<Host> = Vec::new();
    for token in raw.iter().flat_map(|v| v.split([',', ' ', '\t'])) {
        let name = token.trim();
        if name.is_empty() {
            continue;
        }
        let host = ALL
            .iter()
            .copied()
            .find(|h| h.id() == name)
            .ok_or_else(|| unknown_host(name))?;
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    Ok(hosts)
}

/// The message for a name that is not a host, naming the ones that are.
fn unknown_host(name: &str) -> String {
    format!(
        "unknown agent host `{name}` — `memory-wire connect --list` shows the hosts; \
         valid names: {}",
        ALL.iter().map(|h| h.id()).collect::<Vec<_>>().join(", ")
    )
}

/// A host lifecycle event and the `hook` subcommand it runs.
struct Event {
    /// Key under the config's `hooks` object.
    name: &'static str,
    /// `memory-wire hook <lifecycle>` argument.
    lifecycle: &'static str,
}

const EVENTS: [Event; 5] = [
    Event { name: "SessionStart", lifecycle: "session-start" },
    Event { name: "UserPromptSubmit", lifecycle: "prompt" },
    Event { name: "Stop", lifecycle: "stop" },
    // The two moments a session's words stop being available. `PreCompact`
    // fires immediately before compaction discards the conversation;
    // `SessionEnd` fires once, at the end. agentmemory registers the first and
    // hindsight the second — see `docs/INTEGRATION_GAPS.md` §G2.
    Event { name: "PreCompact", lifecycle: "pre-compact" },
    Event { name: "SessionEnd", lifecycle: "session-end" },
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
    Mcp,
    /// The host loads a **plugin directory** and activates it with one key in a
    /// YAML config. It has no JSON document and no per-event command entry, so
    /// neither [`Style::entry`] nor [`Host::mcp_spec`] can express it: the
    /// install is a copy of an embedded tree plus one scalar edit, which is what
    /// [`crate::connect_plugin`] is for.
    Plugin,
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
            Host::Hermes => "hermes",
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
            // YAML, not JSON, and edited by hand rather than re-serialised.
            // `config_rel` is still the right answer for two reasons: it is the
            // file `backup` must preserve before the first write, and it is what
            // the report line names.
            Host::Hermes => ".hermes/config.yaml",
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
            Host::Hermes => ".hermes",
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
            Host::Hermes => "hermes",
        }
    }

    /// Hook entry shape.
    fn style(self) -> Style {
        match self {
            Host::ClaudeCode | Host::Codex => Style::Claude,
            Host::CopilotCli => Style::Copilot,
            Host::Cursor | Host::Opencode => Style::Mcp,
            Host::Hermes => Style::Plugin,
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
            Style::Mcp | Style::Plugin => None,
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

/// One `connect --list` line: the host, whether it is on this machine, and
/// whether it already carries our entries.
///
/// Reads and nothing else. No config is created, edited, moved or removed, and
/// no backup directory is made — this is the input to a decision about which
/// hosts to wire, so a version that rewrote a file while reporting would be
/// editing something the caller is choosing whether to touch at all.
pub fn listing(host: Host, home: &Path, on_path: fn(&str) -> bool) -> String {
    let detected = detected_with(host, home, on_path);
    let wired = detected && wired_with(host, home);
    format!("{:<12} detected={:<3} wired={}", host.id(), yes_no(detected), yes_no(wired))
}

/// `yes` / `no`, so a line is greppable without a parser.
fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

/// Does this host already carry our entries? Parses, never writes.
///
/// A config that cannot be parsed reads as not-wired rather than refused: this
/// is a report, and the refusal that belongs to a write is [`apply`]'s.
fn wired_with(host: Host, home: &Path) -> bool {
    // The plugin host has no key to look for — its install is a directory whose
    // name IS the provider name, so the directory is the installed state. The
    // path is composed from the same two pieces `connect_plugin` builds it from
    // (`.hermes/plugins/<marker>`); that module owns the write, this the read.
    if host.style() == Style::Plugin {
        return home.join(host.detect_rel()).join("plugins").join(MARKER).is_dir();
    }
    let Ok(raw) = paths::read_or_empty(&home.join(host.config_rel())) else {
        return false;
    };
    let Ok(root) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    match host.style() {
        // The MCP hosts are keyed by the server name we register under, so the
        // key is the check. The hook hosts are recognised the way every other
        // pass recognises them: by the marker inside the entry.
        Style::Mcp => host.mcp_spec().is_some_and(|(key, _)| {
            root.get(key).and_then(Value::as_object).is_some_and(|m| m.contains_key(MARKER))
        }),
        _ => EVENTS
            .iter()
            .any(|ev| root.pointer(&format!("/hooks/{}", ev.name)).is_some_and(is_ours)),
    }
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
/// Returns the backup directory, or `None` when there was nothing to preserve
/// (a first install that creates the config, or a host with no plugin directory
/// yet). A directory is copied whole: the Hermes install overwrites a plugin
/// tree, and a backup that kept only its files would be no backup at all.
pub fn backup(
    data_root: &Path,
    home: &Path,
    host: Host,
    ts: &str,
    rel: &Path,
) -> Result<Option<PathBuf>, String> {
    let src = home.join(rel);
    if src.is_dir() {
        return copy_tree(&src, &data_root.join("backups").join(format!("{}-{ts}", host.id())), rel);
    }
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

/// Copy a directory tree, rooted at `dst_root/rel`.
///
/// Recursive by hand rather than by crate: the only caller is the Hermes plugin
/// directory, which is a flat handful of files, and a recursive walk over an
/// unbounded tree is a way to wedge `connect` on a symlink loop.
fn copy_tree(src: &Path, dst_root: &Path, rel: &Path) -> Result<Option<PathBuf>, String> {
    let dir = dst_root.to_path_buf();
    let root = dir.join(rel);
    let mut queue = vec![(src.to_path_buf(), root.clone())];
    while let Some((from, to)) = queue.pop() {
        let entries = std::fs::read_dir(&from).map_err(|e| format!("{}: {e}", from.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("{}: {e}", from.display()))?;
            let file_type = entry
                .file_type()
                .map_err(|e| format!("{}: {e}", entry.path().display()))?;
            // A symlink is skipped, not followed: preserving one would need its
            // target's contents, and following one can leave this directory.
            if file_type.is_symlink() {
                continue;
            }
            let dest = to.join(entry.file_name());
            if file_type.is_dir() {
                std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
                queue.push((entry.path(), dest));
            } else {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("{}: {e}", parent.display()))?;
                }
                std::fs::copy(entry.path(), &dest)
                    .map_err(|e| format!("{}: {e}", dest.display()))?;
            }
        }
    }
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
//
// `pub(crate)` rather than private: `connect_plugin` is a sibling module and its
// tests need to reach this with an injected data root. `connect::run` is not a
// substitute — it resolves the data root through `paths::data_dir`, which reads
// the process environment, so a test using it would write into the real XDG dir.
pub(crate) fn apply(
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

    // One enum, two install surfaces. `Style::Plugin` is not a JSON edit at all —
    // it copies a directory and moves one scalar in a YAML document — so it
    // branches before the read below, which has nothing to parse for that host.
    // Keying the split on `Style` rather than on `host == Host::Hermes` is the
    // point: every other arm of the enum stays on the shared JSON path, and a
    // future plugin-shaped host is a new variant rather than a new side door.
    if style == Style::Plugin {
        return crate::connect_plugin::apply(host, home, uninstall, data_root);
    }

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
    let edit = if style == Style::Mcp {
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
    // The JSON pass as one value, so it can be merged with the codex pass below.
    // Its tail is unchanged from the single-file version of this function; the
    // closure only gives those `return`s somewhere to land.
    let mut outcome = (|| -> Outcome {
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
    })();

    // Codex is the one host with a second file in a second format. Its hooks live
    // in `hooks.json`, above, and its MCP servers in `config.toml` — a TOML
    // document this crate has no parser for, so it is a surgical text edit in
    // `connect_codex`. Additive rather than a replacement: a Codex session needs
    // both the hooks and the tools, and the HTTP MCP endpoint is the only
    // transport Codex can reach.
    if host == Host::Codex {
        outcome = merge(outcome, crate::connect_codex::apply(home, uninstall, data_root));
    }
    outcome
}

/// Two passes over two files are one report line.
///
/// The pass that changed something decides the verb; a pass that was refused or
/// had nothing to do contributes its reason as a note rather than swallowing the
/// other file's work. `Failed` wins outright, because a refused edit is a
/// refusal the user has to see rather than a change quietly reported as done.
fn merge(a: Outcome, b: Outcome) -> Outcome {
    if matches!(b, Outcome::Failed(_)) {
        return b;
    }
    let (a_changed, a_unwired, a_backup, a_events, a_notes) = parts(a);
    let (b_changed, b_unwired, b_backup, b_events, b_notes) = parts(b);
    let mut events = a_events;
    events.extend(b_events);
    let mut notes = a_notes;
    notes.extend(b_notes);
    if !a_changed && !b_changed {
        return Outcome::Already { notes };
    }
    // Both passes run under the same `--uninstall`, so they cannot disagree
    // about the verb; the second clause only covers the case where the pass
    // carrying the verb is the one that was already in place.
    if a_unwired || (!a_changed && b_unwired) {
        Outcome::Unwired { backup: a_backup.or(b_backup), events, notes }
    } else {
        Outcome::Wired { backup: a_backup.or(b_backup), events, notes }
    }
}

/// One outcome as `(changed, unwired, backup, events, notes)`.
///
/// A `Skipped` reason becomes a note rather than a verb: the other file's work
/// still happened, and "left alone: no writable hook slot" says more about what
/// the user is looking at than a bare `wired`.
fn parts(o: Outcome) -> (bool, bool, Option<PathBuf>, Vec<String>, Vec<String>) {
    match o {
        Outcome::Wired { backup, events, notes } => (true, false, backup, events, notes),
        Outcome::Unwired { backup, events, notes } => (true, true, backup, events, notes),
        Outcome::Already { notes } => (false, false, None, Vec::new(), notes),
        Outcome::Skipped(why) => (false, false, None, Vec::new(), vec![why]),
        Outcome::Failed(_) => (false, false, None, Vec::new(), Vec::new()),
    }
}

/// Add or remove our five hook entries under `hooks`.
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
    fn implicit_connect_set_should_exclude_hermes() {
        // Hermes moves a one-slot global (`memory.provider`), so a bare `connect`
        // must not take it. Pinned because the two lists are adjacent and merging
        // them back would be a one-character change.
        assert!(
            !IMPLICIT.contains(&Host::Hermes),
            "a bare `connect` must not activate a memory provider over another"
        );
        assert!(ALL.contains(&Host::Hermes), "hermes is still reachable by name");
        assert_eq!(ALL.len(), IMPLICIT.len() + 1, "every other host stays implicit");
    }

    fn parse(v: &[&str]) -> Result<Vec<Host>, String> {
        parse_hosts(&v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn a_host_list_should_take_commas_spaces_and_repetition() {
        assert_eq!(parse(&["codex"]), Ok(vec![Host::Codex]));
        assert_eq!(parse(&["codex,cursor"]), Ok(vec![Host::Codex, Host::Cursor]));
        assert_eq!(parse(&["codex", "cursor"]), Ok(vec![Host::Codex, Host::Cursor]));
        // The spaced comma form is what a shell script actually writes.
        assert_eq!(parse(&["codex, cursor"]), Ok(vec![Host::Codex, Host::Cursor]));
        assert_eq!(parse(&["hermes"]), Ok(vec![Host::Hermes]));
        // A name repeated inside one call is still one host.
        assert_eq!(parse(&["codex,codex", "codex"]), Ok(vec![Host::Codex]));
        // No host at all is the bare `connect`, decided by the caller, not here.
        assert_eq!(parse(&[]), Ok(Vec::new()));
    }

    #[test]
    fn an_unknown_host_name_should_name_every_valid_one() {
        let err = parse(&["claude_code"]).expect_err("a name that is not a host");
        assert!(err.contains("claude_code"), "{err}");
        for host in ALL {
            assert!(err.contains(host.id()), "{} missing from: {err}", host.id());
        }
    }

    /// Every host is listed whether or not this machine has it: a caller
    /// choosing what to install needs to see what it could install, so a missing
    /// host may not be dropped from the answer.
    #[test]
    fn listing_should_name_an_undetected_host_rather_than_omitting_it() {
        let home = tmp_home("list-none");
        for host in ALL {
            let line = listing(*host, &home, no_path);
            assert!(line.starts_with(&format!("{:<12}", host.id())), "{line}");
            assert!(line.contains("detected=no"), "{line}");
            assert!(line.contains("wired=no"), "{line}");
        }
        assert!(!home.join("data").exists(), "a listing must not create a data dir");
        std::fs::remove_dir_all(&home).ok();
    }

    /// `wired` is read out of the file rather than inferred: our own entry means
    /// wired, a config holding only somebody else's hook does not, and reading
    /// the answer must not change the answer's subject.
    #[test]
    fn listing_should_read_wired_from_the_config_without_editing_it() {
        let home = tmp_home("list-wired");
        let rel = Host::ClaudeCode.config_rel();
        write_config(&home, &rel, FOREIGN);
        let before = std::fs::read_to_string(home.join(&rel)).expect("read");
        assert!(listing(Host::ClaudeCode, &home, no_path).contains("detected=yes wired=no"));
        assert_eq!(
            std::fs::read_to_string(home.join(&rel)).expect("read"),
            before,
            "reading the state must leave the file byte-identical"
        );

        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let wired = std::fs::read_to_string(home.join(&rel)).expect("read");
        assert!(listing(Host::ClaudeCode, &home, no_path).contains("detected=yes wired=yes"));
        assert_eq!(
            std::fs::read_to_string(home.join(&rel)).expect("read"),
            wired,
            "listing a wired config must not rewrite it"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn install_should_be_idempotent_and_preserve_foreign_hooks() {
        let home = tmp_home("idem");
        let rel = Host::ClaudeCode.config_rel();
        write_config(&home, &rel, FOREIGN);

        let first = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(first, Outcome::Wired { .. }), "{:?}", first);
        assert_eq!(count_ours(&home, &rel), 5, "exactly one entry per event");
        let doc = read_config(&home, &rel);
        let entries = doc["hooks"]["SessionStart"].as_array().expect("array");
        assert_eq!(entries.len(), 2, "foreign hook kept, ours added");
        assert!(entries.iter().any(|e| e.to_string().contains("/opt/other/hook.sh")));

        let second = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"));
        assert!(matches!(second, Outcome::Already { .. }), "{second:?}");
        assert_eq!(count_ours(&home, &rel), 5, "re-install must not stack entries");
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
        assert_eq!(count_ours(&home, &rel), 5);
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

    // G2 — the two events that were missing. Every hook-writing host gets all
    // five, each in that host's own entry shape, and a second install changes
    // nothing. One host per style: Claude and Codex share the matcher-group
    // shape, Copilot the flat `bash` one.
    #[test]
    fn install_should_write_five_events_per_host_and_stay_idempotent() {
        for host in [Host::ClaudeCode, Host::Codex, Host::CopilotCli] {
            let home = tmp_home(&format!("five-{}", host.id()));
            let rel = host.config_rel();
            std::fs::create_dir_all(home.join(host.detect_rel())).expect("mkdir");
            let data = home.join("data");

            let first = apply(host, "/bin/memory-wire", &home, false, no_path, &data);
            assert!(matches!(first, Outcome::Wired { .. }), "{}: {first:?}", host.id());
            let doc = read_config(&home, &rel);
            for ev in EVENTS {
                let slot = doc["hooks"][ev.name]
                    .as_array()
                    .unwrap_or_else(|| panic!("{}: no `{}` slot in {doc}", host.id(), ev.name));
                assert_eq!(
                    slot.iter().filter(|v| is_ours(v)).count(),
                    1,
                    "{}: exactly one {} entry",
                    host.id(),
                    ev.name
                );
            }
            // The wire name is what a host actually executes, so it is asserted,
            // not assumed: `PreCompact` must not be a `precompact` or a
            // `preCompact` that no host would ever fire.
            for ev in EVENTS {
                let cmd = doc["hooks"][ev.name].to_string();
                assert!(
                    cmd.contains(&format!("hook {}", ev.lifecycle)),
                    "{}: {} must run `hook {}`",
                    host.id(),
                    ev.name,
                    ev.lifecycle
                );
            }
            assert_eq!(count_ours(&home, &rel), 5, "{}: five entries", host.id());

            let second = apply(host, "/bin/memory-wire", &home, false, no_path, &data);
            assert!(matches!(second, Outcome::Already { .. }), "{}: {second:?}", host.id());
            assert_eq!(
                count_ours(&home, &rel),
                5,
                "{}: a re-install must not stack the new events",
                host.id()
            );

            let out = apply(host, "/bin/memory-wire", &home, true, no_path, &data);
            assert!(matches!(out, Outcome::Unwired { .. }), "{}: {out:?}", host.id());
            // This config held nothing but our five entries, so pruning them
            // leaves no residue to write: the file is removed rather than
            // emptied.
            assert!(
                !home.join(&rel).exists(),
                "{}: an empty residue file must be deleted",
                host.id()
            );
            std::fs::remove_dir_all(&home).ok();
        }
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
        assert_eq!(
            events,
            &[
                "UserPromptSubmit".to_string(),
                "Stop".to_string(),
                "PreCompact".to_string(),
                "SessionEnd".to_string(),
            ]
        );
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
