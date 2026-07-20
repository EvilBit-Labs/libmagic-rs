// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! U7 audit: does `parse_hex_bytes` (and its sub-parsers
//! `parse_mixed_hex_ascii` / `parse_hex_bytes_no_prefix`) consume a whole
//! value token, or silently truncate at the first character that doesn't
//! look like a hex digit and leak the remainder back to the caller?
//!
//! `TypeKind::Regex` no longer routes through this code path (see
//! `parser::grammar::getstr`), but `TypeKind::Search` (and any other
//! string-family type whose bareword value happens to look like hex
//! bytes) still does, via `parse_value`'s `alt(...)`.
//!
//! **Finding (confirmed by the tests below, not inferred):**
//! `parse_mixed_hex_ascii` (used whenever the token starts with a
//! backslash) consumes the WHOLE token -- its final fallback branch,
//! `none_of(" \t\n\r")`, accepts virtually any non-whitespace character,
//! so a mixed escape+literal token like `\x41BC[def` is captured in
//! full. `parse_hex_bytes_no_prefix` (used only when the token does
//! **not** start with a backslash and looks like a run of pure hex
//! digits) DOES truncate: it uses `take_while(is_ascii_hexdigit)`, which
//! stops at the first non-hex-digit character and returns `Ok` with the
//! shorter byte vector, leaving the remainder (e.g. `[42`) in the
//! `IResult`'s remaining-input slot. Through `parse_magic_rule`, that
//! leaked remainder is treated as the start of the rule's *message*
//! field -- corrupting the parsed value AND fabricating message text
//! that was never intended, with no parse error to signal the mistake.
//!
//! This is a genuine, if narrow, bug: it requires a bareword (unquoted,
//! no `\` prefix) token that is (a) an even number of hex-digit
//! characters, (b) contains at least one `a`-`f`/`A`-`F` letter (so
//! `parse_hex_bytes_no_prefix`'s "must look like hex, not a decimal
//! number" guard doesn't reject it), and (c) is immediately followed
//! (no whitespace) by more non-whitespace, non-hex-digit characters.
//! Fixed below by requiring the parsed hex run to be immediately
//! followed by whitespace or end-of-input -- i.e. the ENTIRE token must
//! be hex digits, not just a hex-looking prefix of it.

use super::*;
use crate::EvaluationConfig;
use crate::evaluator::{EvaluationContext, evaluate_rules};

#[test]
fn mixed_escape_and_literal_token_is_captured_whole() {
    // `\x41BC[def`: hex-prefixed byte, then plain ASCII letters, then a
    // literal `[` (not part of any escape), then more plain letters.
    // Confirms `parse_mixed_hex_ascii` does NOT truncate at `[`.
    let (remaining, bytes) = parse_hex_bytes(r"\x41BC[def").expect(r"\x41BC[def");
    assert_eq!(remaining, "", "whole token must be consumed, none leaked");
    assert_eq!(
        bytes,
        vec![0x41, b'B', b'C', b'[', b'd', b'e', b'f'],
        "all intended bytes must be present, in order"
    );
}

#[test]
fn mixed_escape_and_literal_token_stops_only_at_whitespace() {
    let (remaining, bytes) =
        parse_hex_bytes(r"\x7f\x45(LF)[bracket] trailing").expect("mixed token with brackets");
    assert_eq!(remaining, " trailing");
    assert_eq!(
        bytes,
        {
            let mut expected = vec![0x7f, 0x45];
            expected.extend_from_slice(b"(LF)[bracket]");
            expected
        },
        "escape + arbitrary literal punctuation must all be captured"
    );
}

/// Regression test for the confirmed truncation bug in
/// `parse_hex_bytes_no_prefix`: a bareword (no `\` prefix) token that
/// looks like hex digits followed immediately by non-hex-digit
/// characters (no separating whitespace) must NOT silently truncate and
/// leak the remainder. Before the fix, `parse_hex_bytes("4a[42")`
/// returned `Ok(("[42", vec![0x4a]))` -- only the "4a" prefix was
/// captured and "[42" leaked back as if it were separate input.
#[test]
fn no_prefix_hex_token_does_not_truncate_at_trailing_non_hex_chars() {
    // A hex-looking token immediately followed by a non-hex-digit
    // character with NO separating whitespace is not a valid pure-hex
    // token -- it must be rejected outright (Err), not silently
    // truncated to a shorter byte vector with the remainder leaked.
    assert!(
        parse_hex_bytes("4a[42").is_err(),
        "a hex-looking prefix immediately followed by non-hex-digit, \
         non-whitespace characters must not silently truncate"
    );

    // A whitespace boundary is still a legitimate token terminator (this
    // is how `parse_hex_bytes` is meant to be used inside `parse_value`
    // when more input, such as an operator or message, follows).
    assert_eq!(
        parse_hex_bytes("4a1b more text"),
        Ok((" more text", vec![0x4a, 0x1b]))
    );

    // A trailing quote (closing a quoted-value context) is likewise a
    // legitimate boundary, matching the existing
    // `test_parse_hex_bytes_with_remaining_input` coverage.
    assert_eq!(parse_hex_bytes("ab\""), Ok(("\"", vec![0xab])));
}

