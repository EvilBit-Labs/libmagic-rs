// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Operator token parsing tests.
//!
//! Covers `parse_operator`: equality, inequality, bitwise operators,
//! comparison operators, `x` (any-value), and whitespace/precedence
//! handling.

use super::*;

// Operator parsing tests
#[test]
fn test_parse_operator_equality() {
    // Single equals sign
    assert_eq!(parse_operator("="), Ok(("", Operator::Equal)));

    // Double equals sign
    assert_eq!(parse_operator("=="), Ok(("", Operator::Equal)));

    // With whitespace
    assert_eq!(parse_operator(" = "), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("  ==  "), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("\t=\t"), Ok(("", Operator::Equal)));
}

#[test]
fn test_parse_operator_inequality() {
    // Not equals
    assert_eq!(parse_operator("!="), Ok(("", Operator::NotEqual)));

    // Alternative not equals syntax
    assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));

    // With whitespace
    assert_eq!(parse_operator(" != "), Ok(("", Operator::NotEqual)));
    assert_eq!(parse_operator("  <>  "), Ok(("", Operator::NotEqual)));
    assert_eq!(parse_operator("\t!=\t"), Ok(("", Operator::NotEqual)));
}

#[test]
fn test_parse_operator_bitwise_and() {
    // Bitwise AND
    assert_eq!(parse_operator("&"), Ok(("", Operator::BitwiseAnd)));

    // With whitespace
    assert_eq!(parse_operator(" & "), Ok(("", Operator::BitwiseAnd)));
    assert_eq!(parse_operator("  &  "), Ok(("", Operator::BitwiseAnd)));
    assert_eq!(parse_operator("\t&\t"), Ok(("", Operator::BitwiseAnd)));
}

#[test]
fn test_parse_operator_bitwise_xor() {
    assert_eq!(parse_operator("^"), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator(" ^ "), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator("  ^  "), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator("\t^\t"), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator("^ 0xFF"), Ok(("0xFF", Operator::BitwiseXor)));
    assert!(parse_operator("^^").is_err());
}

#[test]
fn test_parse_operator_bitwise_not() {
    assert_eq!(parse_operator("~"), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator(" ~ "), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator("  ~  "), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator("\t~\t"), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator("~ 0xff"), Ok(("0xff", Operator::BitwiseNot)));
    assert!(parse_operator("~~").is_err());
}

#[test]
fn test_parse_operator_any_value() {
    assert_eq!(parse_operator("x"), Ok(("", Operator::AnyValue)));
    assert_eq!(parse_operator(" x "), Ok(("", Operator::AnyValue)));
    assert_eq!(parse_operator("  x  "), Ok(("", Operator::AnyValue)));
    assert_eq!(parse_operator("\tx\t"), Ok(("", Operator::AnyValue)));
    assert_eq!(parse_operator("x 42"), Ok(("42", Operator::AnyValue)));
    assert_eq!(
        parse_operator("x version"),
        Ok(("version", Operator::AnyValue))
    );
}

#[test]
fn test_parse_operator_with_remaining_input() {
    // Should parse operator and leave remaining input
    assert_eq!(parse_operator("= 123"), Ok(("123", Operator::Equal)));
    assert_eq!(
        parse_operator("!= value"),
        Ok(("value", Operator::NotEqual))
    );
    assert_eq!(parse_operator("& 0xFF"), Ok(("0xFF", Operator::BitwiseAnd)));
    assert_eq!(parse_operator("^ 0xFF"), Ok(("0xFF", Operator::BitwiseXor)));
    assert_eq!(parse_operator("~ 0xff"), Ok(("0xff", Operator::BitwiseNot)));
    assert_eq!(parse_operator("x 42"), Ok(("42", Operator::AnyValue)));
    assert_eq!(
        parse_operator("== \"string\""),
        Ok(("\"string\"", Operator::Equal))
    );
    assert_eq!(parse_operator("<> test"), Ok(("test", Operator::NotEqual)));
}

