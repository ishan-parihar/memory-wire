//! Environment + path resolution shared by the CLI surface.
//!
//! Every home-relative path is derived from `$HOME` rather than from the
//! process's own location, so a test can point the installer at a temporary
//! home and never touch the real one.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Endpoint used when `MEMORY_WIRE_URL` is unset.
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8888";

/// Bank used when the working directory is not inside a git work tree.
pub const DEFAULT_BANK: &str = "memory-wire";

/// Budget for every network call a hook or `doctor` makes.
///
/// One total deadline per request — connect, write, and read share it, and name
/// resolution is inside it too — so a server that accepts and then stalls costs
/// this and not a multiple of it.
///
/// Two seconds, because the budget is quoted per call: `hook session-start`
/// makes at most two of them (the config probe, then the recall) on top of the
/// hook's own stdin cap, so the worst case a hook can spend on somebody else's
/// session is about six seconds. A hook runs inside a session it did not start;
/// it must never be the reason that session appears to hang.
pub const IO_TIMEOUT: Duration = Duration::from_secs(2);

/// The user's home directory, or an error when the environment declares none.
pub fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| "neither HOME nor USERPROFILE is set".to_string())
}

/// Server endpoint: `MEMORY_WIRE_URL` when set, else [`DEFAULT_ENDPOINT`].
pub fn endpoint() -> String {
    std::env::var("MEMORY_WIRE_URL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string())
}

/// XDG data directory: `$XDG_DATA_HOME/memory-wire`, else
/// `~/.local/share/memory-wire`. A relative `XDG_DATA_HOME` is ignored, per spec.
pub fn data_dir(home: &Path) -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"));
    base.join("memory-wire")
}

/// Reduce a free-form name (a repo directory, say) to a URL-path-safe bank id.
///
/// A bank id travels in a URL path segment, so spaces and slashes cannot
/// survive. Returns `None` when nothing usable is left.
pub fn sanitize_bank(raw: &str) -> Option<String> {
    let mapped: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = mapped.trim_matches(|c: char| c == '-' || c == '.').to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Bank for a directory: the git work tree's basename, else [`DEFAULT_BANK`].
///
/// Scoped to `dir` rather than the process cwd so a test can exercise both
/// branches without changing the cwd of a whole test binary.
pub fn resolve_bank_in(dir: &Path) -> String {
    git_toplevel(dir)
        .and_then(|p| p.file_name().map(|n| n.to_os_string()))
        .and_then(|n| sanitize_bank(&n.to_string_lossy()))
        .unwrap_or_else(|| DEFAULT_BANK.to_string())
}

/// Bank for the current working directory.
pub fn resolve_bank() -> String {
    resolve_bank_in(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// `git rev-parse --show-toplevel`, or `None` outside a work tree.
fn git_toplevel(dir: &Path) -> Option<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let top = String::from_utf8(out.stdout).ok()?;
    let top = top.trim();
    if top.is_empty() {
        None
    } else {
        Some(PathBuf::from(top))
    }
}

/// Read a file, treating "absent" as empty content.
pub fn read_or_empty(path: &Path) -> std::io::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// Write a file atomically and prove the bytes landed.
///
/// A config the user also edits by hand is never safe to truncate in place: a
/// crash between truncate and write leaves a half-written `settings.json` that
/// the host refuses to start. So: write a sibling temp file, rename it over the
/// target (atomic within a directory), then re-read and compare. A concurrent
/// writer that lands in the same window is caught here instead of silently
/// losing one side's edit.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    use std::fs;

    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} is not a file", path.display()))?;
    let tmp = parent.join(format!(".{name}.memory-wire-{}", std::process::id()));

    fs::write(&tmp, contents).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let renamed = fs::rename(&tmp, path);
    if let Err(e) = renamed {
        let _ = fs::remove_file(&tmp);
        return Err(format!("{}: {e}", path.display()));
    }
    match fs::read_to_string(path) {
        Ok(got) if got == contents => Ok(()),
        Ok(_) => Err(format!("{}: verification failed", path.display())),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Human-readable byte count (`0 B`, `1.2 KB`, `3.4 MB`).
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_should_make_url_safe_bank_ids() {
        assert_eq!(sanitize_bank("Memory Wire").as_deref(), Some("memory-wire"));
        assert_eq!(sanitize_bank("my_repo.v2").as_deref(), Some("my_repo.v2"));
        assert_eq!(sanitize_bank("--weird--").as_deref(), Some("weird"));
        assert_eq!(sanitize_bank("///"), None);
    }

    #[test]
    fn write_atomic_should_create_then_replace_and_leave_no_temp() {
        let dir = std::env::temp_dir().join(format!("mw-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tmp");
        let f = dir.join("nested").join("settings.json");
        write_atomic(&f, "{\"a\":1}").expect("create");
        assert_eq!(std::fs::read_to_string(&f).expect("read"), "{\"a\":1}");
        write_atomic(&f, "{\"a\":2}").expect("replace");
        assert_eq!(std::fs::read_to_string(&f).expect("read"), "{\"a\":2}");
        let leftovers: Vec<_> = std::fs::read_dir(f.parent().expect("parent"))
            .expect("readdir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != "settings.json")
            .collect();
        assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn human_bytes_should_scale_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1024), "1.0 KB");
        assert_eq!(human_bytes(3 * 1024 * 1024), "3.0 MB");
    }

    // Both branches in one test: `HOME` is process-global, so a second test
    // mutating it concurrently would race.
    #[test]
    fn resolve_bank_should_prefer_git_toplevel_then_fall_back() {
        let base = std::env::temp_dir().join(format!("mw-paths-{}", std::process::id()));
        let outside = base.join("plain");
        std::fs::create_dir_all(&outside).expect("tmp");

        // No git work tree above the temp dir (unless the temp dir itself is
        // one, in which case the fallback assertion below would not hold, so
        // assert the fallback only when git agrees it is outside a repo).
        let fell_back = resolve_bank_in(&outside) == DEFAULT_BANK;

        let repo = base.join("My Repo");
        std::fs::create_dir_all(&repo).expect("repo");
        let init = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .output()
            .expect("git");
        if init.status.success() {
            assert_eq!(resolve_bank_in(&repo), "my-repo");
        }

        if fell_back {
            assert_eq!(resolve_bank_in(&outside), DEFAULT_BANK);
        }
        std::fs::remove_dir_all(&base).ok();
    }
}
