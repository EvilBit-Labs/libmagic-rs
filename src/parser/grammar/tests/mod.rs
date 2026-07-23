// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

mod hex_bytes_truncation;
mod indirect_offset;
mod magic_rule_parsing;
mod meta_types;
mod number_and_offset_parsing;
mod operator_parsing;
mod strength_and_line_helpers;
mod type_and_operator_suffix_parsing;
mod type_parsing;
mod value_literal_parsing;
mod value_parsing;

use super::*;
use crate::parser::ast::Endianness;
use crate::parser::ast::IndirectAdjustmentOp;
use crate::parser::ast::MetaType;
use crate::parser::ast::PStringLengthWidth;
use crate::parser::ast::SearchFlags;
use crate::parser::ast::StringFlags;

/// Helper function to test parsing with various whitespace patterns
#[allow(dead_code)] // TODO: Use this helper in future whitespace tests
fn test_with_whitespace_variants<T, F>(input: &str, expected: &T, parser: F)
where
    T: Clone + PartialEq + std::fmt::Debug,
    F: Fn(&str) -> IResult<&str, T>,
{
    // Test with various whitespace patterns - pre-allocate Vec with known capacity
    let mut whitespace_variants = Vec::with_capacity(9);
    whitespace_variants.extend([
        format!(" {input}"),    // Leading space
        format!("  {input}"),   // Leading spaces
        format!("\t{input}"),   // Leading tab
        format!("{input} "),    // Trailing space
        format!("{input}  "),   // Trailing spaces
        format!("{input}\t"),   // Trailing tab
        format!(" {input} "),   // Both leading and trailing space
        format!("  {input}  "), // Both leading and trailing spaces
        format!("\t{input}\t"), // Both leading and trailing tabs
    ]);

    for variant in whitespace_variants {
        assert_eq!(
            parser(&variant),
            Ok(("", expected.clone())),
            "Failed to parse with whitespace: '{variant}'"
        );
    }
}

/// Helper function to test number parsing with remaining input
fn test_number_with_remaining_input() {
    // Pre-allocate with known capacity for better performance
    let test_cases = [
        ("123abc", 123, "abc"),
        ("0xFF rest", 255, " rest"),
        ("-42 more", -42, " more"),
        ("0x10,next", 16, ",next"),
    ];

    for (input, expected_num, expected_remaining) in test_cases {
        assert_eq!(
            parse_number(input),
            Ok((expected_remaining, expected_num)),
            "Failed to parse number with remaining input: '{input}'"
        );
    }
}
