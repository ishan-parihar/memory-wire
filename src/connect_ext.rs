//! omp and pi: the two hosts memory-wire installs as a **TypeScript file**
//! rather than as a config entry.
//!
//! A file in the home directory of each harness, at a layout each one actually
//! scans. The competitor answers "how do I install this" with a `cp -r` out of a
//! source checkout, which needs a repository, an interpreter and a working
//! directory; `connect` has none of those as preconditions, so the bytes travel
//! inside the binary and installing is a copy. That makes this the mirror of
//! [`crate::connect_plugin`], which covers hermes's Python directory plus one
//! YAML key, rather than a variant of it.
//!
//! It adds the one thing MCP cannot: an MCP tool exists only when the model
//! decides to call it, so on an MCP-only host a memory is invisible until the
//! model thinks to look. The extension hooks `before_agent_start` and returns a
//! system prompt, the one moment either harness lets a third party put text in
//! front of the model unasked. omp keeps its MCP entry too, so an omp session has
//! both: the tools the model may call, and the recall it did not have to ask for.
//! `--uninstall` takes both back off.
//!
//! The three properties are inherited from `connect.rs` rather than reinvented,
//! and each is shaped by the surface being a file rather than a config:
//!
//! - **Never corrupt.** A file at our path that does not carry our marker is
//!   somebody's file, and it is refused byte-identical rather than replaced. Both
//!   markers are required, and the check runs before anything is copied, so a
//!   refusal also leaves no backup behind: a file that merely *mentions*
//!   `memory-wire` is a user file talking about us.
//! - **Idempotent.** A file whose bytes already match the embedded template is
//!   not rewritten, and a second install reports `already-wired` with the version
//!   it found, so a no-op and a real upgrade are told apart from the terminal.
//! - **Reversible.** The pre-existing file is copied to
//!   `backups/<host>-<ts>/` before the first write, and `--uninstall` removes
//!   only a file this install could recognise as ours.
//!
//! The two paths are not interchangeable, and composing them into one would be
//! the bug: omp loads flat `*.ts` files out of `~/.omp/agent/extensions/`, while
//! pi resolves a directory to its `index.ts` under
//! `~/.pi/agent/extensions/<dir>/`. A file written to the other's layout loads
//! nowhere and reports nothing, and the only symptom a user would ever see is a
//! memory backend that has quietly stopped being one.

use std::path::{Path, PathBuf};

use crate::connect::{backup, timestamp, Host, Outcome, MARKER};
use crate::paths;

/// The extension, embedded at build time.
///
/// The competitor answers "how do I install this" with a `cp -r` out of a source
/// checkout, which needs a repository, an interpreter and a working directory.
/// `connect` has none of those as preconditions, so the file travels inside the
/// binary and installing is a copy.
///
    /// Source of truth is `plugin/integrations/extension/memory-wire.ts`. One file
    /// serves both hosts, so there is no per-host template to drift, and
    /// `the_embedded_source_is_the_one_on_disk` asserts the directory and
    /// `include_str!` agree — a file in the directory but not here would install
    /// an incomplete tree and pass every other test in this module.
///
/// `pub(crate)` rather than private because `connect`'s tests assert an installed
/// file equals it, and a test comparing against a re-derived copy of the string
/// would pass whatever the binary happened to hold.
pub(crate) const SOURCE: &str =
    include_str!("../plugin/integrations/extension/memory-wire.ts");

/// Marker the template carries, and the only thing that distinguishes one
/// installed copy from another.
const VERSION_MARKER: &str = "MEMORY_WIRE_EXTENSION_VERSION";

/// The template line the install-time bank is baked into.
const DEFAULT_BANK_LINE: &str = "var DEFAULT_BANK = \"memory-wire\";";

