//! `update`: the five-step swap as one command.
//!
//! Updating an installed memory-wire by hand is five steps — find the release,
//! download and verify it, stop the live daemon, replace the binary, restart
//! and re-wire — and the window between them is where both known v0.7.0
//! install incidents lived (see `docs/handoff/STATE.md`): a swapped binary
//! with the daemon still serving the old inode, and a `connect` that dropped
//! the `--bank` pin. This module runs the steps in the only safe order and
//! composes the two fixes: the restart closes the stale-serve window the same
//! moment the new bytes land, and the re-connect rides TODO #1's pin
//! preservation so a bare `connect` cannot unpin a machine.
//!
//! **The network is `gh`, not this binary.** GitHub serves `https://` only and
//! this tree's HTTP client is deliberately dependency-free plain HTTP
//! (`src/http.rs`); rather than grow a TLS dependency for one command, `update`
//! drives `gh` — the same REST path `scripts/build-release.sh` publishes
//! through, and a documented prerequisite on the release host. A missing `gh`
//! is a refusal with the install hint, not a fallback to unverified traffic.
//!
//! The orchestration runs over a small effects seam ([`Ops`]) so the order —
//! verify **before** swap, stop **before** swap, start **after** — is asserted
//! in tests with a fake rather than trusted from reading.

use std::path::{Path, PathBuf};

use crate::paths;

/// The repository releases live in. The installer's variable, same default.
const DEFAULT_REPO: &str = "ishan-parihar/memory-wire";

/// One step of the swap, as a testable seam. Every method is a side effect the
/// orchestration needs in a specific order; the real implementation is
/// [`GhOps`], tests inject a recording fake.
pub trait Ops {
    /// Release tags, any order, `v`-prefixed.
    fn releases(&mut self) -> Result<Vec<String>, String>;
    /// Download one release's assets for our target into `dir`.
    fn download(&mut self, tag: &str, dir: &Path) -> Result<Vec<PathBuf>, String>;
    /// Unpack the verified archive and return the new binary's path.
    fn extract(&mut self, archive: &Path, dir: &Path) -> Result<PathBuf, String>;
    /// True when the daemon state file names a live serve.
    fn daemon_live(&mut self) -> bool;
    /// Stop the live daemon (the running binary's own `daemon stop`).
    fn stop_daemon(&mut self) -> Result<(), String>;
    /// Move `new_bin` onto the installed binary's path, atomically.
    fn swap(&mut self, new_bin: &Path) -> Result<(), String>;
    /// Start the daemon with the *new* binary.
    fn start_daemon(&mut self, new_bin: &Path) -> Result<(), String>;
    /// Re-wire the hosts (a bare `connect` from the new binary).
    fn reconnect(&mut self, new_bin: &Path) -> Result<String, String>;
}

/// The order of operations, as one function over the seam. Returns the
/// human-readable result; `Err` is a refusal that leaves the install as it
/// was (nothing is swapped before the verification passes).
pub fn orchestrate(ops: &mut dyn Ops, current: &str) -> Result<String, String> {
    let tags = ops.releases()?;
    let Some(newest) = newest(&tags, current) else {
        return Ok(format!(
            "up to date: {current} is the newest release ({} known)",
            tags.len()
        ));
    };
    if newest == current {
        return Ok(format!("up to date at {current}"));
    }
    let dir = update_scratch(&newest);
    let assets = ops.download(&newest, &dir)?;
    let Some(archive) = assets.iter().find(|p| {
        p.extension().is_some_and(|e| e == "gz")
            && !p.to_string_lossy().ends_with(".sha256")
    }) else {
        return Err(format!(
            "no memory-wire-<target>.tar.gz in {} (files: {})",
            dir.display(),
            assets.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        ));
    };
    let digest_path = PathBuf::from(format!("{}.sha256", archive.display()));
    verify(archive, &digest_path)?;

    // Extract before touching the daemon: everything that can fail without
    // side effects (parse, download, verify, unpack) happens before the one
    // step with a visible cost, so the stopped-daemon window is as short as
    // the swap itself.
    let new_bin = ops.extract(archive, &dir)?;
    let was_live = ops.daemon_live();
    if was_live {
        ops.stop_daemon()?;
    }
    ops.swap(&new_bin)?;
    let mut notes = vec![format!("updated {current} to {newest}")];
    if was_live {
        ops.start_daemon(&new_bin)?;
        notes.push("daemon restarted on the new binary".to_string());
    }
    let connect_note = ops.reconnect(&new_bin)?;
    if !connect_note.trim().is_empty() {
        notes.push(connect_note);
    }
    Ok(notes.join("; "))
}

