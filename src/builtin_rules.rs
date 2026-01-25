//! Built-in magic rules compiled at build time.
//!
//! This module contains magic rules that are compiled into the library binary
//! at build time from the `src/builtin_rules.magic` file. The rules are parsed
//! during the build process and converted into Rust code for efficient loading.
//!
//! The `BUILTIN_RULES` static is lazily initialized on first access using
//! `std::sync::LazyLock`, ensuring minimal overhead when not used.
//!
//! # Build-Time Generation
//!
//! During `cargo build`, the build script (`build.rs`):
//! 1. Reads and parses `src/builtin_rules.magic`
//! 2. Converts the magic rules into Rust code
//! 3. Generates a static `LazyLock<Vec<MagicRule>>` containing all rules
//! 4. Writes the generated code to `$OUT_DIR/builtin_rules.rs`
//!
//! This module includes that generated file and provides a public API to access
//! the compiled rules.
//!
//! # Coverage
//!
//! The built-in rules include high-confidence detection patterns for common file types:
//! - **Executables**: ELF, PE/DOS
//! - **Archives**: ZIP, TAR, GZIP
//! - **Images**: JPEG, PNG, GIF, BMP
//! - **Documents**: PDF
//!
//! # Example
//!
//! ```
//! use libmagic_rs::builtin_rules::get_builtin_rules;
//!
//! let rules = get_builtin_rules();
//! println!("Loaded {} built-in rules", rules.len());
//! ```

// Include the build-time generated code containing BUILTIN_RULES static
include!(concat!(env!("OUT_DIR"), "/builtin_rules.rs"));

/// Returns a copy of the built-in magic rules.
///
/// This function provides access to the magic rules compiled at build time from
/// `src/builtin_rules.magic`. The rules are stored in a `LazyLock` static, so
/// initialization only happens on the first call.
///
/// # Rules Included
///
/// The built-in rules include high-confidence file type detection for:
/// - **Executable formats**: ELF (32/64-bit, LSB/MSB), PE/DOS executables
/// - **Archive formats**: ZIP, TAR (POSIX), GZIP
/// - **Image formats**: JPEG/JFIF, PNG, GIF (87a/89a), BMP
/// - **Document formats**: PDF
///
/// # Performance
///
/// The rules are lazily initialized using `LazyLock`, meaning:
/// - First call performs one-time initialization
/// - Subsequent calls are very fast (just cloning the Vec)
/// - Safe to call from multiple threads (initialization is synchronized)
///
/// # Returns
///
/// A cloned `Vec<MagicRule>` containing all built-in magic rules. Each caller
/// gets an independent copy that can be modified without affecting other callers.
///
/// # Examples
///
/// ```
/// use libmagic_rs::builtin_rules::get_builtin_rules;
///
/// let rules = get_builtin_rules();
/// println!("Built-in rules count: {}", rules.len());
///
/// // Rules can be used directly with the evaluator
/// // or combined with custom rules
/// ```
///
/// # See Also
///
/// - [`MagicDatabase::with_builtin_rules()`](crate::MagicDatabase::with_builtin_rules) - Recommended way to use built-in rules
/// - [`MagicDatabase::with_builtin_rules_and_config()`](crate::MagicDatabase::with_builtin_rules_and_config) - With custom configuration
pub fn get_builtin_rules() -> Vec<crate::parser::ast::MagicRule> {
    BUILTIN_RULES.clone()
}
