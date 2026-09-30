//! `install/get-memory-wire.sh`, executed end to end.
//!
//! The same reasoning as `tests/backup.rs`: the installer is the interface, so
//! the installer is what runs. Nothing here touches the network — `MW_LOCAL_ASSET`
//! pointing at a directory of hand-built assets is a mirror of a release
//! directory, and each asset is a real tarball with a real executable inside it,
//! so the runnability probe is exercised for real rather than simulated.
//!
//! What is pinned is the decision, not the bytes: which asset name the installer
//! picked, and what it said about the version it landed. The bug this file exists
//! for is that `linux-x86_64-musl` is a published asset the installer never
//! asked for, so a host below the glibc 2.39 floor installed a binary that could
//! not start and was told the install had worked.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};

/// Kills a stand-in server and removes its scratch directory **even when the test
/// panics**. Without this the assertions between spawn and cleanup were the only
/// thing standing between a failure and a leaked `sleep 600`: five processes and
/// nine directories had already accumulated on this machine from earlier failing
/// runs, which is the cost of an `assert!` placed before the cleanup it should
/// follow. `Drop` runs during unwinding, so the cleanup cannot be skipped by a
/// failed assertion.
struct Cleanup {
    child: Option<Child>,
    dir: PathBuf,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const INSTALLER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/install/get-memory-wire.sh");

/// A fake build that starts: `--version` answers, so the probe accepts it.
fn works(version: &str) -> String {
    format!("#!/bin/sh\necho \"memory-wire {version}\"\ncase \"$1\" in --version) exit 0;; esac\nexit 0\n")
}

/// A fake build shaped like the real failure on a host below the 2.39 floor:
/// the loader finds the version symbol missing and the process dies before main.
/// Nothing about the download or the checksum is wrong, which is the point.
const WONT_START: &str =
    "#!/bin/sh\necho '/lib/x86_64-linux-gnu/libc.so.6: version GLIBC_2.39 not found' >&2\nexit 127\n";

/// Where the installer would put the binary, given the scratch dir it was run in.
fn bin(dir: &Path) -> PathBuf {
    dir.join("bin").join("memory-wire")
}

/// A scratch directory per test, cleaned up on the way out.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mw-installer-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(dir.join("bin")).expect("scratch dir");
    dir
}

/// Build a release directory: one `memory-wire-<target>.tar.gz` per entry, each
/// holding a single `memory-wire` built from the given script.
///
/// The `.sha256` siblings are deliberately absent. `MW_LOCAL_ASSET` is documented
/// as skipping the network, and the checksum lives on the download path — what is
/// under test here is which asset gets chosen, not how a transfer is verified.
fn release_dir(dir: &Path, assets: &[(&str, String)]) -> PathBuf {
    let out = dir.join("assets");
    let stage = dir.join("stage");
    std::fs::create_dir_all(&out).expect("assets dir");
    for (target, script) in assets {
        std::fs::remove_dir_all(&stage).ok();
        std::fs::create_dir_all(&stage).expect("stage dir");
        std::fs::write(stage.join("memory-wire"), script).expect("write the fake binary");
        let tar = out.join(format!("memory-wire-{target}.tar.gz"));
        assert!(
            matches!(
                Command::new("tar")
                    .arg("-czf")
                    .arg(&tar)
                    .arg("-C")
                    .arg(&stage)
                    .arg("memory-wire")
                    .status(),
                Ok(s) if s.success()
            ),
            "tar could not build {}",
            tar.display()
        );
    }
    std::fs::remove_dir_all(&stage).ok();
    out
}

/// `dash` when it is here, because the installer is POSIX `sh` and on a box
/// where `/bin/sh` is bash a bashism is invisible to both `-n` and the run.
fn shell() -> String {
    for cand in ["dash", "sh"] {
        if matches!(
            Command::new(cand).arg("-c").arg("exit 0").status(),
            Ok(s) if s.success()
        ) {
            return cand.to_string();
        }
    }
    "sh".to_string()
}

/// Run the installer the way a user does. `--no-connect` keeps it off the
/// network entirely — the host-wiring and skill sections are not under test, and
/// `HOME` is redirected so nothing can reach the real one.
fn install(dir: &Path, assets: &Path) -> Output {
    Command::new(shell())
        .arg(INSTALLER)
        .arg("--no-connect")
        .current_dir(dir)
        .env("HOME", dir)
        .env("MW_INSTALL_DIR", dir.join("bin"))
        .env("MW_LOCAL_ASSET", assets)
        .output()
        .expect("spawn installer")
}

