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
///
/// The builder takes the bank as well as the executable so it can put
/// `--bank` in the argv; see [`mcp_args`] for why that matters and why it is a
/// default rather than a restriction.
type McpSpec = (&'static str, fn(&str, Option<&str>) -> Value);

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
    /// oh-my-pi (`~/.omp/agent/mcp.json`).
    ///
    /// The MCP entry is the **whole** of what is written here, and it is deliberately
    /// the whole of it. OMP also has a native memory-backend slot
    /// (`~/.omp/agent/config.yml`, `memory.backend`) which on this machine names
    /// `hindsight` — and that slot is closed to third parties: there is no
    /// `registerMemoryBackend` symbol in the binary, the backends (`hindsight`,
    /// `Mnemopi`, `Sharpshooter`, `local-backend`, `messages`, `off-backend`) are
    /// bundled modules inside `@oh-my-pi/pi-coding-agent`, and an OMP extension is
    /// `export default function (pi) {...}` and cannot register one. Adding a named
    /// backend means patching OMP's own source.
    ///
    /// So: **do not** touch `config.yml`, **do not** displace or disable Hindsight,
    /// and do not "helpfully" set `memory.backend` — moving another tool's single
    /// global memory slot unasked is exactly the hazard that keeps `Hermes` out of
    /// [`IMPLICIT`]. An MCP key under `mcpServers` is purely additive: it displaces
    /// nothing, which is why OMP is in the implicit set and hermes is not.
    ///
    /// OMP also gets a native **extension** (`~/.omp/agent/extensions/memory-wire.ts`,
    /// see [`crate::connect_ext`]), because an MCP tool only exists when the model
    /// decides to call it — so on an MCP-only host a memory is invisible until the
    /// model thinks to look. The two are additive: the MCP entry is left in place and
    /// neither replaces the other, and `--uninstall` removes both.
    Omp,
    /// hermes-agent (`~/.hermes/config.yaml` plus `~/.hermes/plugins/`).
    Hermes,
    /// pi (`~/.pi/agent/extensions/memory-wire/index.ts`).
    ///
    /// A separate harness from [`Host::Omp`], not its old name. The two are
    /// conflated because the omp binary contains an explicit
    /// `T.omp || T.pi` config fallback; they are not the same program, and they
    /// do not discover extensions the same way. pi takes **no** config entry and
    /// **no** MCP key — its whole integration surface is the extension, which is
    /// why it is [`Style::Extension`] rather than a variant of either of the two
    /// surfaces above.
    ///
    /// It is in [`IMPLICIT`] for the same reason omp is: the install creates one
    /// new file under a directory we would otherwise have to create anyway, and
    /// displaces nothing. Unlike [`Host::Hermes`] it holds no single global slot.
    Pi,
}

/// Every host, in report order.
pub const ALL: &[Host] = &[
    Host::ClaudeCode,
    Host::Codex,
    Host::CopilotCli,
    Host::Cursor,
    Host::Opencode,
    Host::Omp,
    Host::Hermes,
    Host::Pi,
];

