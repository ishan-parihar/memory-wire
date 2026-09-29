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

/// Bank for a directory, with an explicit override ahead of everything else.
///
/// This is the whole resolution ladder, in the order [`resolve_bank_from`]
/// documents. The `hook` subcommand's `--bank <id>` reaches here as `explicit`,
/// which is the escape hatch that did not exist before: neither
/// `MEMORY_WIRE_BANK` nor any derivation can be beaten from the hook path
/// except by naming the bank.
pub fn resolve_bank_with(explicit: Option<&str>, dir: &Path) -> String {
    let explicit = explicit
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(sanitize_bank);
    if let Some(bank) = explicit {
        return bank;
    }
    env_bank()
        .or_else(|| remote_bank_in(dir))
        .unwrap_or_else(|| basename_bank_in(dir).unwrap_or_else(|| DEFAULT_BANK.to_string()))
}

/// Bank for the current working directory, no explicit override.
pub fn resolve_bank() -> String {
    resolve_bank_with(
        None,
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    )
}

/// Bank for a directory, with no explicit override.
///
/// Scoped to `dir` rather than the process cwd so a test can exercise every
/// branch without changing the cwd of a whole test binary.
///
/// The order, and why it is this order:
///
/// 1. an explicit `--bank <id>` (see [`resolve_bank_with`]);
/// 2. `MEMORY_WIRE_BANK`;
/// 3. `owner/repo` from the repository's `origin` remote;
/// 4. the work tree's top-level basename;
/// 5. [`DEFAULT_BANK`].
///
/// Step 3 is what makes two unrelated projects stop sharing a namespace: the
/// basename alone collides across every parent directory, so `/home/x/api` and
/// `/home/y/api` were both bank `api`, and a repository you merely cloned could
/// read and write the other one's memories. Step 4 stays because a repository
/// with no remote, or a remote with no `owner/repo` path, has nothing better to
/// offer than what it always did.
pub fn resolve_bank_in(dir: &Path) -> String {
    resolve_bank_with(None, dir)
}

/// `MEMORY_WIRE_BANK`, when it is set to something that survives [`sanitize_bank`].
fn env_bank() -> Option<String> {
    std::env::var("MEMORY_WIRE_BANK")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .and_then(|v| sanitize_bank(&v))
}

/// The bank a *pre-0.4.0* build would have derived for `dir`: the work tree's
/// basename, else [`DEFAULT_BANK`].
///
/// Kept as its own function because it is what `doctor` compares the current
/// derivation against to warn about memories left under the old name, and it is
/// the name `--bank` / `MEMORY_WIRE_BANK` can be pointed at to keep using them.
pub fn legacy_bank_in(dir: &Path) -> String {
    basename_bank_in(dir).unwrap_or_else(|| DEFAULT_BANK.to_string())
}

/// Work tree top-level basename, sanitised.
fn basename_bank_in(dir: &Path) -> Option<String> {
    worktree_root(dir)
        .and_then(|p| p.file_name().map(|n| n.to_os_string()))
        .and_then(|n| sanitize_bank(&n.to_string_lossy()))
}

/// `owner-repo` from the `origin` remote of the work tree containing `dir`.
fn remote_bank_in(dir: &Path) -> Option<String> {
    let root = worktree_root(dir)?;
    let config = git_dir(&root)?.join("config");
    let text = std::fs::read_to_string(config).ok()?;
    let url = origin_url(&text)?;
    let owner_repo = owner_repo_from_url(url)?;
    sanitize_bank(&owner_repo)
}

/// The work tree root above `dir`, or `None` when there is no `.git` anywhere up.
///
/// An ancestor walk rather than `git rev-parse --show-toplevel`, and the reason
/// is the same one that shapes the whole of this file's git handling: a spawned
/// `git` collapses every failure into one answer. A missing binary, a timeout,
/// a fork that returns `EAGAIN`, and a directory that simply is not a
/// repository all come back as the same "no", and a hook cannot tell a
/// transient failure from the real thing. The filesystem says which one it was.
/// (This is hindsight's `coding-agents/src/core/git-layout.ts:1-13`, where the
/// `git rev-parse` spawn they removed is documented as having done exactly that.)
fn worktree_root(dir: &Path) -> Option<PathBuf> {
    let mut cur = Some(dir);
    while let Some(d) = cur {
        if d.join(".git").exists() {
            return Some(d.to_path_buf());
        }
        cur = d.parent();
    }
    None
}

