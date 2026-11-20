//! opencode's second install surface: a plugin it auto-loads from
//! `~/.config/opencode/plugins/`, mirroring what [`crate::connect_ext`] does
//! for omp and pi.
//!
//! **The layout changed with OpenCode 2.x, and the old one was silently dead.**
//! 1.x auto-loaded a flat `plugins/*.ts`, verified against 1.18.34 by a live
//! probe (2026-10-07). 2.x discovers local plugins as *directories* and reads
//! `<dir>/index.ts`; a flat file is skipped with no error and no log line at
//! all (verified live on 2.3.2). So an opencode upgraded to 2.x kept reporting
//! `wired` from `doctor`, kept its config, and quietly had no memory plugin --
//! the failure mode this file is most careful about everywhere else.
//!
//! The port that goes with the layout is in the plugin itself: 2.x also
//! changed the module contract from a bare `async (input) => hooks` function to
//! `{ id, setup(ctx) }` (see `plugin/integrations/opencode-plugin/memory-wire.ts`
//! and `opencode.ai/docs/build/plugins/migrate-v1`). Both halves move together
//! -- the right file at the wrong path does not load, and the right path with a
//! v1 export fails the v2 loader's schema.
//!
//! The MCP entry `connect` already writes is the tools the model may call; the
//! plugin is the recall it does not have to ask for -- on a continuation prompt
//! it never looks, which is exactly why the injection has to exist (a
//! forked-session test showed the model making zero recall calls across the
//! turn).
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
/// Install or uninstall opencode's plugin file. See the module docs.
///
/// A `None` from [`plugin_rel`] means the host has no plugin surface, and that
/// is a [`Outcome::Skipped`] rather than a failure: nothing to get wrong.
pub fn apply(
    host: Host,
    home: &Path,
    uninstall: bool,
    data_root: &Path,
    bank: Option<&str>,
) -> Outcome {
    let Some(rel) = plugin_rel(host) else {
        return Outcome::Skipped("no opencode plugin surface for this host".to_string());
    };
    let path = home.join(&rel);
    let probe = crate::connect_common::baked(SOURCE, DEFAULT_BANK_LINE, bank);
    let current = version(&probe).unwrap_or("?");

    // The 1.x layout. Removed on the way in as well as out: a flat
    // `plugins/MEMORY_WIRE_OPENCODE_PLUGIN.ts` left beside the new directory
    // is not merely dead, it is a user-visible answer to "is memory-wire
    // installed here" that is wrong.
    let legacy_path = home.join(LEGACY_PLUGIN_REL);
    let mut removed_legacy = false;
    if legacy_path.is_file() && std::fs::remove_file(&legacy_path).is_ok() {
        removed_legacy = true;
    }

    if uninstall {
        if !path.is_file() {
            return if removed_legacy {
                Outcome::Unwired {
                    backup: None,
                    events: vec![LEGACY_PLUGIN_REL.to_string()],
                    notes: vec!["removed the pre-v2 flat plugin".to_string()],
                }
            } else {
                Outcome::Skipped("no memory-wire plugin to remove".to_string())
            };
        }
        if !crate::connect_common::is_ours(&path, VERSION_MARKER) {
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
    // Same pin-preservation rule as `connect_ext` (TODO #1): a bare re-wire
    // reads the bank the installed plugin was baked with and keeps it, so a
    // pinned machine does not silently split between the plugin's bank and
    // the hooks' bank.
    let baked_pin = existing
        .as_deref()
        .and_then(crate::connect::bank_from_extension);
    let bank = bank.or(baked_pin.as_deref());
    let source = crate::connect_common::baked(SOURCE, DEFAULT_BANK_LINE, bank);
    if let Some(have) = existing.as_deref() {
        if !crate::connect_common::carries_marker(have, VERSION_MARKER) {
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
    if removed_legacy {
        notes.push("removed the pre-v2 flat plugin".to_string());
    }
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

/// Where opencode's plugin goes, home-relative, or `None` for a host that has
/// no plugin surface.
///
/// **A directory, not a file, and that is a v2 requirement rather than a
/// preference.** OpenCode 1.x auto-loaded a flat `plugins/*.ts`; 2.x discovers
/// local plugins as directories and reads `<dir>/index.ts`. A flat file is
/// skipped entirely (verified live on 2.3.2: it never appears in
/// `msg="loading plugin"`), which is the second half of the same break as the
/// plugin-API port in `memory-wire.ts`. `connect` writes no config entry, so
/// the directory is the whole registration.
///
/// The old flat path is [`LEGACY_PLUGIN_REL`] and is removed on upgrade or
/// uninstall, because a stale `plugins/MEMORY_WIRE_OPENCODE_PLUGIN.ts` beside
/// the new directory is dead weight that a user cannot tell from a live
/// install.
pub fn plugin_rel(host: Host) -> Option<PathBuf> {
    match host {
        Host::Opencode => Some(
            PathBuf::from(".config")
                .join("opencode")
                .join("plugins")
                .join(MARKER)
                .join("index.ts"),
        ),
        _ => None,
    }
}

/// The pre-v2 layout, kept only so an upgrade can remove it.
///
/// 1.x wrote `plugins/MEMORY_WIRE_OPENCODE_PLUGIN.ts`. Nothing loads that
/// shape on 2.x, so leaving it behind would leave a file that looks installed
/// and does nothing.
const LEGACY_PLUGIN_REL: &str = ".config/opencode/plugins/MEMORY_WIRE_OPENCODE_PLUGIN.ts";

/// Does this host carry our plugin file? Reads, and never writes.
///
/// For opencode the caller ORs this with the MCP entry rather than ANDing it:
/// an opencode wired by an earlier build has only the MCP key, and reporting
/// `wired=no` there would be a false negative.
pub fn installed(host: Host, home: &Path) -> bool {
    plugin_rel(host)
        .is_some_and(|rel| crate::connect_common::is_ours(&home.join(rel), VERSION_MARKER))
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
        if cursor
            .file_name()
            .is_some_and(|n| n == std::ffi::OsStr::new("plugins"))
        {
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
        std::fs::read_to_string(home.join(plugin_rel(Host::Opencode).expect("rel")))
            .expect("plugin")
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
    fn install_places_a_plugin_directory_with_index_ts() {
        let home = tmp_home("install");
        let out = apply(Host::Opencode, &home, false, &home.join("data"), None);
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        assert_eq!(read_plugin(&home), SOURCE);
        assert!(installed(Host::Opencode, &home));
        // The file is at `plugins/memory-wire/index.ts`, and opencode 2.x reads
        // a directory's index rather than a flat file: the old shape is
        // skipped silently, which is the failure this layout exists to fix.
        let rel = plugin_rel(Host::Opencode).expect("rel");
        assert_eq!(
            rel.file_name().and_then(|n| n.to_str()),
            Some("index.ts"),
            "{rel:?}"
        );
        assert_eq!(
            rel.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str()),
            Some(MARKER)
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn an_upgrade_removes_the_pre_v2_flat_plugin() {
        let home = tmp_home("legacy");
        // What 1.x wrote: a flat file no 2.x loader reads.
        std::fs::create_dir_all(home.join(LEGACY_PLUGIN_REL).parent().expect("parent"))
            .expect("mkdir");
        std::fs::write(home.join(LEGACY_PLUGIN_REL), "// memory-wire\n").expect("seed");
        let out = apply(Host::Opencode, &home, false, &home.join("data"), None);
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        assert!(
            !home.join(LEGACY_PLUGIN_REL).exists(),
            "the stale flat file is gone"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_reinstall_with_the_same_bank_is_a_no_op_and_a_different_bank_is_a_replace() {
        let home = tmp_home("bank-idem");
        apply(
            Host::Opencode,
            &home,
            false,
            &home.join("data"),
            Some("omp"),
        );
        let out = apply(
            Host::Opencode,
            &home,
            false,
            &home.join("data"),
            Some("omp"),
        );
        assert!(matches!(out, Outcome::Already { .. }), "{out:?}");
        let out = apply(
            Host::Opencode,
            &home,
            false,
            &home.join("data"),
            Some("other"),
        );
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        assert!(read_plugin(&home).contains("var DEFAULT_BANK = \"other\";"));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_connect_bank_is_baked_into_the_default_bank() {
        let home = tmp_home("bank-baked");
        apply(
            Host::Opencode,
            &home,
            false,
            &home.join("data"),
            Some("omp"),
        );
        let file = read_plugin(&home);
        assert!(
            file.contains("var DEFAULT_BANK = \"omp\";"),
            "bank baked: {file}"
        );
        assert!(file.contains(VERSION_MARKER));
        // And a reconnect without a bank is only ever a re-render: the file
        // stays ours, and the version still parses.
        assert_eq!(version(&file), Some("3"));
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
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "// my own plugin\n"
        );
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
        assert!(
            !path.parent().expect("parent").exists(),
            "empty plugins/ dir pruned"
        );
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
