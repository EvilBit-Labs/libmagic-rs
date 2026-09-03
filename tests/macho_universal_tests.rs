// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end coverage for per-architecture classification of Mach-O
//! universal ("fat") binaries (issue #378).
//!
//! Two defects blocked parity with GNU `file`. An indirect offset inside a
//! `use` subroutine dropped the subroutine base offset, so `>(8.L)` read
//! `arch[0].cputype` at 8 instead of `arch[0].offset` at `base(8) + 8` and
//! then overran the buffer -- the inner classification never ran. And a
//! `use` site's own `\b` was discarded at parse time, leaving a stray space
//! after the opening bracket.
//!
//! The real chain spans two magic files: `cafebabe` supplies the bracket and
//! the pre-colon CPU name via `mach-o`, while `Mach-O 64-bit executable
//! <cpu>` comes from `mach`'s top-level `0xfeedface` entry, reached only
//! through the `>(8.L) indirect x \b:` re-entry. `mach-o-cpu` itself lives in
//! `mach`, not `cafebabe`.
//!
//! The magic below reproduces both chains at reduced scope so the behavior is
//! pinned hermetically -- system binaries vary across macOS versions and
//! cannot be committed.

#![allow(clippy::expect_used)]

use std::io::Write;

use libmagic_rs::{EvaluationConfig, MagicDatabase};
use tempfile::NamedTempFile;

/// Mirrors the real `cafebabe` + `mach` shape: a fat header whose per-arch
/// `use` invokes a subroutine that names the CPU, dereferences the arch's
/// mach-header offset via `indirect`, and closes the bracket.
const FAT_MAGIC: &str = "0\tbelong\t0xcafebabe\n\
     >4\tbelong\t2\tFat binary with %d architectures:\n\
     >>8\tuse\tarch\t\\b\n\
     >>28\tuse\tarch\t\\b\n\
     0\tname\tarch\t\\b [\n\
     >0\tuse\tcpu\t\\b\n\
     >(8.L)\tindirect\tx\t\\b:\n\
     >0\tbelong\tx\t\\b]\n\
     0\tname\tcpu\n\
     >0\tbelong\t0x01000007\tx86_64\n\
     >0\tbelong\t0x0100000c\tarm64e\n\
     0\tbelong\t0xfeedfacf\tMachO\n\
     >12\tbelong\t2\t\\b 64-bit executable\n\
     >4\tuse\tcpu\n";

const CPU_X86_64: u32 = 0x0100_0007;
const CPU_ARM64E: u32 = 0x0100_000c;
const MACH_MAGIC_64: u32 = 0xfeed_facf;
const FAT_MAGIC_BE: u32 = 0xcafe_babe;
const MH_EXECUTE: u32 = 2;
/// Upper bound on a fixture buffer. An arch offset at or beyond this is
/// treated as dangling rather than grown into.
const MAX_FIXTURE_LEN: usize = 512;

/// One `fat_arch` entry: cputype, cpusubtype, offset, size, align (20 bytes,
/// big-endian), matching `mach-o/fat.h`.
struct Arch {
    cputype: u32,
    offset: u32,
}

