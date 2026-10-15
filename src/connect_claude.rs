//! Claude Code's MCP entry: the second file of a two-surface install.
//!
//! The hooks this host fires live in `~/.claude/settings.json` (the JSON pass in
//! `connect.rs`); its MCP servers live in `~/.claude.json`, a large JSON object
//! the CLI rewrites itself (`numStartups`, `tipsHistory`, …). This module owns
//! exactly one key of that file: `mcpServers["memory-wire"]`.
//!
//! The entry is a URL, not an argv — verified live on this machine, where a
//! working HTTP MCP server is registered as
//! `{"url": "http://…/mcp", "type": "http"}` — so the bank travels the same way
//! [`crate::connect_codex`] sends it: as the `/mcp/<bank>` path segment of the
//! stateless mount, which pins the whole mount and refuses a call naming a
//! different bank.
//!
//! Ownership is the **URL** we wrote, not the key name: a `memory-wire` key
//! whose URL is not ours was written by somebody else and is left alone in both
//! directions, which is the P1.3 rule the argv hosts enforce in
//! [`crate::connect::edit_mcp`]. A JSONC file (`~/.copilot/config.json` carries
//! `//` comments) or anything that does not parse as one JSON object is refused
//! untouched, per the house rule: a malformed host config is refused, not
//! rewritten.

use std::path::Path;

use serde_json::{json, Value};

use crate::connect::{backup, timestamp, Host, Outcome};
use crate::paths;

/// Where Claude Code keeps MCP servers, home-relative. Distinct from the hooks
/// file (`.claude/settings.json`) the JSON pass in `connect.rs` edits.
const CONFIG_REL: &str = ".claude.json";

/// The key inside the config that maps server names to entries.
const SERVERS_KEY: &str = "mcpServers";

/// Install or remove the Claude Code MCP entry. See the module docs.
pub fn apply(home: &Path, uninstall: bool, data_root: &Path, bank: Option<&str>) -> Outcome {
    let rel = CONFIG_REL;
    let path = home.join(rel);
    let raw = match paths::read_or_empty(&path) {
        Ok(s) => s,
        Err(e) => return Outcome::Failed(format!("{}: {e}", path.display())),
    };
    // An absent config is not an error: the CLI creates it on first run, and a
    // config we write from scratch would be overwritten wholesale anyway.
    if raw.trim().is_empty() {
        return Outcome::Skipped(format!("{rel}: absent; the CLI creates it"));
    }
    let mut root: Value = match serde_json::from_str::<Value>(&raw) {
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
    };

    let ours = mcp_url(bank);
    let current = root
        .get(SERVERS_KEY)
        .and_then(|s| s.get(crate::connect::MARKER))
        .and_then(entry_url);

    if uninstall {
        let Some(current) = current else {
            return Outcome::Skipped(format!("{rel}: no memory-wire MCP entry"));
        };
        if current != ours {
            return Outcome::Skipped(format!(
                "{rel}: mcpServers.{} url is not ours; left it alone",
                crate::connect::MARKER
            ));
        }
        let saved = match backup(data_root, home, Host::ClaudeCode, &timestamp(), Path::new(rel)) {
            Ok(b) => b,
            Err(e) => return Outcome::Failed(e),
        };
        // Remove only our key; a `mcpServers` map emptied by that removal is
        // our own residue and goes too, everything else in the file stays.
        if let Some(servers) = root.get_mut(SERVERS_KEY).and_then(Value::as_object_mut) {
            servers.remove(crate::connect::MARKER);
            if servers.is_empty() {
                if let Some(obj) = root.as_object_mut() {
                    obj.remove(SERVERS_KEY);
                }
            }
        }
        let mut text = serde_json::to_string_pretty(&root).unwrap_or_default();
        text.push('\n');
        if let Err(e) = paths::write_atomic(&path, &text) {
            return Outcome::Failed(e);
        }
        return Outcome::Unwired {
            backup: saved,
            events: vec![format!("{rel}: mcpServers.{}", crate::connect::MARKER)],
            notes: Vec::new(),
        };
    }

    if current == Some(ours.clone()) {
        return Outcome::Already {
            notes: vec![format!("{rel}: mcpServers.{} url already current", crate::connect::MARKER)],
        };
    }
    // A `memory-wire` key whose url is not ours was written by somebody else
    // (P1.3): installing over it would displace their server under the guise of
    // adding ours, exactly the hazard the argv hosts refuse in `edit_mcp`.
    if let Some(theirs) = &current {
        return Outcome::Failed(format!(
            "{rel}: mcpServers.{} holds a url that is not ours ({theirs}); left untouched",
            crate::connect::MARKER
        ));
    }
    let saved = match backup(data_root, home, Host::ClaudeCode, &timestamp(), Path::new(rel)) {
        Ok(b) => b,
        Err(e) => return Outcome::Failed(e),
    };
    {
        let obj = root
            .as_object_mut()
            .expect("root checked to be an object above");
        let servers = obj
            .entry(SERVERS_KEY)
            .or_insert_with(|| json!({}));
        let Some(map) = servers.as_object_mut() else {
            return Outcome::Failed(format!(
                "{}: `{SERVERS_KEY}` is not an object; left untouched",
                path.display()
            ));
        };
        map.insert(
            crate::connect::MARKER.to_string(),
            json!({ "url": ours, "type": "http" }),
        );
    }
    let mut text = serde_json::to_string_pretty(&root).unwrap_or_default();
    text.push('\n');
    if let Err(e) = paths::write_atomic(&path, &text) {
        return Outcome::Failed(e);
    }
    Outcome::Wired {
        backup: saved,
        events: vec![format!("{rel}: mcpServers.{}", crate::connect::MARKER)],
        notes: Vec::new(),
    }
}