/// The template as it lands on disk: the `connect --bank` value baked into
/// `DEFAULT_BANK`, or the embedded bytes when no bank was given.
///
/// The same rendering feeds the idempotence compare in `apply`, so a reconnect
/// with the same bank stays `already-wired` and one with a different bank is a
/// real replace — and the extension and the MCP entry of the same install can
/// no longer disagree on the namespace. The escaping is a backstop for a
/// hostile value; bank ids are slug-like.
fn baked(bank: Option<&str>) -> String {
    match bank.map(str::trim).filter(|b| !b.is_empty()) {
        None => SOURCE.to_string(),
        Some(b) => SOURCE.replace(
            DEFAULT_BANK_LINE,
            &format!("var DEFAULT_BANK = \"{}\";", b.replace('\\', "\\\\").replace('"', "\\\"")),
        ),
    }
}

/// Install or uninstall this host's extension file. See the module docs.
///
/// A `None` from [`extension_rel`] means the host has no extension surface, and
/// that is a [`Outcome::Skipped`] rather than a failure: nothing to get wrong.
pub fn apply(host: Host, home: &Path, uninstall: bool, data_root: &Path, bank: Option<&str>) -> Outcome {
    let Some(rel) = extension_rel(host) else {
        return Outcome::Skipped("no extension surface for this host".to_string());
    };
    let path = home.join(&rel);
    let source = baked(bank);
    let current = version(SOURCE).unwrap_or("?");

    if uninstall {
        if !path.is_file() {
            return Outcome::Skipped("no memory-wire extension to remove".to_string());
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
        // delete. Bounded at `extensions/`, so the harness's own `agent/`
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
                "{}: not a memory-wire extension ({VERSION_MARKER} absent); left untouched",
                path.display()
            ));
        }
        if have == source {
            return Outcome::Already {
                notes: vec![format!("extension v{current}")],
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
    // replaced rather than refused — and both versions are named, because that is
    // the only moment a user can tell a real upgrade from a no-op.
    if let Some(was) = existing.as_deref().and_then(version) {
        notes.push(format!("replaced extension v{was} with v{current}"));
    }
    Outcome::Wired {
        backup: saved,
        events: vec![rel.display().to_string()],
        notes,
    }
}

/// Where this host's extension file goes, home-relative, or `None` for a host
/// that has no extension surface.
pub fn extension_rel(host: Host) -> Option<PathBuf> {
    let joined = match host {
        // omp reads every flat `*.ts` file in its extensions directory, so the
        // file *is* the registration — no index, no manifest, no key.
        Host::Omp => PathBuf::from(".omp")
            .join("agent")
            .join("extensions")
            .join(format!("{MARKER}.ts")),
        // pi resolves a directory to its `index.ts` — `indexNames: ["index.ts",
        // "index.js"]`, from the resolver the omp binary carries — so a bare file
        // dropped in the extensions directory is not discovered.
        Host::Pi => PathBuf::from(".pi")
            .join("agent")
            .join("extensions")
            .join(MARKER)
            .join("index.ts"),
        _ => return None,
    };
    Some(joined)
}

/// Does this host carry our extension file? Reads, and never writes.
///
/// For omp the caller ORs this with the MCP entry rather than ANDing it: an omp
/// that has only the MCP key wired, as every omp wired by an earlier build does,
/// is genuinely connected, and reporting `wired=no` there would be a false
/// negative the user has no way to act on.
pub fn installed(host: Host, home: &Path) -> bool {
    extension_rel(host).is_some_and(|rel| is_ours(&home.join(rel)))
}

/// Does the file at our path carry our marker? See [`carries_marker`] for why
/// both markers are required rather than one.
fn is_ours(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|s| carries_marker(&s))
}

/// The test for "this file is ours to replace".
///
/// Both markers, not just the name: a file that merely mentions `memory-wire` is
/// a user file that happens to be talking about us, and overwriting it is the
/// one thing this module exists to avoid.
fn carries_marker(source: &str) -> bool {
    source.contains(MARKER) && source.contains(VERSION_MARKER)
}