#[test]
fn test_parse_operator_precedence() {
    // Test that longer operators are matched first
    // This ensures "==" is parsed as Equal, not "=" followed by "="
    assert_eq!(parse_operator("=="), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("== extra"), Ok(("extra", Operator::Equal)));

    // Test that "!=" is parsed correctly, not as "!" followed by "="
    assert_eq!(parse_operator("!="), Ok(("", Operator::NotEqual)));
    assert_eq!(
        parse_operator("!= extra"),
        Ok(("extra", Operator::NotEqual))
    );

    // Test that "<>" is parsed correctly, not as "<" followed by ">"
    assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));
    assert_eq!(
        parse_operator("<> extra"),
        Ok(("extra", Operator::NotEqual))
    );

    // Test that "<=" is parsed as LessEqual, not "<" followed by "="
    assert_eq!(parse_operator("<="), Ok(("", Operator::LessEqual)));
    assert_eq!(
        parse_operator("<= extra"),
        Ok(("extra", Operator::LessEqual))
    );

    // Test that ">=" is parsed as GreaterEqual, not ">" followed by "="
    assert_eq!(parse_operator(">="), Ok(("", Operator::GreaterEqual)));
    assert_eq!(
        parse_operator(">= extra"),
        Ok(("extra", Operator::GreaterEqual))
    );
}

#[test]
fn test_parse_operator_invalid_input() {
    // Should fail on invalid operators
    assert!(parse_operator("").is_err());
    assert!(parse_operator("abc").is_err());
    assert!(parse_operator("123").is_err());
    assert!(parse_operator("===").is_err()); // Too many equals
    assert!(parse_operator("&&").is_err()); // Double ampersand not supported
    assert!(parse_operator("^^").is_err()); // Double caret not supported
    assert!(parse_operator("~~").is_err()); // Double tilde not supported
}

#[test]
fn test_parse_operator_bare_bang_is_not_equal() {
    // magic(5) uses bare `!` as a "not equal" prefix on values, e.g.
    // `!0xb8c0078e`. Both bare `!` and `!=` map to NotEqual.
    assert_eq!(parse_operator("!"), Ok(("", Operator::NotEqual)));
    assert_eq!(
        parse_operator("!0xb8c0078e"),
        Ok(("0xb8c0078e", Operator::NotEqual))
    );
    assert_eq!(parse_operator("!IHISK"), Ok(("IHISK", Operator::NotEqual)));
}

#[test]
fn test_parse_operator_edge_cases() {
    // Test operators at start of various contexts - multispace0 consumes all whitespace
    assert_eq!(parse_operator("=\n"), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("!=\r\n"), Ok(("", Operator::NotEqual)));
    assert_eq!(parse_operator("&\t\t"), Ok(("", Operator::BitwiseAnd)));

    // Test with mixed whitespace
    assert_eq!(parse_operator(" \t = \t "), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("\t != \t"), Ok(("", Operator::NotEqual)));
    assert_eq!(parse_operator(" \t& \t "), Ok(("", Operator::BitwiseAnd)));
    assert_eq!(parse_operator(" ^\n"), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator(" ~\r\n"), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator(" x\t\t"), Ok(("", Operator::AnyValue)));
}

#[test]
fn test_parse_operator_common_magic_file_patterns() {
    // Test patterns commonly found in magic files
    assert_eq!(
        parse_operator("= 0x7f454c46"),
        Ok(("0x7f454c46", Operator::Equal))
    );
    assert_eq!(parse_operator("!= 0"), Ok(("0", Operator::NotEqual)));
    assert_eq!(
        parse_operator("& 0xFF00"),
        Ok(("0xFF00", Operator::BitwiseAnd))
    );
    assert_eq!(
        parse_operator("== \"ELF\""),
        Ok(("\"ELF\"", Operator::Equal))
    );
    assert_eq!(parse_operator("<> \"\""), Ok(("\"\"", Operator::NotEqual)));
    assert_eq!(parse_operator("^ 0xff"), Ok(("0xff", Operator::BitwiseXor)));
    assert_eq!(parse_operator("~ 0x80"), Ok(("0x80", Operator::BitwiseNot)));
    assert_eq!(
        parse_operator("x version"),
        Ok(("version", Operator::AnyValue))
    );

    // Test with various spacing patterns found in real magic files
    assert_eq!(
        parse_operator("=\t0x504b0304"),
        Ok(("0x504b0304", Operator::Equal))
    );
    assert_eq!(parse_operator("!=  0"), Ok(("0", Operator::NotEqual)));
    assert_eq!(
        parse_operator("&   0xFFFF"),
        Ok(("0xFFFF", Operator::BitwiseAnd))
    );
}

