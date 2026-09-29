//! TOON encoder — Token-Oriented Object Notation, spec v4.1 (2026-07-26).
//!
//! The format is specified at <https://github.com/toon-format/spec/blob/main/SPEC.md>;
//! section numbers in the doc comments below are that document's. TOON is a
//! line-oriented, indentation-based encoding of the JSON data model that declares
//! an array's length and field list once instead of repeating them per row, which
//! is where the token saving over JSON comes from on a table-shaped payload.
//!
//! # Why a hand-written encoder and not a crate
//!
//! Only one shape is ever emitted — a flat table of memory rows plus a few scalar
//! header fields — so the whole of the format that has to be right is §6 (header
//! syntax), §7 (quoting and escaping), §9.1/§9.3/§9.4 (which form a value takes),
//! §11 (delimiter scoping) and §12 (whitespace). That is a few hundred lines, and
//! the crate would have to be pinned to a spec that is still a Working Draft, so a
//! dependency would be more risk than the code it replaced.
//!
//! # Scope, stated rather than implied
//!
//! Comma is the document delimiter, so every header omits its delimiter symbol
//! (§11: "Comma (default): header omits the delimiter symbol") and the active
//! delimiter of every scope is a comma. This module therefore emits exactly one
//! form family:
//!
//! - a document of `key: value` lines, optionally followed by one array field;
//! - that array in the **empty** form (`name: []`, §9.1), the **tabular** form
//!   (a `{fields}` header plus one row per element, §9.3) when every element
//!   shares one key set, or the **list** form (`- ` items, §9.4) when they do not.
//!
//! Numbers, nested objects deeper than one level, arrays-of-arrays, and the keyed
//! tabular form (§9.5) are not implemented: nothing on the memory read routes
//! produces them, and a partial encoder that silently mis-renders a value it was
//! never asked to render is worse than one that cannot be pointed at that value.
//!
//! # Two output invariants the spec states and this module keeps
//!
//! §12: lines are separated by LF, indentation is spaces only, there is never a
//! trailing space, and **the document does not end with a newline**. The last one
//! is easy to get wrong — `writeln!` in a loop produces it — so
//! [`document`] joins with `'\n'` and never appends a trailing one.

/// One decoded-cell value.
///
/// Only the two leaf kinds a memory row can hold: text, or JSON `null` for an
/// absent `created_at`. A row cell is a primitive in TOON (§9.3), and `null` is
/// a primitive, so an unstamped memory stays uniform with its stamped neighbours
/// instead of forcing the whole table into list form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    /// A string value, encoded per §7.2 and §7.1.
    Text(String),
    /// A number, already in the canonical decimal form §2 requires.
    ///
    /// Carrying the rendered digits rather than an `f64` is deliberate: §2's
    /// canonical form for a `usize` is exactly its decimal spelling, so parsing a
    /// float to get there would only risk the exponent notation and trailing-zero
    /// rules the section exists to forbid. A caller must therefore pass a string
    /// that is *already* canonical — `2` and `0`, never `2.0` or `2e0`.
    Number(String),
    /// The JSON `null` literal (§2).
    Null,
}

/// One array element: its fields in encounter order, each with a cell.
pub type Row = Vec<(String, Cell)>;

/// The three renderings a TOON array field can take, chosen from the data rather
/// than by preference (§1.4: "Which form an encoder emits follows from the
/// value's shape … not from encoder preference").
#[derive(Debug, Clone, PartialEq, Eq)]
enum ArrayForm {
    /// No elements: `name: []` (§9.1, the only form an encoder may emit here).
    Empty,
    /// Uniform elements: a tabular header plus one row per element (§9.3).
    Tabular {
        /// The declared field list, in header order.
        fields: Vec<String>,
        /// One cell per leaf field, in the same order.
        rows: Vec<Vec<Cell>>,
    },
    /// Non-uniform elements: one `- ` list item per element (§9.4, §10).
    List(Vec<Row>),
}

