//! `connect --with-skills`: deploy the memory-wire agent skill into each wired
//! host's skills directory.
//!
//! # Why this exists
//!
//! `connect` writes hooks and MCP entries, and until now it never deployed
//! `plugin/skills/memory-wire/SKILL.md` — the prose that tells a model what
//! memory-wire *is* and when to reach for it. The hooks and MCP entries make the
//! tools callable; the skill makes them *known*. A host with the entries but not
//! the skill has a memory the model never thinks to look for.
//!
//! # Defensive write, same as every other `connect` surface
//!
//! - **Never corrupt.** The embedded template is the source of truth. A file at
//!   the target path whose bytes already match it is left alone (idempotent); a
//!   file whose bytes differ is **refused**, because it is somebody's edited
//!   copy and overwriting it would silently discard their changes. A refusal
//!   names the path and says why.
//! - **Atomic.** The write goes through [`crate::paths::write_atomic`]
//!   (temp file + rename + re-read verify), so a crash mid-write cannot leave a
//!   half-written skill.
//! - **Reversible.** `--uninstall` removes only a file whose bytes this build
//!   wrote, i.e. one that matches the embedded template. A modified copy is
//!   left in place on uninstall for the same reason it is refused on install.
//!
//! # Hosts without a skills tree are skipped, not guessed at
//!
//! `Host::skills_rel` returns `None` for OMP, Hermes and pi: they load plugins
//! or extensions, not a `skills/<name>/SKILL.md` directory. Writing a skill file
//! where nothing reads it would report success and never load.

use std::path::{Path, PathBuf};

use crate::connect::{backup, timestamp, Host};
use crate::paths;

/// The skill, embedded at build time.
///
/// Source of truth is `plugin/skills/memory-wire/SKILL.md`. `include_str!` means
/// the binary carries the bytes that ship, and a test asserts the directory and
/// the embedded string agree — a file in the directory but not here would
/// install an incomplete tree and pass every other test.
pub(crate) const SKILL: &str = include_str!("../plugin/skills/memory-wire/SKILL.md");

/// The directory name under the host's skills root for this skill.
const SKILL_DIR: &str = "memory-wire";

/// What one host's skill write did.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// The file did not exist and now holds the embedded template.
    Created,
    /// The file already held exactly these bytes.
    Unchanged,
    /// Refused: a file existed with different bytes, and was left untouched.
    Refused(String),
}

impl Applied {
    /// One report line, shaped like the other `connect` lines.
    pub fn line(&self, host: Host) -> String {
        let rel = match host.skills_rel() {
            Some((dir, file)) => format!("{}/{SKILL_DIR}/{file}", dir),
            None => return format!("{:<20} skipped (no skills directory)", host.id()),
        };
        match self {
            Applied::Created => format!("{rel:<48} created"),
            Applied::Unchanged => format!("{rel:<48} unchanged"),
            Applied::Refused(why) => format!("{rel:<48} REFUSED: {why}"),
        }
    }
}

/// The path this skill installs to for `host`, or `None` when the host has no
/// skills directory.
pub fn skill_path(host: Host, home: &Path) -> Option<PathBuf> {
    let (dir, file) = host.skills_rel()?;
    Some(home.join(dir).join(SKILL_DIR).join(file))
}

/// Install the skill into `host`'s skills directory under `home`.
///
/// `data_root` is the XDG data directory, used for the backup on first write.
/// Returns what happened; a refusal is not a failure of the whole run, so the
/// caller can continue to the next host.
pub fn apply(host: Host, home: &Path, data_root: &Path, uninstall: bool) -> Applied {
    let Some(path) = skill_path(host, home) else {
        // A host with no skills tree. Not a failure: `--with-skills` over every
        // host should skip the ones it cannot serve rather than error.
        return Applied::Unchanged;
    };

    if uninstall {
        return remove(host, home, &path);
    }

    let existing = match paths::read_or_empty(&path) {
        Ok(s) => s,
        Err(e) => return Applied::Refused(format!("{}: {e}", path.display())),
    };

    if existing == SKILL {
        return Applied::Unchanged;
    }
    if !existing.is_empty() {
        // A non-empty file with different bytes is somebody's edited copy. We
        // never clobber it, and we say so by name.
        return Applied::Refused(format!(
            "{} exists and differs from the memory-wire template; left untouched",
            path.display()
        ));
    }

    // First write: preserve nothing (the file is empty or absent), then write
    // atomically and re-read verify.
    if let Err(why) = paths::write_atomic(&path, SKILL) {
        return Applied::Refused(format!("{}: {why}", path.display()));
    }
    // `backup` takes a path relative to `home`; the skill path is absolute, so
    // strip the home prefix. A file that was empty or absent has nothing to
    // preserve, so a strip failure here is not an error worth failing the write
    // over — the write already landed atomically and verified.
    if let Ok(rel) = path.strip_prefix(home) {
        let _ = backup(data_root, home, host, &timestamp(), rel);
    }
    Applied::Created
}