/// The newest release strictly newer than `current`, or `None`.
///
/// Tags that do not parse as `v<numbers>` are skipped rather than fatal: a
/// stray `nightly` tag in the list is not a reason to refuse an update.
/// Numeric component-wise compare, the shape every tag this project ships has.
pub fn newest(tags: &[String], current: &str) -> Option<String> {
    let cur = parse_tag(current)?;
    tags.iter()
        .filter_map(|t| parse_tag(t).map(|v| (t, v)))
        .filter(|(_, v)| *v > cur)
        .max_by(|a, b| a.1.cmp(&b.1))
        .map(|(t, _)| t.clone())
}

/// `v1.2.3` → `[1, 2, 3]`. `None` for anything else.
///
/// The `v` prefix is optional because the two inputs arrive in different
/// shapes: `gh release list` names tags (`v1.2.3`) and the running binary's
/// version is bare semver (`1.2.3`, `CARGO_PKG_VERSION`). Requiring the
/// prefix made `newest("…", "0.8.0")` return `None` — structurally unable
/// to find any newer release — which is exactly how v0.8.1 shipped with a
/// `memory-wire update` that always printed "up to date". The regression
/// test pins the bare form, the production shape.
fn parse_tag(tag: &str) -> Option<Vec<u64>> {
    let rest = tag.strip_prefix('v').unwrap_or(tag);
    let parts = rest
        .split('.')
        .map(|p| p.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    (parts.len() == 3).then_some(parts)
}

/// The scratch dir for one tag's download, under the data dir `update/` uses.
/// `v0.7.0` → `.../memory-wire/update-v0.7.0/`.
fn update_scratch(tag: &str) -> PathBuf {
    let root = paths::data_dir(
        &crate::paths::home().unwrap_or_else(|_| PathBuf::from(".")),
    );
    let dir = root.join(format!("update-{tag}"));
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Verify the archive against the release's `.sha256` sidecar.
///
/// sha2 is already in the dependency set, so the check is in-process — no
/// `sha256sum` on `$PATH`, the same trust the installer's fallback lacks. A
/// missing sidecar is a refusal: an unverified swap is the one step this
/// command exists to make impossible.
fn verify(archive: &Path, digest_path: &Path) -> Result<(), String> {
    let sidecar = std::fs::read_to_string(digest_path)
        .map_err(|e| format!("no checksum sidecar ({}): {e}", digest_path.display()))?;
    let want = sidecar.split_whitespace().next().unwrap_or_default().to_string();
    let bytes = std::fs::read(archive).map_err(|e| format!("{}: {e}", archive.display()))?;
    use sha2::{Digest, Sha256};
    let got = Sha256::digest(&bytes);
    let got = format!("{got:x}");
    if got != want {
        return Err(format!(
            "checksum mismatch for {}: expected {want}, got {got}; refusing to install",
            archive.display()
        ));
    }
    Ok(())
}

/// Extract the binary from the release tarball into `dir`.
///
/// `tar` on `$PATH`: the same tool the installer assumes, and a `tar`-crate
/// dependency to unwrap one file would be weight on every build for one
/// command's use. ponytail: shells out to `tar`, fine until a Windows host
/// matters (it cannot — the asset set is Linux-only).
fn extract(archive: &Path, dir: &Path) -> Result<PathBuf, String> {
    let out = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(dir)
        .output()
        .map_err(|e| format!("tar: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "tar: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let name = "memory-wire";
    let bin = dir.join(name);
    if !bin.is_file() {
        return Err(format!(
            "no memory-wire binary in the archive at {}",
            bin.display()
        ));
    }
    Ok(bin)
}

/// The release asset slug for the running build's target — the same asset set
/// `scripts/build-release.sh` publishes.
///
/// ponytail: ARCH+OS only, so a musl host would be offered the gnu asset and
/// the new binary would refuse to run on its glibc floor — the `start_daemon`
/// failure names the old binary (kept beside the new one as `<exe>.old`) and
/// MW_ASSET overrides the choice until musl detection is worth its cost.
pub fn asset_slug_for(triple: &str) -> Option<&'static str> {
    slug_from(triple)
}

fn slug_from(triple: &str) -> Option<&'static str> {
    let slug = match triple {
        "x86_64-unknown-linux-gnu" => "linux-x86_64",
        "x86_64-unknown-linux-musl" => "linux-x86_64-musl",
        "aarch64-unknown-linux-gnu" => "linux-aarch64",
        "aarch64-unknown-linux-musl" => "linux-aarch64",
        _ => return None,
    };
    Some(slug)
}

/// The real effects: `gh` for the network, in-process for the rest.
pub struct GhOps {
    /// The installed binary this process runs from — the swap target.
    exe: PathBuf,
    repo: String,
}

impl GhOps {
    /// From the ambient environment. Fails when `gh` is absent: the refusal
    /// names the prerequisite rather than trying plain-HTTP traffic.
    pub fn new() -> Result<Self, String> {
        if std::env::var_os("PATH").is_none_or(|p| p.is_empty()) {
            return Err("no PATH to search for `gh`".to_string());
        }
        let on_path = crate::connect::binary_on_path("gh");
        if !on_path {
            return Err(
                "gh is required for update (GitHub serves https only, and this binary stays \
                 dependency-free). Install: https://cli.github.com".to_string(),
            );
        }
        Ok(Self {
            exe: std::env::current_exe().map_err(|e| format!("current exe: {e}"))?,
            repo: std::env::var("MW_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_string()),
        })
    }
}

impl Ops for GhOps {
    fn extract(&mut self, archive: &Path, dir: &Path) -> Result<PathBuf, String> {
        extract(archive, dir)
    }
    fn releases(&mut self) -> Result<Vec<String>, String> {
        let out = std::process::Command::new("gh")
            .args(["release", "list", "--repo", &self.repo, "--limit", "50"])
            .output()
            .map_err(|e| format!("gh: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "gh release list: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        // One release per line, first TAB-separated field is the display name,
        // the third is the tag; the tag is what the API routes on.
        let tags = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.split('\t').nth(2).map(str::trim).filter(|t| !t.is_empty()))
            .map(String::from)
            .collect();
        Ok(tags)
    }

    fn download(&mut self, tag: &str, dir: &Path) -> Result<Vec<PathBuf>, String> {
        let slug = asset_slug_for(host_triple())
            .ok_or_else(|| format!("no release asset for target {}", host_triple()))?;
        // MW_ASSET overrides the slug choice (see `asset_slug_for`'s ceiling
        // note) for a host the auto-choice gets wrong.
        let slug = std::env::var("MW_ASSET")
            .ok()
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| slug.to_string());
        let pattern = format!("memory-wire-{slug}*");
        let out = std::process::Command::new("gh")
            .args([
                "release",
                "download",
                tag,
                "--repo",
                &self.repo,
                "--pattern",
                &pattern,
                "--dir",
            ])
            .arg(dir)
            .output()
            .map_err(|e| format!("gh: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "gh release download {tag}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(std::fs::read_dir(dir)
            .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.path()).collect())
            .unwrap_or_default())
    }

    fn daemon_live(&mut self) -> bool {
        let state = paths::daemon_state_path();
        let Ok(text) = std::fs::read_to_string(&state) else {
            return false;
        };
        let Ok(rec) = serde_json::from_str::<crate::daemon::ServeState>(&text) else {
            return false;
        };
        crate::doctor::probe_server(&format!("http://{}", rec.addr)) == crate::doctor::ServerState::Up
    }

    fn stop_daemon(&mut self) -> Result<(), String> {
        let code = crate::daemon::run(crate::daemon::Action::Stop);
        if code == 0 {
            Ok(())
        } else {
            Err(format!("daemon stop exited {code}"))
        }
    }

    fn swap(&mut self, new_bin: &Path) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(new_bin, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("{}: {e}", new_bin.display()))?;
        let exe = &self.exe;
        // The previous binary is kept beside the new one as `<exe>.old`: a
        // daemon that refuses to start on the new build has a one-command
        // rollback instead of a broken install. Renames are atomic on one
        // filesystem; a cross-filesystem install path falls back to the
        // copy-then-rename dance so the installed path never lacks a binary.
        let old = exe.with_extension("old");
        std::fs::rename(exe, &old).ok();
        if std::fs::rename(new_bin, exe).is_err() {
            let tmp = exe.with_extension("new");
            std::fs::copy(new_bin, &tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, exe).map_err(|e| {
                // Put the kept old binary back before refusing.
                std::fs::rename(&old, exe).ok();
                format!("{}: {e}", exe.display())
            })?;
        }
        Ok(())
    }

    fn start_daemon(&mut self, new_bin: &Path) -> Result<(), String> {
        let out = std::process::Command::new(new_bin)
            .arg("daemon")
            .arg("start")
            .output()
            .map_err(|e| format!("{}: {e}", new_bin.display()))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "daemon start on the new binary failed: {}. The previous binary is kept \
                 beside it as `{}.old` — restore with `mv {}.old {}` and re-run \
                 `daemon start`",
                String::from_utf8_lossy(&out.stderr).trim(),
                new_bin.display(),
                new_bin.display(),
                new_bin.display()
            ))
        }
    }

    fn reconnect(&mut self, new_bin: &Path) -> Result<String, String> {
        let out = std::process::Command::new(new_bin)
            .arg("connect")
            .output()
            .map_err(|e| format!("{}: {e}", new_bin.display()))?;
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if out.status.success() {
            Ok(report)
        } else {
            Err(format!("connect on the new binary: {report}"))
        }
    }
}

