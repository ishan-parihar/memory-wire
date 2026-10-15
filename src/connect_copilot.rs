//! Copilot CLI's MCP entry: the second file of a two-surface install.
//!
//! The hooks this host fires live in `~/.copilot/settings.json` (the JSON pass
//! in `connect.rs`); its MCP servers live in `~/.copilot/mcp-config.json` — a
//! plain, user-editable definitions file, distinct from the tool-managed
//! `~/.copilot/config.json` (JSONC, rewritten by the CLI) that keeps this host
//! out of hand-editing. The schema is GitHub's own, from the Copilot CLI
//! configuration reference and the github-mcp-server installation guide:
//!
//! ```json
//! { "mcpServers": { "<name>": { "type": "http", "url": "https://…" } } }
//! ```
//!
//! The entry we write is the HTTP form — the same stateless mount the codex
//! and claude-code entries dial — with the bank pinned as the `/mcp/<bank>`
//! path segment, which pins the whole mount and refuses a call naming a
//! different bank. Ownership is the **URL** we wrote, not the key name: a
//! `memory-wire` key holding a URL that is not ours was written by somebody
//! else and is left untouched in both directions, the same P1.3 rule
//! [`crate::connect::edit_mcp`] enforces for argv hosts.

use std::path::Path;

use serde_json::{json, Value};

use crate::connect::{backup, timestamp, Host, Outcome};
use crate::paths;

/// Where Copilot CLI keeps MCP server definitions, home-relative. Verified
/// against GitHub's Copilot CLI configuration-directory reference.
const CONFIG_REL: &str = ".copilot/mcp-config.json";

/// The key inside the config that maps server names to entries.
const SERVERS_KEY: &str = "mcpServers";

