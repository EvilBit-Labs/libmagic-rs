# Commit Message Style for libmagic-rs

Use Conventional Commits: `<type>(<scope>): <description>`

- **Types**: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`
- **Scopes** (required): `parser`, `evaluator`, `cli`, `lib`, `io`, `output`, `ast`, `types`, `operators`, `offset`, `regex`, `endian`, `magic`, `format`, `bench`, `docs`, `test`, `ci`, `deps`, `security`, etc.
- **Description**: imperative, capitalized, ≤72 chars, no period
- **Body** (optional): blank line, bullet list, explain what/why
- **Footer** (optional): blank line, issue refs (`Closes #123`) or `BREAKING CHANGE:`
- **Breaking changes**: add `!` after type/scope or use `BREAKING CHANGE:`

Examples:

- `feat(parser): add support for indirect offset resolution`
- `fix(evaluator): handle malformed magic rules gracefully`
- `docs(readme): update installation instructions for Rust 1.85+`
- `chore(deps): update memmap2 to v0.9 for security patches`
