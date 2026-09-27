//! Hook-based auto-capture (from agentmemory).
//!
//! SessionStart / PostToolUse / Stop hooks -> dedup -> privacy filter ->
//! raw observation -> index. See PLAN.md Phase 2.

use std::sync::LazyLock;

use regex::Regex;
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Redaction rules
//
// Every pattern below is linear: quantifiers are either bounded (`{7,15}`) or
// greedy over a disjoint character class, and none of them nest. Combined with
// the `regex` crate's non-backtracking NFA simulation (no Perl-style backtracking
// stack), that rules out catastrophic blowup -- cost stays O(len x states).
//
// Each `Regex::new` used to run on every `redact_pii` call and dominated the
// function (~0.208 ms/call in release, ~23% of retain). Compiling them once
// behind `LazyLock` leaves only the scans.
// ---------------------------------------------------------------------------

static PRIVATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<private>.*?</private>").expect("private regex")
});

/// Paired BEGIN/END armoured blocks, multiline, case-insensitive. The algorithm
/// label stays inside a character class that excludes newlines, so it can never
/// run past the dashes.
static PEM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----")
        .expect("pem regex")
});

static OPENAI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"sk-[A-Za-z0-9_-]{8,}").expect("openai regex"));

static GITHUB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"ghp_[A-Za-z0-9]{8,}|github_pat_[A-Za-z0-9_]{8,}").expect("github regex")
});

static AWS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"AKIA[0-9A-Z]{16}").expect("aws regex"));

static BEARER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Bearer\s+[A-Za-z0-9._~+\-/=]{8,}").expect("bearer regex")
});

/// Bot / user / app / refresh / session Slack tokens, plus the app-level `xapp-`.
static SLACK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"xox[abprs]-[A-Za-z0-9-]{10,}|xapp-[A-Za-z0-9-]{10,}").expect("slack regex")
});

/// Google API keys are exactly `AIza` plus 35 URL-safe characters.
static GOOGLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"AIza[0-9A-Za-z_\-]{35}").expect("google regex"));

/// Compact JWT serialization: an `eyJ` header plus two more base64url segments.
static JWT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"eyJ[-A-Za-z0-9_]+\.[-A-Za-z0-9_]+\.[-A-Za-z0-9_]+").expect("jwt regex")
});

static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9\-]+(?:\.[A-Za-z]{2,})+").expect("email regex")
});

/// Phone numbers, deliberately conservative: every alternative carries either a
/// leading `+` or an explicit separator shape (3-3-4, optionally behind a
/// country-code group), so bare integers, versions, dates and UUIDs are left
/// alone. Alternatives are spelled out rather than factored into a repeated
/// group: an optional separator inside `(?:...){0,3}` makes the match split in
/// surprising ways (`+1 (555) 123-4567` became two partial hits).
static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\+[0-9]{7,15}\b",                                           // +919205112559
        r"|\+[0-9]{1,3}[ \-][0-9]{2,5}[ \-][0-9]{3,4}(?:[ \-][0-9]{2,5})?", // +91 92051 12559
        r"|\+[0-9]{1,3}[ .\-]?\(?[0-9]{3}\)?[ .\-][0-9]{3}[ .\-][0-9]{4}",   // +1 (555) 123-4567
        r"|[0-9]{1,3}[ .\-]\(?[0-9]{3}\)?[ .\-][0-9]{3}[ .\-][0-9]{4}",      // 1-800-555-0199
        r"|\(?[0-9]{3}\)?[ .\-][0-9]{3}[ .\-][0-9]{4}",                     // (555) 123-4567
    ))
    .expect("phone regex")
});

/// Order matters: earlier rules win, so multiline blocks run before the
/// token-shaped substrings that can live inside them -- `Bearer <jwt>` must
/// report `bearer`, not `jwt`.
static RULES: &[(&LazyLock<Regex>, &str)] = &[
    (&PRIVATE_RE, "[REDACTED:private]"),
    (&PEM_RE, "[REDACTED:pem_key]"),
    (&OPENAI_RE, "[REDACTED:api_key]"),
    (&GITHUB_RE, "[REDACTED:github_token]"),
    (&AWS_RE, "[REDACTED:aws_key]"),
    (&BEARER_RE, "[REDACTED:bearer]"),
    (&SLACK_RE, "[REDACTED:slack_token]"),
    (&GOOGLE_RE, "[REDACTED:google_api_key]"),
    (&JWT_RE, "[REDACTED:jwt]"),
    (&EMAIL_RE, "[REDACTED:email]"),
    (&PHONE_RE, "[REDACTED:phone]"),
];

/// Redact secrets/PII before storage (Hindsight Memory-Defense starter set).
///
/// Replaces API keys, tokens, contact details and `<private>` blocks with
/// `[REDACTED:*]` markers. Covered shapes: `<private>` blocks, PEM private-key
/// blocks, `sk-`, `ghp_`/`github_pat_`, `AKIA`, `Bearer`, Slack `xox*`/`xapp-`,
/// Google `AIza`, JWTs, emails and phone numbers (see the `RULES` table).
///
/// Each rule is asked whether it matches before it is applied: `replace_all`
/// returns an owned copy of the *whole* input whether or not anything matched,
/// so without the guard a retain of ordinary prose copied the string once per
/// rule it had nothing to say about. The guard is a scan, and the scan is what
/// `replace_all` would have done anyway — so a rule that does match costs one
/// pass it was already paying.
pub fn redact_pii(input: &str) -> String {
    let mut out = input.to_string();
    for &(re, repl) in RULES {
        if re.is_match(&out) {
            out = re.replace_all(&out, repl).into_owned();
        }
    }
    out
}