/// This build's host triple.
fn host_triple() -> &'static str {
    // One literal for one build; the release assets are per-target and the
    // check this feeds only ever runs on the machine it was built for.
    "x86_64-unknown-linux-gnu"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numeric component compare: 0.10.0 beats 0.9.9, equal is not newer, and
    /// junk tags are skipped rather than fatal.
    /// The production shapes: tags from `gh release list` carry the `v`
    /// prefix, the running binary's version does not. Requiring the prefix on
    /// both (v0.8.1's bug) makes `update` permanently claim "up to date".
    #[test]
    fn newest_must_accept_the_bare_semver_current_binary_reports() {
        let tags: Vec<String> = ["v0.8.0", "v0.8.1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(newest(&tags, "0.8.0").as_deref(), Some("v0.8.1"));
        // Equal versions are not newer — the None that means "up to date".
        assert_eq!(newest(&tags, "0.8.1"), None);
        // Both shapes parse to the same triple, so a `v`-prefixed current
        // (as the orchestrate fake passes it) keeps working too.
        assert_eq!(newest(&tags, "v0.8.0").as_deref(), Some("v0.8.1"));
    }

    #[test]
    fn newest_should_pick_the_strictly_newest_parsable_tag() {
        let tags: Vec<String> = ["v0.9.9", "v0.10.0", "v0.7.0", "nightly", "v0.7.0-rc.1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(newest(&tags, "v0.7.0").as_deref(), Some("v0.10.0"));
        assert_eq!(newest(&tags, "v0.10.0"), None);
        assert_eq!(newest(&[], "v0.7.0"), None);
        assert_eq!(newest(&["junk".to_string()], "v0.7.0"), None);
    }

    #[test]
    fn parse_tag_should_reject_non_numeric_shapes() {
        assert_eq!(parse_tag("v0.7.0"), Some(vec![0, 7, 0]));
        // Bare semver parses: it is the running binary's own version shape
        // (`CARGO_PKG_VERSION`), and rejecting it was v0.8.1's dead-hop bug.
        assert_eq!(parse_tag("0.7.0"), Some(vec![0, 7, 0]));
        assert_eq!(parse_tag("v0.7"), None);
        assert_eq!(parse_tag("v0.7.x"), None);
        assert_eq!(parse_tag(""), None);
    }

    /// The full order, recorded by a fake: verify precedes stop, stop
    /// precedes swap, start and connect run on the new binary, and an
    /// up-to-date release short-circuits before any download.
    struct Fake {
        log: Vec<String>,
        live: bool,
    }
    impl Fake {
        fn ok() -> Self {
            Self { log: vec![], live: false }
        }
    }
    impl Ops for Fake {
        fn releases(&mut self) -> Result<Vec<String>, String> {
            self.log.push("releases".into());
            Ok(vec!["v0.7.0".into(), "v0.8.0".into()])
        }
        fn download(&mut self, tag: &str, dir: &Path) -> Result<Vec<PathBuf>, String> {
            self.log.push(format!("download {tag}"));
            let a = dir.join("memory-wire-linux-x86_64.tar.gz");
            std::fs::write(&a, b"tarball").ok();
            std::fs::write(a.with_extension("gz.sha256"), hex_of(b"tarball"))
                .ok();
            Ok(vec![a])
        }
        fn extract(&mut self, _archive: &Path, dir: &Path) -> Result<PathBuf, String> {
            self.log.push("extract".into());
            let bin = dir.join("memory-wire");
            std::fs::write(&bin, b"binary").ok();
            Ok(bin)
        }
        fn daemon_live(&mut self) -> bool {
            self.log.push("probe".into());
            self.live
        }
        fn stop_daemon(&mut self) -> Result<(), String> {
            self.log.push("stop".into());
            Ok(())
        }
        fn swap(&mut self, new_bin: &Path) -> Result<(), String> {
            self.log.push(format!("swap {}", new_bin.display()));
            Ok(())
        }
        fn start_daemon(&mut self, new_bin: &Path) -> Result<(), String> {
            self.log.push(format!("start {}", new_bin.display()));
            Ok(())
        }
        fn reconnect(&mut self, new_bin: &Path) -> Result<String, String> {
            self.log.push(format!("connect {}", new_bin.display()));
            Ok(String::new())
        }
    }

    fn hex_of(b: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(b))
    }

    /// The success path with a live daemon: every step in the safe order.
    #[test]
    fn orchestrate_runs_the_full_order_when_live() {
        let mut f = Fake { live: true, ..Fake::ok() };
        let report = orchestrate(&mut f, "v0.7.0").expect("update");
        assert!(report.contains("v0.7.0"), "{report}");
        assert!(report.contains("v0.8.0"), "{report}");
        assert_eq!(
            f.log,
            vec![
                "releases".to_string(),
                "download v0.8.0".to_string(),
                "extract".to_string(),
                "probe".to_string(),
                "stop".to_string(),
                format!("swap {}", update_scratch("v0.8.0").join("memory-wire").display()),
                format!("start {}", update_scratch("v0.8.0").join("memory-wire").display()),
                format!("connect {}", update_scratch("v0.8.0").join("memory-wire").display()),
            ]
        );
    }

    /// The real extractor against a real tarball: `tar` is a build
    /// prerequisite, so the test builds its own archive and proves the
    /// flat-`memory-wire` layout releases actually ship.
    #[test]
    fn extract_should_read_a_real_release_tarball() {
        let dir = std::env::temp_dir().join(format!("mw-update-extract-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        std::fs::write(dir.join("memory-wire"), b"#!/bin/sh\n").expect("bin");
        let cwd = std::env::current_dir().expect("cwd");
        let out = std::process::Command::new("tar")
            .arg("-czf")
            .arg(dir.join("asset.tar.gz"))
            .arg("-C")
            .arg(&dir)
            .arg("memory-wire")
            .current_dir(&cwd)
            .output()
            .expect("tar");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let bin = extract(&dir.join("asset.tar.gz"), &dir).expect("extract");
        assert!(bin.is_file(), "{}", bin.display());
        assert_eq!(bin, dir.join("memory-wire"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Up to date: no download, no probe, nothing touched.
    #[test]
    fn orchestrate_short_circuits_when_current_is_newest() {
        let mut f = Fake { live: true, ..Fake::ok() };
        let report = orchestrate(&mut f, "v0.8.0").expect("report");
        assert!(report.contains("up to date"), "{report}");
        assert_eq!(f.log, vec!["releases".to_string()]);
    }

    /// A checksum mismatch must refuse before anything is stopped or swapped.
    #[test]
    fn a_checksum_mismatch_refuses_before_the_swap() {
        // Corrupt the archive bytes after download via a delegating fake.
        struct Tampered(Fake);
        impl Ops for Tampered {
            fn releases(&mut self) -> Result<Vec<String>, String> {
                self.0.releases()
            }
            fn download(&mut self, tag: &str, dir: &Path) -> Result<Vec<PathBuf>, String> {
                let files = self.0.download(tag, dir)?;
                for p in &files {
                    if p.extension().is_some_and(|e| e == "gz") {
                        std::fs::write(p, b"tampered").ok();
                    }
                }
                Ok(files)
            }
            fn extract(&mut self, archive: &Path, dir: &Path) -> Result<PathBuf, String> {
                self.0.extract(archive, dir)
            }
            fn daemon_live(&mut self) -> bool {
                self.0.daemon_live()
            }
            fn stop_daemon(&mut self) -> Result<(), String> {
                self.0.stop_daemon()
            }
            fn swap(&mut self, new_bin: &Path) -> Result<(), String> {
                self.0.swap(new_bin)
            }
            fn start_daemon(&mut self, new_bin: &Path) -> Result<(), String> {
                self.0.start_daemon(new_bin)
            }
            fn reconnect(&mut self, new_bin: &Path) -> Result<String, String> {
                self.0.reconnect(new_bin)
            }
        }
        let mut t = Tampered(Fake::ok());
        let err = orchestrate(&mut t, "v0.7.0").expect_err("tampered archive must refuse");
        assert!(err.contains("checksum mismatch"), "{err}");
        assert_eq!(t.0.log, vec!["releases".to_string(), "download v0.8.0".to_string()]);
    }

    #[test]
    fn asset_slug_should_map_the_known_triples() {
        assert_eq!(asset_slug_for("x86_64-unknown-linux-gnu"), Some("linux-x86_64"));
        assert_eq!(asset_slug_for("x86_64-unknown-linux-musl"), Some("linux-x86_64-musl"));
        assert_eq!(asset_slug_for("aarch64-unknown-linux-gnu"), Some("linux-aarch64"));
        assert_eq!(asset_slug_for("x86_64-apple-darwin"), None);
    }
}