/// The `url` of an entry, when the entry is an object carrying a string `url`.
fn entry_url(entry: &Value) -> Option<String> {
    entry.get("url")?.as_str().map(String::from)
}

/// The URL Claude Code should dial. Same resolution and same bank-in-path rule
/// as [`crate::connect_codex::mcp_url`] — one mount, one bank spelling, two
/// hosts whose only difference is which file the URL lives in.
fn mcp_url(bank: Option<&str>) -> String {
    let endpoint = paths::endpoint();
    let base = endpoint.trim_end_matches('/');
    match bank.filter(|b| !b.is_empty()) {
        Some(b) => format!("{base}/mcp/{b}"),
        None => format!("{base}/mcp"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::Outcome;

    fn tmp_home(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-claude-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        dir
    }

    fn write(home: &Path, body: &str) {
        std::fs::write(home.join(CONFIG_REL), body).expect("write");
    }

    fn read(home: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(home.join(CONFIG_REL)).expect("read"))
            .expect("parse")
    }

    /// Install adds exactly our key under `mcpServers`, leaving the CLI's own
    /// keys untouched, and reports the file it touched.
    #[test]
    fn install_adds_our_key_and_preserves_the_rest() {
        let home = tmp_home("install");
        write(&home, r#"{"numStartups":42,"mcpServers":{"other":{"url":"http://x/mcp","type":"http"}}}"#);
        let out = apply(&home, false, &home.join("data"), Some("omp"));
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let doc = read(&home);
        assert_eq!(doc["numStartups"], 42, "foreign key survives");
        assert_eq!(doc["mcpServers"]["other"]["url"], "http://x/mcp");
        assert_eq!(
            doc["mcpServers"][crate::connect::MARKER]["url"],
            format!("{}/mcp/omp", paths::default_endpoint())
        );
        assert_eq!(doc["mcpServers"][crate::connect::MARKER]["type"], "http");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A second install with the same bank is `Already`, byte-identical file.
    #[test]
    fn reinstall_is_already_and_rewrites_nothing() {
        let home = tmp_home("already");
        write(&home, "{}");
        let first = apply(&home, false, &home.join("data"), None);
        assert!(matches!(first, Outcome::Wired { .. }), "{first:?}");
        let after = std::fs::read_to_string(home.join(CONFIG_REL)).expect("read");
        let second = apply(&home, false, &home.join("data"), None);
        assert!(matches!(second, Outcome::Already { .. }), "{second:?}");
        assert_eq!(std::fs::read_to_string(home.join(CONFIG_REL)).expect("read"), after);
        std::fs::remove_dir_all(&home).ok();
    }

    /// Uninstall removes only our key; a foreign server and the CLI's own keys
    /// survive, and a `mcpServers` we emptied goes entirely.
    #[test]
    fn uninstall_removes_only_ours() {
        let home = tmp_home("uninstall");
        write(&home, "{}");
        assert!(matches!(
            apply(&home, false, &home.join("data"), Some("omp")),
            Outcome::Wired { .. }
        ));
        // Sneak a foreign server in beside ours, as a real install would have.
        let with_foreign = read(&home);
        let mut doc = with_foreign;
        doc["mcpServers"]["other"] = json!({"url": "http://x/mcp", "type": "http"});
        std::fs::write(
            home.join(CONFIG_REL),
            serde_json::to_string_pretty(&doc).expect("ser"),
        )
        .expect("write");
        let out = apply(&home, true, &home.join("data"), Some("omp"));
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        let doc = read(&home);
        assert!(doc.get("mcpServers").and_then(|s| s.get(crate::connect::MARKER)).is_none());
        assert!(doc["mcpServers"].get("other").is_some(), "foreign server survives");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A `memory-wire` key somebody else wrote is not ours: install refuses
    /// (they asked for a wire and did not get one), uninstall skips with the
    /// reason (they asked for a teardown and there was nothing of ours). Both
    /// leave the file byte-identical.
    #[test]
    fn a_foreign_entry_under_our_name_is_refused_both_ways() {
        let foreign = r#"{"mcpServers":{"memory-wire":{"url":"http://elsewhere/mcp","type":"http"}}}"#;
        let home = tmp_home("foreign-in");
        write(&home, foreign);
        let out = apply(&home, false, &home.join("data"), None);
        assert!(matches!(&out, Outcome::Failed(why) if why.contains("not ours")), "{out:?}");
        assert_eq!(
            std::fs::read_to_string(home.join(CONFIG_REL)).expect("read"),
            foreign,
            "refusal must leave the file byte-identical"
        );
        assert!(!home.join("data/backups").exists(), "no backup for a refusal");
        std::fs::remove_dir_all(&home).ok();

        let home = tmp_home("foreign-un");
        write(&home, foreign);
        let out = apply(&home, true, &home.join("data"), None);
        assert!(matches!(&out, Outcome::Skipped(why) if why.contains("not ours")), "{out:?}");
        assert_eq!(
            std::fs::read_to_string(home.join(CONFIG_REL)).expect("read"),
            foreign
        );
        std::fs::remove_dir_all(&home).ok();
    }

    /// The uninstall side of ownership: an entry we installed under a banked
    /// URL is only removable with that same bank, or the refusal says whose it
    /// is rather than deleting a URL we did not write.
    #[test]
    fn uninstall_with_a_different_bank_refuses() {
        let home = tmp_home("unwire-bank");
        write(&home, "{}");
        assert!(matches!(
            apply(&home, false, &home.join("data"), Some("omp")),
            Outcome::Wired { .. }
        ));
        let out = apply(&home, true, &home.join("data"), Some("other"));
        assert!(matches!(&out, Outcome::Skipped(why) if why.contains("not ours")), "{out:?}");
        let doc = read(&home);
        assert!(doc["mcpServers"].get(crate::connect::MARKER).is_some(), "entry kept");
        std::fs::remove_dir_all(&home).ok();
    }

    /// Malformed JSON is refused untouched — the CLI rewrites this file itself,
    /// so a shape we do not understand is information, not garbage to fix.
    #[test]
    fn malformed_json_is_refused_untouched() {
        let home = tmp_home("malformed");
        write(&home, "{ oops");
        let out = apply(&home, false, &home.join("data"), None);
        assert!(matches!(&out, Outcome::Failed(why) if why.contains("malformed")), "{out:?}");
        assert_eq!(std::fs::read_to_string(home.join(CONFIG_REL)).expect("read"), "{ oops");
        std::fs::remove_dir_all(&home).ok();
    }

    /// An absent config is a skip, not a write: the CLI owns this file's
    /// lifecycle and creates it on first run.
    #[test]
    fn an_absent_config_is_skipped_without_creating_it() {
        let home = tmp_home("absent");
        let out = apply(&home, false, &home.join("data"), None);
        assert!(matches!(&out, Outcome::Skipped(why) if why.contains("absent")), "{out:?}");
        assert!(!home.join(CONFIG_REL).exists(), "nothing may be created");
        std::fs::remove_dir_all(&home).ok();
    }
}
