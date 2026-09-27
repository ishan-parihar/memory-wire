//! The `sqlite3 .backup` recipe from INSTALL_FOR_AGENTS.md, executed.
//!
//! The docs are only worth the round trip if the round trip is tested, so this
//! runs the documented commands rather than an in-process equivalent: `.backup`
//! through the `sqlite3` CLI, restore by `cp` once the store is closed, and the
//! stale `-wal`/`-shm` removed on the way.
//!
//! No `backup` subcommand ships (H7.4): the recipe is the interface, so the
//! recipe is what is tested.

use std::path::{Path, PathBuf};
use std::process::Command;

use memory_wire::api::MemoryService;
use memory_wire::memory::Bank;
use memory_wire::store::{SqliteStore, Store};

/// A scratch directory per test, cleaned up on the way out.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mw-backup-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Run a shell command, failing the test with its output when it does not work.
fn sh(dir: &Path, script: &str) -> String {
    let out = Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .output()
        .expect("spawn sh");
    assert!(
        out.status.success(),
        "`{script}` failed ({}): {}{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// INSTALL step 1 of the recipe, verbatim against a store no live writer holds.
/// `sqlite3` is a prerequisite of the documented recipe, so a missing one fails
/// here rather than passing the test unverified.
fn backup(store: &Path) {
    let dir = store.parent().expect("store dir").to_path_buf();
    let db = q(store);
    let said = sh(&dir, &format!("sqlite3 '{db}' \".backup '{db}.bak'\""));
    assert!(said.is_empty(), "unexpected output: {said}");
    let copy = PathBuf::from(format!("{db}.bak"));
    assert!(copy.is_file(), ".backup produced no file at {}", copy.display());
}

/// INSTALL step 2 of the recipe: the store is closed, the sidecars go, the copy
/// lands. The `rm` is the part that matters — see the second test.
fn restore(store: &Path) {
    sh(
        store.parent().expect("store dir"),
        &format!("rm -f '{db}' '{db}-wal' '{db}-shm' && cp '{db}.bak' '{db}'", db = q(store)),
    );
}

/// Row count straight from SQL, so no assertion leans on the code path that
/// just wrote the rows.
fn rows(store: &Path) -> i64 {
    rusqlite::Connection::open(store)
        .expect("open")
        .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
        .expect("count")
}

/// The store path as the shell sees it.
fn q(store: &Path) -> String {
    store.display().to_string()
}

/// Does this file carry a `memories` table? A copy of a live WAL store does not:
/// the schema itself is still sitting in the `-wal`.
fn has_memories_table(store: &Path) -> bool {
    rusqlite::Connection::open(store)
        .and_then(|conn| {
            conn.query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='memories'",
                [],
                |r| r.get::<_, i64>(0),
            )
        })
        .is_ok()
}

/// The driver's complaint about querying a file that is not a store.
fn rows_err(store: &Path) -> String {
    let got = rusqlite::Connection::open(store).and_then(|conn| {
        conn.query_row("SELECT COUNT(*) FROM memories", [], |r| r.get::<_, i64>(0))
    });
    match got {
        Ok(n) => format!("{n} rows"),
        Err(e) => e.to_string(),
    }
}

/// A service over an existing bank, the way `serve` would open one.
fn service(store: &Path) -> MemoryService<SqliteStore> {
    let svc = MemoryService::new(SqliteStore::open(store).expect("open"));
    svc.store
        .put_bank(&Bank { id: "demo".into(), name: "demo".into() })
        .expect("bank");
    svc
}

#[test]
fn a_sqlite3_backup_should_restore_every_row_and_the_fts_index() {
    let dir = scratch("roundtrip");
    let store = dir.join("agents.db");
    let (kept, doomed) = {
        let svc = service(&store);
        let kept = svc.retain("demo", "auth uses jose middleware for tokens", None);
        let doomed = svc.retain("demo", "rate limiting via token bucket", None);
        svc.retain("demo", "postgres is not on the roadmap", None)
            .expect("retain");
        (kept.expect("retain"), doomed.expect("retain"))
    }; // dropped here: the handle is closed, so the WAL is folded away

    assert_eq!(rows(&store), 3, "fixture must start with three rows");
    backup(&store);

    // Damage the store the way a bad afternoon would: drop a row and rewrite the
    // bank config, so a stale file is detectably not the backup.
    {
        let svc = MemoryService::new(SqliteStore::open(&store).expect("reopen"));
        assert!(svc.delete_memory("demo", &doomed).expect("delete"));
        svc.set_bank_config("demo", r#"{"recallMaxTokens":99}"#)
            .expect("config");
    }
    assert_eq!(rows(&store), 2, "the damage must land before the restore");
    assert!(
        MemoryService::new(SqliteStore::open(&store).expect("reopen"))
            .recall("demo", "rate limiting", 2000)
            .expect("recall")
            .is_empty(),
        "the deleted row must be unfindable while it is deleted"
    );

    restore(&store);

    assert_eq!(rows(&store), 3, "a restore must bring the row back");
    let svc = MemoryService::new(SqliteStore::open(&store).expect("reopen after restore"));
    // The FTS index came across with the pages, so the restored row is not merely
    // present but findable again — the part a copy of a live WAL store loses.
    let hits = svc.recall("demo", "rate limiting", 2000).expect("recall");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].memory.content.contains("token bucket"), "{hits:?}");
    assert_eq!(hits[0].memory.id, doomed, "the restored row keeps its id");
    let survivors = svc.recall("demo", "jose", 2000).expect("recall");
    assert_eq!(survivors.len(), 1, "{survivors:?}");
    assert!(survivors[0].memory.content.contains("jose middleware"));
    assert_eq!(svc.bank_stats("demo").expect("stats").memories, 3);
    // The config the damage wrote is gone too: a restore is whole-store, not a
    // row merge.
    assert_eq!(
        svc.bank_config("demo").expect("config"),
        "{}",
        "the post-backup config must not survive the restore"
    );
    let _ = kept;
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn copying_the_main_file_alone_should_not_be_a_backup() {
    // The caveat the recipe exists to avoid, as observed rather than asserted
    // from theory — two failures of a naive `cp`, both on one live fixture, and
    // both of them silent:
    //
    //   1. The copy is not a store. At rest the schema and every recent row live
    //      in the `-wal`, so `cp` on the main file yields a file with no
    //      `memories` table at all — a backup you cannot restore from.
    //   2. Restoring a copy without removing the sidecars is worse than not
    //      restoring: the live `-wal` still sitting next to the file replays
    //      over it, and you get the store as it is now, not the copy.
    let dir = scratch("wal");
    let store = dir.join("agents.db");
    let live = {
        let svc = service(&store);
        svc.retain("demo", "auth uses jose middleware for tokens", None)
            .expect("retain");
        // Held open across every copy below, on purpose: that is the hazard.
        svc
    };
    sh(&dir, &format!("cp '{db}' '{db}.bak'", db = q(&store)));
    let copied = PathBuf::from(format!("{}.bak", q(&store)));
    assert!(
        !has_memories_table(&copied),
        "a plain cp of a live WAL store must not be a usable store"
    );
    assert!(
        rows_err(&copied).contains("no such table"),
        "the copy is empty, not merely stale"
    );

    // Written after the copy, so it exists only in the `-wal` from here on.
    live.retain("demo", "written after the copy, only in the wal", None)
        .expect("retain");
    assert_eq!(rows(&store), 2);

    // Naive restore of that same copy, with the sidecars left where they are.
    sh(&dir, &format!("cp '{db}.bak' '{db}'", db = q(&store)));
    assert_eq!(
        rows(&store),
        2,
        "the stale -wal replays over the restored file: this is the trap"
    );
    drop(live);

    // The documented restore of the *same* copy: with the sidecars gone there is
    // nothing to replay, so what you get is what the `cp` captured — an empty
    // file, because that is what a naive `cp` of a live store captures.
    restore(&store);
    assert!(
        !has_memories_table(&store),
        "a naive cp has no schema to restore: failure 1, all over again"
    );
    std::fs::remove_dir_all(&dir).ok();
}