/// The hosts a bare `memory-wire connect` wires without being asked.
///
/// Deliberately not [`ALL`]. Every host here is additive: a hook entry, an MCP key
/// or an extension file added to a directory, leaving whatever was already there
/// working. [`Host::Hermes`] is the one exception — it activates exactly one memory
/// provider, so wiring it moves a single global slot, from `agentmemory` to us, on
/// this machine, unasked. A bare `connect` should not trade one memory system for
/// another; `connect hermes` is one word longer and says what it does. OMP has the
/// same single-slot `memory.backend` and the same reason for leaving *that* untouched;
/// see the [`Host::Omp`] doc for why its MCP entry and extension are still additive,
/// and therefore implicit. Pi has no such slot at all: its whole surface is a new file.
///
/// One rule keeps this list honest: a host is implicit when wiring it can only add,
/// and named-only when it can take something away. That is why the note `main.rs`
/// prints for the hosts left out can keep naming exactly one hazard.
pub const IMPLICIT: &[Host] = &[
    Host::ClaudeCode,
    Host::Codex,
    Host::CopilotCli,
    Host::Cursor,
    Host::Opencode,
    Host::Omp,
    Host::Pi,
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
    /// The host imports a **source file** out of a directory it owns, and the
    /// file *is* the registration — no manifest, no key, nothing to merge. This
    /// is [`crate::connect_ext`], and it is a whole surface rather than a second
    /// half of one, so it branches before the JSON read the way [`Style::Plugin`]
    /// does.
    Extension,
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
            Host::Omp => "omp",
            Host::Hermes => "hermes",
            Host::Pi => "pi",
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
            Host::Omp => ".omp/agent/mcp.json",
            // YAML, not JSON, and edited by hand rather than re-serialised.
            // `config_rel` is still the right answer for two reasons: it is the
            // file `backup` must preserve before the first write, and it is what
            // the report line names.
            Host::Hermes => ".hermes/config.yaml",
            // pi reads no config for this. The extension file is named here for
            // the same two reasons as the line above, and because it is the only
            // answer this method can give a host whose whole surface is one file.
            Host::Pi => ".pi/agent/extensions/memory-wire/index.ts",
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
            Host::Omp => ".omp",
            Host::Hermes => ".hermes",
            Host::Pi => ".pi",
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
            Host::Omp => "omp",
            Host::Hermes => "hermes",
            Host::Pi => "pi",
        }
    }

    /// Hook entry shape.
    fn style(self) -> Style {
        match self {
            Host::ClaudeCode | Host::Codex => Style::Claude,
            Host::CopilotCli => Style::Copilot,
            Host::Cursor | Host::Opencode | Host::Omp => Style::Mcp,
            Host::Hermes => Style::Plugin,
            Host::Pi => Style::Extension,
        }
    }

    /// Where this host keeps its MCP server map, and the entry shape it wants.
    ///
    /// `None` for hosts that expose no MCP registration. The shapes mirror what
    /// each host's own config already uses: cursor's `mcpServers` takes
    /// `command`/`args`, opencode's `mcp` takes `type: local` plus a `command`
    /// array, omp's `mcpServers` takes `type: stdio` with the same
    /// `command`/`args` pair — an entry written in another host's shape is
    /// silently ignored.
    fn mcp_spec(self) -> Option<McpSpec> {
        // `--bank` is appended to `mcp` when one is given, and it is a *default*,
        // not a lock: measured, a stdio server started with `--bank omp` serves
        // `resources/list` from `omp` while a `tools/call` naming any other bank
        // still succeeds. That matters because the tools take `bank` as an
        // argument and the MCP `resources/list` method takes no parameters at
        // all -- over stdio there is no URL to read a bank from, so without this
        // flag the resources surface is permanently empty on a stdio host. With
        // it, `resources/list` returns the bank's memories and nothing is
        // restricted that was not restricted before.
        match self {
            Host::Cursor => Some((
                "mcpServers",
                |exe: &str, bank: Option<&str>| {
                    json!({ "command": exe, "args": mcp_args(bank) })
                },
            )),
            Host::Opencode => Some((
                "mcp",
                |exe: &str, bank: Option<&str>| {
                    let mut argv = vec![exe.to_string()];
                    argv.extend(mcp_args(bank));
                    json!({ "type": "local", "command": argv, "enabled": true })
                },
            )),
            // `type` is what omp's own mcp-schema asks for (every live stdio entry
            // carries it), and `command` is the absolute install path rather than a
            // bare name: omp does not inherit memory-wire's install dir onto its
            // PATH, so a bare `memory-wire` would fail to launch. The sibling
            // `cloakctl` entry is spelled the same way.
            Host::Omp => Some((
                "mcpServers",
                |exe: &str, bank: Option<&str>| {
                    json!({ "type": "stdio", "command": exe, "args": mcp_args(bank) })
                },
            )),
            _ => None,
        }
    }
}

/// `["mcp", "--bank", id]` for an MCP stdio command, or just `["mcp"]`.
///
/// One place, because three hosts spell the same argv and a bank flag that
/// reached one of them but not the others would be a silent partial fix: the two
/// that got it would browse resources and the one that did not would see an
/// empty list with no error.
fn mcp_args(bank: Option<&str>) -> Vec<String> {
    let mut args = vec!["mcp".to_string()];
    if let Some(b) = bank.map(str::trim).filter(|b| !b.is_empty()) {
        args.push("--bank".to_string());
        args.push(b.to_string());
    }
    args
}

/// The command string a hook entry runs, with the bank baked in when given.
///
/// Why the flag and not an `env` block in the host's config: `resolve_bank_with`
/// does honour `MEMORY_WIRE_BANK`, but it reads the *process environment*, and
/// whether a host propagates a JSON/TOML `env` key into a hook it spawns is
/// per-host and undocumented. `--bank` is already accepted by the `hook`
/// subcommand and is step 1 of the resolution ladder, so writing it into the
/// command we were going to write anyway works identically on every hooked host
/// and depends on nothing outside this binary. Both parts are quoted: the install
/// path can contain a space, and so can a bank name the caller typed.
fn hook_cmd(exe: &str, lifecycle: &str, bank: Option<&str>) -> String {
    match bank.map(str::trim).filter(|b| !b.is_empty()) {
        Some(b) => format!(
            "{} hook {} --bank {}",
            shell_quote(exe),
            lifecycle,
            shell_quote(b)
        ),
        None => format!("{} hook {lifecycle}", shell_quote(exe)),
    }
}