#[test]
fn search_literal_no_prefix_hex_lookalike_round_trips_through_parse_value() {
    // Behavioral confirmation at the `parse_value` level (the entry
    // point `parse_magic_rule` uses for string-family types): a
    // bareword token that looks like hex digits but is immediately
    // followed by non-hex-digit characters must not corrupt the parsed
    // value or leak text into what would become the rule's message.
    //
    // `"ab[cd message"` is used (rather than a leading-digit token like
    // `"4a[cd message"`) to isolate this assertion to the hex-bytes path
    // under audit: a leading decimal digit would also be greedily
    // consumed by `parse_numeric_value`'s `digit1` (e.g. `"4a..."` still
    // yields `Ok(Value::Uint(4))` with `"a..."` leaking as remaining --
    // a pre-existing, separate greediness in the *numeric* fallback,
    // out of scope for this hex-bytes-specific audit). Starting with a
    // letter (`'a'`) means neither `parse_float_value` nor
    // `parse_numeric_value` can match at all, so the only way
    // `parse_value` could have returned `Ok` here was via the hex-bytes
    // path -- which, before this fix, DID: `parse_value("ab[cd message")`
    // returned `Ok(("[cd message", Value::Bytes(vec![0xab])))`, silently
    // truncating the value to one byte and leaking "[cd message" as if
    // it were separate input (`parse_magic_rule` would have rendered it
    // as the rule's message text).
    //
    // After the fix: the malformed pure-hex-lookalike token is rejected
    // by the hex-bytes path, so `parse_value` itself returns `Err`,
    // exactly as it would for any other unrecognized bareword token
    // (see `test_parse_value_invalid_input`). The string-family bareword
    // fallback (`parse_bare_string_value`) in `parse_magic_rule` then
    // takes over and captures the ENTIRE token, brackets included.
    assert!(parse_value("ab[cd message").is_err());

    match parse_value("4a1bcd 0x41 more") {
        Ok((remaining, Value::Bytes(bytes))) => {
            assert_eq!(remaining, " 0x41 more");
            assert_eq!(bytes, vec![0x4a, 0x1b, 0xcd]);
        }
        other => panic!("expected whitespace-terminated hex bytes, got {other:?}"),
    }
}

#[test]
fn search_rule_with_malformed_hex_lookalike_literal_captures_whole_value() {
    // Full `parse_magic_rule` round-trip (not just `parse_value`): a
    // `search` rule whose bareword literal looks like a hex-byte prefix
    // but is immediately followed by non-hex characters must parse with
    // the ENTIRE literal as the value and the actual trailing words as
    // the message -- not a 1-byte value with "[cd real message" mangled
    // into the message field.
    let input = "0 search/16 ab[cd real message";
    let (remaining, rule) = parse_magic_rule(input).expect(input);
    assert_eq!(remaining, "");
    assert_eq!(
        rule.value,
        Value::String("ab[cd".to_string()),
        "the whole malformed-hex-lookalike token must be captured as one value \
         via the bareword string fallback, not truncated to a partial Value::Bytes"
    );
    assert_eq!(rule.message, "real message");
}

/// U7 (plan scenario "Search literal round-trip"): parse-correctness must
/// translate to MATCH-correctness, not just parse-shape. The corrected
/// whole-token literal from the test above (`ab[cd`, previously truncated
/// to a 1-byte `Value::Bytes` by the confirmed `parse_hex_bytes_no_prefix`
/// bug) is driven through the full evaluator against a buffer that
/// CONTAINS the literal, asserting the rule actually matches. Without this
/// test, a regression that silently reintroduces truncation (e.g. back to
/// a 1-byte value matching `\xab` alone) could still parse successfully
/// while producing wrong evaluation results -- the "non-crashing wrong
/// answer" failure mode the plan's Verification Contract calls out.
#[test]
fn search_rule_with_malformed_hex_lookalike_literal_matches_buffer_containing_it() {
    let input = "0 search/16 ab[cd real message";
    let (_, rule) = parse_magic_rule(input).expect(input);
    assert_eq!(rule.value, Value::String("ab[cd".to_string()));

    let mut context = EvaluationContext::new(EvaluationConfig::default());

    // Positive: the buffer contains the whole "ab[cd" literal -- must match.
    let matches = evaluate_rules(std::slice::from_ref(&rule), b"xxab[cdyy", &mut context)
        .expect("evaluation must not error");
    assert_eq!(
        matches.len(),
        1,
        "the whole 5-byte literal must match when present in the buffer, got {matches:?}"
    );
    assert_eq!(matches[0].message, "real message");

    // Negative: a buffer containing only the truncated 1-byte value
    // (`0xab`) followed by DIFFERENT bytes (not `[cd`) must NOT match --
    // proving the rule requires the full literal, not just its prefix.
    context.reset();
    let no_match = evaluate_rules(std::slice::from_ref(&rule), b"xxab99yy", &mut context)
        .expect("evaluation must not error");
    assert!(
        no_match.is_empty(),
        "a buffer containing only the truncated 1-byte prefix must not match, got {no_match:?}"
    );
}