/// Hash content with SHA-256 (dedup key).
///
/// Returns the 32-byte digest of `content`.
pub fn hash_content(content: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

    #[test]
    fn redact_should_mask_openai_keys() {
        assert_eq!(redact_pii("key sk-abcDEF123456"), "key [REDACTED:api_key]");
    }

    #[test]
    fn redact_should_mask_github_tokens() {
        assert!(redact_pii("tok ghp_abcdefgh12345678").contains("[REDACTED:github_token]"));
    }

    #[test]
    fn redact_should_mask_aws_keys() {
        assert!(redact_pii("id AKIAIOSFODNN7EXAMPLE").contains("[REDACTED:aws_key]"));
    }

    #[test]
    fn redact_should_mask_private_blocks() {
        assert_eq!(
            redact_pii("a <private>secret</private> b"),
            "a [REDACTED:private] b"
        );
    }

    #[test]
    fn redact_should_passthrough_clean_text() {
        assert_eq!(redact_pii("hello world"), "hello world");
    }

    #[test]
    fn redact_should_match_private_blocks_case_insensitively() {
        assert_eq!(
            redact_pii("a <PRIVATE>secret</PRIVATE> b"),
            "a [REDACTED:private] b"
        );
        assert_eq!(
            redact_pii("a <Private>secret</Private> b"),
            "a [REDACTED:private] b"
        );
    }

    #[test]
    fn redact_should_mask_pem_private_key_blocks() {
        let pem = "before\n-----BEGIN RSA PRIVATE KEY-----\n\
                   MIIEowIBAAKCAQEA7Zx0h9Ab3Cd4Ef5Gh6Ij7Kl8Mn9Op0Qr\n\
                   -----END RSA PRIVATE KEY-----\nafter";
        assert_eq!(redact_pii(pem), "before\n[REDACTED:pem_key]\nafter");
    }

    #[test]
    fn redact_should_mask_pem_blocks_case_insensitively() {
        let pem = "-----BEGIN private key-----\nMIIabcDEF\n-----END PRIVATE KEY-----";
        assert_eq!(redact_pii(pem), "[REDACTED:pem_key]");
    }

    #[test]
    fn redact_should_leave_public_pem_blocks_alone() {
        // Certificates are public; only PRIVATE KEY material is redacted.
        let cert = "-----BEGIN CERTIFICATE-----\nMIIBszCCAV2gAwIBAgIB\n-----END CERTIFICATE-----";
        assert_eq!(redact_pii(cert), cert);
    }

    #[test]
    fn redact_should_mask_jwts() {
        assert_eq!(redact_pii(&format!("t={JWT}")), "t=[REDACTED:jwt]");
        // A bare `eyJ` with no segments is not a token.
        assert_eq!(redact_pii("the eyJ prefix alone"), "the eyJ prefix alone");
    }

    #[test]
    fn redact_should_keep_bearer_winning_over_jwt() {
        assert_eq!(
            redact_pii(&format!("Authorization: Bearer {JWT}")),
            "Authorization: [REDACTED:bearer]"
        );
    }

    #[test]
    fn redact_should_mask_slack_tokens() {
        assert!(redact_pii("s xoxb-123456789012-123456789012-abcdefghijkl")
            .contains("[REDACTED:slack_token]"));
        assert!(redact_pii("s xoxp-123456789012-abcdefghijkl").contains("[REDACTED:slack_token]"));
        assert!(redact_pii("s xoxa-123456789012-abcdefghijkl").contains("[REDACTED:slack_token]"));
        assert!(redact_pii("s xapp-1-A0923Y3F5R-1234567890123-abc")
            .contains("[REDACTED:slack_token]"));
        // Too short to be a token.
        assert_eq!(redact_pii("s xoxa-"), "s xoxa-");
    }

    #[test]
    fn redact_should_mask_google_api_keys() {
        // AIza + 35 chars, the documented Google API key shape.
        assert_eq!(
            redact_pii("k=AIza0123456789abcdefghijklmnopqrstUVWXY"),
            "k=[REDACTED:google_api_key]"
        );
        // The bare prefix is not a key.
        assert_eq!(redact_pii("AIza is a prefix"), "AIza is a prefix");
    }

    #[test]
    fn redact_should_mask_email_addresses() {
        assert_eq!(
            redact_pii("mail me at ishan@example.com ok"),
            "mail me at [REDACTED:email] ok"
        );
        assert_eq!(redact_pii("a.b+tag@sub.domain.co.uk"), "[REDACTED:email]");
        // Scoped package names are not addresses.
        assert_eq!(redact_pii("@types/node"), "@types/node");
    }

    #[test]
    fn redact_should_mask_phone_numbers() {
        assert_eq!(redact_pii("call +919205112559 now"), "call [REDACTED:phone] now");
        assert_eq!(redact_pii("tel +1 (555) 123-4567."), "tel [REDACTED:phone].");
        assert_eq!(redact_pii("1-800-555-0199"), "[REDACTED:phone]");
        assert_eq!(redact_pii("555-123-4567"), "[REDACTED:phone]");
    }

    #[test]
    fn redact_should_not_treat_plain_numbers_as_phones() {
        for clean in [
            "release 1.2.3 on 2026-09-27 has 3 bugs",
            "took 1234567 ms",
            "id 550e8400-e29b-41d4-a716-446655440000",
            "cost 1,234.56 usd at 12:34:56",
        ] {
            assert_eq!(redact_pii(clean), clean, "false positive on {clean:?}");
        }
    }
}