#[test]
fn test_parse_operator_all_variants() {
    // Ensure all operator variants are tested
    let test_cases = [
        ("=", Operator::Equal),
        ("==", Operator::Equal),
        ("!=", Operator::NotEqual),
        ("<>", Operator::NotEqual),
        ("<", Operator::LessThan),
        (">", Operator::GreaterThan),
        ("<=", Operator::LessEqual),
        (">=", Operator::GreaterEqual),
        ("&", Operator::BitwiseAnd),
        ("^", Operator::BitwiseXor),
        ("~", Operator::BitwiseNot),
        ("x", Operator::AnyValue),
    ];

    for (input, expected) in test_cases {
        assert_eq!(
            parse_operator(input),
            Ok(("", expected)),
            "Failed to parse operator: '{input}'"
        );
    }
}

#[test]
fn test_parse_operator_less_than() {
    // Basic less-than
    assert_eq!(parse_operator("<"), Ok(("", Operator::LessThan)));

    // With whitespace
    assert_eq!(parse_operator(" < "), Ok(("", Operator::LessThan)));
    assert_eq!(parse_operator("  <  "), Ok(("", Operator::LessThan)));
    assert_eq!(parse_operator("\t<\t"), Ok(("", Operator::LessThan)));

    // With remaining input
    assert_eq!(parse_operator("< 42"), Ok(("42", Operator::LessThan)));
}

#[test]
fn test_parse_operator_greater_than() {
    // Basic greater-than
    assert_eq!(parse_operator(">"), Ok(("", Operator::GreaterThan)));

    // With whitespace
    assert_eq!(parse_operator(" > "), Ok(("", Operator::GreaterThan)));
    assert_eq!(parse_operator("  >  "), Ok(("", Operator::GreaterThan)));
    assert_eq!(parse_operator("\t>\t"), Ok(("", Operator::GreaterThan)));

    // With remaining input
    assert_eq!(parse_operator("> 42"), Ok(("42", Operator::GreaterThan)));
}

#[test]
fn test_parse_operator_less_equal() {
    // Basic less-or-equal
    assert_eq!(parse_operator("<="), Ok(("", Operator::LessEqual)));

    // With whitespace
    assert_eq!(parse_operator(" <= "), Ok(("", Operator::LessEqual)));
    assert_eq!(parse_operator("  <=  "), Ok(("", Operator::LessEqual)));
    assert_eq!(parse_operator("\t<=\t"), Ok(("", Operator::LessEqual)));

    // With remaining input
    assert_eq!(parse_operator("<= 42"), Ok(("42", Operator::LessEqual)));
}

#[test]
fn test_parse_operator_greater_equal() {
    // Basic greater-or-equal
    assert_eq!(parse_operator(">="), Ok(("", Operator::GreaterEqual)));

    // With whitespace
    assert_eq!(parse_operator(" >= "), Ok(("", Operator::GreaterEqual)));
    assert_eq!(parse_operator("  >=  "), Ok(("", Operator::GreaterEqual)));
    assert_eq!(parse_operator("\t>=\t"), Ok(("", Operator::GreaterEqual)));

    // With remaining input
    assert_eq!(parse_operator(">= 42"), Ok(("42", Operator::GreaterEqual)));
}

#[test]
fn test_parse_operator_comparison_disambiguation() {
    // <> still parses as NotEqual
    assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));

    // <= parses as LessEqual, not LessThan with "=" remaining
    assert_eq!(parse_operator("<="), Ok(("", Operator::LessEqual)));

    // >= parses as GreaterEqual, not GreaterThan with "=" remaining
    assert_eq!(parse_operator(">="), Ok(("", Operator::GreaterEqual)));

    // "< >" (with space) parses as LessThan with "> " remaining
    assert_eq!(parse_operator("< >"), Ok((">", Operator::LessThan)));

    // "> =" (with space) parses as GreaterThan with "= " remaining
    assert_eq!(parse_operator("> ="), Ok(("=", Operator::GreaterThan)));
}

#[test]
fn test_parse_operator_whitespace_handling() {
    // Test comprehensive whitespace handling
    let operators = ["=", "==", "!=", "<>", "&"];
    let whitespace_patterns = [
        "",     // No whitespace
        " ",    // Single space
        "  ",   // Multiple spaces
        "\t",   // Tab
        "\t\t", // Multiple tabs
        " \t",  // Mixed space and tab
        "\t ",  // Mixed tab and space
    ];

    for op in operators {
        for leading_ws in whitespace_patterns {
            for trailing_ws in whitespace_patterns {
                let input = format!("{leading_ws}{op}{trailing_ws}");
                let result = parse_operator(&input);

                assert!(
                    result.is_ok(),
                    "Failed to parse operator with whitespace: '{input}'"
                );

                let (remaining, _) = result.unwrap();
                assert_eq!(remaining, "", "Unexpected remaining input for: '{input}'");
            }
        }
    }
}
