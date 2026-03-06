# Epic Brief: libmagic-rs Phase 1 MVP

## Summary

Complete Phase 1 MVP of libmagic-rs, a pure Rust implementation of libmagic for file type detection. The core infrastructure (AST, parser components, evaluator, I/O, output formatters, CLI skeleton, error handling) is complete. This Epic focuses on implementing the critical missing pieces: text magic file parsing (binary .mgc format is explicitly deferred to Phase 2 following the proven OpenBSD approach), end-to-end integration, comprehensive testing, and comprehensive documentation. The goal is to deliver a production-ready, memory-safe alternative to C-based libmagic that achieves high compatibility with GNU file command, works with system text magic databases, and is ready for early adopters with >85% test coverage and complete API documentation (rustdoc + mdbook).

## Context & Problem

### Who's Affected

**Primary Users:**

- **Rust Application Developers** who need file type detection in their applications and want memory-safe alternatives to C libraries
- **Security-Conscious Organizations** requiring file type identification without the memory safety vulnerabilities present in C-based libmagic
- **CLI Users** seeking a drop-in replacement for the GNU `file` command with better safety guarantees

**Secondary Users:**

- **Library Maintainers** who depend on libmagic and want to migrate to safer alternatives
- **DevOps Engineers** integrating file type detection into build pipelines and automation workflows

### Current Pain

**Incomplete Implementation:** The libmagic-rs project has solid foundations (tasks 1-13 complete) but lacks the critical functionality needed for real-world usage:

- Cannot parse magic files (the DSL that defines file type detection rules)
- Cannot load system magic databases (text `.magic` format - binary `.mgc` is deferred to Phase 2)
- Missing end-to-end integration between parsing and evaluation
- No test infrastructure to validate compatibility with GNU file
- Lacks documentation for library consumers

**Memory Safety Concerns:** The C-based libmagic has a history of memory safety vulnerabilities (buffer overflows, out-of-bounds reads) that pose security risks. Developers need a memory-safe alternative that provides the same functionality without these risks.

**Ecosystem Gap:** While Rust's ecosystem is growing, there's no production-ready, fully-featured file type detection library that:

- Works with existing magic databases (compatibility)
- Provides both CLI and library interfaces
- Achieves performance parity with libmagic
- Offers comprehensive documentation and migration guides

### Where in the Product

This Epic addresses the **core functionality gap** in libmagic-rs:

1. **Magic File Parsing Layer** - Currently missing, needs implementation for text format (binary .mgc deferred to Phase 2 per OpenBSD approach)
2. **Integration Layer** - MagicDatabase loading and file evaluation pipeline needs completion
3. **Validation Layer** - Test infrastructure required to ensure compatibility and correctness
4. **Documentation Layer** - API documentation and examples needed for library adoption

### Scope Appetite

**Phase 1 MVP Boundaries:**

- **In Scope**: Absolute offsets, basic types (byte/short/long/string), core operators (=, !=, &), nested rules, text magic file support (Magdir directory loading), stdin input support, high GNU file compatibility, comprehensive testing, comprehensive documentation (rustdoc + mdbook)
- **Out of Scope (Stretch Goals)**: Indirect offsets, regex support, Aho-Corasick optimization, PE/Mach-O format detection, advanced operators, performance optimization
- **Timeline**: Quality and completeness over speed - no hard deadlines
- **Success Criteria**:
  - CLI works end-to-end with file and stdin input
  - 100% compatibility with GNU file for common types (ELF, PE, ZIP, JPEG, PNG, PDF)
  - 95%+ compatibility for full test corpus (using text magic files)
  - > 85% test coverage
  - Complete API documentation: rustdoc for all public APIs + mdbook documentation site with guides, examples, and migration information

### Value Proposition

Completing Phase 1 MVP delivers:

- **For Developers**: A production-ready, memory-safe file type detection library with clean Rust APIs
- **For Security Teams**: Elimination of memory safety vulnerabilities present in C libmagic
- **For CLI Users**: A drop-in replacement for GNU file with better safety guarantees
- **For the Ecosystem**: A foundation for future enhancements (regex, advanced formats, optimizations)

### Key Assumptions

1. Existing third-party test corpus (`file:third_party/tests/`) is sufficient for compatibility validation
2. Binary `.mgc` format is explicitly deferred to Phase 2 following the proven OpenBSD approach (see `file:docs/research/magic-mgc-format-analysis.md`)
   - **Rationale**: All successful reimplementations (OpenBSD, PolyFile, arcana) parse text format only. Binary .mgc has version lock-in, platform-specific issues, and high complexity (~3,000 lines). Text format is the stable interface.
3. Built-in fallback rules for common file types will enable out-of-the-box functionality
4. Vertical slice approach (text format first, then binary) minimizes risk and enables early validation
5. Existing design documentation (`file:.kiro/specs/rust-libmagic-implementation/`) accurately represents the technical approach
6. No performance targets for MVP - focus on correctness and compatibility, optimize in Phase 3