/// How many spaces one indentation level is (§12: default 2, configurable; tabs
/// MUST NOT be used). Fixed at 2 because nothing on these routes nests deeper
/// than one level, so a knob would be a config for a value that never changes.
const INDENT: &str = "  ";

/// The active and document delimiter: comma (§11, default).
const DELIM: char = ',';

/// Quote and escape one string value, per §7.2 (when) and §7.1 (how).
///
/// The comma is the active delimiter for every scope this module emits, so a
/// value containing one is quoted; so is a value containing a colon, bracket or
/// brace, because those are structural in a key-value line, a header's bracket
/// segment, or its field list.
pub fn scalar(value: &str) -> String {
    if needs_quotes(value) {
        let mut out = String::with_capacity(value.len() + 2);
        out.push('"');
        escape_into(value, &mut out);
        out.push('"');
        out
    } else {
        value.to_string()
    }
}

/// Encode one object key or one field-list name, per §7.3.
///
/// Unquoted only when the whole name is ASCII and matches `^[A-Za-z_][A-Za-z0-9_.]*$`;
/// anything else is quoted and escaped. Deliberately stricter than §7.4, which is
/// what a decoder must *accept*: this is what an encoder may *emit*.
pub fn key(name: &str) -> String {
    let mut chars = name.chars();
    let first_ok = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_');
    let rest_ok = chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    if !name.is_empty() && first_ok && rest_ok {
        name.to_string()
    } else {
        let mut out = String::with_capacity(name.len() + 2);
        out.push('"');
        escape_into(name, &mut out);
        out.push('"');
        out
    }
}

/// Render a whole TOON document: `fields` as `key: value` lines in the order
/// given, then the array field `name` holding `rows`.
///
/// No trailing newline, per §12. Every line ends in a real line break only
/// between lines, and indentation is spaces (§12).
pub fn document(fields: &[(String, Cell)], name: &str, rows: &[Row]) -> String {
    let mut out = String::new();
    for (k, v) in fields {
        out.push_str(&key(k));
        out.push_str(": ");
        out.push_str(&encode_cell(v));
        out.push('\n');
    }
    match array_form(rows) {
        ArrayForm::Empty => {
            out.push_str(&key(name));
            out.push_str(": []");
        }
        ArrayForm::Tabular { fields, rows } => {
            out.push_str(&tabular_header(name, &fields, rows.len()));
            for row in &rows {
                out.push('\n');
                out.push_str(INDENT);
                let cells: Vec<String> = row.iter().map(encode_cell).collect();
                out.push_str(&cells.join(&DELIM.to_string()));
            }
        }
        ArrayForm::List(items) => {
            out.push_str(&key(name));
            out.push_str(&format!("[{}]:", items.len()));
            for item in &items {
                out.push('\n');
                out.push_str(INDENT);
                // §10: a list-item object's *first* field rides on the hyphen
                // line; the rest are siblings one level in.
                let Some((first, rest)) = item.split_first() else {
                    continue;
                };
                out.push_str("- ");
                out.push_str(&key(&first.0));
                out.push_str(": ");
                out.push_str(&encode_cell(&first.1));
                for (k, v) in rest {
                    out.push('\n');
                    out.push_str(INDENT);
                    out.push_str(INDENT);
                    out.push_str(&key(k));
                    out.push_str(": ");
                    out.push_str(&encode_cell(v));
                }
            }
        }
    }
    out
}

/// Render one memory as a single-object document — the `GET .../memories/:mid`
/// form, which is a root object of `key: value` lines (§5, §8) with no array.
///
/// Takes [`Cell`]s rather than strings so an absent `created_at` can be the JSON
/// `null` literal instead of a fabricated empty string.
pub fn object(fields: &[(String, Cell)]) -> String {
    let mut out = String::new();
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&key(k));
        out.push_str(": ");
        out.push_str(&encode_cell(v));
    }
    out
}

/// `name[N]{f1,f2}:` — the tabular array header of §9.3, with the comma
/// delimiter symbol omitted because comma is active (§11).
fn tabular_header(name: &str, fields: &[String], count: usize) -> String {
    let encoded: Vec<String> = fields.iter().map(|f| key(f)).collect();
    format!("{}[{count}]{{{}}}:", key(name), encoded.join(&DELIM.to_string()))
}

