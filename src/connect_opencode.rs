//! opencode's second install surface: a plugin file it auto-loads from
//! `~/.config/opencode/plugins/`, mirroring what [`crate::connect_ext`] does
//! for omp and pi.
//!
//! A flat `*.ts` file in that directory is the registration — the loader is not
//! config-driven. Verified against opencode 1.18.34 by a live probe
//! (2026-10-07): a flat file in that directory loaded with no config entry,
//! `experimental.chat.system.transform` fired per request, and a string pushed
//! onto its `system` array reached the model. The MCP entry `connect` already
//! writes is the tools the model may call; this file is the recall it does not
//! have to ask for — on a continuation prompt it never looks, which is exactly
//! why the injection has to exist (a forked-session test showed the model
//! making zero recall calls across the turn).
//!
//! The three properties are inherited from `connect_ext` rather than
//! reinvented, and each is shaped by the surface being a file rather than a
//! config:
//!
//! - **Never corrupt.** A file at our path that does not carry our marker is
//!   somebody's plugin, and it is refused byte-identical rather than replaced.
//! - **Idempotent.** A file whose bytes already match the rendered template is
//!   not rewritten, and a second install reports `already-wired` with the
//!   version it found, so a no-op and a real upgrade are told apart. Rendered,
//!   not embedded: a `connect --bank` bakes the bank into the file, and the
//!   same rendering feeds the compare, so a reconnect with the *same* bank is
//!   still a no-op and one with a *different* bank is a real replace.
//! - **Reversible.** The pre-existing file is copied to
//!   `backups/<host>-<ts>/` before the first write, and `--uninstall` removes
//!   only a file this install could recognise as ours.

use std::path::{Path, PathBuf};

use crate::connect::{backup, timestamp, Host, Outcome, MARKER};
use crate::paths;

/// The plugin, embedded at build time.
///
/// Source of truth is `plugin/integrations/opencode-plugin/memory-wire.ts`, and
/// `the_embedded_source_is_the_one_on_disk` asserts the directory and
/// `include_str!` agree — a file in the directory but not here would install
/// something this binary never ships and pass every other test.
pub(crate) const SOURCE: &str =
    include_str!("../plugin/integrations/opencode-plugin/memory-wire.ts");

/// Marker the template carries, and the only thing that distinguishes one
/// installed copy from another.
const VERSION_MARKER: &str = "MEMORY_WIRE_OPENCODE_PLUGIN_VERSION";

/// The template line the install-time bank is baked into.
const DEFAULT_BANK_LINE: &str = "var DEFAULT_BANK = \"memory-wire\";";

/// The template as it lands on disk: the `connect --bank` value baked into
/// `DEFAULT_BANK`, or the embedded bytes when no bank was given.
///
/// Bank ids are slug-like; the escaping is a backstop for a hostile value
/// rather than an expectation.
fn baked(bank: Option<&str>) -> String {
    match bank.map(str::trim).filter(|b| !b.is_empty()) {
        None => SOURCE.to_string(),
        Some(b) => SOURCE.replace(
            DEFAULT_BANK_LINE,
            &format!("var DEFAULT_BANK = \"{}\";", b.replace('\\', "\\\\").replace('"', "\\\"")),
        ),
    }
}

