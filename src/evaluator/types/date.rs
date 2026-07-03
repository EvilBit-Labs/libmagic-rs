// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::{TypeReadError, read_bytes_at};
use crate::parser::ast::{Endianness, Value};

/// Day-of-week names matching GNU `file` output format.
const DAY_NAMES: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];

/// Month names matching GNU `file` output format.
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Safely reads a 32-bit Unix timestamp from the buffer at the specified offset
/// and formats it as a human-readable date string.
///
/// The 4 bytes are interpreted as an unsigned 32-bit integer representing seconds
/// since the Unix epoch (1970-01-01 00:00:00 UTC). The result is returned as a
/// `Value::String` formatted like `"Thu Jan  1 00:00:00 1970"`, matching GNU `file`
/// output for `date` types.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to start reading from
/// * `endian` - The byte order to use when interpreting the bytes
/// * `utc` - Whether to format as UTC (true) or local time (false)
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::evaluator::types::read_date;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// // Unix epoch (0) in big-endian
/// let buffer = &[0x00, 0x00, 0x00, 0x00];
/// let result = read_date(buffer, 0, Endianness::Big, true).unwrap();
/// assert_eq!(result, Value::String("Thu Jan  1 00:00:00 1970".to_string()));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if fewer than 4 bytes are available at the
/// requested offset.
pub fn read_date(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
    utc: bool,
) -> Result<Value, TypeReadError> {
    let arr: [u8; 4] = read_bytes_at(buffer, offset)?;

    let secs = match endian {
        Endianness::Little => u32::from_le_bytes(arr),
        Endianness::Big => u32::from_be_bytes(arr),
        Endianness::Native => u32::from_ne_bytes(arr),
    };

    Ok(Value::String(format_unix_timestamp_32(secs, utc)))
}

/// Safely reads a 64-bit Unix timestamp from the buffer at the specified offset
/// and formats it as a human-readable date string.
///
/// The 8 bytes are interpreted as an unsigned 64-bit integer representing seconds
/// since the Unix epoch (1970-01-01 00:00:00 UTC). The result is returned as a
/// `Value::String` formatted like `"Thu Jan  1 00:00:00 1970"`, matching GNU `file`
/// output for `qdate` types.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to start reading from
/// * `endian` - The byte order to use when interpreting the bytes
/// * `utc` - Whether to format as UTC (true) or local time (false)
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::evaluator::types::read_qdate;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// // Unix epoch (0) in little-endian
/// let buffer = &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
/// let result = read_qdate(buffer, 0, Endianness::Little, true).unwrap();
/// assert_eq!(result, Value::String("Thu Jan  1 00:00:00 1970".to_string()));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if fewer than 8 bytes are available at the
/// requested offset.
pub fn read_qdate(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
    utc: bool,
) -> Result<Value, TypeReadError> {
    let arr: [u8; 8] = read_bytes_at(buffer, offset)?;

    let secs = match endian {
        Endianness::Little => u64::from_le_bytes(arr),
        Endianness::Big => u64::from_be_bytes(arr),
        Endianness::Native => u64::from_ne_bytes(arr),
    };

    Ok(Value::String(format_unix_timestamp_64(secs, utc)))
}

/// Formats a numeric timestamp value (from a rule operand) as a date string.
///
/// This is the shared formatter used by `coerce_value_to_type` to normalize
/// numeric expected values into the same `Value::String` representation produced
/// by `read_date` / `read_qdate`, ensuring operator comparisons work correctly.
pub(crate) fn format_timestamp_value(secs: u64, utc: bool) -> String {
    format_unix_timestamp_64(secs, utc)
}

/// Formats a 32-bit Unix timestamp as a human-readable date string.
fn format_unix_timestamp_32(secs: u32, utc: bool) -> String {
    format_unix_timestamp_64(u64::from(secs), utc)
}

/// Returns the local timezone offset in seconds east of UTC for the given timestamp.
///
/// Uses the `chrono` crate to determine the UTC offset for the given timestamp
/// in-process, without spawning external processes. Returns 0 if the offset
/// cannot be determined (e.g., timestamps that overflow `i64`).
#[allow(clippy::cast_possible_truncation)]
fn local_utc_offset_secs(unix_secs: u64) -> i64 {
    use chrono::{DateTime, Local, Offset};

    let Ok(ts) = i64::try_from(unix_secs) else {
        return 0;
    };

    let Some(utc_dt) = DateTime::from_timestamp(ts, 0) else {
        return 0;
    };

    let local_dt = utc_dt.with_timezone(&Local);
    i64::from(local_dt.offset().fix().local_minus_utc())
}