/// Pick the form §9 mandates for these rows.
///
/// §9.3's detection: every element is an object with at least one key and they all
/// share one key set. A page mixing memories that have capture `context` with ones
/// that do not fails that test, so it takes the list form of §9.4 — which is why
/// this is decided from the data and not assumed.
fn array_form(rows: &[Row]) -> ArrayForm {
    let Some(first) = rows.first() else {
        return ArrayForm::Empty;
    };
    let fields: Vec<&String> = first.iter().map(|(k, _)| k).collect();
    let uniform = rows.iter().all(|row| {
        row.len() == fields.len()
            && row.iter().zip(&fields).all(|((k, _), f)| &k == f)
    });
    if uniform {
        ArrayForm::Tabular {
            fields: fields.into_iter().cloned().collect(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|(_, v)| v.clone()).collect())
                .collect(),
        }
    } else {
        ArrayForm::List(rows.to_vec())
    }
}

/// One cell, as the text a row line carries.
fn encode_cell(cell: &Cell) -> String {
    match cell {
        Cell::Text(s) => scalar(s),
        Cell::Number(s) => s.clone(),
        Cell::Null => "null".to_string(),
    }
}

/// Whether §7.2 requires this value to be quoted.
///
/// Every condition in that list, not the three a caller might guess at. A value
/// that needs quoting and is emitted bare decodes as the wrong *type* or swallows
/// the rest of the row: `42` unquoted decodes as a number, `a: b` unquoted splits
/// at the colon, and `-` alone is a list-item marker.
fn needs_quotes(value: &str) -> bool {
    // Empty, or whitespace at either edge — §12 trims spaces around every token,
    // so a bare value with an edge space does not survive the round trip.
    if value.is_empty() || value.starts_with(' ') || value.ends_with(' ') {
        return true;
    }
    if value.starts_with('\t') || value.ends_with('\t') {
        return true;
    }
    // Reserved literals, case-sensitive: a bare `null` decodes as JSON null.
    if matches!(value, "true" | "false" | "null") {
        return true;
    }
    // Numeric-like: bare, it decodes as a number (§4).
    if is_numeric_like(value) {
        return true;
    }
    // Structure: the active delimiter, plus the characters that open or close a
    // header, a field list, or a key-value line.
    if value.contains(DELIM)
        || value.contains(':')
        || value.contains('"')
        || value.contains('\\')
        || value.contains('[')
        || value.contains(']')
        || value.contains('{')
        || value.contains('}')
    {
        return true;
    }
    // Any C0 control, HTAB included: LF and CR would also break the line format.
    if value.chars().any(|c| c <= '\u{1f}') {
        return true;
    }
    // A leading hyphen or number sign: `- x` is a list item, `# x` is a comment.
    value.starts_with('-') || value.starts_with('#')
}

/// The §4 numeric grammar, ASCII digits only, case-insensitive on the exponent.
///
/// Deliberately not a host number parser: a wider grammar here would quote fewer
/// strings and produce cells that do not decode back to the string they came from.
fn is_numeric_like(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut i = 0;
    if matches!(bytes.first(), Some(b'+') | Some(b'-')) {
        i += 1;
    }
    let int_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == int_start {
        return false;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return false;
        }
    }
    if i < bytes.len() && (bytes[i] | 0x20) == b'e' {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == bytes.len()
}