/// Install or uninstall opencode's plugin file. See the module docs.
///
/// A `None` from [`plugin_rel`] means the host has no plugin surface, and that
/// is a [`Outcome::Skipped`] rather than a failure: nothing to get wrong.
pub fn apply(host: Host, home: &Path, uninstall: bool, data_root: &Path, bank: Option<&str>) -> Outcome {
    let Some(rel) = plugin_rel(host) else {
        return Outcome::Skipped("no opencode plugin surface for this host".to_string());
    };
    let path = home.join(&rel);
    let source = baked(bank);
    let current = version(&source).unwrap_or("?");

    if uninstall {
        if !path.is_file() {
            return Outcome::Skipped("no memory-wire plugin to remove".to_string());
        }
        if !is_ours(&path) {
            return Outcome::Failed(format!(
                "{}: does not carry the {MARKER} marker; left untouched",
                path.display()
            ));
        }
        let saved = match backup(data_root, home, host, &timestamp(), &rel) {
            Ok(b) => b,
            Err(e) => return Outcome::Failed(e),
        };
        if let Err(e) = std::fs::remove_file(&path) {
            return Outcome::Failed(format!("{}: {e}", path.display()));
        }
        // Same rule `connect_plugin` applies to `plugins/`: a directory this
        // removal emptied is our own residue, and one it did not is not ours to
        // delete. Bounded at `plugins/`, so opencode's own `.config/opencode/`
        // survives.
        prune_empty_dirs(path.parent().unwrap_or(Path::new("")));
        return Outcome::Unwired {
            backup: saved,
            events: vec![rel.display().to_string()],
            notes: Vec::new(),
        };
    }

    let existing = std::fs::read_to_string(&path).ok();
    if let Some(have) = existing.as_deref() {
        if !carries_marker(have) {
            return Outcome::Failed(format!(
                "{}: not a memory-wire plugin ({VERSION_MARKER} absent); left untouched",
                path.display()
            ));
        }
        if have == source {
            return Outcome::Already {
                notes: vec![format!("plugin v{current}")],
            };
        }
    }

    let saved = match backup(data_root, home, host, &timestamp(), &rel) {
        Ok(b) => b,
        Err(e) => return Outcome::Failed(e),
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Outcome::Failed(format!("{}: {e}", parent.display()));
        }
    }
    if let Err(e) = paths::write_atomic(&path, &source) {
        return Outcome::Failed(format!("{}: {e}", path.display()));
    }

    let mut notes = Vec::new();
    // A file carrying a different version of ours is still ours, so it is
    // replaced rather than refused — and both versions are named, because that
    // is the only moment a user can tell a real upgrade from a no-op.
    if let Some(was) = existing.as_deref().and_then(version) {
        notes.push(format!("replaced plugin v{was} with v{current}"));
    }
    Outcome::Wired {
        backup: saved,
        events: vec![rel.display().to_string()],
        notes,
    }
}

/// Where opencode's plugin file goes, home-relative, or `None` for a host that
/// has no plugin surface.
///
/// A flat file there loads with no config entry — the file is the
/// registration, so the path is the whole install.
pub fn plugin_rel(host: Host) -> Option<PathBuf> {
    match host {
        Host::Opencode => Some(
            PathBuf::from(".config")
                .join("opencode")
                .join("plugins")
                .join(format!("{MARKER}.ts")),
        ),
        _ => None,
    }
}

/// Does this host carry our plugin file? Reads, and never writes.
///
/// For opencode the caller ORs this with the MCP entry rather than ANDing it:
/// an opencode wired by an earlier build has only the MCP key, and reporting
/// `wired=no` there would be a false negative.
pub fn installed(host: Host, home: &Path) -> bool {
    plugin_rel(host).is_some_and(|rel| is_ours(&home.join(rel)))
}

/// Does the file at our path carry our marker? See [`carries_marker`] for why
/// both markers are required rather than one.
fn is_ours(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|s| carries_marker(&s))
}

/// The test for "this file is ours to replace".
///
/// Both markers, not just the name: a file that merely mentions `memory-wire`
/// is a user's own plugin talking about a memory backend, and overwriting it
/// is the one thing this module exists to avoid.
fn carries_marker(source: &str) -> bool {
    source.contains(MARKER) && source.contains(VERSION_MARKER)
}

/// The version the marker carries, or `None` for a file that has none.
///
/// Same parsing discipline as `connect_ext`'s: a marker line whose value is
/// not digits reads as absent, so a mangled marker reports the upgrade as
/// replacing an unknown version rather than failing to read one.
fn version(source: &str) -> Option<&str> {
    source.lines().find_map(|line| {
        let rest = line.split_once(VERSION_MARKER)?.1;
        let rest = rest.trim_start().trim_start_matches([':', '=']).trim();
        let digits = rest.strip_prefix('v').unwrap_or(rest);
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        // Leading zeros are normalised so `v007` and `7` compare equal, with
        // `0` kept as `0`.
        let trimmed = digits.trim_start_matches('0');
        Some(if trimmed.is_empty() { "0" } else { trimmed })
    })
}