/// Formats a 64-bit Unix timestamp (seconds since epoch) as a human-readable date
/// string matching GNU `file` output: `"Www Mmm DD HH:MM:SS YYYY"`.
///
/// When `utc` is true, the timestamp is formatted in UTC. When false, the system's
/// local timezone offset is applied.
///
/// Uses signed `i128` arithmetic so that negative timezone adjustments near epoch
/// produce valid pre-1970 dates instead of clamping to zero.
///
/// Uses an O(1) civil-date conversion algorithm based on days since epoch, avoiding
/// any iterative year-walking that could hang on large timestamps.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::integer_division,
    clippy::modulo_arithmetic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
// Indexing is invariant-safe: `day_of_week` is `rem_euclid(7)` into a
// 7-element array and `month - 1` is in [0, 11] by the civil-calendar
// algorithm below.
#[allow(clippy::indexing_slicing)]
fn format_unix_timestamp_64(secs: u64, utc: bool) -> String {
    // Use i128 for safe arithmetic with timezone offsets, supporting pre-epoch results
    let effective_secs: i128 = if utc {
        i128::from(secs)
    } else {
        let offset = local_utc_offset_secs(secs);
        i128::from(secs) + i128::from(offset)
    };

    // Day of week: Jan 1 1970 was a Thursday (index 0 in DAY_NAMES)
    // Use Euclidean division/remainder for correct handling of negative values
    let total_days = effective_secs.div_euclid(86400);
    let day_of_week = total_days.rem_euclid(7) as usize;
    let dow_name = DAY_NAMES[day_of_week];

    // Break total seconds into time-of-day components
    let day_secs = effective_secs.rem_euclid(86400);
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    // O(1) civil-date conversion using Howard Hinnant's algorithm, adapted for i128.
    // Shift epoch from 1970-01-01 to 0000-03-01 for easier leap-year math.
    // Use Euclidean division for correct handling of negative day counts.
    let z = total_days + 719_468; // days from 0000-03-01 to Unix epoch
    let era = z.div_euclid(146_097); // 400-year era
    let doe = z - era * 146_097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // year of era
    let y = yoe + era * 400; // absolute year (March-based)
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month index [0, 11] (March=0)
    let day = doy - (153 * mp + 2) / 5 + 1; // day of month [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // calendar month [1, 12]
    let year = if month <= 2 { y + 1 } else { y }; // adjust year for Jan/Feb

    let month_name = MONTH_NAMES[(month - 1) as usize];

    format!("{dow_name} {month_name} {day:2} {hour:02}:{minute:02}:{second:02} {year}")
}

#[cfg(test)]
mod tests {
    // Restriction lints without an allow-*-in-tests config option;
    // test-only arithmetic on fixture timestamps.
    #![allow(clippy::integer_division)]

    use super::*;

    #[test]
    fn test_read_date_endianness() {
        let cases: Vec<(&[u8], Endianness, &str)> = vec![
            // Epoch in LE
            (
                &[0x00, 0x00, 0x00, 0x00],
                Endianness::Little,
                "Thu Jan  1 00:00:00 1970",
            ),
            // Epoch in BE
            (
                &[0x00, 0x00, 0x00, 0x00],
                Endianness::Big,
                "Thu Jan  1 00:00:00 1970",
            ),
            // 1_000_000_000 = 0x3B9ACA00 in BE
            (
                &[0x3B, 0x9A, 0xCA, 0x00],
                Endianness::Big,
                "Sun Sep  9 01:46:40 2001",
            ),
            // 1_000_000_000 = 0x3B9ACA00 in LE (bytes reversed)
            (
                &[0x00, 0xCA, 0x9A, 0x3B],
                Endianness::Little,
                "Sun Sep  9 01:46:40 2001",
            ),
        ];

        for (buffer, endian, expected) in cases {
            let result = read_date(buffer, 0, endian, true).unwrap();
            assert_eq!(
                result,
                Value::String(expected.to_string()),
                "endian={endian:?}, expected={expected}"
            );
        }
    }

    #[test]
    fn test_read_date_native_endian() {
        // Epoch bytes -- both LE and BE are all zeros, so native must also work
        let buffer = &[0x00, 0x00, 0x00, 0x00];
        let result = read_date(buffer, 0, Endianness::Native, true).unwrap();
        assert_eq!(
            result,
            Value::String("Thu Jan  1 00:00:00 1970".to_string())
        );
    }