/// Build a fat binary: an 8-byte `fat_header`, one 20-byte `fat_arch` per
/// architecture, then a 16-byte mach header at each arch's declared offset.
///
/// `arch[0]` begins at file offset 8, so its `offset` field lands at 16 --
/// the byte `>(8.L)` must reach through `base(8) + 8`.
fn build_fat(arches: &[Arch]) -> Vec<u8> {
    let header_len = 8 + arches.len() * 20;
    let mut buf = Vec::new();
    buf.extend_from_slice(&FAT_MAGIC_BE.to_be_bytes());
    buf.extend_from_slice(
        &u32::try_from(arches.len())
            .expect("arch count")
            .to_be_bytes(),
    );
    for a in arches {
        buf.extend_from_slice(&a.cputype.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // cpusubtype
        buf.extend_from_slice(&a.offset.to_be_bytes());
        buf.extend_from_slice(&16u32.to_be_bytes()); // size
        buf.extend_from_slice(&0u32.to_be_bytes()); // align
    }
    debug_assert_eq!(buf.len(), header_len, "fat header must be 8 + 20 per arch");

    // Lay down a mach header at each declared offset that falls inside the
    // fixture's bounds. An offset beyond `MAX_FIXTURE_LEN` is deliberately
    // left dangling so the out-of-range path can be exercised -- the buffer
    // is never grown to satisfy it.
    for a in arches {
        let at = a.offset as usize;
        if at < header_len || at + 16 > MAX_FIXTURE_LEN {
            continue;
        }
        // Grow to reach the offset, then write unconditionally. An earlier
        // version only wrote when the buffer had to grow, so an offset landing
        // inside already-written bytes (unordered offsets, or a fixture with
        // leading data) silently left zeros there and contradicted this
        // function's contract.
        if buf.len() < at + 16 {
            buf.resize(at + 16, 0);
        }
        let header: Vec<u8> = MACH_MAGIC_64
            .to_be_bytes()
            .into_iter()
            .chain(a.cputype.to_be_bytes())
            .chain(0u32.to_be_bytes()) // cpusubtype
            .chain(MH_EXECUTE.to_be_bytes())
            .collect();
        buf.get_mut(at..at + 16)
            .expect("buffer was just grown to cover this arch's header")
            .copy_from_slice(&header);
    }
    buf
}

fn db() -> MagicDatabase {
    let mut f = NamedTempFile::new().expect("temp magic file");
    f.write_all(FAT_MAGIC.as_bytes()).expect("write magic");
    f.flush().expect("flush magic");
    MagicDatabase::load_from_file_with_config(f.path(), EvaluationConfig::default())
        .expect("magic must load")
}

fn describe(buf: &[u8]) -> String {
    db().evaluate_buffer(buf).expect("evaluate").description
}

#[test]
fn two_architecture_fat_binary_renders_per_arch_inner_classification() {
    let buf = build_fat(&[
        Arch {
            cputype: CPU_X86_64,
            offset: 48,
        },
        Arch {
            cputype: CPU_ARM64E,
            offset: 64,
        },
    ]);
    // Mirrors GNU `file`'s shape exactly: no space after `[`, none around the
    // colon, and the inner classification present for each architecture.
    assert_eq!(
        describe(&buf),
        "Fat binary with 2 architectures: \
         [x86_64:MachO 64-bit executable x86_64] \
         [arm64e:MachO 64-bit executable arm64e]",
        "each arch must render [<cpu>:<inner classification>] with no stray spaces"
    );
}

#[test]
fn opening_bracket_has_no_stray_space() {
    // The `use` site's own `\b` is the only thing suppressing this space; if
    // the parser drops it again the assertion above fails in a way that is
    // easy to misread as an indirect-offset problem. Pin it directly.
    let buf = build_fat(&[
        Arch {
            cputype: CPU_X86_64,
            offset: 48,
        },
        Arch {
            cputype: CPU_ARM64E,
            offset: 64,
        },
    ]);
    let desc = describe(&buf);
    assert!(
        desc.contains("[x86_64:"),
        "expected `[x86_64:` with no interior spaces, got: {desc}"
    );
    assert!(
        !desc.contains("[ "),
        "no bracket may be followed by a space, got: {desc}"
    );
}

#[test]
fn arch_offset_past_end_of_buffer_skips_inner_detail_without_erroring() {
    // A truncated or hostile fat header must degrade to the outer
    // classification rather than propagating a BufferOverrun.
    //
    // Note this one passes pre-fix as well: the old code also failed to
    // produce an inner classification here, for the wrong reason. It is a
    // defensive test for out-of-range dereferences, not discriminating
    // coverage for #378 -- the other four tests in this file are.
    let buf = build_fat(&[
        Arch {
            cputype: CPU_X86_64,
            offset: 9999,
        },
        Arch {
            cputype: CPU_ARM64E,
            offset: 9999,
        },
    ]);
    let desc = describe(&buf);
    assert!(
        desc.starts_with("Fat binary with 2 architectures:"),
        "outer classification must survive an out-of-range arch offset, got: {desc}"
    );
    assert!(
        !desc.contains("MachO"),
        "no inner classification is reachable, got: {desc}"
    );
}

#[test]
fn arch_offset_pointing_at_unrecognized_bytes_keeps_brackets_balanced() {
    // arch[1] points at zero bytes, which match no rule: the bracket must
    // still close so the description does not end mid-group.
    let mut buf = build_fat(&[
        Arch {
            cputype: CPU_X86_64,
            offset: 48,
        },
        Arch {
            cputype: CPU_ARM64E,
            offset: 64,
        },
    ]);
    // Clobber arch[1]'s mach magic. `expect` keeps the fixture honest if the
    // builder's layout ever changes, rather than silently skipping the setup.
    buf.get_mut(64..68)
        .expect("fixture places arch[1]'s mach header at offset 64")
        .fill(0);
    let desc = describe(&buf);
    assert_eq!(
        desc.matches('[').count(),
        desc.matches(']').count(),
        "brackets must stay balanced when an arch fails to classify, got: {desc}"
    );
    assert!(
        desc.contains("[arm64e:]"),
        "the indirect rule still matches (its test is `x`) so its `:` prints \
         with nothing after it, got: {desc}"
    );
}

#[test]
fn indirect_pointer_site_is_read_per_arch_not_from_a_shared_base() {
    // The regression this file exists for: both arches must dereference
    // their OWN offset field (16 and 36), not a single shared one. Give them
    // different mach headers and assert both inner names appear.
    let buf = build_fat(&[
        Arch {
            cputype: CPU_X86_64,
            offset: 48,
        },
        Arch {
            cputype: CPU_ARM64E,
            offset: 64,
        },
    ]);
    let desc = describe(&buf);
    assert!(
        desc.contains("[x86_64:MachO 64-bit executable x86_64]"),
        "arch[0] must dereference its own offset field, got: {desc}"
    );
    assert!(
        desc.contains("[arm64e:MachO 64-bit executable arm64e]"),
        "arch[1] must dereference its own offset field at base(28) + 8, got: {desc}"
    );
}
