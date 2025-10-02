//! Abstract Syntax Tree definitions for magic rules
//!
//! This module contains the core data structures that represent parsed magic rules
//! and their components, including offset specifications, type kinds, operators, and values.

use serde::{Deserialize, Serialize};

/// Offset specification for locating data in files
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OffsetSpec {
    /// Absolute offset from file start
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::OffsetSpec;
    ///
    /// let offset = OffsetSpec::Absolute(0x10); // Read at byte 16
    /// let negative = OffsetSpec::Absolute(-4); // 4 bytes before current position
    /// ```
    Absolute(i64),

    /// Indirect offset through pointer dereferencing
    ///
    /// Reads a pointer value at `base_offset`, interprets it according to `pointer_type`
    /// and `endian`, then adds `adjustment` to get the final offset.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{OffsetSpec, TypeKind, Endianness};
    ///
    /// let indirect = OffsetSpec::Indirect {
    ///     base_offset: 0x20,
    ///     pointer_type: TypeKind::Long { endian: Endianness::Little, signed: false },
    ///     adjustment: 4,
    ///     endian: Endianness::Little,
    /// };
    /// ```
    Indirect {
        /// Base offset to read pointer from
        base_offset: i64,
        /// Type of pointer value
        pointer_type: TypeKind,
        /// Adjustment to add to pointer value
        adjustment: i64,
        /// Endianness for pointer reading
        endian: Endianness,
    },

    /// Relative offset from previous match position
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::OffsetSpec;
    ///
    /// let relative = OffsetSpec::Relative(8); // 8 bytes after previous match
    /// ```
    Relative(i64),

    /// Offset from end of file (negative values move towards start)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::OffsetSpec;
    ///
    /// let from_end = OffsetSpec::FromEnd(-16); // 16 bytes before end of file
    /// ```
    FromEnd(i64),
}

/// Data type specifications for interpreting bytes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TypeKind {
    /// Single byte
    Byte,
    /// 16-bit integer
    Short {
        /// Byte order
        endian: Endianness,
        /// Whether value is signed
        signed: bool,
    },
    /// 32-bit integer
    Long {
        /// Byte order
        endian: Endianness,
        /// Whether value is signed
        signed: bool,
    },
    /// String data
    String {
        /// Maximum length to read
        max_length: Option<usize>,
    },
}

/// Comparison and bitwise operators
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Operator {
    /// Equality comparison
    Equal,
    /// Inequality comparison
    NotEqual,
    /// Bitwise AND operation
    BitwiseAnd,
}

/// Value types for rule matching
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    /// Unsigned integer value
    Uint(u64),
    /// Signed integer value
    Int(i64),
    /// Byte sequence
    Bytes(Vec<u8>),
    /// String value
    String(String),
}

/// Endianness specification for multi-byte values
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Endianness {
    /// Little-endian byte order (least significant byte first)
    Little,
    /// Big-endian byte order (most significant byte first)
    Big,
    /// Native system byte order (matches target architecture)
    Native,
}

/// Magic rule representation in the AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicRule {
    /// Offset specification for where to read data
    pub offset: OffsetSpec,
    /// Type of data to read and interpret
    pub typ: TypeKind,
    /// Comparison operator to apply
    pub op: Operator,
    /// Expected value for comparison
    pub value: Value,
    /// Human-readable message for this rule
    pub message: String,
    /// Child rules that are evaluated if this rule matches
    pub children: Vec<MagicRule>,
    /// Indentation level for hierarchical rules
    pub level: u32,
}