/// The real git directory for a work tree root.
///
/// Normally `<root>/.git`, but a worktree or a submodule has a `.git` *file*
/// holding `gitdir: <path>`. That path is resolved one level and no further:
/// a linked worktree's gitdir has no `config` of its own, it points at the
/// parent repository's through `commondir`, and chasing that chain is how a
/// resolver ends up confidently reading the wrong repository. When it does not
/// resolve, [`remote_bank_in`] yields `None` and the caller falls back to the
/// basename — a fallback that is merely less specific, never wrong.
fn git_dir(root: &Path) -> Option<PathBuf> {
    let dot_git = root.join(".git");
    let meta = std::fs::metadata(&dot_git).ok()?;
    if meta.is_dir() {
        return Some(dot_git);
    }
    if !meta.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(&dot_git).ok()?;
    let target = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("gitdir:"))
        .map(str::trim)
        .filter(|t| !t.is_empty())?;
    let path = Path::new(target);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    })
}

/// The `url` of `[remote "origin"]` in a git config file's text.
///
/// Plain string handling, no config parser and no `git config` call. The file
/// is INI-shaped: a line starting with `[` opens a section, `key = value`
/// carries a key, and everything after the first `=` is the value.
fn origin_url(config: &str) -> Option<&str> {
    let mut in_origin = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_origin = is_origin_section(line);
            continue;
        }
        if !in_origin {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim().eq_ignore_ascii_case("url") {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

/// Whether a `[…]` header names `remote "origin"`.
///
/// Accepts both spellings git accepts: `remote "origin"` (what git writes) and
/// the older `remote.origin`.
fn is_origin_section(header: &str) -> bool {
    let inner = header
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let Some((key, sub)) = inner
        .split_once('.')
        .or_else(|| inner.split_once(char::is_whitespace))
    else {
        return false;
    };
    key.trim().eq_ignore_ascii_case("remote")
        && sub.trim().trim_matches('"').eq_ignore_ascii_case("origin")
}

/// The last two path components of a git remote URL, as `owner/repo`.
///
/// Handles the four forms real remotes come in — `https://host/o/r.git`,
/// `git@host:o/r.git`, `ssh://git@host/o/r.git`, and the same without the
/// `.git` suffix — by throwing away the scheme (or the scp-style `user@host:`)
/// and keeping the tail. `None` when the URL has no `owner/repo` to keep, so
/// the caller falls back to the basename rather than inventing a name.
fn owner_repo_from_url(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/');
    // Two shapes, and they put the path in different places.
    //
    // `https://host/o/r`, `ssh://git@host/o/r` — the scheme is before `://`
    // and the authority runs from there to the first `/`, so the path is
    // everything after that slash.
    //
    // `git@host:o/r` — scp syntax, no scheme at all. The last `:` is what
    // separates host from path, and what follows it IS the path: there is no
    // authority left to drop, and treating the `o` in `owner` as one is how
    // this read `acme/api` as the single component `api`.
    let path = match url.split_once("://") {
        Some((_, rest)) => rest.split_once('/')?.1,
        None => url.rsplit_once(':')?.1,
    };
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 {
        return None;
    }
    Some(format!("{}/{}", segs[segs.len() - 2], segs[segs.len() - 1]))
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

    /// A throwaway directory, removed when the returned guard drops.
    ///
    /// `TempDir` would be the obvious choice and is deliberately not used: the
    /// crate takes no dev-dependencies, and a five-line drop-guard is cheaper
    /// than one.
    struct Tmp(PathBuf);

    impl Tmp {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("mw-{tag}-{}", std::process::id()));
            std::fs::remove_dir_all(&p).ok();
            std::fs::create_dir_all(&p).expect("tmp dir");
            Self(p)
        }

        /// A git work tree with this `origin` URL, at `name` under the root.
        fn repo(&self, name: &str, origin: &str) -> PathBuf {
            let dir = self.0.join(name);
            let git = dir.join(".git");
            std::fs::create_dir_all(&git).expect("git dir");
            std::fs::write(
                git.join("config"),
                format!(
                    "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = {origin}\n\t\
                     fetch = +refs/heads/*:refs/remotes/origin/*\n"
                ),
            )
            .expect("config");
            dir
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    /// The bug, stated as a test: two unrelated projects that happen to share a
    /// directory name shared a bank, and a repository you merely cloned could
    /// read and write the other one's memories. Same basename, different
    /// parents, different remotes — so a basename-derived id cannot tell them
    /// apart, and this fails on the pre-fix code.
    #[test]
    fn same_basename_under_different_parents_should_not_share_a_bank() {
        let t = Tmp::new("paths-collide");
        let x = t.repo("x/api", "https://github.com/acme/api.git");
        let y = t.repo("y/api", "https://github.com/other/api.git");

        let bx = resolve_bank_in(&x);
        let by = resolve_bank_in(&y);
        assert_eq!(bx, "acme-api");
        assert_eq!(by, "other-api");
        assert_ne!(bx, by, "two projects share one memory namespace");

        // Same directories, pre-fix naming: this is exactly the collision.
        assert_eq!(legacy_bank_in(&x), legacy_bank_in(&y));
    }

    #[test]
    fn an_explicit_bank_should_beat_everything_else() {
        let t = Tmp::new("paths-explicit");
        let repo = t.repo("api", "https://github.com/acme/api.git");
        assert_eq!(resolve_bank_with(Some("chosen"), &repo), "chosen");
        // An unusable override falls through rather than becoming a bank id that
        // cannot survive a URL path segment.
        assert_eq!(resolve_bank_with(Some("   "), &repo), "acme-api");
        assert_eq!(resolve_bank_with(Some("///"), &repo), "acme-api");
    }

    // `MEMORY_WIRE_BANK` is process-global, so every assertion about it rides in
    // this one test — a second test mutating it concurrently would race.
    #[test]
    fn the_env_var_should_beat_derivation_but_lose_to_an_explicit_bank() {
        let t = Tmp::new("paths-env");
        let repo = t.repo("api", "https://github.com/acme/api.git");
        let outside = t.0.join("plain");
        std::fs::create_dir_all(&outside).expect("plain dir");

        // Absent: derivation stands, both for a repo and for a bare directory.
        assert_eq!(resolve_bank_with(None, &repo), "acme-api");
        assert_eq!(resolve_bank_with(None, &outside), DEFAULT_BANK);

        std::env::set_var("MEMORY_WIRE_BANK", "from-env");
        assert_eq!(resolve_bank_with(None, &repo), "from-env");
        assert_eq!(resolve_bank_with(None, &outside), "from-env");
        assert_eq!(resolve_bank_with(Some("explicit"), &repo), "explicit");

        // Blank and unusable values are "not set", not a bank called "///".
        std::env::set_var("MEMORY_WIRE_BANK", "  ");
        assert_eq!(resolve_bank_with(None, &repo), "acme-api");
        std::env::set_var("MEMORY_WIRE_BANK", "///");
        assert_eq!(resolve_bank_with(None, &repo), "acme-api");

        std::env::remove_var("MEMORY_WIRE_BANK");
    }

    #[test]
    fn every_remote_url_form_should_yield_the_same_owner_repo() {
        for url in [
            "https://github.com/acme/api.git",
            "git@github.com:acme/api.git",
            "ssh://git@github.com/acme/api.git",
            "ssh://git@github.com:2222/acme/api.git",
            "https://github.com/acme/api",
            "git@gitlab.example.com:acme/api.git",
            "https://github.com/acme/api.git/",
        ] {
            assert_eq!(
                owner_repo_from_url(url).as_deref(),
                Some("acme/api"),
                "{url}"
            );
            assert_eq!(sanitize_bank(&owner_repo_from_url(url).expect(url)).as_deref(), Some("acme-api"), "{url}");
        }
    }

    #[test]
    fn a_remote_without_an_owner_repo_should_fall_through_to_the_basename() {
        // No owner/repo to read: a bare host, a local path, a malformed line.
        for url in [
            "https://github.com/",
            "https://github.com",
            "https://github.com/api",
            "https://github.com/only",
            "/srv/git/repo.git",
            "not a url at all",
        ] {
            assert_eq!(owner_repo_from_url(url), None, "{url}");
        }

        // End to end: a repo whose origin names no owner/repo still resolves,
        // to the name it always did.
        let t = Tmp::new("paths-noremote-path");
        let repo = t.repo("api", "https://github.com/api");
        assert_eq!(resolve_bank_in(&repo), "api");

        // And so does one with no origin at all.
        let bare = t.0.join("local-only");
        std::fs::create_dir_all(bare.join(".git")).expect("git dir");
        std::fs::write(bare.join(".git").join("config"), "[core]\n\tbare = false\n")
            .expect("config");
        assert_eq!(resolve_bank_in(&bare), "local-only");
    }

    #[test]
    fn a_git_dir_file_should_be_followed_to_its_config() {
        let t = Tmp::new("paths-gitfile");
        // A worktree: `<root>/.git` is a FILE naming the real git directory.
        let real = t.0.join("main/.git");
        std::fs::create_dir_all(&real).expect("git dir");
        std::fs::write(
            real.join("config"),
            "[remote \"origin\"]\n\turl = git@github.com:acme/api.git\n",
        )
        .expect("config");

        // Relative `gitdir:`, resolved against the worktree root.
        let work = t.0.join("work/api");
        std::fs::create_dir_all(&work).expect("work tree");
        std::fs::write(work.join(".git"), "gitdir: ../../main/.git\n").expect("gitdir file");
        assert_eq!(resolve_bank_in(&work), "acme-api");

        // Absolute `gitdir:`, the form a submodule uses.
        let sub = t.0.join("sub/api");
        std::fs::create_dir_all(&sub).expect("work tree");
        std::fs::write(sub.join(".git"), format!("gitdir: {}\n", real.display()))
            .expect("gitdir file");
        assert_eq!(resolve_bank_in(&sub), "acme-api");

        // A `gitdir:` that names nothing on disk resolves to no bank, and the
        // caller falls back rather than guessing at a config that is not there.
        let broken = t.0.join("broken/api");
        std::fs::create_dir_all(&broken).expect("work tree");
        std::fs::write(broken.join(".git"), "gitdir: /nonexistent/git-dir\n").expect("gitdir");
        assert_eq!(resolve_bank_in(&broken), "api");
    }

    #[test]
    fn a_directory_with_no_git_should_fall_back_to_the_default_bank() {
        let t = Tmp::new("paths-nogit");
        let plain = t.0.join("plain");
        std::fs::create_dir_all(&plain).expect("dir");
        assert_eq!(resolve_bank_in(&plain), DEFAULT_BANK);
        assert_eq!(legacy_bank_in(&plain), DEFAULT_BANK);
    }

    #[test]
    fn origin_url_should_read_the_url_of_the_origin_section_only() {
        let config = "[core]\n\tbare = false\n\
                      [remote \"upstream\"]\n\turl = https://github.com/other/repo.git\n\
                      [remote \"origin\"]\n\turl = https://github.com/acme/api.git\n\
                      \tfetch = +refs/heads/*:refs/remotes/origin/*\n\
                      [branch \"main\"]\n\tremote = origin\n";
        assert_eq!(origin_url(config), Some("https://github.com/acme/api.git"));

        // The older dotted spelling, and a key before the url in the section.
        assert_eq!(
            origin_url("[remote.origin]\n\turl = https://github.com/acme/api.git\n"),
            Some("https://github.com/acme/api.git")
        );
        assert_eq!(
            origin_url("[remote \"origin\"]\n\tpushurl = x\n\turl = ssh://git@h/acme/api.git\n"),
            Some("ssh://git@h/acme/api.git")
        );

        // No origin section, and an origin section with no url.
        assert_eq!(origin_url("[core]\n\tbare = false\n"), None);
        assert_eq!(origin_url("[remote \"origin\"]\n\tfetch = +refs/*\n"), None);
    }

    // A work tree nested below the root resolves to the ROOT's remote, because
    // that is the repository the files belong to.
    #[test]
    fn a_nested_directory_should_resolve_to_its_repositories_bank() {
        let t = Tmp::new("paths-nested");
        let repo = t.repo("api", "https://github.com/acme/api.git");
        let nested = repo.join("src/deep");
        std::fs::create_dir_all(&nested).expect("nested");
        assert_eq!(resolve_bank_in(&nested), "acme-api");
    }

    #[test]
    fn legacy_bank_in_should_still_report_the_basename() {
        let t = Tmp::new("paths-legacy");
        let repo = t.repo("My Repo", "https://github.com/acme/api.git");
        assert_eq!(legacy_bank_in(&repo), "my-repo");
        assert_ne!(legacy_bank_in(&repo), resolve_bank_in(&repo));
    }
}
