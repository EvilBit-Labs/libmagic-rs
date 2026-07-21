// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end coverage for bare `search` (no `/N` range).
//!
//! magic(5) documents the `search` count as required, but the reference GNU
//! `file` binary accepts a bare `search` and treats it as `str_range == 0` --
//! scan from the rule's offset to end-of-buffer. rmagic previously rejected
//! bare `search` at parse time, so the tolerant loader dropped the rule *and
//! its subtree*. That silently lost real detections: the PDF page-count chain
//! (`>8 search /Count` -> `>>&0 regex [0-9]+ \b, %s pages`), VCF/SAM
//! bioinformatics signatures, ICC, sfnt font name tables, and more.
//!
//! These tests pin the recovered behavior against a synthetic PDF buffer and
//! guard the negative control (a bounded `search/N` must NOT reach past its
//! window).

#![allow(clippy::expect_used)]

use std::io::Write;

use libmagic_rs::{EvaluationConfig, MagicDatabase};
use tempfile::NamedTempFile;

/// Write `magic` to a temp file and load it into a `MagicDatabase`.
fn db_from_magic(magic: &str) -> MagicDatabase {
    let mut f = NamedTempFile::new().expect("temp magic file");
    f.write_all(magic.as_bytes()).expect("write magic");
    f.flush().expect("flush magic");
    MagicDatabase::load_from_file_with_config(f.path(), EvaluationConfig::default())
        .expect("magic must load")
}

/// The GNU `file` PDF page-count chain uses a bare `search` for `/Count`:
///
/// ```text
/// 0    name    pdf
/// >8   search      /Count
/// >>&0 regex       [0-9]+      \b, %s pages
/// 0    string  %PDF-       PDF document
/// >0   use     pdf
/// ```
///
/// Before this fix the `>8 search /Count` line failed to parse, so the whole
/// `pdf` subroutine lost its page-count grandchild and rmagic printed only
/// `PDF document`. It now recovers `, N pages`, matching GNU `file`.
#[test]
fn bare_search_recovers_pdf_page_count() {
    let magic = "0\tname\tpdf\n\
                 >8\tsearch\t\t/Count\n\
                 >>&0\tregex\t\t[0-9]+\t\\b, %s pages\n\
                 0\tstring\t%PDF-\tPDF document\n\
                 >0\tuse\tpdf\n";
    let db = db_from_magic(magic);

    // Minimal PDF: header, then a `/Count 3` object well past offset 8.
    let mut buf = b"%PDF-1.3\n".to_vec();
    buf.extend_from_slice(b"1 0 obj\n<< /Type /Pages /Count 3 /Kids [] >>\nendobj\n");

    // This minimal magic omits the real DB's `>5 byte x \b, version %c`
    // children, so no version fragment appears -- the point under test is
    // the recovered `, N pages` from the bare-`search` -> regex grandchild.
    let result = db.evaluate_buffer(&buf).expect("evaluate PDF buffer");
    assert_eq!(
        result.description.as_str(),
        "PDF document, 3 pages",
        "bare `search /Count` must recover the page count via the regex grandchild"
    );
}

/// Bare `search` scans to end-of-buffer: the pattern is found even when it
/// sits far past any typical bounded window.
#[test]
fn bare_search_scans_to_end_of_buffer() {
    let magic = "0\tstring\tSTART\tmarker\n\
                 >0\tsearch\t\tDEEP\t\\b, found deep\n";
    let db = db_from_magic(magic);

    let mut buf = b"START".to_vec();
    buf.extend(std::iter::repeat_n(b'.', 6000));
    buf.extend_from_slice(b"DEEP");

    let result = db.evaluate_buffer(&buf).expect("evaluate deep buffer");
    assert_eq!(
        result.description.as_str(),
        "marker, found deep",
        "bare search must scan the whole remaining buffer, not a bounded window"
    );
}

/// Negative control: a bounded `search/N` must NOT reach a pattern past its
/// window. This guards against accidentally making every `search` open.
#[test]
fn bounded_search_respects_its_window() {
    let magic = "0\tstring\tSTART\tmarker\n\
                 >0\tsearch/16\tDEEP\t\\b, found deep\n";
    let db = db_from_magic(magic);

    let mut buf = b"START".to_vec();
    buf.extend(std::iter::repeat_n(b'.', 100));
    buf.extend_from_slice(b"DEEP");

    let result = db.evaluate_buffer(&buf).expect("evaluate bounded buffer");
    assert_eq!(
        result.description.as_str(),
        "marker",
        "search/16 must not find a pattern 100+ bytes past the offset"
    );
}