/// Install or remove the Copilot CLI MCP entry. See the module docs.
pub fn apply(home: &Path, uninstall: bool, data_root: &Path, bank: Option<&str>) -> Outcome {
    let rel = CONFIG_REL;
    let path = home.join(rel);
    let raw = paths::read_or_empty(&path).unwrap_or_default();
    // An absent file is the pristine case, not an error: this is a user
    // definitions file, ours to create exactly the way cursor's `mcp.json`
    // is — unlike `~/.copilot/config.json`, which the CLI owns.
    let root: Value = if raw.trim().is_empty() {
        json!({})
    } else {
        match serde_json::from_str::<Value>(&raw) {
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
        let saved = match backup(data_root, home, Host::CopilotCli, &timestamp(), Path::new(rel)) {
            Ok(b) => b,
            Err(e) => return Outcome::Failed(e),
        };
        // Remove only our key; a `mcpServers` map the removal empties is our
        // own residue and goes too, the file itself stays — it is the user's
        // definitions file and other tools may legitimately expect it to
        // exist once it ever has.
        let mut root = root;
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
            notes: vec![format!(
                "{rel}: mcpServers.{} url already current",
                crate::connect::MARKER
            )],
        };
    }
    // A `memory-wire` key whose url is not ours was written by somebody else
    // (P1.3): installing over it would displace their server under the guise
    // of adding ours.
    if let Some(theirs) = &current {
        return Outcome::Failed(format!(
            "{rel}: mcpServers.{} holds a url that is not ours ({theirs}); left untouched",
            crate::connect::MARKER
        ));
    }
    let saved = match backup(data_root, home, Host::CopilotCli, &timestamp(), Path::new(rel)) {
        Ok(b) => b,
        Err(e) => return Outcome::Failed(e),
    };
    let mut root = root;
    {
        let obj = root
            .as_object_mut()
            .expect("root checked to be an object above");
        let servers = obj.entry(SERVERS_KEY).or_insert_with(|| json!({}));
        let Some(map) = servers.as_object_mut() else {
            return Outcome::Failed(format!(
                "{}: `{SERVERS_KEY}` is not an object; left untouched",
                path.display()
            ));
        };
        map.insert(crate::connect::MARKER.to_string(), json!({ "type": "http", "url": ours }));
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

/// Is our entry present in `~/.copilot/mcp-config.json`? Reads and nothing
/// else; the write side is [`apply`]. Used by `connect --list`'s wired answer,
/// so a hooks-only install and an MCP-only install both read as wired.
pub(crate) fn installed(home: &Path) -> bool {
    let Ok(raw) = paths::read_or_empty(&home.join(CONFIG_REL)) else {
        return false;
    };
    serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|root| {
            root.get(SERVERS_KEY)
                .and_then(Value::as_object)
                .map(|m| m.contains_key(crate::connect::MARKER))
        })
        .unwrap_or(false)
}

/// The `url` of an entry, when the entry is an object carrying a string `url`.
fn entry_url(entry: &Value) -> Option<String> {
    entry.get("url")?.as_str().map(String::from)
}

/// The URL Copilot CLI should dial. Same resolution and same bank-in-path rule
/// as [`crate::connect_codex::mcp_url`] and [`crate::connect_claude::mcp_url`]
/// — one mount, one bank spelling, three hosts whose only difference is which
/// file the URL lives in.
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
        let dir = std::env::temp_dir().join(format!("mw-copilot-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        dir
    }

    fn write(home: &Path, body: &str) {
        let path = home.join(CONFIG_REL);
        std::fs::create_dir_all(path.parent().unwrap_or(home)).expect("mkdir");
        std::fs::write(path, body).expect("write");
    }

    fn read(home: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(home.join(CONFIG_REL)).expect("read"))
            .expect("parse")
    }

    /// Install creates the definitions file on a pristine machine, with the
    /// HTTP entry GitHub's own guide documents.
    #[test]
    fn install_creates_the_entry_in_githubs_shape() {
        let home = tmp_home("install");
        let out = apply(&home, false, &home.join("data"), Some("omp"));
        assert!(matches!(out, Outcome::Wired { .. }), "{out:?}");
        let doc = read(&home);
        assert_eq!(doc["mcpServers"][crate::connect::MARKER]["type"], "http");
        assert_eq!(
            doc["mcpServers"][crate::connect::MARKER]["url"],
            format!("{}/mcp/omp", paths::default_endpoint())
        );
        std::fs::remove_dir_all(&home).ok();
    }

    /// Re-install with the same bank is `Already`, byte-identical.
    #[test]
    fn reinstall_is_already_and_rewrites_nothing() {
        let home = tmp_home("already");
        assert!(matches!(
            apply(&home, false, &home.join("data"), None),
            Outcome::Wired { .. }
        ));
        let after = std::fs::read_to_string(home.join(CONFIG_REL)).expect("read");
        let second = apply(&home, false, &home.join("data"), None);
        assert!(matches!(second, Outcome::Already { .. }), "{second:?}");
        assert_eq!(std::fs::read_to_string(home.join(CONFIG_REL)).expect("read"), after);
        std::fs::remove_dir_all(&home).ok();
    }

    /// Uninstall removes only our key; a foreign server survives.
    #[test]
    fn uninstall_removes_only_ours() {
        let home = tmp_home("uninstall");
        assert!(matches!(
            apply(&home, false, &home.join("data"), Some("omp")),
            Outcome::Wired { .. }
        ));
        let mut doc = read(&home);
        doc["mcpServers"]["other"] = json!({"type": "http", "url": "http://x/mcp"});
        std::fs::write(
            home.join(CONFIG_REL),
            serde_json::to_string_pretty(&doc).expect("ser"),
        )
        .expect("write");
        let out = apply(&home, true, &home.join("data"), Some("omp"));
        assert!(matches!(out, Outcome::Unwired { .. }), "{out:?}");
        let doc = read(&home);
        assert!(doc["mcpServers"].get(crate::connect::MARKER).is_none());
        assert!(doc["mcpServers"].get("other").is_some(), "foreign server survives");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A `memory-wire` key somebody else wrote is not ours: install refuses,
    /// uninstall skips, both byte-identical.
    #[test]
    fn a_foreign_entry_under_our_name_is_refused_both_ways() {
        let foreign = r#"{"mcpServers":{"memory-wire":{"type":"http","url":"http://elsewhere/mcp"}}}"#;
        let home = tmp_home("foreign-in");
        write(&home, foreign);
        let out = apply(&home, false, &home.join("data"), None);
        assert!(matches!(&out, Outcome::Failed(why) if why.contains("not ours")), "{out:?}");
        assert_eq!(
            std::fs::read_to_string(home.join(CONFIG_REL)).expect("read"),
            foreign
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

    /// Malformed JSON is refused untouched — a definitions file this writer
    /// did not produce is information, not garbage to fix.
    #[test]
    fn malformed_json_is_refused_untouched() {
        let home = tmp_home("malformed");
        write(&home, "{ oops");
        let out = apply(&home, false, &home.join("data"), None);
        assert!(matches!(&out, Outcome::Failed(why) if why.contains("malformed")), "{out:?}");
        assert_eq!(std::fs::read_to_string(home.join(CONFIG_REL)).expect("read"), "{ oops");
        std::fs::remove_dir_all(&home).ok();
    }
}