impl Style {
    /// The entry to append for one lifecycle, or `None` for MCP-only hosts.
    fn entry(self, exe: &str, lifecycle: &str, bank: Option<&str>) -> Option<Value> {
        match self {
            // Quoted like the Copilot `bash` field: an install path can contain
            // a space, and both are shell command strings.
            Style::Claude => Some(json!({
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": hook_cmd(exe, lifecycle, bank),
                    "timeout": HOOK_TIMEOUT,
                }],
            })),
            Style::Copilot => Some(json!({
                "type": "command",
                "bash": hook_cmd(exe, lifecycle, bank),
                "timeoutSec": HOOK_TIMEOUT,
            })),
            Style::Mcp | Style::Plugin | Style::Extension => None,
        }
    }
}

/// Why a host takes no wiring at all.
///
/// No host among [`ALL`] lands here any more — cursor, opencode and omp all
/// accept an MCP server entry, and the other three take hooks. The case is kept
/// so a future MCP-only host with no known writable key reports why instead of
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
    // The extension host is its own file and nothing else.
    if host.style() == Style::Extension {
        return crate::connect_ext::installed(host, home);
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
        Style::Mcp => {
            // OMP answers "or" rather than "and", because it has two additive
            // surfaces: an MCP-only omp wired by an earlier build is genuinely
            // connected, and reporting `wired=no` there would be a false negative
            // the user has no way to act on. `connect omp` still installs both.
            host.mcp_spec().is_some_and(|(key, _)| {
                root.get(key).and_then(Value::as_object).is_some_and(|m| m.contains_key(MARKER))
            }) || crate::connect_ext::installed(host, home)
        }
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
pub fn run(host: Host, exe: &str, home: &Path, uninstall: bool, bank: Option<&str>) -> Outcome {
    apply(
        host,
        exe,
        home,
        uninstall,
        binary_on_path,
        &paths::data_dir(home),
        bank,
    )
}

// Seven parameters, kept flat: `on_path` and `data_root` are the two injected
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
    bank: Option<&str>,
) -> Outcome {
    if !detected_with(host, home, on_path) {
        return Outcome::Skipped("not detected on this machine".to_string());
    }
    let style = host.style();

    // One enum, two install surfaces. `Style::Plugin` is not a JSON edit at all —
    // it copies a directory and moves one scalar in a YAML document — and
    // `Style::Extension` writes a single source file, so both branch before the
    // read below, which has nothing to parse for either host. Keying the split on
    // `Style` rather than on `host == Host::Hermes` is the point: every other arm
    // of the enum stays on the shared JSON path, and a future plugin-shaped or
    // extension-shaped host is a new variant rather than a new side door.
    if style == Style::Plugin {
        return crate::connect_plugin::apply(host, home, uninstall, data_root);
    }
    if style == Style::Extension {
        return crate::connect_ext::apply(host, home, uninstall, data_root);
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
        edit_mcp(&mut root, key, (!uninstall).then(|| entry(exe, bank)))
    } else {
        edit_hooks(&mut root, style, exe, uninstall, bank)
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

    // Two hosts have a second file in a second format, merged into one report
    // line. Codex's hooks live in `hooks.json`, above, and its MCP servers in
    // `config.toml` — a TOML document this crate has no parser for, so it is a
    // surgical text edit in `connect_codex`. OMP's MCP entry is in
    // `mcp.json`, above, and its extension is a TypeScript file this crate
    // writes whole from an embedded template. Both are additive rather than a
    // replacement: a Codex session needs the hooks *and* the tools, and an omp
    // session wants the tools *and* the injection, because an MCP tool exists
    // only when the model decides to call it.
    outcome = match host {
        Host::Codex => merge(outcome, crate::connect_codex::apply(home, uninstall, data_root, bank)),
        Host::Omp => merge(outcome, crate::connect_ext::apply(host, home, uninstall, data_root)),
        _ => outcome,
    };
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
fn edit_hooks(
    root: &mut Value,
    style: Style,
    exe: &str,
    uninstall: bool,
    bank: Option<&str>,
) -> Result<Edit, Refusal> {
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
                style.entry(exe, ev.lifecycle, bank)
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

    /// The rule the [`IMPLICIT`] doc is derived from, stated as a test so adding a
    /// host cannot quietly break it. Implicit means *can only add*: a new file in
    /// a directory, a key beside what is already there. Named-only means it can
    /// take something away, and that is the only reason hermes is excluded.
    #[test]
    fn pi_is_implicit_because_its_install_can_only_add() {
        assert!(
            IMPLICIT.contains(&Host::Pi),
            "pi's whole surface is a new file under a directory, so it displaces nothing"
        );
        assert!(
            !IMPLICIT.contains(&Host::Hermes),
            "hermes holds a single global provider slot, so it is named-only"
        );
        // And pi really is a separate host rather than omp under another name:
        // the two resolve to different homes and different extension layouts.
        assert_ne!(Host::Pi.id(), Host::Omp.id());
        assert_ne!(Host::Pi.config_rel(), Host::Omp.config_rel());
        assert_ne!(Host::Pi.detect_rel(), Host::Omp.detect_rel());
    }

    /// `connect omp` writes **two** additive surfaces, and neither displaces the
    /// other: an omp that had the MCP entry from an earlier build keeps it and
    /// gains the extension beside it. A foreign server in the same map is the
    /// third party neither pass may touch.
    #[test]
    fn connect_omp_should_keep_its_mcp_entry_and_add_the_extension() {
        let home = tmp_home("omp-both");
        std::fs::create_dir_all(home.join(Host::Omp.detect_rel())).expect("omp home");
        // A server of somebody else's, so `mcp.json` survives our uninstall —
        // on a first install ours is the only key, and an emptied config is
        // removed by the shared path rather than rewritten as `{}`.
        write_config(
            &home,
            &Host::Omp.config_rel(),
            r#"{"mcpServers":{"cloakctl":{"type":"stdio","command":"/bin/cloakctl"}}}"#,
        );
        let out = apply(
            Host::Omp,
            "/bin/memory-wire",
            &home,
            false,
            no_path,
            &home.join("data"),
            None
        );
        let Outcome::Wired { events, .. } = &out else {
            panic!("{out:?}");
        };
        assert!(
            events.iter().any(|e| e == "mcpServers"),
            "the MCP entry is still written: {events:?}"
        );
        assert!(
            events.iter().any(|e| e.contains("memory-wire.ts")),
            "the extension is written too: {events:?}"
        );
        let doc = read_config(&home, &Host::Omp.config_rel());
        assert!(doc["mcpServers"]["memory-wire"]["args"].is_array(), "{doc}");
        assert!(
            doc["mcpServers"]["cloakctl"].is_object(),
            "a foreign server is untouched: {doc}"
        );
        let ext = home.join(crate::connect_ext::extension_rel(Host::Omp).expect("rel"));
        assert_eq!(std::fs::read_to_string(&ext).expect("read"), crate::connect_ext::SOURCE);

        // And uninstall takes back only our two, leaving the foreign one.
        let out = apply(
            Host::Omp,
            "/bin/memory-wire",
            &home,
            true,
            no_path,
            &home.join("data"),
            None
        );
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        assert!(!ext.exists(), "the extension is removed");
        let doc = read_config(&home, &Host::Omp.config_rel());
        assert!(
            doc["mcpServers"].get("memory-wire").is_none(),
            "our MCP entry is removed: {doc}"
        );
        assert!(
            doc["mcpServers"]["cloakctl"].is_object(),
            "and nobody else's was: {doc}"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    /// An omp wired by an earlier build — MCP only, no extension — is genuinely
    /// connected and must not read as `wired=no`, which would be a false negative
    /// the user has no way to act on.
    #[test]
    fn an_mcp_only_omp_still_reads_as_wired() {
        let home = tmp_home("omp-mcponly");
        let rel = Host::Omp.config_rel();
        std::fs::create_dir_all(home.join(Host::Omp.detect_rel())).expect("omp home");
        write_config(
            &home,
            &rel,
            r#"{"mcpServers":{"memory-wire":{"type":"stdio","command":"/old/memory-wire","args":["mcp"]}}}"#,
        );
        assert_eq!(listing_fields(&home, Host::Omp), ["detected=yes", "wired=yes"]);
        assert_eq!(listing_fields(&home, Host::Pi), ["detected=no", "wired=no"]);
        std::fs::remove_dir_all(&home).ok();
    }

    /// `wired` has to be read out of the file for pi too, and reading it must not
    /// create anything — pi's install is a directory we would otherwise have to
    /// conjure up.
    #[test]
    fn listing_should_read_pis_state_without_writing_it() {
        let home = tmp_home("pi-list");
        std::fs::create_dir_all(home.join(Host::Pi.detect_rel())).expect("pi home");
        let before = read_dir_names(&home);
        assert!(listing(Host::Pi, &home, no_path).contains("detected=yes wired=no"));
        assert_eq!(read_dir_names(&home), before, "a listing writes nothing");

        let out = apply(
            Host::Pi,
            "/bin/memory-wire",
            &home,
            false,
            no_path,
            &home.join("data"),
            None
        );
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        assert_eq!(listing_fields(&home, Host::Pi), ["detected=yes", "wired=yes"]);
        std::fs::remove_dir_all(&home).ok();
    }

    /// A foreign file at pi's path reads as *not* wired, so `--list` tells the
    /// truth about a path we are refusing to write.
    #[test]
    fn a_foreign_pi_extension_does_not_read_as_wired() {
        let home = tmp_home("pi-foreign");
        std::fs::create_dir_all(home.join(Host::Pi.detect_rel())).expect("pi home");
        let path = home.join(crate::connect_ext::extension_rel(Host::Pi).expect("rel"));
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, "// my own extension\n").expect("write");
        assert_eq!(listing_fields(&home, Host::Pi), ["detected=yes", "wired=no"]);
        std::fs::remove_dir_all(&home).ok();
    }

    /// The `detected=` / `wired=` fields of one `connect --list` line, split out
    /// rather than matched as a substring: `detected` is padded to three columns
    /// so the columns line up, which means `no` is followed by *two* spaces and
    /// `yes` by one. A substring assertion over the padded line would pass for
    /// `yes` and silently fail for `no` — the half that matters here.
    fn listing_fields(home: &Path, host: Host) -> Vec<String> {
        listing(host, home, no_path)
            .split_whitespace()
            .skip(1)
            .map(str::to_string)
            .collect()
    }

    /// Names directly under `home`, sorted — the "nothing was created" snapshot.
    fn read_dir_names(home: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(home)
            .expect("home")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
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

        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);
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

        let first = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);
        assert!(matches!(first, Outcome::Wired { .. }), "{:?}", first);
        assert_eq!(count_ours(&home, &rel), 5, "exactly one entry per event");
        let doc = read_config(&home, &rel);
        let entries = doc["hooks"]["SessionStart"].as_array().expect("array");
        assert_eq!(entries.len(), 2, "foreign hook kept, ours added");
        assert!(entries.iter().any(|e| e.to_string().contains("/opt/other/hook.sh")));

        let second = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);
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
        let out = apply(Host::ClaudeCode, "/new/dir/memory-wire", &home, false, no_path, &home.join("data"), None);
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
        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);
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
        apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);

        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, true, no_path, &home.join("data"), None);
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        assert_eq!(count_ours(&home, &rel), 0, "all our entries removed");
        let doc = read_config(&home, &rel);
        assert_eq!(doc["hooks"]["SessionStart"].as_array().expect("array").len(), 1);
        assert!(doc["hooks"]["SessionStart"]
            .to_string()
            .contains("/opt/other/hook.sh"));
        // The event keys we created and emptied are pruned.
        assert!(doc["hooks"].get("Stop").is_none(), "{doc}");

        let again = apply(Host::ClaudeCode, "/bin/memory-wire", &home, true, no_path, &home.join("data"), None);
        assert!(matches!(again, Outcome::Skipped(_) | Outcome::Already { .. }), "{again:?}");
        std::fs::remove_dir_all(&home).ok();
    }

    /// Our own entry under one event, located by MARKER the way the other tests
    /// do: index 0 is not ours when the host already had hooks.
    fn our_entry(doc: &Value, event: &str) -> Value {
        doc["hooks"][event]
            .as_array()
            .unwrap_or_else(|| panic!("{event} is not an array"))
            .iter()
            .find(|e| e.to_string().contains(MARKER))
            .unwrap_or_else(|| panic!("no memory-wire entry under {event}"))
            .clone()
    }

    /// The command string out of one of our entries. The two hook styles nest it
    /// differently -- Claude wraps it in an inner `hooks` array, Copilot puts it
    /// on the entry -- so one helper reads both rather than each test guessing.
    fn cmd_of(entry: &Value, field: &str) -> String {
        entry
            .get("hooks")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get(field))
            .or_else(|| entry.get(field))
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("no {field} in {entry}"))
            .to_string()
    }

    /// The finding this flag exists for. A hook command is written once and then
    /// run by a host we do not control, forever, with no way to pass it a bank --
    /// so if the memories live in a shared bank and the per-project bank is empty,
    /// the only way to reach them is to bake `--bank` into the command we wrote.
    #[test]
    fn a_named_bank_should_be_baked_into_every_written_hook_command() {
        for (host, field) in [(Host::ClaudeCode, "command"), (Host::CopilotCli, "bash")] {
            let home = tmp_home(&format!("bank-{field}"));
            let rel = host.config_rel();
            write_config(&home, &rel, "{}");
            let out = apply(
                host,
                "/bin/memory-wire",
                &home,
                false,
                no_path,
                &home.join("data"),
                Some("omp"),
            );
            assert!(matches!(out, Outcome::Wired { .. }), "{host:?} {out:?}");
            let doc = read_config(&home, &rel);
            for ev in EVENTS {
                let cmd = cmd_of(&our_entry(&doc, ev.name), field);
                assert!(
                    cmd.ends_with(&format!("hook {} --bank omp", ev.lifecycle)),
                    "{host:?} {} wrote {cmd:?}",
                    ev.name
                );
            }
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// Absent by default. A wiring must not grow a `--bank` nobody asked for,
    /// because the flag is baked into a command the user will later read.
    #[test]
    fn without_a_named_bank_the_command_should_be_unchanged() {
        let home = tmp_home("bank-default");
        let rel = Host::ClaudeCode.config_rel();
        write_config(&home, &rel, "{}");
        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let doc = read_config(&home, &rel);
        let cmd = cmd_of(&our_entry(&doc, "SessionStart"), "command");
        assert_eq!(cmd, "/bin/memory-wire hook session-start");
        assert!(!cmd.contains("--bank"), "no flag nobody asked for: {cmd}");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A blank `--bank` is not a bank. It must not write `--bank ''`, which the
    /// ladder would read as unset and fall through -- looking like it had worked.
    #[test]
    fn a_blank_bank_should_write_no_flag() {
        let home = tmp_home("bank-blank");
        let rel = Host::ClaudeCode.config_rel();
        write_config(&home, &rel, "{}");
        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &home.join("data"), Some("   "));
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let doc = read_config(&home, &rel);
        let cmd = cmd_of(&our_entry(&doc, "SessionStart"), "command");
        assert_eq!(cmd, "/bin/memory-wire hook session-start");
        std::fs::remove_dir_all(&home).ok();
    }

    /// The gap this closes. Measured on the live machine: with no `--bank` in the
    /// argv, all three stdio hosts served `resources/list` -> 0, because the MCP
    /// `resources/list` method takes no parameters and a stdio server has no URL
    /// to read a bank from. With it, the same call returns the bank's memories --
    /// and a `tools/call` naming a *different* bank still succeeds, so this is a
    /// default and not a lock.
    #[test]
    fn a_named_bank_should_reach_the_mcp_argv_as_well_as_the_hooks() {
        for (host, path) in [
            (Host::Cursor, &["mcpServers", "memory-wire", "args"][..]),
            (Host::Opencode, &["mcp", "memory-wire", "command"][..]),
            (Host::Omp, &["mcpServers", "memory-wire", "args"][..]),
        ] {
            let home = tmp_home(&format!("mcpbank-{}", host.id()));
            let rel = host.config_rel();
            write_config(&home, &rel, "{}");
            let out = apply(
                host,
                "/bin/memory-wire",
                &home,
                false,
                no_path,
                &home.join("data"),
                Some("omp"),
            );
            assert!(matches!(out, Outcome::Wired { .. }), "{host:?} {out:?}");
            let doc = read_config(&home, &rel);
            let argv: Vec<String> = {
                let mut node = &doc;
                for key in path {
                    node = &node[*key];
                }
                match node {
                    Value::Array(a) => a
                        .iter()
                        .map(|v| v.as_str().unwrap_or_default().to_string())
                        .collect(),
                    other => vec![other.as_str().unwrap_or_default().to_string()],
                }
            };
            let joined = argv.join(" ");
            assert!(
                joined.contains("--bank omp"),
                "{host:?} wrote {joined:?} with no bank in the argv"
            );
            assert!(joined.contains("mcp"), "{host:?} lost the subcommand: {joined:?}");
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// Absent by default, same as the hooks: a bank nobody asked for must not
    /// appear in a config the user will later read.
    #[test]
    fn without_a_named_bank_the_mcp_argv_should_be_just_mcp() {
        let home = tmp_home("mcpbank-default");
        let rel = Host::Omp.config_rel();
        write_config(&home, &rel, "{}");
        let out = apply(Host::Omp, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let doc = read_config(&home, &rel);
        assert_eq!(doc["mcpServers"]["memory-wire"]["args"], json!(["mcp"]));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn copilot_style_should_use_a_flat_bash_entry() {
        let home = tmp_home("copilot");
        let rel = Host::CopilotCli.config_rel();
        write_config(&home, &rel, r#"{"hooks":{"SessionStart":[{"type":"command","bash":"bash '/x.sh'","timeoutSec":10}]}}"#);
        let out = apply(Host::CopilotCli, "/opt/my tools/memory-wire", &home, false, no_path, &home.join("data"), None);
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
        let out = apply(Host::ClaudeCode, "/opt/my tools/memory-wire", &home, false, no_path, &home.join("data"), None);
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

        let skipped = apply(Host::Cursor, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);
        assert!(matches!(&skipped, Outcome::Skipped(w) if w.contains("not detected")), "{skipped:?}");

        std::fs::create_dir_all(home.join(Host::Cursor.detect_rel())).expect("mkdir");
        assert!(detected_with(Host::Cursor, &home, no_path));
        // Detected is enough now: an MCP-only host is wired, not skipped.
        let wired = apply(Host::Cursor, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);
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

            let first = apply(host, "/bin/memory-wire", &home, false, no_path, &data, None);
            assert!(matches!(first, Outcome::Wired { .. }), "{}: {first:?}", host.id());
            let doc = read_config(&home, &rel);
            assert_eq!(doc[key][MARKER], entry, "{} entry shape", host.id());
            assert_eq!(doc[key].as_object().expect("map").len(), 2, "{} foreign server kept", host.id());
            assert_eq!(our_entries(&doc, key), 1, "{}: one server per file", host.id());

            let second = apply(host, "/bin/memory-wire", &home, false, no_path, &data, None);
            assert!(matches!(second, Outcome::Already { .. }), "{}: {second:?}", host.id());
            assert_eq!(our_entries(&doc, key), 1, "{}: a re-install must not stack", host.id());

            // A moved binary replaces the stale command rather than adding one.
            let moved = apply(host, "/new/dir/memory-wire", &home, false, no_path, &data, None);
            assert!(matches!(moved, Outcome::Wired { .. }), "{}: {moved:?}", host.id());
            let doc = read_config(&home, &rel);
            assert_eq!(our_entries(&doc, key), 1, "{}: {moved:?}", host.id());
            assert!(
                std::fs::read_to_string(home.join(&rel)).expect("read").contains("/new/dir/memory-wire"),
                "{}: stale command must be replaced",
                host.id()
            );

            let out = apply(host, "/new/dir/memory-wire", &home, true, no_path, &data, None);
            assert!(matches!(out, Outcome::Unwired { .. }), "{}: {out:?}", host.id());
            let doc = read_config(&home, &rel);
            assert_eq!(our_entries(&doc, key), 0, "{}: our entry removed", host.id());
            assert!(doc[key]["other"]["url"] == json!("http://127.0.0.1:9/mcp"), "{doc}");

            let again = apply(host, "/new/dir/memory-wire", &home, true, no_path, &data, None);
            assert!(
                matches!(again, Outcome::Skipped(_) | Outcome::Already { .. }),
                "{}: {again:?}",
                host.id()
            );
            std::fs::remove_dir_all(&home).ok();
        }
    }

    // OMP is wired through the same MCP path as cursor and opencode, but its own
    // config carries two sibling top-level keys (`$schema`, `enabledServers`) and
    // every stdio entry declares `type`. So this is checked against a copy of a
    // real `~/.omp/agent/mcp.json` shape rather than the one-key fixture the
    // generic MCP test uses: write, idempotent re-run, uninstall, and the promise
    // that only our key moved.
    /// The `$schema` value from the OMP fixture, as the one key whose URL is
    /// spelled out rather than pattern-matched.
    const OMP_SCHEMA: &str = "https://raw.githubusercontent.com/can1357/oh-my-pi/main/packages/coding-agent/src/config/mcp-schema.json";

    #[test]
    fn omp_should_write_one_stdio_entry_and_leave_the_rest_of_mcp_json_alone() {
        let live_shape = format!(
            r#"{{
  "$schema": "{OMP_SCHEMA}",
  "enabledServers": ["sourcehound", "cloakctl"],
  "mcpServers": {{
    "browseros-neo": {{ "type": "http", "url": "http://127.0.0.1:9211/mcp" }},
    "sourcehound": {{ "type": "stdio", "command": "sourcehound", "args": ["mcp"] }},
    "cloakctl": {{ "type": "stdio", "command": "/home/ishanp/.local/bin/cloakctl-mcp", "args": [] }}
  }}
}}
"#
        );
        let live_shape = live_shape.as_str();

        let home = tmp_home("omp");
        let rel = Host::Omp.config_rel();
        assert_eq!(rel, Path::new(".omp/agent/mcp.json"), "the documented path");
        assert_eq!(Host::Omp.detect_rel(), ".omp");
        assert_eq!(Host::Omp.binary(), "omp");
        let data = home.join("data");
        std::fs::create_dir_all(home.join(Host::Omp.detect_rel())).expect("mkdir");
        write_config(&home, &rel, live_shape);

        let first = apply(Host::Omp, "/opt/mw/memory-wire", &home, false, no_path, &data, None);
        assert!(matches!(first, Outcome::Wired { .. }), "{first:?}");
        let doc = read_config(&home, &rel);
        assert_eq!(
            doc["mcpServers"][MARKER],
            json!({ "type": "stdio", "command": "/opt/mw/memory-wire", "args": ["mcp"] }),
            "omp wants `type: stdio`, an absolute command, and the `mcp` subcommand"
        );
        assert_eq!(
            our_entries(&doc, "mcpServers"),
            1,
            "one server per file: {doc}"
        );
        // The two sibling keys are not ours to touch, and the three foreign
        // servers keep their exact entry shape.
        assert_eq!(doc["$schema"], json!(OMP_SCHEMA), "{doc}");
        assert_eq!(doc["enabledServers"], json!(["sourcehound", "cloakctl"]), "{doc}");
        assert_eq!(
            doc["mcpServers"]["cloakctl"]["command"],
            json!("/home/ishanp/.local/bin/cloakctl-mcp"),
            "a foreign entry must survive intact: {doc}"
        );
        assert_eq!(
            doc["mcpServers"]["browseros-neo"]["url"],
            json!("http://127.0.0.1:9211/mcp"),
            "an http entry must survive intact: {doc}"
        );

        let second = apply(Host::Omp, "/opt/mw/memory-wire", &home, false, no_path, &data, None);
        assert!(matches!(second, Outcome::Already { .. }), "{second:?}");
        assert_eq!(
            our_entries(&read_config(&home, &rel), "mcpServers"),
            1,
            "a re-install must not stack"
        );

        let out = apply(Host::Omp, "/opt/mw/memory-wire", &home, true, no_path, &data, None);
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        let doc = read_config(&home, &rel);
        assert_eq!(our_entries(&doc, "mcpServers"), 0, "our key removed");
        assert_eq!(
            doc["mcpServers"].as_object().expect("map still there").len(),
            3,
            "foreign servers keep the map alive, so only our key goes: {doc}"
        );
        assert_eq!(doc["enabledServers"], json!(["sourcehound", "cloakctl"]), "{doc}");

        let again = apply(Host::Omp, "/opt/mw/memory-wire", &home, true, no_path, &data, None);
        assert!(
            matches!(again, Outcome::Skipped(_) | Outcome::Already { .. }),
            "{again:?}"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    // The project rule is that a malformed config is REFUSED untouched rather than
    // overwritten — asserted per host, because the refusal is a property of the
    // shared read path only for as long as nobody adds a host that bypasses it.
    #[test]
    fn a_malformed_omp_config_should_be_refused_without_a_backup() {
        let home = tmp_home("ompbad");
        let rel = Host::Omp.config_rel();
        let broken = "{ \"mcpServers\": { \"memory-wire\": oops";
        write_config(&home, &rel, broken);
        std::fs::create_dir_all(home.join(Host::Omp.detect_rel())).expect("mkdir");
        let out = apply(Host::Omp, "/opt/mw/memory-wire", &home, false, no_path, &home.join("data"), None);
        assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
        assert!(out.is_failure());
        assert_eq!(
            std::fs::read_to_string(home.join(&rel)).expect("read"),
            broken,
            "refusal must leave the file byte-identical"
        );
        assert!(
            !home.join("data/backups").exists(),
            "a refused host must not create a backup"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    // The generated note about hosts a bare `connect` skips must not go stale: it
    // is derived from `ALL - IMPLICIT`, and hermes is the only host on the wrong
    // side of that line. OMP's additive MCP entry does not move it there.
    #[test]
    fn omp_should_be_implicit_because_its_mcp_entry_displaces_nothing() {
        assert!(
            IMPLICIT.contains(&Host::Omp),
            "writing an MCP key adds a server; it does not take over a slot"
        );
        assert!(ALL.contains(&Host::Omp));
        let skipped: Vec<&str> = ALL
            .iter()
            .filter(|h| !IMPLICIT.contains(h))
            .map(|h| h.id())
            .collect();
        assert_eq!(
            skipped,
            vec!["hermes"],
            "the only host a bare `connect` declines is the one that moves memory.provider"
        );
    }

    // A non-object map is a refusal, not something to reshape.
    #[test]
    fn a_non_object_mcp_key_should_be_refused_untouched() {
        let home = tmp_home("mcpbad");
        let rel = Host::Opencode.config_rel();
        let broken = r#"{"mcp": ["not","an","object"]}"#;
        write_config(&home, &rel, broken);
        std::fs::create_dir_all(home.join(Host::Opencode.detect_rel())).expect("mkdir");
        let out = apply(Host::Opencode, "/bin/memory-wire", &home, false, no_path, &home.join("data"), None);
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
        for host in [Host::Cursor, Host::Opencode, Host::Omp] {
            let home = tmp_home(&format!("empty-{}", host.id()));
            let rel = host.config_rel();
            let data = home.join("data");
            std::fs::create_dir_all(home.join(host.detect_rel())).expect("mkdir");

            apply(host, "/bin/memory-wire", &home, false, no_path, &data, None);
            assert!(home.join(&rel).is_file(), "{}: install created the config", host.id());
            let out = apply(host, "/bin/memory-wire", &home, true, no_path, &data, None);
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
            apply(host, "/bin/memory-wire", &home, false, no_path, &data, None);
            apply(host, "/bin/memory-wire", &home, true, no_path, &data, None);
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
        apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &data, None);
        assert!(home.join(&rel).is_file());
        let out = apply(Host::ClaudeCode, "/bin/memory-wire", &home, true, no_path, &data, None);
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

            let first = apply(host, "/bin/memory-wire", &home, false, no_path, &data, None);
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

            let second = apply(host, "/bin/memory-wire", &home, false, no_path, &data, None);
            assert!(matches!(second, Outcome::Already { .. }), "{}: {second:?}", host.id());
            assert_eq!(
                count_ours(&home, &rel),
                5,
                "{}: a re-install must not stack the new events",
                host.id()
            );

            let out = apply(host, "/bin/memory-wire", &home, true, no_path, &data, None);
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
        let first = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &data, None);
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

        let second = apply(Host::ClaudeCode, "/bin/memory-wire", &home, false, no_path, &data, None);
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
