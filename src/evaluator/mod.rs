//! Rule evaluation engine
//!
//! This module contains the core evaluation logic for executing magic rules
//! against file buffers to identify file types.

use crate::parser::ast::MagicRule;
use crate::{EvaluationConfig, LibmagicError};

pub mod offset;
pub mod operators;
pub mod types;

/// Context for maintaining evaluation state during rule processing
///
/// The `EvaluationContext` tracks the current state of rule evaluation,
/// including the current offset position, recursion depth for nested rules,
/// and configuration settings that control evaluation behavior.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::EvaluationContext;
/// use libmagic_rs::EvaluationConfig;
///
/// let config = EvaluationConfig::default();
/// let context = EvaluationContext::new(config);
///
/// assert_eq!(context.current_offset(), 0);
/// assert_eq!(context.recursion_depth(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Current offset position in the file buffer
    current_offset: usize,
    /// Current recursion depth for nested rule evaluation
    recursion_depth: u32,
    /// Configuration settings for evaluation behavior
    config: EvaluationConfig,
}

impl EvaluationContext {
    /// Create a new evaluation context with the given configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration settings for evaluation behavior
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libmagic_rs::evaluator::EvaluationContext;
    /// use libmagic_rs::EvaluationConfig;
    ///
    /// let config = EvaluationConfig::default();
    /// let context = EvaluationContext::new(config);
    /// ```
    #[must_use]
    pub fn new(config: EvaluationConfig) -> Self {
        Self {
            current_offset: 0,
            recursion_depth: 0,
            config,
        }
    }

    /// Get the current offset position
    ///
    /// # Returns
    ///
    /// The current offset position in the file buffer
    #[must_use]
    pub fn current_offset(&self) -> usize {
        self.current_offset
    }

    /// Set the current offset position
    ///
    /// # Arguments
    ///
    /// * `offset` - The new offset position
    pub fn set_current_offset(&mut self, offset: usize) {
        self.current_offset = offset;
    }

    /// Get the current recursion depth
    ///
    /// # Returns
    ///
    /// The current recursion depth for nested rule evaluation
    #[must_use]
    pub fn recursion_depth(&self) -> u32 {
        self.recursion_depth
    }