#[test]
fn bareword_escaped_space_does_not_truncate_the_token() {
    // magic(5) `\ ` (backslash-space) is a literal space that must NOT
    // terminate a bareword string/search value. The real GNU `file` `tex`
    // rule `0 search/4096 %\ -*-latex-*- LaTeX document text` must parse
    // with the FULL 13-byte pattern "% -*-latex-*-" as the value and
    // "LaTeX document text" as the message. Before the fix the token
    // truncated at the escaped space to "%", turning the rule into
    // `search "%"` -- which false-matches nearly any binary (a `%` byte
    // appears in almost every file), mislabeling gzip/JPEG/etc. as LaTeX.
    let input = r"0 search/4096 %\ -*-latex-*- LaTeX document text";
    let (remaining, rule) = parse_magic_rule(input).expect(input);
    assert_eq!(remaining, "");
    assert_eq!(
        rule.value,
        Value::String("% -*-latex-*-".to_string()),
        "the escaped space must be a literal space inside the value, not a token terminator"
    );
    assert_eq!(rule.message, "LaTeX document text");

    // Match correctness: the rule must fire ONLY when the full literal is
    // present, and must NOT fire on a buffer that merely contains a `%`.
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let hit = evaluate_rules(
        std::slice::from_ref(&rule),
        b"junk % -*-latex-*- more",
        &mut context,
    )
    .expect("evaluation must not error");
    assert_eq!(hit.len(), 1, "must match when the full pattern is present");
    assert_eq!(hit[0].message, "LaTeX document text");

    context.reset();
    let miss = evaluate_rules(
        std::slice::from_ref(&rule),
        b"100% binary \x1f\x8b data with a percent",
        &mut context,
    )
    .expect("evaluation must not error");
    assert!(
        miss.is_empty(),
        "a buffer with a bare `%` (but not the full pattern) must NOT match, got {miss:?}"
    );
}

#[test]
fn bareword_value_beginning_with_backslash_resolves_full_token_and_matches_xml() {
    // The system-DB sgml rule `0 string \<?xml\ version=` begins with a
    // backslash, so its bareword value is captured by `parse_value`'s
    // `parse_mixed_hex_ascii` branch (which fires whenever the token
    // starts with `\`) BEFORE the `parse_bare_string_value` fallback is
    // reached. Before the getstr-parity fix in `parse_mixed_hex_ascii`,
    // the unrecognized `\<` escape survived as a literal 0x5c byte and the
    // escaped space `\ ` terminated the token: the value truncated to
    // `Value::Bytes(b"\\<?xml\\")` and ` version=` leaked into the rule's
    // message -- so real `<?xml ...` documents (and every XML plist) fell
    // through to "ASCII text" instead of "XML document text".
    //
    // After the fix, `\<` -> `<` and `\ ` -> a literal space that
    // CONTINUES the token, so the full 14-byte pattern `<?xml version=`
    // is captured and the message is clean.
    let input = r"0 string \<?xml\ version= XML document text";
    let (remaining, rule) = parse_magic_rule(input).expect(input);
    assert_eq!(remaining, "");
    assert_eq!(
        rule.value,
        Value::Bytes(b"<?xml version=".to_vec()),
        "the whole getstr-resolved pattern must be captured: `\\<` drops the \
         backslash and `\\ ` is a literal space that does not terminate the token"
    );
    assert_eq!(rule.message, "XML document text");

    // Match correctness: a real XML document must match; a non-XML text
    // buffer must not.
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let hit = evaluate_rules(
        std::slice::from_ref(&rule),
        br#"<?xml version="1.0" encoding="UTF-8"?>"#,
        &mut context,
    )
    .expect("evaluation must not error");
    assert_eq!(hit.len(), 1, "a real <?xml document must match");
    assert_eq!(hit[0].message, "XML document text");

    context.reset();
    let miss = evaluate_rules(
        std::slice::from_ref(&rule),
        b"plain ASCII text with no xml prolog",
        &mut context,
    )
    .expect("evaluation must not error");
    assert!(
        miss.is_empty(),
        "a non-XML text buffer must NOT match, got {miss:?}"
    );
}