/// Drop `dir` and the empty directories above it, stopping after removing an
/// empty `plugins/` — the directory this install may have created.
///
/// Best effort by design, as in `connect_ext`: a `DirectoryNotEmpty` or a
/// permission error means somebody else is using that directory, and neither
/// is worth failing an uninstall over. Bounded at four so a path that somehow
/// never reaches `plugins/` cannot walk an arbitrary chain upward.
fn prune_empty_dirs(dir: &Path) {
    let mut cursor = dir;
    for _ in 0..4 {
        if std::fs::remove_dir(cursor).is_err() {
            return;
        }
        if cursor.file_name().is_some_and(|n| n == std::ffi::OsStr::new("plugins")) {
            break;
        }
        match cursor.parent() {
            Some(p) => cursor = p,
            None => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-ocplug-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp home");
        dir
    }

    fn read_plugin(home: &Path) -> String {
        std::fs::read_to_string(home.join(plugin_rel(Host::Opencode).expect("rel"))).expect("plugin")
    }

    #[test]
    fn the_embedded_source_is_the_one_on_disk() {
        assert_eq!(
            SOURCE,
            std::fs::read_to_string(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/plugin/integrations/opencode-plugin/memory-wire.ts"
                ))
            .expect("read"),
            "edit the source in plugin/integrations/opencode-plugin/; the binary is compiled with that"
        );
    }

    #[test]
    fn install_places_a_flat_plugin_file_in_opencodes_plugins_dir() {
        let home = tmp_home("install");
        let out = apply(Host::Opencode, &home, false, &home.join("data"), None);
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        assert_eq!(read_plugin(&home), SOURCE);
        assert!(installed(Host::Opencode, &home));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_reinstall_with_the_same_bank_is_a_no_op_and_a_different_bank_is_a_replace() {
        let home = tmp_home("bank-idem");
        apply(Host::Opencode, &home, false, &home.join("data"), Some("omp"));
        let out = apply(Host::Opencode, &home, false, &home.join("data"), Some("omp"));
        assert!(matches!(out, Outcome::Already { .. }), "{out:?}");
        let out = apply(Host::Opencode, &home, false, &home.join("data"), Some("other"));
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        assert!(read_plugin(&home).contains("var DEFAULT_BANK = \"other\";"));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_connect_bank_is_baked_into_the_default_bank() {
        let home = tmp_home("bank-baked");
        apply(Host::Opencode, &home, false, &home.join("data"), Some("omp"));
        let file = read_plugin(&home);
        assert!(file.contains("var DEFAULT_BANK = \"omp\";"), "bank baked: {file}");
        assert!(file.contains(VERSION_MARKER));
        // And a reconnect without a bank is only ever a re-render: the file
        // stays ours, and the version still parses.
        assert_eq!(version(&file), Some("1"));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_foreign_plugin_file_is_refused_and_left_byte_identical() {
        let home = tmp_home("foreign");
        let path = home.join(plugin_rel(Host::Opencode).expect("rel"));
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, "// my own plugin\n").expect("foreign");
        let out = apply(Host::Opencode, &home, false, &home.join("data"), None);
        assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "// my own plugin\n");
        // And uninstall is refused too, so neither direction can harm it.
        let out = apply(Host::Opencode, &home, true, &home.join("data"), None);
        assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
        assert!(path.is_file());
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn uninstall_removes_only_ours() {
        let home = tmp_home("uninstall");
        apply(Host::Opencode, &home, false, &home.join("data"), None);
        let path = home.join(plugin_rel(Host::Opencode).expect("rel"));
        assert!(path.is_file());
        let out = apply(Host::Opencode, &home, true, &home.join("data"), None);
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        assert!(!path.exists(), "removed");
        assert!(!path.parent().expect("parent").exists(), "empty plugins/ dir pruned");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn other_hosts_have_no_plugin_surface() {
        let home = tmp_home("skip");
        let out = apply(Host::ClaudeCode, &home, false, &home.join("data"), None);
        assert!(matches!(out, Outcome::Skipped(_)), "{out:?}");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_marker_line_with_a_mangled_version_reports_none() {
        assert_eq!(version(&format!("// {VERSION_MARKER}\n")), None);
        assert_eq!(version(&format!("// {VERSION_MARKER} 3\n")), Some("3"));
        assert_eq!(version(&format!("// {VERSION_MARKER} v004\n")), Some("4"));
    }
}