/// Remove the skill file on `--uninstall`, but only if it is one this build
/// wrote (bytes match the embedded template). A modified copy is left alone.
fn remove(_host: Host, _home: &Path, path: &Path) -> Applied {
    let existing = match paths::read_or_empty(path) {
        Ok(s) => s,
        Err(e) => return Applied::Refused(format!("{}: {e}", path.display())),
    };
    if existing.is_empty() {
        return Applied::Unchanged;
    }
    if existing != SKILL {
        return Applied::Refused(format!(
            "{} differs from the memory-wire template; left in place",
            path.display()
        ));
    }
    match std::fs::remove_file(path) {
        Ok(()) => Applied::Created, // "created" and "removed" both report as a change; the line is the same shape
        Err(e) => Applied::Refused(format!("{}: {e}", path.display())),
    }
}

/// Write the skill into every host in `hosts`, returning whether any write was
/// refused (the caller's exit code).
pub fn apply_all(hosts: &[Host], home: &Path, data_root: &Path, uninstall: bool) -> bool {
    let mut failed = false;
    for host in hosts {
        let applied = apply(*host, home, data_root, uninstall);
        if matches!(applied, Applied::Refused(_)) {
            failed = true;
        }
        println!("{}", applied.line(*host));
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_home() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "mw-skills-{}-{}",
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create home");
        dir
    }

    #[test]
    fn skill_creates_then_is_a_no_op() {
        let home = fresh_home();
        let data = home.join("data");
        let host = Host::ClaudeCode;

        let first = apply(host, &home, &data, false);
        assert_eq!(first, Applied::Created, "first write creates the file");

        let path = skill_path(host, &home).expect("claude has a skills dir");
        let on_disk = std::fs::read_to_string(&path).expect("read back");
        assert_eq!(on_disk, SKILL, "the installed bytes are the template");

        let second = apply(host, &home, &data, false);
        assert_eq!(second, Applied::Unchanged, "second run is a no-op");
    }

    #[test]
    fn skill_refuses_a_modified_copy() {
        let home = fresh_home();
        let data = home.join("data");
        let host = Host::Codex;

        let path = skill_path(host, &home).expect("codex has a skills dir");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, "# my edited skill\n").expect("seed");

        let applied = apply(host, &home, &data, false);
        assert!(matches!(applied, Applied::Refused(_)), "got {applied:?}");
        let after = std::fs::read_to_string(&path).expect("read back");
        assert_eq!(after, "# my edited skill\n", "the edited copy is untouched");
    }

    #[test]
    fn skill_hosts_without_a_tree_are_skipped() {
        let home = fresh_home();
        let data = home.join("data");
        for host in [Host::Omp, Host::Hermes, Host::Pi] {
            assert!(host.skills_rel().is_none(), "{} has no skills tree", host.id());
            let applied = apply(host, &home, &data, false);
            assert_eq!(applied, Applied::Unchanged);
        }
    }

    #[test]
    fn uninstall_removes_only_our_copy() {
        let home = fresh_home();
        let data = home.join("data");
        let host = Host::Cursor;

        apply(host, &home, &data, false);
        let path = skill_path(host, &home).expect("cursor has a skills dir");
        assert!(path.exists(), "the file was written");

        apply(host, &home, &data, true);
        assert!(!path.exists(), "uninstall removed our copy");
    }

    #[test]
    fn uninstall_leaves_a_modified_copy_alone() {
        let home = fresh_home();
        let data = home.join("data");
        let host = Host::CopilotCli;
        let path = skill_path(host, &home).expect("copilot has a skills dir");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, "# edited\n").expect("seed");

        apply(host, &home, &data, true);
        assert!(path.exists(), "a modified copy is left in place on uninstall");
    }

    #[test]
    fn the_embedded_source_is_the_one_on_disk() {
        let disk = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plugin/skills/memory-wire/SKILL.md"
        ))
        .expect("read SKILL.md");
        assert_eq!(SKILL, disk, "include_str! and the file on disk must agree");
    }
}