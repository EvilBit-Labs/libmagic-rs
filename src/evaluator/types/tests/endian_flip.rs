// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `flip_type_endian` tests (issue #236 `use \^name` endian-flip support).

use super::*;

/// `flip_type_endian` mirrors libmagic `softmagic.c` `cvt_flip`: it swaps
/// only the explicit little/big-endian numeric, float, and date families,
/// leaves `Endianness::Native` untouched, and returns every non-endian type
/// (including `String16`, which is deliberately absent from `cvt_flip`)
/// unchanged. The `signed`/`utc` attributes are preserved.
#[test]
fn test_flip_type_endian_matches_cvt_flip() {
    use crate::parser::ast::TypeKind;

    // (input, expected-after-flip). Endian-bearing types swap LE<->BE.
    let flipped: &[(TypeKind, TypeKind)] = &[
        (
            TypeKind::Short {
                endian: Endianness::Little,
                signed: true,
            },
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true,
            },
        ),
        (
            TypeKind::Long {
                endian: Endianness::Big,
                signed: false,
            },
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
        ),
        (
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: true,
            },
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: true,
            },
        ),
        (
            TypeKind::Float {
                endian: Endianness::Big,
            },
            TypeKind::Float {
                endian: Endianness::Little,
            },
        ),
        (
            TypeKind::Double {
                endian: Endianness::Little,
            },
            TypeKind::Double {
                endian: Endianness::Big,
            },
        ),
        (
            TypeKind::Date {
                endian: Endianness::Big,
                utc: true,
            },
            TypeKind::Date {
                endian: Endianness::Little,
                utc: true,
            },
        ),
        (
            TypeKind::QDate {
                endian: Endianness::Little,
                utc: false,
            },
            TypeKind::QDate {
                endian: Endianness::Big,
                utc: false,
            },
        ),
    ];
    for (input, expected) in flipped {
        assert_eq!(
            &flip_type_endian(input),
            expected,
            "endian-bearing type must swap LE<->BE: {input:?}"
        );
    }

    // Native-endian and non-endian types must be returned unchanged.
    let unchanged: &[TypeKind] = &[
        TypeKind::Short {
            endian: Endianness::Native,
            signed: true,
        },
        TypeKind::Long {
            endian: Endianness::Native,
            signed: false,
        },
        TypeKind::Byte { signed: true },
        // String16 is intentionally NOT in libmagic's cvt_flip.
        TypeKind::String16 {
            endian: Endianness::Big,
        },
        TypeKind::String16 {
            endian: Endianness::Little,
        },
    ];
    for typ in unchanged {
        assert_eq!(
            &flip_type_endian(typ),
            typ,
            "native/non-endian type must be unchanged: {typ:?}"
        );
    }
}
