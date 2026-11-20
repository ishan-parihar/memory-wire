//! Helpers the per-host `connect_*` modules share, extracted because the
//! duplication was real and invisible.
//!
//! Four functions existed two or three times each, byte-identical in body and
//! differing only in the constant each call site supplied: [`mcp_url`] in
//! claude, copilot **and** codex; [`baked`], [`carries_marker`] and [`is_ours`]
//! in both file-installing hosts; [`preserve_comment`] in codex and hermes.
//! A diff of every pair was empty. Call sites now import these directly rather
//! than going through a same-named wrapper, because a wrapper whose body is a
//! single call adds a hop and a second doc comment for nothing.
//!
//! The copy that actually costs something is the *rule*, not the lines: the
//! bank travels as a path segment in three hosts and the stateless mount pins
//! the subtree, so a host that put it somewhere else would produce an entry that
//! looks right and refuses every call. One function, three callers, one place
//! to change that rule.
//!
//! What is deliberately *not* here:
//!
//! - [`crate::connect::is_ours`] is a different function that happens to share
//!   a name. It tests a `serde_json::Value` entry for the marker; this module's
//!   [`is_ours`] reads a file from disk. Collapsing them would be worse than the
//!   duplication — one would have to serialize a value to a string it already
//!   holds.
//! - `version` and `prune_empty_dirs` differ between `connect_ext` and
//!   `connect_opencode` by the directory name they stop at (`extensions` vs
//!   `plugins`) and by how they treat a non-empty directory. Parameterising a
//!   two-line difference to save one copy is not a win; it is a second thing to
//!   read to find out what actually happens.
//!
//! The module stays `pub(crate)`: every function is an implementation detail of
//! [`crate::connect`], and making them public would add a surface no host needs.

use std::path::Path;

use crate::connect::MARKER;

/// The MCP endpoint URL for a bank, as the URL-style hosts write it.
///
/// The bank is a path segment, not a query parameter, because the stateless
/// mount pins the whole `/mcp/<bank>` subtree and refuses a call naming a
/// different bank. A host that carried the bank some other way would get an
/// entry that looks right and refuses every call.
pub(crate) fn mcp_url(bank: Option<&str>) -> String {
    let endpoint = crate::paths::endpoint();
    let base = endpoint.trim_end_matches('/');
    match bank.filter(|b| !b.is_empty()) {
        Some(b) => format!("{base}/mcp/{b}"),
        None => format!("{base}/mcp"),
    }
}

/// Render an embedded TypeScript template with the `connect --bank` value baked
/// into its `DEFAULT_BANK` line.
///
/// `source` and `default_bank_line` are per-host because the two templates are
/// different files; the *rule* is not, and the rule is what was copied.
///
/// The same rendering feeds the idempotence compare in each `apply`, so a
/// reconnect with the same bank stays `already-wired` and one with a different
/// bank is a real replace — and the extension and the MCP entry of the same
/// install cannot disagree on the namespace. The escaping is a backstop for a
/// hostile value; bank ids are slug-like.
pub(crate) fn baked(source: &str, default_bank_line: &str, bank: Option<&str>) -> String {
    match bank.map(str::trim).filter(|b| !b.is_empty()) {
        None => source.to_string(),
        Some(b) => source.replace(
            default_bank_line,
            &format!(
                "var DEFAULT_BANK = \"{}\";",
                b.replace('\\', "\\\\").replace('"', "\\\"")
            ),
        ),
    }
}

/// Whether an installed source is ours to replace.
///
/// Both markers, not just the name: a file that merely mentions `memory-wire` is
/// a user file that happens to be talking about us, and overwriting it is the
/// one thing these modules exist to avoid.
pub(crate) fn carries_marker(source: &str, version_marker: &str) -> bool {
    source.contains(MARKER) && source.contains(version_marker)
}

/// Whether the file at `path` carries both markers, i.e. whether this install
/// owns it. An unreadable file is not ours: a permission error is not evidence
/// of ownership.
pub(crate) fn is_ours(path: &Path, version_marker: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|s| carries_marker(&s, version_marker))
}

/// Replace `line`'s value while keeping its trailing `# comment`.
///
/// Both the codex TOML pass and the hermes YAML pass edit a line they did not
/// write, and a user's inline comment on a line we are about to rewrite is the
/// most likely thing in the file to be silently lost. The comment starts at the
/// first `" #"`, which is a heuristic: it cannot tell a `#` inside a quoted
/// string from a real comment, and the alternative — a TOML and a YAML parser
/// for one line — is out of proportion. Values we write are URLs and slugs, so
/// a `#` inside one is not a case this has to handle.
pub(crate) fn preserve_comment(line: &str, replacement: &str) -> String {
    match line.find(" #") {
        Some(at) => format!("{} {}", replacement, &line[at..]),
        None => replacement.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_url_carries_the_bank_as_a_segment_and_none_does_not() {
        // The endpoint is machine state (env, daemon, else the default), so
        // assert the *shape* the three URL hosts depend on rather than a value.
        // `connect_codex`'s own tests pin the concrete default.
        let bare = mcp_url(None);
        assert!(bare.ends_with("/mcp"), "no bank is a bare /mcp: {bare}");
        assert!(
            mcp_url(Some("work")).ends_with("/mcp/work"),
            "the bank is a path segment, not a query parameter"
        );
        assert_eq!(mcp_url(Some("")), bare, "an empty bank is no bank");
        assert!(!mcp_url(Some("")).ends_with('/'), "no trailing slash");
    }

    #[test]
    fn baked_replaces_the_default_bank_line_and_leaves_everything_else() {
        let src = "// head\nvar DEFAULT_BANK = \"memory-wire\";\ntail();\n";
        let got = baked(src, "var DEFAULT_BANK = \"memory-wire\";", Some("prod"));
        assert!(got.contains("var DEFAULT_BANK = \"prod\";"), "{got}");
        assert!(got.contains("// head") && got.contains("tail();"));
        assert_eq!(baked(src, "var DEFAULT_BANK = \"memory-wire\";", None), src);
        assert_eq!(
            baked(src, "var DEFAULT_BANK = \"memory-wire\";", Some(" ")),
            src
        );
    }

    #[test]
    fn baked_escapes_a_hostile_bank_value_rather_than_breaking_the_literal() {
        let src = "var DEFAULT_BANK = \"memory-wire\";";
        let got = baked(src, "var DEFAULT_BANK = \"memory-wire\";", Some("a\"b\\c"));
        assert!(got.contains("a\\\"b\\\\c"), "escaped, not injected: {got}");
    }

    #[test]
    fn carries_marker_needs_both_markers() {
        // MARKER is lowercase `memory-wire`; the version marker is per-host.
        let mine = "// memory-wire\n// MEMORY_WIRE_VERSION = 3\n";
        assert!(carries_marker(mine, "MEMORY_WIRE_VERSION"));
        assert!(
            !carries_marker("// memory-wire only\n", "MEMORY_WIRE_VERSION"),
            "the name alone must not be ownership"
        );
        assert!(
            !carries_marker("// MEMORY_WIRE_VERSION = 3\n", "MEMORY_WIRE_VERSION"),
            "the version alone must not be ownership"
        );
    }

    #[test]
    fn preserve_comment_keeps_the_trailing_comment_and_drops_it_when_absent() {
        assert_eq!(
            preserve_comment("provider: old  # my note", "provider: new"),
            "provider: new  # my note"
        );
        assert_eq!(
            preserve_comment("provider: old", "provider: new"),
            "provider: new"
        );
    }
}