/// Both streams together. The version report is stdout, the diagnostics are
/// stderr, and a test that asserted on one would silently miss the other.
fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// What the *installed* binary answers, which is the only claim that matters:
/// the file on disk has to run.
fn installed_version(dir: &Path) -> String {
    let out = Command::new(bin(dir))
        .arg("--version")
        .output()
        .expect("run the installed binary");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn musl_fallback_should_install_the_static_asset_when_the_gnu_one_cannot_start() {
    let dir = scratch("fallback");
    let assets = release_dir(
        &dir,
        &[
            ("linux-x86_64", WONT_START.into()),
            ("linux-x86_64-musl", works("0.6.0")),
        ],
    );

    let out = install(&dir, &assets);
    assert!(out.status.success(), "install failed: {}", said(&out));
    let log = said(&out);

    // The gnu asset was tried and rejected on evidence, not skipped on a guess.
    assert!(
        log.contains("memory-wire-linux-x86_64.tar.gz does not run on this host"),
        "{log}"
    );
    assert!(log.contains("GLIBC_2.39 not found"), "{log}");
    assert!(
        log.contains(&format!(
            "installed memory-wire-linux-x86_64-musl -> {}",
            bin(&dir).display()
        )),
        "{log}"
    );
    assert_eq!(installed_version(&dir), "memory-wire 0.6.0", "{log}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_glibc_asset_should_still_win_on_a_host_where_it_runs() {
    let dir = scratch("common");
    let assets = release_dir(
        &dir,
        &[
            ("linux-x86_64", works("0.6.0")),
            // Distinctly versioned, so "installed the musl one anyway" cannot pass.
            ("linux-x86_64-musl", works("9.9.9-static")),
        ],
    );

    let out = install(&dir, &assets);
    assert!(out.status.success(), "install failed: {}", said(&out));
    let log = said(&out);

    // The common case stays common: no fallback is announced, tried or named.
    assert!(
        !log.contains("does not run on this host"),
        "a healthy host must never reach the fallback:\n{log}"
    );
    assert!(
        log.contains(&format!(
            "installed memory-wire-linux-x86_64 -> {}",
            bin(&dir).display()
        )),
        "{log}"
    );
    assert!(!log.contains("-musl"), "{log}");
    assert_eq!(installed_version(&dir), "memory-wire 0.6.0", "{log}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn no_usable_asset_should_name_the_glibc_floor_and_the_escape_hatch() {
    let dir = scratch("nofit");
    let assets = release_dir(&dir, &[("linux-x86_64", WONT_START.into())]);

    let out = install(&dir, &assets);
    assert!(
        !out.status.success(),
        "a host with no runnable asset must not exit 0: {}",
        said(&out)
    );
    let log = said(&out);
    for must in [
        "no memory-wire asset runs on this host",
        "tried: linux-x86_64 linux-x86_64-musl",
        "GLIBC_2.39 not found",
        "glibc >= 2.39",
        "MW_LOCAL_ASSET",
    ] {
        assert!(log.contains(must), "missing {must:?} in:\n{log}");
    }
    // Nothing may be left behind: a binary that cannot start is worse than none.
    assert!(!bin(&dir).exists(), "a failed install wrote a binary anyway");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn version_report_should_say_which_direction_the_install_went() {
    let dir = scratch("versions");
    let step = |version: &str| {
        let assets = release_dir(&dir, &[("linux-x86_64", works(version))]);
        let out = install(&dir, &assets);
        assert!(out.status.success(), "install {version}: {}", said(&out));
        said(&out)
    };

    assert!(step("0.5.1").contains("version 0.5.1 (first install"), "first install");
    assert!(step("0.6.0").contains("version 0.5.1 -> 0.6.0 (upgrade)"), "upgrade");
    assert!(step("0.6.0").contains("version 0.6.0 -> 0.6.0 (no change)"), "no-op");
    assert!(step("0.4.0").contains("version 0.6.0 -> 0.4.0 (DOWNGRADE)"), "downgrade");
    std::fs::remove_dir_all(&dir).ok();
}

/// The old install is a real ELF copied to the install path, because that is the
/// only thing whose `argv[0]` *is* that path. The kernel rewrites `argv` for a
/// `#!/bin/sh` fixture to `/bin/sh <script>`, so a shell-script fake can never
/// reproduce what a real `memory-wire serve` looks like to `ps`. `memory-wire
/// daemon start` re-execs `current_exe()`, so the real daemon does look like this.
#[cfg(target_os = "linux")]
#[test]
fn a_running_server_should_be_reported_as_still_on_the_old_binary() {
    let dir = scratch("serving");
    // Taken at the top so every path below — including an assertion failure —
    // kills the stand-in and removes the directory.
    let mut cleanup = Cleanup { child: None, dir: dir.clone() };
    // The stand-in must carry the real argv shape, because the installer decides what
    // is a "running server" from the process's arguments: `$3` must be `serve`.
    // A copied `/bin/sleep 600` had `$3 == "600"` and was therefore correctly
    // ignored -- the test had stopped exercising the reporting it exists for.
    // It also has to answer `--version`, because the installer reads the old
    // version before it replaces anything, and a script that only blocked would
    // hang that call.
    //
    // It blocks on an unwritten fifo rather than on `sleep`, which is the only
    // form that satisfies both requirements at once. `sleep 600` forks a child,
    // so `Child::kill` kills the wrapper and orphans the sleeper -- one survived
    // a passing run. `exec sleep 600` fixes that but replaces argv with
    // `sleep 600`, and the installer identifies a running server from argv, so
    // the process stops being findable. `read` from a fifo blocks *in the shell
    // itself*: no child to orphan, argv intact, and `kill` lands on the process
    // the test actually spawned.
    let hold = dir.join("hold");
    std::process::Command::new("mkfifo")
        .arg(&hold)
        .status()
        .expect("create the fifo the stand-in blocks on");
    let old_binary = format!(
        "#!/bin/sh\ncase \"$1\" in --version) echo 'memory-wire 0.5.0'; exit 0;; esac\nread line < {}\n",
        hold.display()
    );
    // Staged and renamed, not written in place: exec'ing a file a write fd may
    // still be open on is ETXTBSY, which would fail the setup rather than
    // anything the installer does.
    let staged = dir.join("bin").join("memory-wire.staged");
    std::fs::write(&staged, old_binary).expect("stage the old binary");
    std::fs::rename(&staged, bin(&dir)).expect("put the old binary in place");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin(&dir), std::fs::Permissions::from_mode(0o755))
            .expect("make the old binary executable");
    }

    // Spawning can transiently fail with ETXTBSY while the test harness's other
    // threads are writing and exec'ing in the same process — six sequential
    // runs never reproduce it, and it disappears under `--test-threads=1`, so it
    // is contention in the fixture and not in the installer, whose install step
    // is a `rename` (which does not take ETXTBSY even over a live binary). Retry
    // a bounded number of times rather than assert, so a scheduling accident
    // cannot be reported as a product failure.
    let mut server = None;
    let mut last_err = None;
    for attempt in 0..50 {
        match Command::new(bin(&dir)).arg("serve").spawn() {
            Ok(child) => {
                server = Some(child);
                break;
            }
            Err(e) if e.raw_os_error() == Some(26) => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(20 + attempt * 20));
            }
            Err(e) => panic!("start the old server: {e}"),
        }
    }
    let server = server.unwrap_or_else(|| panic!("start the old server: ETXTBSY 50x: {last_err:?}"));
    cleanup.child = Some(server);
    std::thread::sleep(std::time::Duration::from_millis(500));

    let assets = release_dir(&dir, &[("linux-x86_64", works("0.6.0"))]);
    let out = install(&dir, &assets);
    assert!(out.status.success(), "{}", said(&out));
    let log = said(&out);

    // It named the situation, and it named this process, and it did not act:
    // restarting a daemon is the user's call, because it may be under systemd.
    let server = cleanup.child.as_mut().expect("the stand-in server is live");
    assert!(log.contains("a server is still running the old"), "{log}");
    assert!(log.contains(&format!("(pid {})", server.id())), "{log}");
    assert!(log.contains("daemon stop &&"), "{log}");
    assert!(
        server.try_wait().expect("poll the server").is_none(),
        "the installer must report the old server, never stop it"
    );
    assert_eq!(installed_version(&dir), "memory-wire 0.6.0", "{log}");

    // Cleanup happens in `Cleanup::drop`, including on the panic path above.
    drop(cleanup);
}