/// Escape the inside of a quoted token, per §7.1's table.
///
/// Only the five named escapes plus `\uXXXX` for the remaining C0 controls. A
/// literal `"` or `\` left unescaped would end or corrupt the token, and a raw LF
/// would end the line and turn the rest of a memory into a second row.
fn escape_into(value: &str, out: &mut String) {
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c <= '\u{1f}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(fields: &[(&str, &str)]) -> Row {
        fields
            .iter()
            .map(|(k, v)| ((*k).to_string(), Cell::Text((*v).to_string())))
            .collect()
    }

    fn num(n: usize) -> (String, Cell) {
        ("total".to_string(), Cell::Number(n.to_string()))
    }

    // ── §7.2 quoting ────────────────────────────────────────────────────────
    // The three conditions a caller would guess (comma, quote, newline) are a
    // subset of the spec's list. Each case below is one condition, and the ones
    // past the guessable three are the ones that silently change a cell's *type*
    // or swallow a row if left bare.

    #[test]
    fn a_plain_value_is_emitted_bare() {
        assert_eq!(scalar("auth uses jose"), "auth uses jose");
        assert_eq!(scalar("the deploy key rotates"), "the deploy key rotates");
    }

    #[test]
    fn a_value_holding_the_delimiter_is_quoted() {
        assert_eq!(scalar("a,b"), "\"a,b\"");
    }

    #[test]
    fn a_value_holding_a_quote_is_quoted_and_the_quote_escaped() {
        assert_eq!(scalar("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn a_value_holding_a_newline_is_quoted_and_the_newline_escaped() {
        assert_eq!(scalar("one\ntwo"), "\"one\\ntwo\"");
    }

    #[test]
    fn a_numeric_value_is_quoted_so_it_decodes_back_as_a_string() {
        // Bare `42` decodes as a number (§4), which would not round-trip.
        assert_eq!(scalar("42"), "\"42\"");
        assert_eq!(scalar("-3.14"), "\"-3.14\"");
        assert_eq!(scalar("1e-6"), "\"1e-6\"");
        assert_eq!(scalar("+1"), "\"+1\"");
        // Leading zeros are a string by §4's own rule, but §7.2 still quotes it.
        assert_eq!(scalar("05"), "\"05\"");
    }

    #[test]
    fn a_number_that_is_not_numeric_like_stays_bare() {
        // A `.5`, a `1.`, a hex form and a separated literal are all strings to a
        // decoder, so quoting them would only cost tokens.
        assert_eq!(scalar(".5"), ".5");
        assert_eq!(scalar("1."), "1.");
        assert_eq!(scalar("0x10"), "0x10");
        assert_eq!(scalar("1_000"), "1_000");
    }

    #[test]
    fn a_reserved_literal_is_quoted() {
        assert_eq!(scalar("true"), "\"true\"");
        assert_eq!(scalar("false"), "\"false\"");
        assert_eq!(scalar("null"), "\"null\"");
        // Case-sensitive, so these are ordinary strings and stay cheap.
        assert_eq!(scalar("True"), "True");
        assert_eq!(scalar("NULL"), "NULL");
    }

    #[test]
    fn an_empty_or_edge_whitespace_value_is_quoted() {
        assert_eq!(scalar(""), "\"\"");
        assert_eq!(scalar(" lead"), "\" lead\"");
        assert_eq!(scalar("trail "), "\"trail \"");
        // An interior space is safe unquoted (§7.2).
        assert_eq!(scalar("in ner"), "in ner");
    }

    #[test]
    fn a_value_holding_structure_is_quoted() {
        for (raw, encoded) in [
            ("a:b", "\"a:b\""),
            ("[x]", "\"[x]\""),
            ("{y}", "\"{y}\""),
            ("a[b", "\"a[b\""),
            ("c}d", "\"c}d\""),
            // The backslash is both a reason to quote and something to escape.
            ("back\\slash", "\"back\\\\slash\""),
        ] {
            assert_eq!(scalar(raw), encoded, "{raw:?} must be quoted");
        }
    }

    #[test]
    fn a_number_cell_is_emitted_bare_so_it_decodes_as_a_number() {
        // The mirror of the numeric-string rule: a count that is a number must
        // not be quoted, or it comes back as the string "2".
        assert_eq!(encode_cell(&Cell::Number("2".to_string())), "2");
        assert_eq!(encode_cell(&Cell::Number("0".to_string())), "0");
        assert_eq!(
            document(&[("total".to_string(), Cell::Number("2".to_string()))], "m", &[]),
            "total: 2\nm: []"
        );
    }

    #[test]
    fn a_leading_hyphen_or_number_sign_is_quoted() {
        assert_eq!(scalar("-"), "\"-\"");
        assert_eq!(scalar("-x"), "\"-x\"");
        assert_eq!(scalar("#tag"), "\"#tag\"");
        // Not at position 0, so no structural meaning and no quote.
        assert_eq!(scalar("a-b"), "a-b");
        assert_eq!(scalar("a#b"), "a#b");
    }

    #[test]
    fn a_control_character_is_escaped_as_lowercase_hex() {
        assert_eq!(scalar("a\u{0}b"), "\"a\\u0000b\"");
        assert_eq!(scalar("a\u{1f}b"), "\"a\\u001fb\"");
        assert_eq!(scalar("a\tb"), "\"a\\tb\"");
        assert_eq!(scalar("a\rb"), "\"a\\rb\"");
    }

    #[test]
    fn non_ascii_text_stays_literal_utf8() {
        // §7.1: supplementary scalars MUST be emitted as literal UTF-8, and
        // SHOULD be for the rest of the BMP. No \uXXXX for a printable é.
        assert_eq!(scalar("café ✓ 🚀"), "café ✓ 🚀");
    }

    // ── §7.3 keys ───────────────────────────────────────────────────────────

    #[test]
    fn a_key_is_bare_only_when_it_matches_the_unquoted_pattern() {
        for k in ["id", "_x", "created_at", "a.b", "A1"] {
            assert_eq!(key(k), k, "{k} should be bare");
        }
        for k in ["", "1a", "a-b", "a b", "a/b", "é", "a:b"] {
            assert_eq!(key(k), format!("\"{k}\""), "{k:?} should be quoted");
        }
    }

    // ── §6 / §9.3 / §9.4 / §9.1 document forms ──────────────────────────────

    #[test]
    fn a_uniform_table_takes_the_tabular_form() {
        let rows = vec![
            row(&[("id", "f9fbd954"), ("created_at", "2026-09-29T10:04:11Z"), ("content", "deploy key rotates")]),
            row(&[("id", "a1c2e3f4"), ("created_at", "2026-09-28T08:11:00Z"), ("content", "auth uses jose")]),
        ];
        assert_eq!(
            document(&[num(2)], "memories", &rows),
            concat!(
                "total: 2\n",
                "memories[2]{id,created_at,content}:\n",
                "  f9fbd954,\"2026-09-29T10:04:11Z\",deploy key rotates\n",
                "  a1c2e3f4,\"2026-09-28T08:11:00Z\",auth uses jose",
            )
        );
    }

    #[test]
    fn a_tabular_row_quotes_only_the_cells_that_need_it() {
        let rows = vec![row(&[
            ("id", "f9fbd954"),
            ("created_at", "2026-09-29T10:04:11Z"),
            ("content", "one,two"),
        ])];
        let out = document(&[], "memories", &rows);
        // The id and the content-as-written are bare apart from the comma in the
        // last cell. The timestamp is quoted because §7.2 makes a colon a
        // mandatory quote trigger — and it has to be: §9.3 reads the first
        // unquoted colon at row depth as the end of the table.
        assert_eq!(
            out,
            "memories[1]{id,created_at,content}:\n  \
             f9fbd954,\"2026-09-29T10:04:11Z\",\"one,two\""
        );
    }

    #[test]
    fn a_timestamp_cell_is_quoted_because_a_colon_is_structural() {
        // Not a choice this module makes. §7.2 lists a colon among the values an
        // encoder MUST quote, and §9.3's row/key-value disambiguation reads the
        // first *unquoted* colon at row depth as the end of the table — so a bare
        // `10:04:11` can end the rows early depending on which column it lands
        // in. Two quote characters per row is the cheaper of the two costs.
        assert_eq!(scalar("2026-09-29T10:04:11Z"), "\"2026-09-29T10:04:11Z\"");
        // The rest of an RFC 3339 stamp is bare-safe, which is why the rule does
        // not force quoting on most content.
        assert_eq!(scalar("20260929T100411Z"), "20260929T100411Z");
    }

    #[test]
    fn a_null_cell_is_the_json_null_literal() {
        let rows = vec![vec![
            ("id".to_string(), Cell::Text("m1".to_string())),
            ("created_at".to_string(), Cell::Null),
            ("content".to_string(), Cell::Text("hi".to_string())),
        ]];
        assert_eq!(
            document(&[], "memories", &rows),
            "memories[1]{id,created_at,content}:\n  m1,null,hi"
        );
    }

    #[test]
    fn a_mixed_key_set_takes_the_list_form() {
        // One memory carries capture context and the other does not, so the page
        // is not a uniform table and §9.3's tabular form is not available.
        let rows = vec![
            row(&[("id", "a"), ("content", "first")]),
            row(&[("id", "b"), ("content", "second"), ("context", "saw the prompt")]),
        ];
        let out = document(&[], "memories", &rows);
        assert_eq!(
            out,
            "memories[2]:\n  - id: a\n    content: first\n  - id: b\n    content: second\n    context: saw the prompt"
        );
    }

    #[test]
    fn an_empty_array_takes_the_empty_value_form() {
        // §9.1: `name: []`. The legacy `name[0]:` form MUST NOT be emitted.
        assert_eq!(document(&[num(0)], "memories", &[]), "total: 0\nmemories: []");
    }

    #[test]
    fn a_single_element_table_still_declares_its_length() {
        let rows = vec![row(&[("id", "only")])];
        assert_eq!(document(&[], "memories", &rows), "memories[1]{id}:\n  only");
    }

    // ── §12 whitespace invariants ────────────────────────────────────────────

    #[test]
    fn the_document_never_ends_with_a_newline() {
        let rows = vec![row(&[("id", "a"), ("content", "x")]), row(&[("id", "b"), ("content", "y")])];
        let tabular = document(&[num(2)], "memories", &rows);
        assert!(!tabular.ends_with('\n'), "tabular: {tabular:?}");

        let mixed = vec![
            row(&[("id", "a")]),
            row(&[("id", "b"), ("context", "c")]),
        ];
        let list = document(&[], "memories", &mixed);
        assert!(!list.ends_with('\n'), "list: {list:?}");

        let empty = document(&[], "memories", &[]);
        assert!(!empty.ends_with('\n'), "empty: {empty:?}");
    }

    #[test]
    fn no_line_carries_trailing_spaces_and_indent_is_spaces() {
        let mixed = vec![row(&[("id", "a")]), row(&[("id", "b"), ("context", "c")])];
        for out in [document(&[], "memories", &mixed), document(&[], "memories", &[])] {
            for line in out.lines() {
                assert!(!line.ends_with(' '), "trailing space in {line:?}");
                assert!(!line.contains('\t'), "tab in {line:?}");
                let indent = line.len() - line.trim_start_matches(' ').len();
                assert_eq!(indent % 2, 0, "indent not a multiple of 2 in {line:?}");
            }
        }
    }

    // ── single-object form ──────────────────────────────────────────────────

    #[test]
    fn an_object_is_key_value_lines_with_no_trailing_newline() {
        assert_eq!(
            object(&[
                ("id".to_string(), Cell::Text("f9fbd954".to_string())),
                ("content".to_string(), Cell::Text("auth uses jose".to_string())),
            ]),
            "id: f9fbd954\ncontent: auth uses jose"
        );
    }

    #[test]
    fn an_object_value_is_quoted_on_the_same_rules() {
        assert_eq!(
            object(&[("content".to_string(), Cell::Text("a\nb".to_string()))]),
            "content: \"a\\nb\""
        );
    }

    #[test]
    fn an_object_null_field_is_the_json_null_literal() {
        assert_eq!(
            object(&[
                ("id".to_string(), Cell::Text("m1".to_string())),
                ("created_at".to_string(), Cell::Null),
            ]),
            "id: m1\ncreated_at: null"
        );
    }
}