// TODO: Add validation methods for MagicRule:
// - validate() method to check rule consistency
// - Ensure message is not empty and contains valid characters
// - Validate that value type matches the TypeKind
// - Check that child rule levels are properly nested
// - Validate offset specifications are reasonable
// - Add bounds checking for level depth to prevent stack overflow

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_spec_absolute() {
        let offset = OffsetSpec::Absolute(42);
        assert_eq!(offset, OffsetSpec::Absolute(42));

        // Test negative offset
        let negative = OffsetSpec::Absolute(-10);
        assert_eq!(negative, OffsetSpec::Absolute(-10));
    }

    #[test]
    fn test_offset_spec_indirect() {
        let indirect = OffsetSpec::Indirect {
            base_offset: 0x20,
            pointer_type: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            adjustment: 4,
            endian: Endianness::Little,
        };

        match indirect {
            OffsetSpec::Indirect {
                base_offset,
                adjustment,
                ..
            } => {
                assert_eq!(base_offset, 0x20);
                assert_eq!(adjustment, 4);
            }
            _ => panic!("Expected Indirect variant"),
        }
    }

    #[test]
    fn test_offset_spec_relative() {
        let relative = OffsetSpec::Relative(8);
        assert_eq!(relative, OffsetSpec::Relative(8));

        // Test negative relative offset
        let negative_relative = OffsetSpec::Relative(-4);
        assert_eq!(negative_relative, OffsetSpec::Relative(-4));
    }

    #[test]
    fn test_offset_spec_from_end() {
        let from_end = OffsetSpec::FromEnd(-16);
        assert_eq!(from_end, OffsetSpec::FromEnd(-16));

        // Test positive from_end (though unusual)
        let positive_from_end = OffsetSpec::FromEnd(8);
        assert_eq!(positive_from_end, OffsetSpec::FromEnd(8));
    }

    #[test]
    fn test_offset_spec_debug() {
        let offset = OffsetSpec::Absolute(100);
        let debug_str = format!("{offset:?}");
        assert!(debug_str.contains("Absolute"));
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_offset_spec_clone() {
        let original = OffsetSpec::Indirect {
            base_offset: 0x10,
            pointer_type: TypeKind::Short {
                endian: Endianness::Big,
                signed: true,
            },
            adjustment: -2,
            endian: Endianness::Big,
        };

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_offset_spec_serialization() {
        let offset = OffsetSpec::Absolute(42);

        // Test JSON serialization
        let json = serde_json::to_string(&offset).expect("Failed to serialize");
        let deserialized: OffsetSpec = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(offset, deserialized);
    }

    #[test]
    fn test_offset_spec_indirect_serialization() {
        let indirect = OffsetSpec::Indirect {
            base_offset: 0x100,
            pointer_type: TypeKind::Long {
                endian: Endianness::Native,
                signed: false,
            },
            adjustment: 12,
            endian: Endianness::Native,
        };

        // Test JSON serialization for complex variant
        let json = serde_json::to_string(&indirect).expect("Failed to serialize");
        let deserialized: OffsetSpec = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(indirect, deserialized);
    }

    #[test]
    fn test_all_offset_spec_variants() {
        let variants = vec![
            OffsetSpec::Absolute(0),
            OffsetSpec::Absolute(-100),
            OffsetSpec::Indirect {
                base_offset: 0x20,
                pointer_type: TypeKind::Byte,
                adjustment: 0,
                endian: Endianness::Little,
            },
            OffsetSpec::Relative(50),
            OffsetSpec::Relative(-25),
            OffsetSpec::FromEnd(-8),
            OffsetSpec::FromEnd(4),
        ];

        // Test that all variants can be created and are distinct
        for (i, variant) in variants.iter().enumerate() {
            for (j, other) in variants.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        variant, other,
                        "Variants at indices {i} and {j} should be different"
                    );
                }
            }
        }
    }

    #[test]
    fn test_endianness_variants() {
        let endianness_values = vec![Endianness::Little, Endianness::Big, Endianness::Native];

        for endian in endianness_values {
            let indirect = OffsetSpec::Indirect {
                base_offset: 0,
                pointer_type: TypeKind::Long {
                    endian,
                    signed: false,
                },
                adjustment: 0,
                endian,
            };

            // Verify the endianness is preserved
            match indirect {
                OffsetSpec::Indirect {
                    endian: actual_endian,
                    ..
                } => {
                    assert_eq!(endian, actual_endian);
                }
                _ => panic!("Expected Indirect variant"),
            }
        }
    }

    // Value enum tests
    #[test]
    fn test_value_uint() {
        let value = Value::Uint(42);
        assert_eq!(value, Value::Uint(42));

        // Test large values
        let large_value = Value::Uint(u64::MAX);
        assert_eq!(large_value, Value::Uint(u64::MAX));
    }

    #[test]
    fn test_value_int() {
        let positive = Value::Int(100);
        assert_eq!(positive, Value::Int(100));

        let negative = Value::Int(-50);
        assert_eq!(negative, Value::Int(-50));

        // Test extreme values
        let max_int = Value::Int(i64::MAX);
        let min_int = Value::Int(i64::MIN);
        assert_eq!(max_int, Value::Int(i64::MAX));
        assert_eq!(min_int, Value::Int(i64::MIN));
    }

    #[test]
    fn test_value_bytes() {
        let empty_bytes = Value::Bytes(vec![]);
        assert_eq!(empty_bytes, Value::Bytes(vec![]));

        let some_bytes = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        assert_eq!(some_bytes, Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]));

        // Test that different byte sequences are not equal
        let other_bytes = Value::Bytes(vec![0x50, 0x4b, 0x03, 0x04]);
        assert_ne!(some_bytes, other_bytes);
    }

    #[test]
    fn test_value_string() {
        let empty_string = Value::String(String::new());
        assert_eq!(empty_string, Value::String(String::new()));

        let hello = Value::String("Hello, World!".to_string());
        assert_eq!(hello, Value::String("Hello, World!".to_string()));

        // Test Unicode strings
        let unicode = Value::String("🦀 Rust".to_string());
        assert_eq!(unicode, Value::String("🦀 Rust".to_string()));
    }

    #[test]
    fn test_value_comparison() {
        // Test that different value types are not equal
        let uint_val = Value::Uint(42);
        let int_val = Value::Int(42);
        let bytes_val = Value::Bytes(vec![42]);
        let string_val = Value::String("42".to_string());

        assert_ne!(uint_val, int_val);
        assert_ne!(uint_val, bytes_val);
        assert_ne!(uint_val, string_val);
        assert_ne!(int_val, bytes_val);
        assert_ne!(int_val, string_val);
        assert_ne!(bytes_val, string_val);
    }

    #[test]
    fn test_value_debug() {
        let uint_val = Value::Uint(123);
        let debug_str = format!("{uint_val:?}");
        assert!(debug_str.contains("Uint"));
        assert!(debug_str.contains("123"));

        let string_val = Value::String("test".to_string());
        let debug_str = format!("{string_val:?}");
        assert!(debug_str.contains("String"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_value_clone() {
        let original = Value::Bytes(vec![1, 2, 3, 4]);
        let cloned = original.clone();
        assert_eq!(original, cloned);

        // Verify they are independent copies
        match (original, cloned) {
            (Value::Bytes(orig_bytes), Value::Bytes(cloned_bytes)) => {
                assert_eq!(orig_bytes, cloned_bytes);
                // They should have the same content but be different Vec instances
            }
            _ => panic!("Expected Bytes variants"),
        }
    }

    #[test]
    fn test_value_serialization() {
        let values = vec![
            Value::Uint(42),
            Value::Int(-100),
            Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            Value::String("ELF executable".to_string()),
        ];

        for value in values {
            // Test JSON serialization
            let json = serde_json::to_string(&value).expect("Failed to serialize Value");
            let deserialized: Value =
                serde_json::from_str(&json).expect("Failed to deserialize Value");
            assert_eq!(value, deserialized);
        }
    }

    #[test]
    fn test_value_serialization_edge_cases() {
        // Test empty collections
        let empty_bytes = Value::Bytes(vec![]);
        let json = serde_json::to_string(&empty_bytes).expect("Failed to serialize empty bytes");
        let deserialized: Value =
            serde_json::from_str(&json).expect("Failed to deserialize empty bytes");
        assert_eq!(empty_bytes, deserialized);

        let empty_string = Value::String(String::new());
        let json = serde_json::to_string(&empty_string).expect("Failed to serialize empty string");
        let deserialized: Value =
            serde_json::from_str(&json).expect("Failed to deserialize empty string");
        assert_eq!(empty_string, deserialized);

        // Test extreme values
        let max_uint = Value::Uint(u64::MAX);
        let json = serde_json::to_string(&max_uint).expect("Failed to serialize max uint");
        let deserialized: Value =
            serde_json::from_str(&json).expect("Failed to deserialize max uint");
        assert_eq!(max_uint, deserialized);

        let min_int = Value::Int(i64::MIN);
        let json = serde_json::to_string(&min_int).expect("Failed to serialize min int");
        let deserialized: Value =
            serde_json::from_str(&json).expect("Failed to deserialize min int");
        assert_eq!(min_int, deserialized);
    }

    // TypeKind tests
    #[test]
    fn test_type_kind_byte() {
        let byte_type = TypeKind::Byte;
        assert_eq!(byte_type, TypeKind::Byte);
    }

    #[test]
    fn test_type_kind_short() {
        let short_little_endian = TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        };
        let short_big_endian = TypeKind::Short {
            endian: Endianness::Big,
            signed: true,
        };

        assert_ne!(short_little_endian, short_big_endian);
        assert_eq!(short_little_endian, short_little_endian.clone());
    }

    #[test]
    fn test_type_kind_long() {
        let long_native = TypeKind::Long {
            endian: Endianness::Native,
            signed: true,
        };

        match long_native {
            TypeKind::Long { endian, signed } => {
                assert_eq!(endian, Endianness::Native);
                assert!(signed);
            }
            _ => panic!("Expected Long variant"),
        }
    }

    #[test]
    fn test_type_kind_string() {
        let unlimited_string = TypeKind::String { max_length: None };
        let limited_string = TypeKind::String {
            max_length: Some(256),
        };

        assert_ne!(unlimited_string, limited_string);
        assert_eq!(unlimited_string, unlimited_string.clone());
    }

    #[test]
    fn test_type_kind_serialization() {
        let types = vec![
            TypeKind::Byte,
            TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            TypeKind::Long {
                endian: Endianness::Big,
                signed: true,
            },
            TypeKind::String { max_length: None },
            TypeKind::String {
                max_length: Some(128),
            },
        ];

        for typ in types {
            let json = serde_json::to_string(&typ).expect("Failed to serialize TypeKind");
            let deserialized: TypeKind =
                serde_json::from_str(&json).expect("Failed to deserialize TypeKind");
            assert_eq!(typ, deserialized);
        }
    }

    // Operator tests
    #[test]
    fn test_operator_variants() {
        let operators = [Operator::Equal, Operator::NotEqual, Operator::BitwiseAnd];

        for (i, op) in operators.iter().enumerate() {
            for (j, other) in operators.iter().enumerate() {
                if i == j {
                    assert_eq!(op, other);
                } else {
                    assert_ne!(op, other);
                }
            }
        }
    }

    #[test]
    fn test_operator_serialization() {
        let operators = vec![Operator::Equal, Operator::NotEqual, Operator::BitwiseAnd];

        for op in operators {
            let json = serde_json::to_string(&op).expect("Failed to serialize Operator");
            let deserialized: Operator =
                serde_json::from_str(&json).expect("Failed to deserialize Operator");
            assert_eq!(op, deserialized);
        }
    }

    // MagicRule tests
    #[test]
    fn test_magic_rule_creation() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
        };

        assert_eq!(rule.message, "ELF magic");
        assert_eq!(rule.level, 0);
        assert!(rule.children.is_empty());
    }

    #[test]
    fn test_magic_rule_with_children() {
        let child_rule = MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(1),
            message: "32-bit".to_string(),
            children: vec![],
            level: 1,
        };

        let parent_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            message: "ELF executable".to_string(),
            children: vec![child_rule],
            level: 0,
        };

        assert_eq!(parent_rule.children.len(), 1);
        assert_eq!(parent_rule.children[0].level, 1);
        assert_eq!(parent_rule.children[0].message, "32-bit");
    }

    #[test]
    fn test_magic_rule_serialization() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(16),
            typ: TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::NotEqual,
            value: Value::Uint(0),
            message: "Non-zero short value".to_string(),
            children: vec![],
            level: 2,
        };

        let json = serde_json::to_string(&rule).expect("Failed to serialize MagicRule");
        let deserialized: MagicRule =
            serde_json::from_str(&json).expect("Failed to deserialize MagicRule");

        assert_eq!(rule.message, deserialized.message);
        assert_eq!(rule.level, deserialized.level);
        assert_eq!(rule.children.len(), deserialized.children.len());
    }
}