    #[test]
    fn test_read_date_utc_vs_local() {
        // Use a known timestamp: 1_000_000_000 (2001-09-09 01:46:40 UTC)
        let buffer = &[0x3B, 0x9A, 0xCA, 0x00]; // BE
        let utc_result = read_date(buffer, 0, Endianness::Big, true).unwrap();
        let local_result = read_date(buffer, 0, Endianness::Big, false).unwrap();

        // UTC must produce the known string
        assert_eq!(
            utc_result,
            Value::String("Sun Sep  9 01:46:40 2001".to_string()),
            "UTC date should match expected"
        );

        // Both should return Value::String
        match (&utc_result, &local_result) {
            (Value::String(utc_s), Value::String(local_s)) => {
                // If the system timezone offset differs from UTC, the strings will differ
                let offset = local_utc_offset_secs(1_000_000_000);
                if offset != 0 {
                    assert_ne!(
                        utc_s, local_s,
                        "UTC and local should differ when timezone offset is non-zero"
                    );
                }
            }
            _ => panic!("Expected Value::String for both utc and local"),
        }
    }

    #[test]
    fn test_read_date_at_offset() {
        // Two bytes of padding, then epoch in BE
        let buffer = &[0xaa, 0xbb, 0x00, 0x00, 0x00, 0x00];
        let result = read_date(buffer, 2, Endianness::Big, true).unwrap();
        assert_eq!(
            result,
            Value::String("Thu Jan  1 00:00:00 1970".to_string())
        );
    }

    #[test]
    fn test_read_date_returns_value_string() {
        let buffer = &[0x00, 0x00, 0x00, 0x00];
        match read_date(buffer, 0, Endianness::Big, true).unwrap() {
            Value::String(_) => {}
            other => panic!("Expected Value::String, got {other:?}"),
        }
    }

    #[test]
    fn test_read_date_buffer_overrun() {
        // Too few bytes
        assert_eq!(
            read_date(&[0x00, 0x00, 0x80], 0, Endianness::Little, true).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 3,
            }
        );