/// The version a source declares, read out of the marker line.
///
/// Parsed rather than duplicated as a constant: a second copy is a second thing
/// to forget to bump, and the marker's whole purpose is to let a future installer
/// tell two installed copies apart. The marker is found *inside* the line rather
/// than at its start, so the template is free to keep it in a comment or a string
/// literal — which is where it is. `v1`, `1`, `= 1`, `: 1` and leading zeros all
/// parse, because punctuation and a version-shaped prefix are otherwise a silent
/// way to downgrade this to "replace nothing" and report every upgrade as a no-op.
fn version(source: &str) -> Option<&str> {
    source.lines().find_map(|line| {
        let rest = line.split_once(VERSION_MARKER)?.1;
        let rest = rest.trim_start().trim_start_matches([':', '=']).trim();
        let digits = rest.strip_prefix('v').unwrap_or(rest);
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        // Leading zeros are normalised so `v007` and `7` compare equal, with `0`
        // kept as `0` rather than normalising to the empty string and reading as
        // "no version declared".
        let trimmed = digits.trim_start_matches('0');
        Some(if trimmed.is_empty() { "0" } else { trimmed })
    })
}

/// Drop `dir` and the empty directories above it, stopping at the harness's
/// `extensions/`.
///
/// The two layouts make "one level up" mean different things — pi's file sits in
/// a `memory-wire/` directory below `extensions/`, omp's sits directly in it — so
/// a fixed depth either leaves pi's residue behind or takes omp's `agent/` with
/// it. Naming the directory to stop at says what is meant: everything we created
/// under `extensions/`, that directory itself when nothing else is in it, and
/// never the harness's own tree above that.
///
/// Best effort by design, as in `connect_plugin`: a `DirectoryNotEmpty` or a
/// permission error means somebody else is using that directory, and neither is
/// worth failing an uninstall over. Bounded at four so a path that somehow never
/// reaches `extensions/` cannot walk an arbitrary chain upward.
fn prune_empty_dirs(dir: &Path) {
    let mut cursor = dir;
    // Bounded at four so a path that somehow never reaches an `extensions`
    // directory cannot walk an arbitrary chain upward; the early return on the
    // name is what normally stops it.
    for _ in 0..4 {
        let empty = std::fs::read_dir(cursor).map(|mut e| e.next().is_none()).unwrap_or(false);
        if !empty {
            return;
        }
        let at_extensions = cursor.file_name().is_some_and(|n| n == "extensions");
        let Some(parent) = cursor.parent() else {
            return;
        };
        if std::fs::remove_dir(cursor).is_err() {
            return;
        }
        if at_extensions {
            return;
        }
        cursor = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch home with the host's detection directory present, so detection
    /// passes on `$HOME` alone and cannot depend on `PATH`.
    fn detected(tag: &str, host: Host) -> PathBuf {
        let home = tmp_home(tag);
        std::fs::create_dir_all(home.join(host.detect_rel())).expect("host home");
        home
    }

    /// Names directly under `dir`, sorted — for a "nothing was created here" or
    /// "nothing is left here" claim.
    fn dir_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// A PATH lookup that finds nothing, so detection cannot depend on what
    /// happens to be installed on the machine running the tests.
    fn no_path(_name: &str) -> bool {
        false
    }

    fn tmp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-ext-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp home");
        dir
    }

    fn run(host: Host, home: &Path, uninstall: bool) -> Outcome {
        crate::connect::apply(host, "memory-wire", home, uninstall, no_path, &home.join("data"), None)
    }

    fn read_ext(home: &Path, host: Host) -> String {
        std::fs::read_to_string(home.join(extension_rel(host).expect("rel"))).expect("extension")
    }

    /// The reason an outcome carries, so a message can be asserted without
    /// depending on which variant a merge landed on. `render` is no use for that:
    /// it consumes the outcome, and `Outcome` is deliberately not `Clone`.
    fn reasons(out: &Outcome) -> String {
        match out {
            Outcome::Wired { notes, .. }
            | Outcome::Unwired { notes, .. }
            | Outcome::Already { notes } => notes.join(", "),
            Outcome::Skipped(why) | Outcome::Failed(why) => why.clone(),
        }
    }

    // -- the two layouts --------------------------------------------------

    /// The layout is the whole point: written to the other's layout, each harness
    /// loads nothing and says nothing, and there is no error to notice it by. Both
    /// paths are composed from the pieces the resolvers in each binary use — omp's
    /// flat `*.ts` scan of its extensions directory, and pi's
    /// `indexNames: ["index.ts", "index.js"]` directory resolution.
    #[test]
    fn the_two_hosts_get_their_own_discovery_layout() {
        assert_eq!(
            extension_rel(Host::Pi).expect("pi has an extension"),
            PathBuf::from(".pi/agent/extensions/memory-wire/index.ts"),
            "pi auto-discovers a directory's index.ts, not a flat file"
        );
        assert_eq!(
            extension_rel(Host::Omp).expect("omp has an extension"),
            PathBuf::from(".omp/agent/extensions/memory-wire.ts"),
            "omp loads flat *.ts files out of its extensions directory"
        );
        for host in [Host::ClaudeCode, Host::Codex, Host::Hermes] {
            assert!(extension_rel(host).is_none(), "{} has no extension surface", host.id());
        }
    }

    /// `include_str!` and the file on disk are the same bytes, so a released
    /// binary and the repository cannot drift: editing the template without
    /// rebuilding installs the old extension, which is exactly the failure a
    /// copy of an in-tree file cannot have.
    #[test]
    fn the_embedded_source_is_the_one_on_disk() {
        let on_disk = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/integrations/extension/memory-wire.ts");
        assert_eq!(
            std::fs::read_to_string(&on_disk).unwrap_or_default(),
            SOURCE,
            "the template on disk and the copy in the binary differ"
        );
    }

    /// The file an install writes is the embedded source, byte for byte. A drift
    /// here would be invisible: both files parse, both register both events, and
    /// only one of them is what `connect` promised to install.
    #[test]
    fn the_installed_file_is_byte_identical_to_the_embedded_source() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("identical", host);
            assert!(matches!(run(host, &home, false), Outcome::Wired { .. }));
            assert_eq!(read_ext(&home, host), SOURCE, "{} installed a different file", host.id());
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// The version is read out of the template rather than declared beside it, so
    /// the two cannot drift: a second copy is a second thing to forget to bump.
    /// The template has to declare one — a marker nothing reads is a comment — and
    /// a line that merely mentions the word is not a declaration, case included.
    #[test]
    fn the_connect_bank_is_baked_into_the_extension() {
        let home = tmp_home("bank-baked");
        apply(Host::Omp, &home, false, &home.join("data"), Some("omp"));
        let file = read_ext(&home, Host::Omp);
        assert!(file.contains("var DEFAULT_BANK = \"omp\";"), "bank baked: {file}");
        // A reconnect with the same bank then reports already-wired, because
        // the idempotence compare runs against the same rendering.
        let out = apply(Host::Omp, &home, false, &home.join("data"), Some("omp"));
        assert!(matches!(out, Outcome::Already { .. }), "{out:?}");
        let out = apply(Host::Omp, &home, false, &home.join("data"), Some("other"));
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        assert!(read_ext(&home, Host::Omp).contains("var DEFAULT_BANK = \"other\";"));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_version_is_read_out_of_the_marker_rather_than_declared_here() {
        assert_eq!(version(SOURCE), Some("2"), "the template declares no readable version");
        assert_eq!(version("// MEMORY_WIRE_EXTENSION_VERSION: 3"), Some("3"));
        assert_eq!(version("const V = 1; // MEMORY_WIRE_EXTENSION_VERSION 12"), Some("12"));
        // Tolerating the `v` prefix and leading zeros is deliberate: a template
        // is free to write `v0.5.1`-shaped text, and a parse that returned
        // nothing would silently downgrade every upgrade to "replace nothing".
        assert_eq!(version("// MEMORY_WIRE_EXTENSION_VERSION v2"), Some("2"));
        assert_eq!(version("// MEMORY_WIRE_EXTENSION_VERSION 007"), Some("7"));
        assert_eq!(version("// MEMORY_WIRE_EXTENSION_VERSION 0"), Some("0"), "v0 is a version");
        // A line that only mentions the word is not a declaration.
        assert_eq!(
            version("// the MEMORY_WIRE_EXTENSION_VERSION marker is read here"),
            None
        );
        assert_eq!(version("// memory-wire-extension-version 1"), None, "case-sensitive");
    }

    // -- the install ------------------------------------------------------

    /// omp's outcome is a merge of two passes, so its events carry the MCP key
    /// too; the extension's own event is the one asserted here, and a first
    /// install backs up nothing because there is nothing to preserve.
    #[test]
    fn install_writes_the_extension_and_reports_where() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("write", host);
            let Outcome::Wired { events, backup, .. } = run(host, &home, false) else {
                panic!("{}: expected a wired outcome", host.id());
            };
            let rel = extension_rel(host).expect("rel").display().to_string();
            assert!(events.contains(&rel), "{}: {events:?}", host.id());
            assert!(backup.is_none(), "{}: nothing to preserve on a first install", host.id());
            assert!(home.join(extension_rel(host).expect("rel")).is_file());
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// The directory is created, because neither host creates it for us and pi's
    /// path is two levels we do not own: `extensions/`, then `memory-wire/`.
    #[test]
    fn install_creates_the_directory_it_writes_into() {
        let home = tmp_home("mkdir");
        // `~/.pi` exists, the extensions tree under it does not.
        std::fs::create_dir_all(home.join(".pi")).expect("pi home");
        assert!(matches!(run(Host::Pi, &home, false), Outcome::Wired { .. }));
        assert!(home.join(".pi/agent/extensions/memory-wire/index.ts").is_file());
        std::fs::remove_dir_all(&home).ok();
    }

    /// A second install is a no-op: the bytes already match, so the file is not
    /// rewritten and its mtime does not move. Both matter — the mtime is what
    /// makes "we left it alone" checkable rather than merely claimed.
    #[test]
    fn a_second_install_reports_already_and_rewrites_nothing() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("idem", host);
            assert!(matches!(run(host, &home, false), Outcome::Wired { .. }));
            let after = read_ext(&home, host);
            let stamp = std::fs::metadata(home.join(extension_rel(host).expect("rel")))
                .expect("stat")
                .modified()
                .expect("mtime");
            assert!(
                matches!(run(host, &home, false), Outcome::Already { .. }),
                "{}: a second install is a no-op",
                host.id()
            );
            assert_eq!(read_ext(&home, host), after);
            assert_eq!(
                std::fs::metadata(home.join(extension_rel(host).expect("rel")))
                    .expect("stat")
                    .modified()
                    .expect("mtime"),
                stamp,
                "{}: an unchanged file is not rewritten",
                host.id()
            );
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// An older build of *our* extension is ours to replace, and the swap is
    /// named, so a real upgrade is told apart from a no-op. A foreign file at the
    /// same path is the neighbouring case, and is refused instead.
    #[test]
    fn a_stale_extension_is_replaced_and_the_versions_named() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("stale", host);
            let path = home.join(extension_rel(host).expect("rel"));
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            // A real v0: the marker is present, so this is ours to replace.
            let older =
                "// memory-wire\n// MEMORY_WIRE_EXTENSION_VERSION 0\nexport default function (pi) {}\n";
            std::fs::write(&path, older).expect("write");
            let Outcome::Wired { notes, .. } = run(host, &home, false) else {
                panic!("{}: expected a wired outcome", host.id());
            };
            assert!(
                notes.iter().any(|n| n.contains("replaced extension v0 with v")),
                "{}: the version swap is not reported: {notes:?}",
                host.id()
            );
            assert_eq!(read_ext(&home, host), SOURCE);
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// A foreign file at our path is refused byte-identical, and nothing is
    /// installed for it — no file, and no backup directory either, because a
    /// refused host must leave no trace for the next run to find. The file below
    /// *mentions* `memory-wire`, which is exactly what a user's own notes about
    /// this project would contain; the second marker is what tells them apart.
    #[test]
    fn a_foreign_extension_is_refused_and_left_byte_identical() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("foreign", host);
            let path = home.join(extension_rel(host).expect("rel"));
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            // A file that names us, and is not ours: the marker substring is not
            // enough, which is why both markers are required.
            let hostile = "// memory-wire: my own notes\nimport something from \"npm\";\n";
            std::fs::write(&path, hostile).expect("write");
            let out = run(host, &home, false);
            let Outcome::Failed(why) = &out else {
                panic!("{}: expected a refusal, got {out:?}", host.id());
            };
            assert!(out.is_failure(), "{}: a refusal must exit nonzero", host.id());
            assert!(why.contains("left untouched"), "{why}");
            assert_eq!(
                std::fs::read_to_string(&path).expect("read"),
                hostile,
                "{}: refusal must leave the file byte-identical",
                host.id()
            );
            assert!(
                !home.join("data/backups").exists(),
                "{}: a refused host must not create a backup",
                host.id()
            );
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// Install then uninstall leaves the host home holding only what the harness
    /// itself owns. Everything we created is gone: the file, pi's `memory-wire/`
    /// directory or omp's `extensions/`, and that `extensions/` directory when
    /// nothing else was in it. `agent/` is the harness's own and survives.
    #[test]
    fn install_then_uninstall_leaves_the_home_as_it_found_it() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("roundtrip", host);
            assert!(matches!(run(host, &home, false), Outcome::Wired { .. }));
            assert!(matches!(run(host, &home, true), Outcome::Unwired { .. }));
            assert!(
                !home.join(extension_rel(host).expect("rel")).exists(),
                "{}: the file is gone",
                host.id()
            );
            // `agent/` is the harness's own directory and survives, so the claim
            // is the three specific paths rather than a listing that would need
            // an excuse attached to it.
            assert!(
                !home
                    .join(host.detect_rel())
                    .join("agent/extensions/memory-wire")
                    .exists(),
                "{}: the directory we created is gone",
                host.id()
            );
            assert!(
                !home
                    .join(host.detect_rel())
                    .join("agent/extensions")
                    .exists(),
                "{}: an extensions directory we emptied is gone too",
                host.id()
            );
            assert!(
                home.join(host.detect_rel()).join("agent").is_dir(),
                "{}: the harness's own directory survives, whatever the layout",
                host.id()
            );
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// A stale file that carries a name we would write is still refused unless it
    /// carries the version marker too — the pair is what says "this is ours,
    /// replace me", and the name alone is not enough. The neighbouring test covers
    /// a real v0, which *is* replaced.
    #[test]
    fn a_marker_without_a_version_is_refused() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("noversion", host);
            let path = home.join(extension_rel(host).expect("rel"));
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            let unversioned = "// memory-wire\nexport default function (pi) {}\n";
            std::fs::write(&path, unversioned).expect("write");
            let out = run(host, &home, false);
            assert!(
                matches!(&out, Outcome::Failed(w) if w.contains("MEMORY_WIRE_EXTENSION_VERSION")),
                "{}: expected a refusal, got {out:?}",
                host.id()
            );
            assert_eq!(std::fs::read_to_string(&path).expect("read"), unversioned);
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// A sibling in the directory we prune: for pi that directory is
    /// `extensions/`, so a `remove_dir_all` there would take a third-party
    /// extension with it.
    #[test]
    fn a_sibling_extension_survives_our_uninstall() {
        for (host, sibling) in [
            (Host::Pi, ".pi/agent/extensions/other/index.ts"),
            (Host::Omp, ".omp/agent/extensions/other.ts"),
        ] {
            let home = detected("sibling", host);
            let sib = home.join(sibling);
            std::fs::create_dir_all(sib.parent().expect("parent")).expect("mkdir");
            std::fs::write(&sib, "// somebody else's extension\n").expect("write");
            assert!(matches!(run(host, &home, false), Outcome::Wired { .. }));
            assert!(matches!(run(host, &home, true), Outcome::Unwired { .. }));
            assert!(sib.is_file(), "{}: a dir we did not create is not removed", host.id());
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// pi's file sits one level *below* `extensions/`, so its removal has to walk
    /// up one and stop there, leaving the harness's `agent/` alone even when it
    /// is empty.
    #[test]
    fn pi_removes_the_directory_it_created_but_not_the_harness_tree() {
        let home = detected("prune", Host::Pi);
        assert!(matches!(run(Host::Pi, &home, false), Outcome::Wired { .. }));
        assert!(matches!(run(Host::Pi, &home, true), Outcome::Unwired { .. }));
        assert!(!home.join(".pi/agent/extensions/memory-wire").exists());
        assert!(!home.join(".pi/agent/extensions").exists());
        assert!(home.join(".pi/agent").is_dir(), "the harness tree is not ours to remove");
        assert!(home.join(".pi").is_dir());
        std::fs::remove_dir_all(&home).ok();
    }

    /// A pre-existing host directory is not ours to remove, and the prune is
    /// bounded so a pi install which removed the only extension in `extensions/`
    /// cannot walk up into it. `settings.json` is what a real pi has there, and is
    /// also what keeps the directory from being empty — the case the bound is for.
    #[test]
    fn a_pre_existing_agent_directory_survives_our_prune() {
        let home = detected("preexisting", Host::Pi);
        std::fs::create_dir_all(home.join(".pi/agent")).expect("agent home");
        std::fs::write(home.join(".pi/agent/settings.json"), "{}\n").expect("write");
        assert!(matches!(run(Host::Pi, &home, false), Outcome::Wired { .. }));
        assert!(matches!(run(Host::Pi, &home, true), Outcome::Unwired { .. }));
        assert!(!home.join(".pi/agent/extensions").exists());
        assert_eq!(
            std::fs::read_to_string(home.join(".pi/agent/settings.json")).expect("read"),
            "{}\n"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    /// omp's file sits *in* `extensions/`, so the prune reaches no further than
    /// that one directory, and the harness's `agent/` above it survives.
    #[test]
    fn omp_removes_only_the_extensions_directory() {
        let home = detected("prune-omp", Host::Omp);
        assert!(matches!(run(Host::Omp, &home, false), Outcome::Wired { .. }));
        assert!(matches!(run(Host::Omp, &home, true), Outcome::Unwired { .. }));
        assert!(!home.join(".omp/agent/extensions").exists());
        assert!(home.join(".omp/agent").is_dir());
        std::fs::remove_dir_all(&home).ok();
    }

    /// A non-empty `extensions/` stops the prune one level below it, so a
    /// directory the user is still looking at survives.
    #[test]
    fn a_non_empty_extensions_directory_is_left_in_place() {
        let home = detected("nonempty", Host::Pi);
        let sib = home.join(".pi/agent/extensions/other/index.ts");
        std::fs::create_dir_all(sib.parent().expect("parent")).expect("mkdir");
        std::fs::write(&sib, "// somebody else's extension\n").expect("write");
        assert!(matches!(run(Host::Pi, &home, false), Outcome::Wired { .. }));
        assert!(matches!(run(Host::Pi, &home, true), Outcome::Unwired { .. }));
        assert!(!home.join(".pi/agent/extensions/memory-wire").exists());
        assert!(home.join(".pi/agent/extensions").is_dir());
        assert!(sib.is_file());
        std::fs::remove_dir_all(&home).ok();
    }

    /// An uninstall with nothing to remove says so rather than failing. omp is a
    /// merge of two passes, so its no-op lands on `Already` carrying both reasons
    /// as notes — the shape `connect`'s codex uninstall already has, and why its
    /// test accepts either.
    #[test]
    fn uninstalling_an_absent_extension_says_so_rather_than_failing() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("gone", host);
            let out = run(host, &home, true);
            assert!(
                matches!(&out, Outcome::Skipped(_) | Outcome::Already { .. }),
                "{}: expected a no-op, got {out:?}",
                host.id()
            );
            assert!(!out.is_failure());
            let why = reasons(&out);
            assert!(
                why.contains("no memory-wire extension to remove"),
                "{}: the reason is not reported: {why}",
                host.id()
            );
            assert!(!home.join(extension_rel(host).expect("rel")).exists());
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// The uninstall refuses exactly as the install does: a foreign file at our
    /// path is not ours to delete, and `--uninstall` is not a licence to clear
    /// the directory.
    #[test]
    fn uninstall_refuses_a_file_it_did_not_write() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("notours", host);
            let path = home.join(extension_rel(host).expect("rel"));
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            let foreign = "// my notes, which mention memory-wire\n";
            std::fs::write(&path, foreign).expect("write");
            let out = run(host, &home, true);
            assert!(
                matches!(&out, Outcome::Failed(w) if w.contains("left untouched")),
                "{}: expected a refusal, got {out:?}",
                host.id()
            );
            assert_eq!(std::fs::read_to_string(&path).expect("read"), foreign);
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// The pre-edit file is backed up before the first write, under a directory
    /// named for the host, holding the bytes the file had rather than the ones
    /// now in it.
    #[test]
    fn a_pre_existing_extension_is_backed_up_before_the_first_write() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("backup", host);
            let path = home.join(extension_rel(host).expect("rel"));
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            let older = "// memory-wire, older build\n// MEMORY_WIRE_EXTENSION_VERSION 0\n";
            std::fs::write(&path, older).expect("write");
            let Outcome::Wired { backup, .. } = run(host, &home, false) else {
                panic!("{}: expected a wired outcome", host.id());
            };
            let b = backup.expect("a backup, because the file already existed");
            assert!(
                b.to_string_lossy().contains(&format!("{}-", host.id())),
                "the backup directory is named for the host: {}",
                b.display()
            );
            assert_eq!(
                std::fs::read_to_string(b.join(extension_rel(host).expect("rel")))
                    .expect("backed up copy"),
                older,
                "{}: the backup is the file as it was",
                host.id()
            );
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// An absent host is a `Skipped`, not a failure: a machine that has never run
    /// pi is not a machine in need of a pi extension, and conjuring the host home
    /// would be the installer inventing an installation.
    #[test]
    fn a_machine_without_the_host_is_skipped_not_created() {
        for host in [Host::Pi, Host::Omp] {
            let home = tmp_home("absent");
            assert!(matches!(run(host, &home, false), Outcome::Skipped(_)));
            assert!(
                !home.join(host.detect_rel()).exists(),
                "{}: no host home is conjured up",
                host.id()
            );
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// `installed` is what `connect --list` reads, so it has to agree with what
    /// `connect` writes — including reporting a foreign file as *not* installed
    /// rather than as installed.
    #[test]
    fn the_installed_read_agrees_with_the_install() {
        for host in [Host::Pi, Host::Omp] {
            let home = detected("read", host);
            assert!(!installed(host, &home), "{}: nothing is installed yet", host.id());
            // Reading must not conjure the directory it looks in, or two of
            // `connect --list` on a machine that has neither host would leave
            // behind the very trees it was asked about.
            assert_eq!(dir_names(&home), [host.detect_rel()], "{}: a read wrote", host.id());
            assert!(matches!(run(host, &home, false), Outcome::Wired { .. }));
            assert!(installed(host, &home));
            assert!(matches!(run(host, &home, true), Outcome::Unwired { .. }));
            assert!(!installed(host, &home));
            assert!(!installed(Host::ClaudeCode, &home), "a host with no rel is never installed");
            std::fs::remove_dir_all(&home).ok();
        }
    }
}