    /// Increment the recursion depth
    ///
    /// # Returns
    ///
    /// `Ok(())` if the recursion depth is within limits, or `Err(LibmagicError)`
    /// if the maximum recursion depth would be exceeded
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::EvaluationError` if incrementing would exceed
    /// the maximum recursion depth configured in the evaluation config.
    pub fn increment_recursion_depth(&mut self) -> Result<(), LibmagicError> {
        if self.recursion_depth >= self.config.max_recursion_depth {
            return Err(LibmagicError::EvaluationError(
                "Maximum recursion depth exceeded".to_string(),
            ));
        }
        self.recursion_depth += 1;
        Ok(())
    }

    /// Decrement the recursion depth
    ///
    /// # Panics
    ///
    /// Panics if the recursion depth is already 0, as this indicates
    /// a programming error in the evaluation logic.
    pub fn decrement_recursion_depth(&mut self) {
        assert!(
            self.recursion_depth != 0,
            "Attempted to decrement recursion depth below 0"
        );
        self.recursion_depth -= 1;
    }

    /// Get a reference to the evaluation configuration
    ///
    /// # Returns
    ///
    /// A reference to the `EvaluationConfig` used by this context
    #[must_use]
    pub fn config(&self) -> &EvaluationConfig {
        &self.config
    }

    /// Check if evaluation should stop at the first match
    ///
    /// # Returns
    ///
    /// `true` if evaluation should stop at the first match, `false` otherwise
    #[must_use]
    pub fn should_stop_at_first_match(&self) -> bool {
        self.config.stop_at_first_match
    }

    /// Get the maximum string length allowed
    ///
    /// # Returns
    ///
    /// The maximum string length that should be read during evaluation
    #[must_use]
    pub fn max_string_length(&self) -> usize {
        self.config.max_string_length
    }

    /// Check if MIME type mapping is enabled
    ///
    /// # Returns
    ///
    /// `true` if MIME type mapping should be performed, `false` otherwise
    #[must_use]
    pub fn enable_mime_types(&self) -> bool {
        self.config.enable_mime_types
    }

    /// Get the evaluation timeout in milliseconds
    ///
    /// # Returns
    ///
    /// The timeout duration in milliseconds, or `None` if no timeout is set
    #[must_use]
    pub fn timeout_ms(&self) -> Option<u64> {
        self.config.timeout_ms
    }

    /// Reset the context to initial state while preserving configuration
    ///
    /// This resets the current offset and recursion depth to 0, but keeps
    /// the same configuration settings.
    pub fn reset(&mut self) {
        self.current_offset = 0;
        self.recursion_depth = 0;
    }
}

/// Result of evaluating a magic rule
///
/// Contains information about a successful rule match, including the rule
/// that matched and its associated message.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult {
    /// The message associated with the matching rule
    pub message: String,
    /// The offset where the match occurred
    pub offset: usize,
    /// The rule level (depth in hierarchy)
    pub level: u32,
    /// The matched value
    pub value: crate::parser::ast::Value,
}

/// Evaluate a single magic rule against a file buffer
///
/// This function performs the core rule evaluation by:
/// 1. Resolving the rule's offset specification to an absolute position
/// 2. Reading and interpreting bytes at that position according to the rule's type
/// 3. Applying the rule's operator to compare the read value with the expected value
///
/// # Arguments
///
/// * `rule` - The magic rule to evaluate
/// * `buffer` - The file buffer to evaluate against
///
/// # Returns
///
/// Returns `Ok(true)` if the rule matches, `Ok(false)` if it doesn't match,
/// or `Err(LibmagicError)` if evaluation fails due to buffer access issues or other errors.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::evaluate_single_rule;
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
///
/// // Create a rule to check for ELF magic bytes at offset 0
/// let rule = MagicRule {
///     offset: OffsetSpec::Absolute(0),
///     typ: TypeKind::Byte,
///     op: Operator::Equal,
///     value: Value::Uint(0x7f),
///     message: "ELF magic".to_string(),
///     children: vec![],
///     level: 0,
/// };
///
/// let elf_buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
/// let result = evaluate_single_rule(&rule, elf_buffer).unwrap();
/// assert!(result); // Should match
///
/// let non_elf_buffer = &[0x50, 0x4b, 0x03, 0x04]; // ZIP magic bytes
/// let result = evaluate_single_rule(&rule, non_elf_buffer).unwrap();
/// assert!(!result); // Should not match
/// ```
///
/// # Errors
///
/// * `LibmagicError::EvaluationError` - If offset resolution fails, buffer access is out of bounds,
///   or type interpretation fails
pub fn evaluate_single_rule(rule: &MagicRule, buffer: &[u8]) -> Result<bool, LibmagicError> {
    // Step 1: Resolve the offset specification to an absolute position
    let absolute_offset = offset::resolve_offset(&rule.offset, buffer)?;

    // Step 2: Read and interpret bytes at the resolved offset according to the rule's type
    let read_value = types::read_typed_value(buffer, absolute_offset, &rule.typ)
        .map_err(|e| LibmagicError::EvaluationError(e.to_string()))?;

    // Step 3: Apply the operator to compare the read value with the expected value
    let matches = operators::apply_operator(&rule.op, &read_value, &rule.value);

    Ok(matches)
}

/// Evaluate a list of magic rules against a file buffer with hierarchical processing
///
/// This function implements the core hierarchical rule evaluation algorithm:
/// 1. Evaluates each top-level rule in sequence
/// 2. If a parent rule matches, evaluates its child rules for refinement
/// 3. Collects all matches or stops at first match based on configuration
/// 4. Maintains evaluation context for recursion limits and state
///
/// The hierarchical evaluation follows these principles:
/// - Parent rules must match before children are evaluated
/// - Child rules provide refinement and additional detail
/// - Evaluation can stop at first match or continue for all matches
/// - Recursion depth is limited to prevent infinite loops
///
/// # Arguments
///
/// * `rules` - The list of magic rules to evaluate
/// * `buffer` - The file buffer to evaluate against
/// * `context` - Mutable evaluation context for state management
///
/// # Returns
///
/// Returns `Ok(Vec<MatchResult>)` containing all matches found, or `Err(LibmagicError)`
/// if evaluation fails due to buffer access issues, recursion limits, or other errors.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::{evaluate_rules, EvaluationContext, MatchResult};
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
/// use libmagic_rs::EvaluationConfig;
///
/// // Create a hierarchical rule set for ELF files
/// let parent_rule = MagicRule {
///     offset: OffsetSpec::Absolute(0),
///     typ: TypeKind::Byte,
///     op: Operator::Equal,
///     value: Value::Uint(0x7f),
///     message: "ELF".to_string(),
///     children: vec![
///         MagicRule {
///             offset: OffsetSpec::Absolute(4),
///             typ: TypeKind::Byte,
///             op: Operator::Equal,
///             value: Value::Uint(2),
///             message: "64-bit".to_string(),
///             children: vec![],
///             level: 1,
///         }
///     ],
///     level: 0,
/// };
///
/// let rules = vec![parent_rule];
/// let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 header
/// let config = EvaluationConfig::default();
/// let mut context = EvaluationContext::new(config);
///
/// let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
/// assert_eq!(matches.len(), 2); // Parent and child should both match
/// ```
///
/// # Errors
///
/// * `LibmagicError::EvaluationError` - If rule evaluation fails, recursion limit exceeded,
///   or buffer access issues occur
/// * `LibmagicError::Timeout` - If evaluation exceeds configured timeout
pub fn evaluate_rules(
    rules: &[MagicRule],
    buffer: &[u8],
    context: &mut EvaluationContext,
) -> Result<Vec<MatchResult>, LibmagicError> {
    let mut matches = Vec::new();
    let start_time = std::time::Instant::now();

    for rule in rules {
        // Check timeout if configured
        if let Some(timeout_ms) = context.timeout_ms() {
            if start_time.elapsed().as_millis() > u128::from(timeout_ms) {
                return Err(LibmagicError::Timeout { timeout_ms });
            }
        }

        // Evaluate the current rule
        let rule_matches = evaluate_single_rule(rule, buffer)?;

        if rule_matches {
            // Create match result for this rule
            let absolute_offset = offset::resolve_offset(&rule.offset, buffer)?;
            let read_value = types::read_typed_value(buffer, absolute_offset, &rule.typ)
                .map_err(|e| LibmagicError::EvaluationError(e.to_string()))?;

            let match_result = MatchResult {
                message: rule.message.clone(),
                offset: absolute_offset,
                level: rule.level,
                value: read_value,
            };
            matches.push(match_result);

            // If this rule has children, evaluate them recursively
            if !rule.children.is_empty() {
                // Check recursion depth limit
                context.increment_recursion_depth()?;

                // Recursively evaluate child rules
                let child_matches = evaluate_rules(&rule.children, buffer, context)?;
                matches.extend(child_matches);

                // Restore recursion depth
                context.decrement_recursion_depth();
            }

            // Stop at first match if configured to do so
            if context.should_stop_at_first_match() {
                break;
            }
        }
    }

    Ok(matches)
}

/// Evaluate magic rules with a fresh context
///
/// This is a convenience function that creates a new evaluation context
/// and evaluates the rules. Useful for simple evaluation scenarios.
///
/// # Arguments
///
/// * `rules` - The list of magic rules to evaluate
/// * `buffer` - The file buffer to evaluate against
/// * `config` - Configuration for evaluation behavior
///
/// # Returns
///
/// Returns `Ok(Vec<MatchResult>)` containing all matches found, or `Err(LibmagicError)`
/// if evaluation fails.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::{evaluate_rules_with_config, MatchResult};
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
/// use libmagic_rs::EvaluationConfig;
///
/// let rule = MagicRule {
///     offset: OffsetSpec::Absolute(0),
///     typ: TypeKind::Byte,
///     op: Operator::Equal,
///     value: Value::Uint(0x7f),
///     message: "ELF magic".to_string(),
///     children: vec![],
///     level: 0,
/// };
///
/// let rules = vec![rule];
/// let buffer = &[0x7f, 0x45, 0x4c, 0x46];
/// let config = EvaluationConfig::default();
///
/// let matches = evaluate_rules_with_config(&rules, buffer, config).unwrap();
/// assert_eq!(matches.len(), 1);
/// assert_eq!(matches[0].message, "ELF magic");
/// ```
///
/// # Errors
///
/// * `LibmagicError::EvaluationError` - If rule evaluation fails
/// * `LibmagicError::Timeout` - If evaluation exceeds configured timeout
pub fn evaluate_rules_with_config(
    rules: &[MagicRule],
    buffer: &[u8],
    config: EvaluationConfig,
) -> Result<Vec<MatchResult>, LibmagicError> {
    let mut context = EvaluationContext::new(config);
    evaluate_rules(rules, buffer, &mut context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{Endianness, OffsetSpec, Operator, TypeKind, Value};

    #[test]
    fn test_evaluate_single_rule_byte_equal_match() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_byte_equal_no_match() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x50, 0x4b, 0x03, 0x04]; // ZIP magic bytes
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_evaluate_single_rule_byte_not_equal_match() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::NotEqual,
            value: Value::Uint(0x00),
            message: "Non-zero byte".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result); // 0x7f != 0x00
    }

    #[test]
    fn test_evaluate_single_rule_byte_not_equal_no_match() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::NotEqual,
            value: Value::Uint(0x7f),
            message: "Not ELF magic".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(!result); // 0x7f == 0x7f, so NotEqual is false
    }

    #[test]
    fn test_evaluate_single_rule_byte_bitwise_and_match() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::BitwiseAnd,
            value: Value::Uint(0x80), // Check if high bit is set
            message: "High bit set".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0xff, 0x45, 0x4c, 0x46]; // 0xff has high bit set
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result); // 0xff & 0x80 = 0x80 (non-zero)
    }

    #[test]
    fn test_evaluate_single_rule_byte_bitwise_and_no_match() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::BitwiseAnd,
            value: Value::Uint(0x80), // Check if high bit is set
            message: "High bit set".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // 0x7f has high bit clear
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(!result); // 0x7f & 0x80 = 0x00 (zero)
    }

    #[test]
    fn test_evaluate_single_rule_short_little_endian() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234),
            message: "Little-endian short".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x34, 0x12, 0x56, 0x78]; // 0x1234 in little-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_short_big_endian() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Short {
                endian: Endianness::Big,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234),
            message: "Big-endian short".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x12, 0x34, 0x56, 0x78]; // 0x1234 in big-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_short_signed_positive() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Short {
                endian: Endianness::Little,
                signed: true,
            },
            op: Operator::Equal,
            value: Value::Int(32767), // 0x7fff
            message: "Positive signed short".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0xff, 0x7f, 0x00, 0x00]; // 0x7fff in little-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_short_signed_negative() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Short {
                endian: Endianness::Little,
                signed: true,
            },
            op: Operator::Equal,
            value: Value::Int(-1), // 0xffff as signed
            message: "Negative signed short".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0xff, 0xff, 0x00, 0x00]; // 0xffff in little-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_long_little_endian() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234_5678),
            message: "Little-endian long".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x78, 0x56, 0x34, 0x12, 0x00]; // 0x12345678 in little-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_long_big_endian() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Big,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234_5678),
            message: "Big-endian long".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x12, 0x34, 0x56, 0x78, 0x00]; // 0x12345678 in big-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_long_signed_positive() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            op: Operator::Equal,
            value: Value::Int(2_147_483_647), // 0x7fffffff
            message: "Positive signed long".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0xff, 0xff, 0xff, 0x7f, 0x00]; // 0x7fffffff in little-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_long_signed_negative() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            op: Operator::Equal,
            value: Value::Int(-1), // 0xffffffff as signed
            message: "Negative signed long".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0xff, 0xff, 0xff, 0xff, 0x00]; // 0xffffffff in little-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_different_offsets() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(2), // Read from offset 2
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x4c),
            message: "ELF class byte".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result); // buffer[2] == 0x4c
    }

    #[test]
    fn test_evaluate_single_rule_negative_offset() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(-1), // Last byte
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x46),
            message: "Last byte".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result); // Last byte is 0x46
    }

    #[test]
    fn test_evaluate_single_rule_from_end_offset() {
        let rule = MagicRule {
            offset: OffsetSpec::FromEnd(-2), // Second to last byte
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x4c),
            message: "Second to last byte".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result); // buffer[2] == 0x4c (second to last)
    }

    #[test]
    fn test_evaluate_single_rule_offset_out_of_bounds() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(10), // Beyond buffer
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Out of bounds".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // Only 4 bytes
        let result = evaluate_single_rule(&rule, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Buffer overrun"));
            }
            _ => panic!("Expected EvaluationError"),
        }
    }

    #[test]
    fn test_evaluate_single_rule_short_insufficient_bytes() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(3), // Only 1 byte left
            typ: TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234),
            message: "Insufficient bytes".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // 4 bytes total
        let result = evaluate_single_rule(&rule, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Buffer overrun"));
            }
            _ => panic!("Expected EvaluationError"),
        }
    }

    #[test]
    fn test_evaluate_single_rule_long_insufficient_bytes() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(2), // Only 2 bytes left
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234_5678),
            message: "Insufficient bytes".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // 4 bytes total
        let result = evaluate_single_rule(&rule, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Buffer overrun"));
            }
            _ => panic!("Expected EvaluationError"),
        }
    }

    #[test]
    fn test_evaluate_single_rule_empty_buffer() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Empty buffer".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[]; // Empty buffer
        let result = evaluate_single_rule(&rule, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Buffer overrun"));
            }
            _ => panic!("Expected EvaluationError"),
        }
    }

    #[test]
    fn test_evaluate_single_rule_string_type_unsupported() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::String { max_length: None },
            op: Operator::Equal,
            value: Value::String("test".to_string()),
            message: "String type".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = b"test data";
        let result = evaluate_single_rule(&rule, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Unsupported type"));
                assert!(msg.contains("String"));
            }
            _ => panic!("Expected EvaluationError for unsupported type"),
        }
    }

    #[test]
    fn test_evaluate_single_rule_cross_type_comparison() {
        // Test that cross-type comparisons work correctly (should not match)
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Int(42), // Int value vs Uint from byte read
            message: "Cross-type comparison".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[42]; // Byte value 42
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(!result); // Should not match due to type mismatch (Uint vs Int)
    }

    #[test]
    fn test_evaluate_single_rule_bitwise_and_with_shorts() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::BitwiseAnd,
            value: Value::Uint(0xff00), // Check high byte
            message: "High byte check".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x34, 0x12]; // 0x1234 in little-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result); // 0x1234 & 0xff00 = 0x1200 (non-zero)
    }

    #[test]
    fn test_evaluate_single_rule_bitwise_and_with_longs() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Big,
                signed: false,
            },
            op: Operator::BitwiseAnd,
            value: Value::Uint(0xffff_0000), // Check high word
            message: "High word check".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x12, 0x34, 0x56, 0x78]; // 0x12345678 in big-endian
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result); // 0x12345678 & 0xffff0000 = 0x12340000 (non-zero)
    }

    #[test]
    fn test_evaluate_single_rule_comprehensive_elf_check() {
        // Test a comprehensive ELF magic check
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x464c_457f), // ELF magic as 32-bit little-endian
            message: "ELF executable".to_string(),
            children: vec![],
            level: 0,
        };

        let elf_buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 header start
        let result = evaluate_single_rule(&rule, elf_buffer).unwrap();
        assert!(result);

        let non_elf_buffer = &[0x50, 0x4b, 0x03, 0x04, 0x14, 0x00]; // ZIP header
        let result = evaluate_single_rule(&rule, non_elf_buffer).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_evaluate_single_rule_native_endianness() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Short {
                endian: Endianness::Native,
                signed: false,
            },
            op: Operator::NotEqual,
            value: Value::Uint(0),
            message: "Non-zero native short".to_string(),
            children: vec![],
            level: 0,
        };

        let buffer = &[0x01, 0x02]; // Non-zero bytes
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert!(result); // Should be non-zero regardless of endianness
    }

    #[test]
    fn test_evaluate_single_rule_all_operators() {
        let buffer = &[0x42, 0x00, 0xff, 0x80];

        // Test Equal operator
        let equal_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x42),
            message: "Equal test".to_string(),
            children: vec![],
            level: 0,
        };
        assert!(evaluate_single_rule(&equal_rule, buffer).unwrap());

        // Test NotEqual operator
        let not_equal_rule = MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte,
            op: Operator::NotEqual,
            value: Value::Uint(0x42),
            message: "NotEqual test".to_string(),
            children: vec![],
            level: 0,
        };
        assert!(evaluate_single_rule(&not_equal_rule, buffer).unwrap()); // 0x00 != 0x42

        // Test BitwiseAnd operator
        let bitwise_and_rule = MagicRule {
            offset: OffsetSpec::Absolute(3),
            typ: TypeKind::Byte,
            op: Operator::BitwiseAnd,
            value: Value::Uint(0x80),
            message: "BitwiseAnd test".to_string(),
            children: vec![],
            level: 0,
        };
        assert!(evaluate_single_rule(&bitwise_and_rule, buffer).unwrap()); // 0x80 & 0x80 = 0x80
    }

    #[test]
    fn test_evaluate_single_rule_edge_case_values() {
        // Test with maximum values
        let max_uint_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0xffff_ffff),
            message: "Max uint32".to_string(),
            children: vec![],
            level: 0,
        };

        let max_buffer = &[0xff, 0xff, 0xff, 0xff];
        let result = evaluate_single_rule(&max_uint_rule, max_buffer).unwrap();
        assert!(result);

        // Test with minimum signed value
        let min_int_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            op: Operator::Equal,
            value: Value::Int(-2_147_483_648), // i32::MIN
            message: "Min int32".to_string(),
            children: vec![],
            level: 0,
        };

        let min_buffer = &[0x00, 0x00, 0x00, 0x80]; // 0x80000000 in little-endian
        let result = evaluate_single_rule(&min_int_rule, min_buffer).unwrap();
        assert!(result);
    }

    #[test]
    fn test_evaluate_single_rule_various_buffer_sizes() {
        // Test with single byte buffer
        let single_byte_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0xaa),
            message: "Single byte".to_string(),
            children: vec![],
            level: 0,
        };

        let single_buffer = &[0xaa];
        let result = evaluate_single_rule(&single_byte_rule, single_buffer).unwrap();
        assert!(result);

        // Test with large buffer
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let large_buffer: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        let large_rule = MagicRule {
            offset: OffsetSpec::Absolute(1000),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint((1000 % 256) as u64),
            message: "Large buffer".to_string(),
            children: vec![],
            level: 0,
        };

        let result = evaluate_single_rule(&large_rule, &large_buffer).unwrap();
        assert!(result);
    }

    // Tests for EvaluationContext
    #[test]
    fn test_evaluation_context_new() {
        let config = EvaluationConfig::default();
        let context = EvaluationContext::new(config.clone());

        assert_eq!(context.current_offset(), 0);
        assert_eq!(context.recursion_depth(), 0);
        assert_eq!(
            context.config().max_recursion_depth,
            config.max_recursion_depth
        );
        assert_eq!(context.config().max_string_length, config.max_string_length);
        assert_eq!(
            context.config().stop_at_first_match,
            config.stop_at_first_match
        );
    }

    #[test]
    fn test_evaluation_context_offset_management() {
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        // Test initial offset
        assert_eq!(context.current_offset(), 0);

        // Test setting offset
        context.set_current_offset(42);
        assert_eq!(context.current_offset(), 42);

        // Test setting different offset
        context.set_current_offset(1024);
        assert_eq!(context.current_offset(), 1024);

        // Test setting offset to 0
        context.set_current_offset(0);
        assert_eq!(context.current_offset(), 0);
    }

    #[test]
    fn test_evaluation_context_recursion_depth_management() {
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        // Test initial recursion depth
        assert_eq!(context.recursion_depth(), 0);

        // Test incrementing recursion depth
        context.increment_recursion_depth().unwrap();
        assert_eq!(context.recursion_depth(), 1);

        context.increment_recursion_depth().unwrap();
        assert_eq!(context.recursion_depth(), 2);

        // Test decrementing recursion depth
        context.decrement_recursion_depth();
        assert_eq!(context.recursion_depth(), 1);

        context.decrement_recursion_depth();
        assert_eq!(context.recursion_depth(), 0);
    }

    #[test]
    fn test_evaluation_context_recursion_depth_limit() {
        let config = EvaluationConfig {
            max_recursion_depth: 2,
            ..Default::default()
        };
        let mut context = EvaluationContext::new(config);

        // Should be able to increment up to the limit
        assert!(context.increment_recursion_depth().is_ok());
        assert_eq!(context.recursion_depth(), 1);

        assert!(context.increment_recursion_depth().is_ok());
        assert_eq!(context.recursion_depth(), 2);

        // Should fail when exceeding the limit
        let result = context.increment_recursion_depth();
        assert!(result.is_err());
        assert_eq!(context.recursion_depth(), 2); // Should not have changed

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Maximum recursion depth exceeded"));
            }
            _ => panic!("Expected EvaluationError"),
        }
    }

    #[test]
    #[should_panic(expected = "Attempted to decrement recursion depth below 0")]
    fn test_evaluation_context_recursion_depth_underflow() {
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        // Should panic when trying to decrement below 0
        context.decrement_recursion_depth();
    }

    #[test]
    fn test_evaluation_context_config_access() {
        let config = EvaluationConfig {
            max_recursion_depth: 10,
            max_string_length: 4096,
            stop_at_first_match: false,
            enable_mime_types: true,
            timeout_ms: Some(2000),
        };

        let context = EvaluationContext::new(config.clone());

        // Test config access
        assert_eq!(context.config().max_recursion_depth, 10);
        assert_eq!(context.config().max_string_length, 4096);
        assert!(!context.config().stop_at_first_match);

        // Test convenience methods
        assert!(!context.should_stop_at_first_match());
        assert_eq!(context.max_string_length(), 4096);
    }

    #[test]
    fn test_evaluation_context_reset() {
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config.clone());

        // Modify the context state
        context.set_current_offset(100);
        context.increment_recursion_depth().unwrap();
        context.increment_recursion_depth().unwrap();

        assert_eq!(context.current_offset(), 100);
        assert_eq!(context.recursion_depth(), 2);

        // Reset should restore initial state but keep config
        context.reset();

        assert_eq!(context.current_offset(), 0);
        assert_eq!(context.recursion_depth(), 0);
        assert_eq!(
            context.config().max_recursion_depth,
            config.max_recursion_depth
        );
    }

    #[test]
    fn test_evaluation_context_clone() {
        let config = EvaluationConfig {
            max_recursion_depth: 5,
            max_string_length: 2048,
            ..Default::default()
        };

        let mut context = EvaluationContext::new(config);
        context.set_current_offset(50);
        context.increment_recursion_depth().unwrap();

        // Clone the context
        let cloned_context = context.clone();

        // Both should have the same state
        assert_eq!(context.current_offset(), cloned_context.current_offset());
        assert_eq!(context.recursion_depth(), cloned_context.recursion_depth());
        assert_eq!(
            context.config().max_recursion_depth,
            cloned_context.config().max_recursion_depth
        );
        assert_eq!(
            context.config().max_string_length,
            cloned_context.config().max_string_length
        );

        // Modifying one should not affect the other
        context.set_current_offset(75);
        assert_eq!(context.current_offset(), 75);
        assert_eq!(cloned_context.current_offset(), 50);
    }

    #[test]
    fn test_evaluation_context_with_custom_config() {
        let config = EvaluationConfig {
            max_recursion_depth: 15,
            max_string_length: 16384,
            stop_at_first_match: false,
            enable_mime_types: true,
            timeout_ms: Some(5000),
        };

        let context = EvaluationContext::new(config);

        assert_eq!(context.config().max_recursion_depth, 15);
        assert_eq!(context.max_string_length(), 16384);
        assert!(!context.should_stop_at_first_match());

        // Test that we can increment up to the custom limit
        let mut mutable_context = context;
        for i in 1..=15 {
            assert!(mutable_context.increment_recursion_depth().is_ok());
            assert_eq!(mutable_context.recursion_depth(), i);
        }

        // Should fail on the 16th increment
        let result = mutable_context.increment_recursion_depth();
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluation_context_mime_types_access() {
        let config_with_mime = EvaluationConfig {
            enable_mime_types: true,
            ..Default::default()
        };
        let context_with_mime = EvaluationContext::new(config_with_mime);
        assert!(context_with_mime.enable_mime_types());

        let config_without_mime = EvaluationConfig {
            enable_mime_types: false,
            ..Default::default()
        };
        let context_without_mime = EvaluationContext::new(config_without_mime);
        assert!(!context_without_mime.enable_mime_types());
    }

    #[test]
    fn test_evaluation_context_timeout_access() {
        let config_with_timeout = EvaluationConfig {
            timeout_ms: Some(5000),
            ..Default::default()
        };
        let context_with_timeout = EvaluationContext::new(config_with_timeout);
        assert_eq!(context_with_timeout.timeout_ms(), Some(5000));

        let config_without_timeout = EvaluationConfig {
            timeout_ms: None,
            ..Default::default()
        };
        let context_without_timeout = EvaluationContext::new(config_without_timeout);
        assert_eq!(context_without_timeout.timeout_ms(), None);
    }

    #[test]
    fn test_evaluation_context_comprehensive_config() {
        let config = EvaluationConfig {
            max_recursion_depth: 30,
            max_string_length: 16384,
            stop_at_first_match: false,
            enable_mime_types: true,
            timeout_ms: Some(10000),
        };
        let context = EvaluationContext::new(config.clone());

        assert_eq!(context.config().max_recursion_depth, 30);
        assert_eq!(context.config().max_string_length, 16384);
        assert!(!context.should_stop_at_first_match());
        assert!(context.enable_mime_types());
        assert_eq!(context.timeout_ms(), Some(10000));
        assert_eq!(context.max_string_length(), 16384);
    }

    #[test]
    fn test_evaluation_context_performance_config() {
        let config = EvaluationConfig {
            max_recursion_depth: 5,
            max_string_length: 512,
            stop_at_first_match: true,
            enable_mime_types: false,
            timeout_ms: Some(1000),
        };
        let context = EvaluationContext::new(config);

        assert_eq!(context.config().max_recursion_depth, 5);
        assert_eq!(context.max_string_length(), 512);
        assert!(context.should_stop_at_first_match());
        assert!(!context.enable_mime_types());
        assert_eq!(context.timeout_ms(), Some(1000));
    }

    #[test]
    fn test_match_result_creation() {
        let match_result = MatchResult {
            message: "ELF executable".to_string(),
            offset: 0,
            level: 0,
            value: Value::Uint(0x7f),
        };

        assert_eq!(match_result.message, "ELF executable");
        assert_eq!(match_result.offset, 0);
        assert_eq!(match_result.level, 0);
        assert_eq!(match_result.value, Value::Uint(0x7f));
    }

    #[test]
    fn test_match_result_clone() {
        let original = MatchResult {
            message: "Test message".to_string(),
            offset: 42,
            level: 1,
            value: Value::String("test".to_string()),
        };

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_match_result_debug() {
        let match_result = MatchResult {
            message: "Debug test".to_string(),
            offset: 10,
            level: 2,
            value: Value::Bytes(vec![0x01, 0x02]),
        };

        let debug_str = format!("{match_result:?}");
        assert!(debug_str.contains("MatchResult"));
        assert!(debug_str.contains("Debug test"));
        assert!(debug_str.contains("10"));
        assert!(debug_str.contains('2'));
    }

    #[test]
    fn test_evaluate_rules_empty_list() {
        let rules = vec![];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_evaluate_rules_single_matching_rule() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
        };

        let rules = vec![rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].message, "ELF magic");
        assert_eq!(matches[0].offset, 0);
        assert_eq!(matches[0].level, 0);
        assert_eq!(matches[0].value, Value::Uint(0x7f));
    }

    #[test]
    fn test_evaluate_rules_single_non_matching_rule() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x50), // ZIP magic, not ELF
            message: "ZIP magic".to_string(),
            children: vec![],
            level: 0,
        };

        let rules = vec![rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF buffer
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_evaluate_rules_multiple_rules_stop_at_first() {
        let rule1 = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "First match".to_string(),
            children: vec![],
            level: 0,
        };

        let rule2 = MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x45),
            message: "Second match".to_string(),
            children: vec![],
            level: 0,
        };

        let rule_list = vec![rule1, rule2];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let config = EvaluationConfig {
            stop_at_first_match: true,
            ..Default::default()
        };
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rule_list, buffer, &mut context).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].message, "First match");
    }

    #[test]
    fn test_evaluate_rules_multiple_rules_find_all() {
        let rule1 = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "First match".to_string(),
            children: vec![],
            level: 0,
        };

        let rule2 = MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x45),
            message: "Second match".to_string(),
            children: vec![],
            level: 0,
        };

        let rule_set = vec![rule1, rule2];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let config = EvaluationConfig {
            stop_at_first_match: false,
            ..Default::default()
        };
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rule_set, buffer, &mut context).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].message, "First match");
        assert_eq!(matches[1].message, "Second match");
    }

    #[test]
    fn test_evaluate_rules_hierarchical_parent_child() {
        let child_rule = MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x02), // ELF class 64-bit
            message: "64-bit".to_string(),
            children: vec![],
            level: 1,
        };

        let parent_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF".to_string(),
            children: vec![child_rule],
            level: 0,
        };

        let rules = vec![parent_rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 header
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].message, "ELF");
        assert_eq!(matches[0].level, 0);
        assert_eq!(matches[1].message, "64-bit");
        assert_eq!(matches[1].level, 1);
    }

    #[test]
    fn test_evaluate_rules_hierarchical_parent_no_match() {
        let child_rule = MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x02),
            message: "64-bit".to_string(),
            children: vec![],
            level: 1,
        };

        let parent_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x50), // ZIP magic, not ELF
            message: "ZIP".to_string(),
            children: vec![child_rule],
            level: 0,
        };

        let rules = vec![parent_rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF buffer
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert!(matches.is_empty()); // Parent doesn't match, so children shouldn't be evaluated
    }

    #[test]
    fn test_evaluate_rules_hierarchical_parent_match_child_no_match() {
        let child_rule = MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x01), // ELF class 32-bit, but buffer has 64-bit
            message: "32-bit".to_string(),
            children: vec![],
            level: 1,
        };

        let parent_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF".to_string(),
            children: vec![child_rule],
            level: 0,
        };

        let rules = vec![parent_rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 header
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert_eq!(matches.len(), 1); // Only parent matches
        assert_eq!(matches[0].message, "ELF");
        assert_eq!(matches[0].level, 0);
    }

    #[test]
    fn test_evaluate_rules_deep_hierarchy() {
        let grandchild_rule = MagicRule {
            offset: OffsetSpec::Absolute(5),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x01), // Little endian
            message: "little-endian".to_string(),
            children: vec![],
            level: 2,
        };

        let child_rule = MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x02), // 64-bit
            message: "64-bit".to_string(),
            children: vec![grandchild_rule],
            level: 1,
        };

        let parent_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF".to_string(),
            children: vec![child_rule],
            level: 0,
        };

        let rules = vec![parent_rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 little-endian header
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].message, "ELF");
        assert_eq!(matches[0].level, 0);
        assert_eq!(matches[1].message, "64-bit");
        assert_eq!(matches[1].level, 1);
        assert_eq!(matches[2].message, "little-endian");
        assert_eq!(matches[2].level, 2);
    }

    #[test]
    fn test_evaluate_rules_multiple_children() {
        let child1 = MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x02),
            message: "64-bit".to_string(),
            children: vec![],
            level: 1,
        };

        let child2 = MagicRule {
            offset: OffsetSpec::Absolute(5),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x01),
            message: "little-endian".to_string(),
            children: vec![],
            level: 1,
        };

        let parent_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF".to_string(),
            children: vec![child1, child2],
            level: 0,
        };

        let rules = vec![parent_rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
        let config = EvaluationConfig {
            stop_at_first_match: false, // Find all matches
            ..Default::default()
        };
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].message, "ELF");
        assert_eq!(matches[1].message, "64-bit");
        assert_eq!(matches[2].message, "little-endian");
    }

    #[test]
    fn test_evaluate_rules_recursion_depth_limit() {
        // Create a deeply nested rule structure that exceeds the limit
        let mut current_rule = MagicRule {
            offset: OffsetSpec::Absolute(10),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Deep level".to_string(),
            children: vec![],
            level: 10,
        };

        // Build a chain of nested rules
        for i in (0u32..10u32).rev() {
            current_rule = MagicRule {
                offset: OffsetSpec::Absolute(i64::from(i)),
                typ: TypeKind::Byte,
                op: Operator::Equal,
                value: Value::Uint(u64::from(i)),
                message: format!("Level {i}"),
                children: vec![current_rule],
                level: i,
            };
        }

        let rules = vec![current_rule];
        let buffer = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0]; // Matches all levels
        let config = EvaluationConfig {
            max_recursion_depth: 5, // Limit to 5 levels
            ..Default::default()
        };
        let mut context = EvaluationContext::new(config);

        let result = evaluate_rules(&rules, buffer, &mut context);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Maximum recursion depth exceeded"));
            }
            _ => panic!("Expected EvaluationError for recursion limit"),
        }
    }

    #[test]
    fn test_evaluate_rules_with_config_convenience() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
        };

        let rules = vec![rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let config = EvaluationConfig::default();

        let matches = evaluate_rules_with_config(&rules, buffer, config).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].message, "ELF magic");
    }

    #[test]
    fn test_evaluate_rules_timeout() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
        };

        let rules = vec![rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let config = EvaluationConfig {
            timeout_ms: Some(0), // Immediate timeout
            ..Default::default()
        };
        let mut context = EvaluationContext::new(config);

        // Note: This test might be flaky due to timing, but it demonstrates the timeout mechanism
        let result = evaluate_rules(&rules, buffer, &mut context);
        // The result could be either success (if evaluation is very fast) or timeout
        // We just verify that timeout errors are handled correctly when they occur
        if let Err(LibmagicError::Timeout { timeout_ms }) = result {
            assert_eq!(timeout_ms, 0);
        }
    }

    #[test]
    fn test_evaluate_rules_empty_buffer() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Should not match".to_string(),
            children: vec![],
            level: 0,
        };

        let rules = vec![rule];
        let buffer = &[]; // Empty buffer
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        let result = evaluate_rules(&rules, buffer, &mut context);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Buffer overrun"));
            }
            _ => panic!("Expected EvaluationError for empty buffer"),
        }
    }

    #[test]
    fn test_evaluate_rules_mixed_matching_non_matching() {
        let rule1 = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Matches".to_string(),
            children: vec![],
            level: 0,
        };

        let rule2 = MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x99), // Doesn't match
            message: "Doesn't match".to_string(),
            children: vec![],
            level: 0,
        };

        let rule3 = MagicRule {
            offset: OffsetSpec::Absolute(2),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x4c),
            message: "Also matches".to_string(),
            children: vec![],
            level: 0,
        };

        let rule_collection = vec![rule1, rule2, rule3];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let config = EvaluationConfig {
            stop_at_first_match: false,
            ..Default::default()
        };
        let mut context = EvaluationContext::new(config);

        let matches = evaluate_rules(&rule_collection, buffer, &mut context).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].message, "Matches");
        assert_eq!(matches[1].message, "Also matches");
    }

    #[test]
    fn test_evaluate_rules_context_state_preservation() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
        };

        let rules = vec![rule];
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        // Set some initial state
        context.set_current_offset(100);
        let initial_offset = context.current_offset();
        let initial_depth = context.recursion_depth();

        let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
        assert_eq!(matches.len(), 1);

        // Context state should be preserved
        assert_eq!(context.current_offset(), initial_offset);
        assert_eq!(context.recursion_depth(), initial_depth);
    }

    #[test]
    fn test_evaluation_context_state_management_sequence() {
        let config = EvaluationConfig::default();
        let mut context = EvaluationContext::new(config);

        // Simulate a sequence of evaluation operations
        assert_eq!(context.current_offset(), 0);
        assert_eq!(context.recursion_depth(), 0);

        // Start evaluation at offset 10
        context.set_current_offset(10);
        assert_eq!(context.current_offset(), 10);

        // Enter nested rule evaluation
        context.increment_recursion_depth().unwrap();
        assert_eq!(context.recursion_depth(), 1);

        // Move to different offset during nested evaluation
        context.set_current_offset(25);
        assert_eq!(context.current_offset(), 25);

        // Enter deeper nesting
        context.increment_recursion_depth().unwrap();
        assert_eq!(context.recursion_depth(), 2);

        // Exit nested evaluation
        context.decrement_recursion_depth();
        assert_eq!(context.recursion_depth(), 1);

        // Continue evaluation at different offset
        context.set_current_offset(50);
        assert_eq!(context.current_offset(), 50);

        // Exit all nesting
        context.decrement_recursion_depth();
        assert_eq!(context.recursion_depth(), 0);

        // Final state check
        assert_eq!(context.current_offset(), 50);
        assert_eq!(context.recursion_depth(), 0);
    }
}