        // Empty buffer
        assert_eq!(
            read_date(&[], 0, Endianness::Big, true).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 0,
            }
        );

        // Offset past end
        assert_eq!(
            read_date(&[0x00; 8], 6, Endianness::Little, true).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 6,
                buffer_len: 8,
            }
        );
    }

    #[test]
    fn test_read_date_offset_overflow() {
        let buffer = &[0x00; 4];
        assert_eq!(
            read_date(buffer, usize::MAX, Endianness::Little, true).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: usize::MAX,
                buffer_len: 4,
            }
        );
    }

    #[test]
    fn test_read_qdate_endianness() {
        let cases: Vec<(&[u8], Endianness, &str)> = vec![
            // Epoch in LE
            (
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                Endianness::Little,
                "Thu Jan  1 00:00:00 1970",
            ),
            // Epoch in BE
            (
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                Endianness::Big,
                "Thu Jan  1 00:00:00 1970",
            ),
            // Epoch in Native
            (
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                Endianness::Native,
                "Thu Jan  1 00:00:00 1970",
            ),
            // 1_000_000_000u64 = 0x000000003B9ACA00 in BE
            (
                &[0x00, 0x00, 0x00, 0x00, 0x3B, 0x9A, 0xCA, 0x00],
                Endianness::Big,
                "Sun Sep  9 01:46:40 2001",
            ),
            // 1_000_000_000u64 in LE
            (
                &[0x00, 0xCA, 0x9A, 0x3B, 0x00, 0x00, 0x00, 0x00],
                Endianness::Little,
                "Sun Sep  9 01:46:40 2001",
            ),
        ];

        for (buffer, endian, expected) in cases {
            let result = read_qdate(buffer, 0, endian, true).unwrap();
            assert_eq!(
                result,
                Value::String(expected.to_string()),
                "endian={endian:?}, expected={expected}"
            );
        }
    }

    #[test]
    fn test_read_qdate_native_endian() {
        // Epoch bytes -- all zeros, so native must work regardless of platform
        let buffer = &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let result = read_qdate(buffer, 0, Endianness::Native, true).unwrap();
        assert_eq!(
            result,
            Value::String("Thu Jan  1 00:00:00 1970".to_string())
        );
    }

    #[test]
    fn test_read_qdate_utc_vs_local() {
        // 1_000_000_000u64 in BE
        let buffer = &[0x00, 0x00, 0x00, 0x00, 0x3B, 0x9A, 0xCA, 0x00];
        let utc_result = read_qdate(buffer, 0, Endianness::Big, true).unwrap();
        let local_result = read_qdate(buffer, 0, Endianness::Big, false).unwrap();

        // UTC must produce the known string
        assert_eq!(
            utc_result,
            Value::String("Sun Sep  9 01:46:40 2001".to_string()),
            "UTC qdate should match expected"
        );

        match (&utc_result, &local_result) {
            (Value::String(utc_s), Value::String(local_s)) => {
                let offset = local_utc_offset_secs(1_000_000_000);
                if offset != 0 {
                    assert_ne!(
                        utc_s, local_s,
                        "UTC and local qdate should differ when timezone offset is non-zero"
                    );
                }
            }
            _ => panic!("Expected Value::String for both utc and local qdate"),
        }
    }

    #[test]
    fn test_read_qdate_at_offset() {
        // Three bytes of padding, then epoch in BE
        let buffer = &[
            0xaa, 0xbb, 0xcc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let result = read_qdate(buffer, 3, Endianness::Big, true).unwrap();
        assert_eq!(
            result,
            Value::String("Thu Jan  1 00:00:00 1970".to_string())
        );
    }

    #[test]
    fn test_read_qdate_returns_value_string() {
        let buffer = &[0x00; 8];
        match read_qdate(buffer, 0, Endianness::Big, true).unwrap() {
            Value::String(_) => {}
            other => panic!("Expected Value::String, got {other:?}"),
        }
    }

    #[test]
    fn test_read_qdate_buffer_overrun() {
        // Too few bytes
        assert_eq!(
            read_qdate(&[0x00; 7], 0, Endianness::Little, true).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 7,
            }
        );

        // Empty buffer
        assert_eq!(
            read_qdate(&[], 0, Endianness::Big, true).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 0,
            }
        );

        // Offset past end
        assert_eq!(
            read_qdate(&[0x00; 16], 10, Endianness::Little, true).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 10,
                buffer_len: 16,
            }
        );
    }

    #[test]
    fn test_read_qdate_offset_overflow() {
        let buffer = &[0x00; 8];
        assert_eq!(
            read_qdate(buffer, usize::MAX, Endianness::Little, true).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: usize::MAX,
                buffer_len: 8,
            }
        );
    }

    #[test]
    fn test_format_unix_timestamp_known_dates() {
        // Verify O(1) algorithm against known dates
        let cases: Vec<(u64, &str)> = vec![
            (0, "Thu Jan  1 00:00:00 1970"),
            (1, "Thu Jan  1 00:00:01 1970"),
            (86400, "Fri Jan  2 00:00:00 1970"),
            // 2000-01-01 00:00:00 UTC = 946684800
            (946_684_800, "Sat Jan  1 00:00:00 2000"),
            // 2001-09-09 01:46:40 UTC = 1000000000
            (1_000_000_000, "Sun Sep  9 01:46:40 2001"),
            // Leap year date: 2000-02-29 = 951782400
            (951_782_400, "Tue Feb 29 00:00:00 2000"),
            // Non-leap year: 2001-03-01 = 983404800
            (983_404_800, "Thu Mar  1 00:00:00 2001"),
            // Max u32 value: 4294967295 = 2106-02-07 06:28:15
            (4_294_967_295, "Sun Feb  7 06:28:15 2106"),
        ];

        for (secs, expected) in cases {
            let result = format_unix_timestamp_64(secs, true);
            assert_eq!(result, expected, "timestamp={secs}");
        }
    }

    #[test]
    fn test_format_unix_timestamp_large_qdate_value() {
        // Verify very large u64 timestamp completes and returns valid string.
        // This would hang with an iterative year-walk algorithm.
        let large_ts: u64 = u64::MAX / 86400 * 86400; // largest aligned day boundary
        let result = format_unix_timestamp_64(large_ts, true);
        // Should complete without hanging and contain a year
        assert!(
            !result.is_empty(),
            "Large timestamp should produce non-empty string"
        );
        // Should contain a valid day-of-week prefix
        assert!(
            DAY_NAMES.iter().any(|d| result.starts_with(d)),
            "Large timestamp result should start with a valid day name: {result}"
        );
    }

    #[test]
    fn test_format_timestamp_value_consistency() {
        // Verify format_timestamp_value produces the same output as read_date
        let secs = 1_000_000_000_u64;
        let expected = format_timestamp_value(secs, true);
        let buffer = &[0x3B, 0x9A, 0xCA, 0x00]; // 1_000_000_000 in BE
        let read_result = read_date(buffer, 0, Endianness::Big, true).unwrap();
        assert_eq!(read_result, Value::String(expected));
    }

    #[test]
    fn test_local_utc_offset_known_timestamp() {
        // Verify that local_utc_offset_secs returns a plausible value
        let offset = local_utc_offset_secs(1_000_000_000);
        // UTC offsets range from -12h to +14h
        assert!(
            (-43200..=50400).contains(&offset),
            "Offset {offset} should be within valid UTC offset range"
        );
    }

    #[test]
    fn test_local_utc_offset_overflow_timestamp() {
        // Timestamps exceeding i64::MAX should return 0
        let offset = local_utc_offset_secs(u64::MAX);
        assert_eq!(offset, 0, "Overflow timestamp should return 0 offset");
    }

    #[test]
    fn test_pre_epoch_local_time_signed_arithmetic() {
        // Directly test the formatting algorithm with a pre-epoch effective time.
        // Simulate timestamp=0 with a -28800 offset (UTC-8) by calling the
        // formatter in UTC mode with a large-enough timestamp and verifying the
        // algorithm handles negative effective seconds correctly.
        //
        // We test the internal algorithm by verifying known pre-epoch equivalent:
        // effective_secs = -28800 corresponds to 1969-12-31 16:00:00
        // We can't directly call format_unix_timestamp_64 with negative input
        // (it takes u64), so we verify via local_utc_offset_secs behavior.
        let offset = local_utc_offset_secs(0);
        if offset < 0 {
            // On west-of-UTC systems, local date at epoch should be Dec 31, 1969
            let result = read_date(&[0x00; 4], 0, Endianness::Big, false).unwrap();
            match result {
                Value::String(s) => {
                    assert!(
                        s.contains("1969"),
                        "Epoch in west-of-UTC zone should show 1969, got: {s}"
                    );
                }
                _ => panic!("Expected Value::String"),
            }
        }
    }

    #[test]
    fn test_utc_vs_local_formatted_strings_date() {
        // Table-driven UTC vs local for read_date with specific expected strings
        let cases: Vec<(&[u8], Endianness, u32, &str)> = vec![
            // Epoch
            (
                &[0x00, 0x00, 0x00, 0x00],
                Endianness::Big,
                0,
                "Thu Jan  1 00:00:00 1970",
            ),
            // 1_000_000_000
            (
                &[0x3B, 0x9A, 0xCA, 0x00],
                Endianness::Big,
                1_000_000_000,
                "Sun Sep  9 01:46:40 2001",
            ),
        ];

        for (buffer, endian, ts, expected_utc) in cases {
            let utc = read_date(buffer, 0, endian, true).unwrap();
            let local = read_date(buffer, 0, endian, false).unwrap();

            // UTC result must match known string
            assert_eq!(
                utc,
                Value::String(expected_utc.to_string()),
                "UTC date for ts={ts}"
            );

            // Local result must be a valid string
            match &local {
                Value::String(s) => {
                    assert!(
                        DAY_NAMES.iter().any(|d| s.starts_with(d)),
                        "Local date should start with valid day: {s}"
                    );
                }
                other => panic!("Expected Value::String for local, got {other:?}"),
            }

            // If timezone offset is non-zero, they must differ
            let offset = local_utc_offset_secs(u64::from(ts));
            if offset != 0 {
                assert_ne!(utc, local, "UTC and local should differ for ts={ts}");
            }
        }
    }

    #[test]
    fn test_utc_vs_local_formatted_strings_qdate() {
        // Table-driven UTC vs local for read_qdate with specific expected strings
        let cases: Vec<(&[u8], Endianness, u64, &str)> = vec![
            // Epoch
            (
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                Endianness::Big,
                0,
                "Thu Jan  1 00:00:00 1970",
            ),
            // 1_000_000_000
            (
                &[0x00, 0x00, 0x00, 0x00, 0x3B, 0x9A, 0xCA, 0x00],
                Endianness::Big,
                1_000_000_000,
                "Sun Sep  9 01:46:40 2001",
            ),
        ];

        for (buffer, endian, ts, expected_utc) in cases {
            let utc = read_qdate(buffer, 0, endian, true).unwrap();
            let local = read_qdate(buffer, 0, endian, false).unwrap();

            // UTC result must match known string
            assert_eq!(
                utc,
                Value::String(expected_utc.to_string()),
                "UTC qdate for ts={ts}"
            );

            // Local result must be a valid string
            match &local {
                Value::String(s) => {
                    assert!(
                        DAY_NAMES.iter().any(|d| s.starts_with(d)),
                        "Local qdate should start with valid day: {s}"
                    );
                }
                other => panic!("Expected Value::String for local qdate, got {other:?}"),
            }

            // If timezone offset is non-zero, they must differ
            let offset = local_utc_offset_secs(ts);
            if offset != 0 {
                assert_ne!(utc, local, "UTC and local qdate should differ for ts={ts}");
            }
        }
    }
}
