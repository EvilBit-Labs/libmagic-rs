// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! The two-pass admission filter on the top-level loop (GOTCHAS S13.7).
//!
//! With no pass set the list evaluates exactly as before; with a pass set,
//! only entries whose first-line test type matches the pass run, and an
//! `indirect` re-entry always runs as a binary pass (upstream
//! `softmagic.c` passes `BINTEST` to the nested `file_softmagic`).

use super::*;
use crate::evaluator::test_type::{PassMode, TopLevelPass};
use crate::parser::grammar::parse_magic_rule;

fn parsed(line: &str) -> MagicRule {
    parse_magic_rule(line).unwrap().1
}

fn context_for(root_rules: &[MagicRule], pass: Option<TopLevelPass>) -> EvaluationContext {
    let config = EvaluationConfig::default().with_stop_at_first_match(false);
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(build_name_table(vec![])),
        root_rules: std::sync::Arc::from(root_rules),
    });
    EvaluationContext::new(config)
        .with_rule_env(env)
        .with_top_level_pass(pass)
}

fn messages(matches: &[RuleMatch]) -> Vec<&str> {
    matches.iter().map(|m| m.message.as_str()).collect()
}

/// A text-type regex entry and a binary-type byte entry over a buffer both
/// match. Which one runs is decided by the pass alone.
#[test]
fn admission_filter_selects_entries_by_pass() {
    let rules = vec![parsed("0 regex QQ TEXTMSG"), parsed("0 byte 0x51 BINMSG")];
    let buffer = b"QQ";
    let cases: &[(&str, Option<TopLevelPass>, &[&str])] = &[
        ("no pass: both run as today", None, &["TEXTMSG", "BINMSG"]),
        (
            "bin pass skips the text entry",
            Some(TopLevelPass {
                mode: PassMode::Bin,
                buffer_is_text: true,
            }),
            &["BINMSG"],
        ),
        (
            "text pass skips the binary entry",
            Some(TopLevelPass {
                mode: PassMode::Text,
                buffer_is_text: true,
            }),
            &["TEXTMSG"],
        ),
    ];
    for (label, pass, expected) in cases {
        let mut context = context_for(&rules, *pass);
        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert_eq!(messages(&matches), *expected, "case {label:?}");
    }
}

/// A plain `string` entry is binary-typed; `string/t` is text-typed.
#[test]
fn text_pass_skips_plain_string_but_admits_string_t() {
    let rules = vec![
        parsed("0 string QQ STRMSG"),
        parsed("0 string/t QQ STRTMSG"),
    ];
    let pass = TopLevelPass {
        mode: PassMode::Text,
        buffer_is_text: true,
    };
    let mut context = context_for(&rules, Some(pass));
    let matches = evaluate_rules(&rules, b"QQ", &mut context).unwrap();
    assert_eq!(messages(&matches), ["STRTMSG"]);
}

/// Root list: a text regex entry whose child is an `indirect` into a region
/// where a second text regex entry would match, then a plain `string`
/// sibling. In a text pass the re-entry is forced binary, so the inner text
/// entry is skipped and the `indirect` is a non-match; the outer pass is
/// restored afterward so the trailing `string` sibling stays filtered.
/// With no pass set the re-entry evaluates as before.
#[test]
fn indirect_reentry_runs_as_bin_pass_and_restores_outer_pass() {
    let mut outer = parsed("0 regex QQ OUTER");
    let mut contains = indirect_rule(5, "\\b, contains:", vec![]);
    contains.level = 1;
    outer.children = vec![contains];
    let rules = vec![
        outer,
        parsed("0 regex \\^INNER INNERMSG"),
        parsed("0 string QQ STRMSG"),
    ];
    let buffer = b"QQxyzINNER";

    let text_pass = TopLevelPass {
        mode: PassMode::Text,
        buffer_is_text: true,
    };
    let mut context = context_for(&rules, Some(text_pass));
    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(
        messages(&matches),
        ["OUTER"],
        "text pass: indirect re-entry must run as BIN (inner text entry skipped, \
         indirect non-match), and the STRMSG sibling must stay filtered"
    );

    let mut context = context_for(&rules, None);
    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(
        messages(&matches),
        ["OUTER", "\\b, contains:", "\\bINNERMSG", "STRMSG"],
        "no pass: the re-entry evaluates as before"
    );
}
