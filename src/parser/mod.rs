//! Magic file parser module
//!
//! This module handles parsing of magic files into an Abstract Syntax Tree (AST)
//! that can be evaluated against file buffers for type identification.

pub mod ast;
pub mod grammar;

// Re-export AST types for convenience
pub use ast::{Endianness, MagicRule, OffsetSpec, Operator, TypeKind, Value};

// Re-export parser functions for convenience
pub use grammar::{parse_number, parse_offset};
