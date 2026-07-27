# Issue #383 — Symlink Follow Policy: Broken-Link Reporting and `-L` / `--no-dereference`

## Issue Summary

GitHub: [EvilBit-Labs/libmagic-rs#383](https://github.com/EvilBit-Labs/libmagic-rs/issues/383) Milestone: none assigned Labels: enhancement, cli, type:feature, requirements, priority:normal Scope decision: full magic(1) parity (both phases), chosen 2026-07-26.

rmagic emits an I/O error to stderr and no stdout classification for a broken symlink, where GNU `file` prints `broken symbolic link to <target>` on stdout and exits 0. rmagic also has no dereference-control flags, so there is no way to ask it to describe a symlink rather than its target.

The issue body framed three questions as open. Two are settled by the AGENTS.md v1.0.0 goal of "95%+ compatibility with GNU file": rmagic follows symlinks by default (already its de-facto behavior via `fs::metadata`), and a broken symlink is reported as a classification rather than an error. The third — whether to add dereference-control flags — was the real fork and is resolved here as **yes, both**, as `-L, --dereference` and `--no-dereference`.

### Governing compatibility contract

This issue is decided under **[ADR-0001](../adr/0001-gnu-file-output-contract.md)**: compatibility with GNU `file` is a contract on **detection results** for identical input. Error messages and tool ergonomics are rmagic's own. Applying the ADR's boundary test — *if the file were readable, would this string describe what it is?* — splits this issue's output cleanly:

- **Binding (detection).** `broken symbolic link to <target>` and `symbolic link to <target>` must match `file` byte-for-byte, including the **verbatim, uncanonicalized** target. Both describe what the path *is* — a link, and where it points. This is the whole substance of the reported bug.
- **Not binding (diagnostic).** `` unreadable symlink `<path>' (No such file or directory) `` describes why rmagic could not resolve the link, and carries errno text. rmagic may word this as it likes. What it must **not** do is mis-classify the case as a broken link with an empty target (`broken symbolic link to ` with a dangling space) — that is a wrong *detection*, not a wording choice.
- **Not binding (ergonomics).** Which letter selects no-dereference, whether `--strict` exists, what exit code an unreadable path produces, and which stream it goes to.

ADR-0001 requires every **detection** divergence to be tracked and closed. This work surfaces exactly one — `directory` classification — and it is **fixed here in Phase 1b**, not deferred to a follow-up issue. See "ADR-0001 contract gaps" under Out of Scope for the full boundary-test disposition of all four candidates.

### `-h` stays bound to `--help` (decided 2026-07-26)

Under the contract above, `-h` is ergonomics, so matching magic(1) here carries no weight — and the local reasons all point the other way.

GNU `file` spells no-dereference `-h, --no-dereference` and has **no short help flag** at all (verified: `file --help` lists a bare `--help`). rmagic will **not** copy that. `-h` remains rmagic's `--help` short flag and `--no-dereference` is long-form only.

Rationale:

1. **Rebinding `-h` is a breaking change to rmagic's own documented CLI.** `docs/CLI_REFERENCE.md:70` and `docs/src/cli-reference.md:65` both currently publish `` `-h, --help` ``. Taking `-h` away silently changes what an existing `rmagic -h` invocation does.
2. **The failure mode of *not* matching `file` is safe; the reverse is not.** Someone porting `file -h x` to rmagic gets a usage screen — visible and self-correcting. Someone typing `rmagic -h` out of near-universal CLI habit and getting a *classification* instead of help is a silent surprise.
3. **No output parity is lost.** Flag spelling does not change what gets printed for a given input, so the governing contract is untouched. Long-form names still match `file` exactly (`--dereference` / `--no-dereference`), and `-L` matches both `file` and the POSIX "logical" convention (`find -L`, `cp -L`) — but that convergence is a convenience, not an obligation.
4. **It removes the clap workaround entirely** — no `disable_help_flag`, no hand-rolled `--help` arg. That is a KISS win per AGENTS.md.

No alternate short letter is assigned. `-P` (the POSIX "physical" counterpart to `-L`) was considered and rejected: `file` already uses `-P` for `--parameter` (verified: `file -P foo=1 x` returns `Unknown param foo=1`), so borrowing it would create a *new* collision with magic(1) while solving nothing that the long form does not already solve.

## Problem Statement

### Current behavior (reproduced against `file-5.41` on macOS, 2026-07-26)

```console
$ rmagic -m /usr/share/file/magic /tmp/d/bad.link /tmp/d/good.link /tmp/d/selfloop
Error processing /tmp/d/bad.link: I/O error: No such file or directory (os error 2)        # stderr
Error processing /tmp/d/selfloop: I/O error: Too many levels of symbolic links (os error 62) # stderr
/tmp/d/good.link: ASCII text                                                                # stdout
$ echo $?
0

$ rmagic -m /usr/share/file/magic -s /tmp/d/bad.link ; echo "exit=$?"
Error processing /tmp/d/bad.link: I/O error: No such file or directory (os error 2)
Error: File not found
exit=3
```

GNU `file` on the identical inputs:

```console
$ file /tmp/d/bad.link /tmp/d/good.link /tmp/d/selfloop
bad.link:  broken symbolic link to missing.txt
good.link: ASCII text
selfloop:  broken symbolic link to selfloop
$ echo $?
0
```

### Where the failure originates

Not in `src/io/`, despite the issue body's scope line, which reads verbatim: *"CLI/IO layer only (`src/main.rs`, `src/io/`) — does not touch magic evaluation."* The chain aborts at `src/lib.rs:411`:

```rust
let file_metadata = fs::metadata(path)?;   // follows symlinks -> ENOENT on a broken link
```

`std::fs::metadata` follows symlinks, so a dangling link fails here **before** `FileBuffer` is ever constructed. The evidence is in the message text: the observed error is the bare `I/O error: No such file or directory (os error 2)`, not `io/mod.rs`'s `Failed to read metadata for file '{path}': {source}` format. A fix confined to `src/io/` never executes.

`src/main.rs:563` then routes the `Err` through `eprintln!("Error processing {}: {}", ...)`. Any fix that only improves the *error message* (e.g. adding an `IoError::BrokenSymlink` variant) still lands on stderr with no stdout line, and the reported bug survives.

### Verified GNU `file` semantics (all measured, not inferred)

The `-h` column below is **`file`'s** `-h` (its no-dereference flag). rmagic spells that column `--no-dereference`; see the flag-naming decision above.

| Input                              | default                                                  | `-L`         | `-h` (rmagic: `--no-dereference`)      |
| ---------------------------------- | -------------------------------------------------------- | ------------ | -------------------------------------- |
| regular file                       | `ASCII text`                                             | `ASCII text` | `ASCII text`                           |
| valid symlink -> file              | `ASCII text`                                             | `ASCII text` | `symbolic link to reg.txt`             |
| valid symlink -> dir               | `directory`                                              | `directory`  | `symbolic link to realdir`             |
| chained symlink                    | `ASCII text` (final target)                              | same         | `symbolic link to good.link` (one hop) |
| broken symlink                     | `broken symbolic link to missing.txt`                    | identical    | identical                              |
| symlink cycle (ELOOP)              | `broken symbolic link to selfloop`                       | identical    | identical                              |
| EACCES (target in `chmod 000` dir) | `broken symbolic link to noperm/target.txt`              | identical    | identical                              |
| empty target (`ln -s "" x`)        | `` unreadable symlink `x' (No such file or directory) `` | identical    | identical                              |
| absolute target                    | (target's type)                                          | same         | `symbolic link to /etc/hosts`          |

**Trap: `file --help` misdocuments its own default.** file-5.41's help text prints `-h, --no-dereference   don't follow symlinks (default)`. The "(default)" is **wrong** — measured, plain `file good.link` classifies the *target*, and setting `POSIXLY_CORRECT=1` does not change that. Default-follow is the real behavior and is what this spec matches. Anyone re-deriving this table from `file --help` instead of from measurement will reach the opposite conclusion.

Four properties fall out of this table and drive the design:

1. **Dereference flags are irrelevant to the broken case.** All three columns are identical. The reported bug is closable without either flag; the flags exist for parity, not for this bug.
2. **The target renders verbatim, as stored in the link.** `libz.1.2.12.dylib -> libz.1.dylib` prints `libz.1.dylib`, not an absolute or parent-joined path. Use `fs::read_link` output directly; do not canonicalize or join with the parent.
3. **ELOOP, ENOENT, and EACCES all collapse to the same output.** `selfloop` is a cycle, not a missing target, and still prints `broken symbolic link to selfloop`; a symlink whose target sits inside a `chmod 000` directory *also* prints the ordinary `broken symbolic link to <target>` (measured — see the EACCES row). The predicate is therefore "`fs::metadata` failed on a path that `symlink_metadata` says is a symlink", not "target does not exist". This single predicate gives `file`-identical output for all three failure modes, which is why no permission-specific branch is needed.
4. **The dereference flags are last-flag-wins, not mutually exclusive.** Measured:

```console
$ file -h -L good.link   ->  good.link: ASCII text                 # -L won
$ file -L -h good.link   ->  good.link: symbolic link to reg.txt   # -h won
```

Both invocations exit 0. Using clap's `conflicts_with` would reject a command line GNU `file` accepts.

## Technical Approach

### Guiding constraint: a symlink report is a classification on stdout — and a broken link is still an I/O failure

`broken symbolic link to X` and `symbolic link to X` must reach **stdout** via the existing `output_result(...)` (`src/main.rs:435`), not a raw `writeln!`. Routing through `output_result` inherits the `path: description` text form and the `--json` form from one code path.

**A broken symlink DOES trip `--strict`.** A dangling link is an unreadable path — exactly the "I/O ... error" category `--strict` documents itself as catching (`src/main.rs:70-73`). The `data`-result carve-out in that same docstring is not an analogue: `data` means "content I could not identify", not "path I could not read". The rule is categorical, not heuristic: `--strict` asks "did every input resolve to readable bytes?", and for a dangling link the answer is no.

**Do not justify this by calling broken symlinks "corruption."** They frequently are not. This spec's own worked corpus — the six broken `/usr/lib/*.dylib` links — exists on a stock, uncorrupted macOS install; those are ordinary OS state, not damage. A consequence follows and must be documented rather than hidden: **`rmagic --strict` over a real filesystem tree will exit non-zero on a healthy machine** wherever such links exist. That is the accepted cost of treating unreadable paths uniformly.

**`--no-dereference` is not an escape hatch here.** Brokenness is flag-independent: under `--no-dereference` a dangling link still reports `broken symbolic link to X` and still trips `--strict`. The flag only exempts a *valid* symlink (line below). The only way to scan a tree containing expected dangling links without a non-zero exit is to not pass `--strict` for that scan. If real usage shows CI teams need "strict about content, tolerant of expected dangling links", that is a follow-up flag (`--strict-symlinks`, or a `--strict` level), not a silent softening of this rule.

This is **not** a GNU parity break. `file` has no `--strict` flag at all — verified: `file --strict /etc/hosts` returns `file: unrecognized option '--strict'`. `--strict` is an rmagic-specific extension, so its semantics are rmagic's to define, and there is no upstream behavior to diverge from. Default (non-`--strict`) exit stays **0**, which is where parity actually applies.

A valid symlink under `--no-dereference` is **not** an I/O failure — the target is readable, rmagic simply chose not to read it. Only the `broken` case counts toward `--strict`.

**Mechanism.** The stdout line and the strict-failure signal must both happen, and the existing `Err` path can't express that: `run_analysis` (`src/main.rs:559-569`) `eprintln!`s every `Err` before recording it, and a broken symlink must not produce stderr noise in the default case. Introduce a small outcome type so `process_file` can report both:

```rust
/// What `process_file` produced for one input path.
enum FileOutcome {
    /// Classified normally; nothing for `--strict` to flag.
    Classified,
    /// Classified and written to stdout, but the path was unreadable.
    /// `--strict` must surface this; the default run must not print to stderr.
    ClassifiedUnreadable(LibmagicError),
}
```

`process_file` returns `Result<FileOutcome, LibmagicError>`. `run_analysis` dispatches three ways:

| Return                        | stderr                  | `first_error`   | exit under `--strict` |
| ----------------------------- | ----------------------- | --------------- | --------------------- |
| `Ok(Classified)`              | silent                  | untouched       | 0                     |
| `Ok(ClassifiedUnreadable(e))` | **silent**              | record if unset | non-zero              |
| `Err(e)`                      | `eprintln!` (unchanged) | record if unset | non-zero              |

The existing `Err` arm is untouched, so every current error path keeps its behavior. The exit code for a strict broken-symlink run comes from the existing `LibmagicError` → exit-code mapping in `main()` — no new mapping.

Note the shape difference from the neighboring directory early-return at `src/main.rs:526`, which returns `Err` and therefore still prints to stderr.

### Placement: a CLI precheck in `process_file`, library untouched

Add a symlink precheck to `process_file` **before** the `file_path.is_dir()` check and before `db.evaluate_file(...)`. This keeps `src/lib.rs` and `src/io/` unchanged and makes the issue's "CLI/IO layer only" scope claim true.

Ordering matters: `Path::is_dir()` follows symlinks, so `dir.link` (a symlink to a directory) reports `is_dir() == true`. Under `--no-dereference` that path must produce `symbolic link to realdir`, so the symlink branch has to run first.

**Reachability verified** (2026-07-26): `clap_stdin::FileOrStdin` does *not* stat or reject directory paths at argument-parse time. `rmagic <dir>` and `rmagic <symlink-to-dir>` both reach `process_file` and fail on its own `is_dir()` check — the observed message is `Path is a directory, not a file: ...`, which is `src/main.rs:529`, not a clap error. A precheck placed ahead of that check therefore runs, and test case #9 is achievable without moving the logic ahead of `FileOrStdin` conversion. (That `Path is a directory, not a file` message is the *current* behavior being measured for reachability — Phase 1b replaces it with a `directory` classification. The reachability conclusion is unaffected: the branch is reached either way.)

Decision logic, expressed as the precheck:

```text
let link_meta = fs::symlink_metadata(path);         // lstat -- does NOT follow
if link_meta says "not a symlink" or the call fails:
    fall through to existing behavior (unchanged)

// path IS a symlink
let target = fs::read_link(path);                   // verbatim stored text
if read_link fails:
    fall through to existing behavior (unchanged)

if target is empty:                                 // `ln -s "" x` -- read_link SUCCEEDS
    report "unreadable symlink `{path}' (No such file or directory)"
                                                    // structurally different; see below

let reachable = fs::metadata(path).is_ok();         // false on ENOENT, ELOOP, or EACCES
let prefix = if reachable { "symbolic link to" }
             else         { "broken symbolic link to" };

if !reachable:                                      // any flag state
    report "{prefix} {target}"                      // -> broken symbolic link to X
else if follows_symlinks == false:                  // --no-dereference, target is fine
    report "{prefix} {target}"                      // -> symbolic link to X
else:
    fall through -- classify the target as today    // default or -L
```

Both flag states share one reachability probe, so a *broken* symlink keeps the `broken` prefix under `--no-dereference` as well as under the default (verified above). Only the `reachable` case branches on the flag. An earlier draft of this pseudocode had the no-dereference branch emit `symbolic link to` unconditionally, which would have silently dropped the `broken` prefix and failed test case 8 and Success Criterion 6.

**The empty-target branch is not hypothetical.** `ln -s "" x` is creatable on macOS and Linux, and `fs::read_link` **succeeds** on it, returning an empty path — so it does *not* land in the `read_link fails` fall-through. Without the explicit branch it would reach the reachability probe, fail it, and render `broken symbolic link to ` with a dangling trailing space. Measured, GNU `file` emits a structurally different message:

```console
$ ln -s "" emptylink && file emptylink
emptylink: unreadable symlink `emptylink' (No such file or directory)
```

**Under ADR-0001 this string is a diagnostic, not a detection result** — it explains why the link could not be resolved and carries errno text — so rmagic is *not* obliged to match it byte-for-byte. Matching it anyway is the recommended default (it costs nothing and helps anyone diffing against `file`), but a clearer rmagic-native wording is equally conformant. Note `file`'s quoting style is `` `name' `` (backtick-open, single-quote-close) and the name is the **link path as given**, not the target.

What is *not* optional is the classification: emitting `broken symbolic link to ` with an empty target would be a wrong detection, and that is binding. This case shares the `ClassifiedUnreadable` outcome with ordinary broken links — it is an unreadable path, so it trips `--strict` on the same rule.

### Flag plumbing

Add to the clap `Args` struct (`src/main.rs:53`), using mutual `overrides_with` for last-flag-wins (clap 4.6.1 supports it; `conflicts_with` would be wrong per property 4 above):

```rust
/// Follow symlinks (default)
#[arg(short = 'L', long, overrides_with = "no_dereference")]
pub dereference: bool,

/// Do not follow symlinks; report the link itself
#[arg(long, overrides_with = "dereference")]
pub no_dereference: bool,
```

Note the absence of a `short` on `no_dereference` — see the flag-naming decision. **No `disable_help_flag`, and no hand-rolled `--help` arg:** clap's default `-h, --help` is left entirely alone, which is the whole point of not taking `-h`.

Expose the resolved policy as one accessor rather than reading two bools at the call site:

```rust
impl Args {
    /// `false` only when `--no-dereference` was the last-specified symlink flag.
    #[must_use]
    pub fn follows_symlinks(&self) -> bool { !self.no_dereference }
}
```

With mutual `overrides_with`, clap clears the loser, so `no_dereference` is true only when `--no-dereference` came last. `-L` is then correctly a no-op accepted for compatibility.

**`overrides_with` was prototyped against clap 4.6.1 and behaves as specified** (scratch crate, 2026-07-26). The prototype used `-h` for no-dereference; the last-flag-wins result is a property of `overrides_with` and is unchanged by renaming the flag to long-only:

```text
--no-dereference -L f.txt   ->  L=true  n=false  follows=true    # -L won
-L --no-dereference f.txt   ->  L=false n=true   follows=false   # --no-dereference won
f.txt                       ->  L=false n=false  follows=true    # default
-h                          ->  prints usage (clap default, untouched)
```

Mutual `overrides_with` reproduces GNU `file`'s last-flag-wins exactly.

**`--no-dereference` is the safe mode for untrusted trees, and the docs must say so.** Under it rmagic never opens or classifies a symlink's target, so it cannot be induced to read an attacker-chosen file by a planted link in an extracted archive or an uploaded directory. That makes it the control a caller should reach for when scanning untrusted input — the same "Mitigation for callers" framing `docs/src/security-assurance.md` §7.2 already uses for `evaluate_buffer`. Phase 2 must record this in the three CLI docs and in §7.2, not leave the flag framed as compatibility-only.

### Result shape: both `description` and `matches` must be populated

The two `output_result` arms read **different fields**, so populating only one of them produces a half-broken result:

- The text arm (`src/main.rs:469`) prints `result.description` and never touches `matches`.
- The JSON arm builds from `result.matches` via `from_library_result` and never touches `description`.

A symlink report has no rule matches, and `description` is normally derived from matches by `concatenate_messages` in `src/lib.rs`. So the synthetic `EvaluationResult` must set **both** explicitly:

- `description` = the symlink string directly (otherwise text output prints `path: ` with nothing after it),
- one synthetic match whose `text` carries the same string (otherwise `--json` emits an empty `matches` array).

**Build it with the `::new()` constructors — a struct literal will not compile.** `EvaluationResult`, `EvaluationMetadata` (both `src/lib.rs`), and `evaluator::RuleMatch` are all `#[non_exhaustive]`. From `src/main.rs` — a different module, and for `RuleMatch` a different crate boundary in the doc-example case — struct-literal construction fails with **E0639** (`cannot create non-exhaustive struct using struct expression`). Verified empirically against a scratch crate depending on `libmagic-rs` by path. Use the public constructors, whose signatures are:

```rust
EvaluationResult::new(description: String, mime_type: Option<String>,
                      confidence: f64, matches: Vec<evaluator::RuleMatch>,
                      metadata: EvaluationMetadata) -> Self
EvaluationMetadata::new(/* see src/lib.rs:805 */) -> Self
evaluator::RuleMatch::new(message: String, offset: usize, level: u32,
                          value: parser::ast::Value,
                          type_kind: parser::ast::TypeKind,
                          confidence: f64) -> Self
```

Without this note an implementer following the prose above ("must set **both** explicitly") would naturally reach for a struct literal and stall Phase 1's Green step on a compile error the spec never predicted.

Confirm during implementation what `from_library_result` does with a synthetic match: the captured JSON for an unknown file shows every match carrying `score: 30` and a `tags: [""]` array, so verify a synthetic entry is neither filtered out nor tag-enriched into something misleading. Pick whatever `score` and `tags` values survive that pipeline cleanly rather than assuming `score: 0` passes through.

## Implementation Plan

TDD throughout: write the failing test, watch it fail, add production code, watch it pass.

The phases are **sequential, not independent**: Phase 2 step 8 extends the precheck function Phase 1 authors, and Phase 3 measures the combined result. Each phase is a focused PR that ships standalone value in order — Phase 1 alone closes the reported bug — but they cannot be merged out of order or in parallel.

### Phase 1 — Broken-symlink reporting (closes the reported bug)

1. **Red:** `tests/cli_integration.rs` — broken symlink prints `broken symbolic link to <target>` on stdout, exits 0, stderr empty.
2. **Green:** add the symlink precheck to `process_file` covering the default/`-L` path only (no flags yet). Construct the result and route through `output_result`.
3. **Red/Green:** ELOOP case (self-referential symlink) produces the same output. 3b. **Red/Green:** EACCES case (target inside a `chmod 000` directory) produces the same `broken symbolic link to <target>` output — verified parity, not an accepted gap. 3c. **Red/Green:** empty-target symlink (`ln -s "" x`) is not mis-classified as `broken symbolic link to ` with an empty target. Assert on that negative (binding); assert the exact diagnostic wording only if the implementation chooses to match `file`'s `` unreadable symlink `x' (No such file or directory) ``.
4. **Red/Green:** add `FileOutcome` and the three-way `run_analysis` dispatch. `--strict` on a broken symlink exits non-zero with stderr silent; without `--strict` the same input exits 0.
5. **Red/Green:** `--json` on a broken symlink emits a coherent `matches[0].text`.
6. **Regression guard:** valid symlink still classifies its target; multi-file invocation keeps the `path: ` prefix.

### Phase 1b — `directory` classification (ADR-0001 contract gap)

Measured: `file realdir` prints `realdir: directory` and exits **0**. rmagic instead returns `Err(Path is a directory, not a file)` from the `is_dir()` early-return at `src/main.rs:526`, printing to stderr with no stdout line. Under ADR-0001 `directory` is a detection result, so this is a contract gap and is closed here.

6a. **Red:** `rmagic <dir>` prints `<dir>: directory` on stdout, exits 0, stderr empty. 6b. **Green:** replace the `is_dir()` early-return's `Err` with a `directory` classification routed through `output_result`, using the same synthetic-result construction as the symlink path (`::new()` constructors — see Result shape). 6c. **Red/Green:** `<dir>` under `--strict` exits **0**. A directory is now a successful detection, not an I/O failure, so it returns `FileOutcome::Classified` — not `ClassifiedUnreadable`. 6d. **Red/Green:** default and `-L` on a symlink-to-directory print `directory` (the *target's* type), matching `file`. This falls out of 6b: the symlink precheck falls through for a reachable link under default/`-L`, landing on the now-classifying `is_dir()` branch. 6e. **Regression guard:** multi-file invocation mixing a directory and a regular file prints both stdout lines with the `path: ` prefix intact.

Note this interacts with the precheck ordering already specified: `--no-dereference` on a symlink-to-directory must still print `symbolic link to realdir` (the precheck runs first), while default/`-L` now prints `directory` instead of erroring. Both are `file`-exact.

### Phase 2 — `-L` / `--no-dereference` flags

07. **Red:** `rmagic --no-dereference good.link` prints `symbolic link to target.txt`.
08. **Green:** add both `Args` fields with mutual `overrides_with` and `follows_symlinks()`. Extend the precheck with the no-dereference branch. **No `disable_help_flag`** — clap's `-h, --help` is untouched.
09. **Red/Green:** `-L --no-dereference` yields `symbolic link to ...`; `--no-dereference -L` yields the target's classification (last-flag-wins).
10. **Red/Green:** `--no-dereference` on a symlink-to-directory prints `symbolic link to realdir` (proves the precheck runs before the `is_dir()` early-return).
11. **Red/Green:** `--no-dereference` on a broken symlink keeps the `broken` prefix.
12. **Red/Green:** `--no-dereference` on a regular file and on a real directory is unchanged from default.
13. **Guard:** `rmagic -h` and `rmagic --help` both still print usage — the pre-existing help binding is preserved, not repurposed.
14. Update all three CLI docs (`docs/CLI_REFERENCE.md`, `docs/src/cli-reference.md`, `docs/src/cli-usage.md`) with the two flags, the broken-symlink output, and the `--strict` interaction. The existing `` `-h, --help` `` rows stay correct and need no edit. No completion regeneration step — completions are generated at runtime from the clap definition and are not checked in.

### Phase 3 — Housekeeping

15. Extract to `src/cli/symlink.rs` when the symlink precheck function, the `FileOutcome` enum, the dereference branch logic, and `follows_symlinks()` together exceed **60 lines** in `src/main.rs`, counting non-blank non-comment lines and excluding tests. Measure once at the end of Phase 2 with `cargo fmt` already applied. `src/main.rs` is 1148 lines against the AGENTS.md 500-600 guideline, so the default expectation is that the extraction happens; the threshold exists to skip it only for a genuinely trivial diff.
16. Add a GOTCHAS entry under a new S17 (or the next free number) recording last-flag-wins, the verbatim-target rule, the ENOENT/ELOOP/EACCES collapse, the empty-target special case, the deliberate `-h` divergence from magic(1), the `file --help` "(default)" misdocumentation, and the `--strict` divergence from `file`.
17. Update `docs/src/security-assurance.md` §7.2 per the Files to Modify table.

## Test Plan

All symlink tests must create their own links in a `TempDir`; `/usr/lib/*.dylib` cases stay manual/macOS-only verification and must not be encoded as assertions.

`FileBuffer::create_symlink` is `pub(crate)` (`src/io/mod.rs:335`) and therefore **not** callable from `tests/cli_integration.rs`. The integration test needs its own helper.

**Use the runtime-skip pattern, not `#[cfg(unix)]`.** The existing precedent at `src/io/mod.rs:1089` (`test_file_buffer_symlink_to_directory_rejection`) is *not* `cfg`-gated: it compiles on every platform, calls the cross-platform `create_symlink`, and `match`es the result — running assertions in the `Ok` arm and printing a skip message in the `Err` arm when privileges are missing. Mirror that shape so the new CLI symlink tests actually execute on Windows CI when the runner has symlink privilege, rather than being compiled out unconditionally:

```rust
fn try_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    { std::os::unix::fs::symlink(target, link) }
    #[cfg(windows)]
    { std::os::windows::fs::symlink_file(target, link) }
}

// at each call site:
match try_symlink(&target, &link) {
    Ok(()) => { /* assertions */ }
    Err(e) => { eprintln!("skipping: symlink creation failed ({e})"); }
}
```

An earlier draft of this section said "mirror the existing Windows skip pattern" but then showed a `#[cfg(unix)]`-gated helper and closed with "gate the whole module on `#[cfg(unix)]`, **or** skip-with-message" — three mutually inconsistent instructions for one decision. The decision is made here: runtime skip. Note `symlink_dir` (not `symlink_file`) is required for the symlink-to-directory case on Windows.

| #   | Case                                                              | Expected                                                                                                             |
| --- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| 1   | broken symlink, default                                           | stdout `broken symbolic link to missing.txt`, exit 0, stderr empty                                                   |
| 2   | symlink cycle (ELOOP)                                             | same shape as #1                                                                                                     |
| 3   | valid symlink, default                                            | classifies the target (regression guard)                                                                             |
| 4   | two files, one broken                                             | both stdout lines present, `path: ` prefix intact                                                                    |
| 5   | broken symlink + `--strict`                                       | classification on stdout, exit **non-zero**, stderr still silent                                                     |
| 5b  | broken symlink, no `--strict`                                     | classification on stdout, exit **0** (parity with `file`)                                                            |
| 6   | broken symlink + `--json`                                         | `matches[0].text` carries the description                                                                            |
| 7   | valid symlink + `--no-dereference`                                | `symbolic link to target.txt`                                                                                        |
| 8   | broken symlink + `--no-dereference`                               | `broken symbolic link to missing.txt`                                                                                |
| 9   | symlink-to-directory + `--no-dereference`                         | `symbolic link to realdir`                                                                                           |
| 10  | valid symlink + `-L`                                              | classifies the target (no-op)                                                                                        |
| 11  | `--no-dereference -L` then `-L --no-dereference`                  | last-flag-wins, both exit 0                                                                                          |
| 12  | regular file + `--no-dereference`                                 | unchanged classification                                                                                             |
| 13  | absolute-target symlink + `--no-dereference`                      | target rendered verbatim, not canonicalized                                                                          |
| 14  | `-h` and `--help`                                                 | both print usage — the pre-existing help binding is preserved                                                        |
| 15  | real directory + `--no-dereference`                               | `directory` on stdout, exit 0 — same as default; proves the flag does not alter non-symlink paths                    |
| 15b | real directory, default                                           | `directory` on stdout, exit 0, stderr empty (ADR-0001 gap closed)                                                    |
| 15c | real directory + `--strict`                                       | exit **0** — a directory is a successful detection, not an I/O failure                                               |
| 15d | symlink-to-directory, default and `-L`                            | `directory` (the target's type), not `symbolic link to realdir`                                                      |
| 15e | directory + regular file in one invocation                        | both stdout lines, `path: ` prefix intact                                                                            |
| 16  | broken symlink with a multi-segment relative target (`../../x/y`) | target rendered verbatim, no canonicalization or parent-joining                                                      |
| 17  | valid symlink + `--no-dereference` + `--strict`                   | exit **0** — a readable target is not an I/O failure                                                                 |
| 18  | EACCES: symlink whose target sits in a `chmod 000` directory      | `broken symbolic link to <target>` — same as ENOENT/ELOOP, matching `file` (measured). Restore mode in test teardown |
| 19  | empty-target symlink (`ln -s "" x`)                               | **not** `broken symbolic link to ` with an empty target (binding). Diagnostic wording is rmagic's choice             |

Unit-testable pieces (the precheck's decision function, `Args::follows_symlinks()`) get table-driven `#[cfg(test)]` coverage in their module per the AGENTS.md test-style preference.

## Files to Modify

| File                             | Change                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/main.rs`                    | `Args`: `dereference` / `no_dereference` fields (no `short` on the latter) and `follows_symlinks()`. No `disable_help_flag`. New `FileOutcome` enum; `process_file` returns it; `run_analysis` three-way dispatch. Symlink precheck ahead of the `is_dir()` check. Replace the `is_dir()` early-return `Err` (line 526) with a `directory` classification through `output_result` (Phase 1b). Update every manual `Args { ... }` construction in unit tests (GOTCHAS S7.4). |
| `tests/cli_integration.rs`       | `#[cfg(unix)]` symlink helper + every case in the Test Plan table                                                                                                                                                                                                                                                                                                                                                                                                           |
| `docs/CLI_REFERENCE.md`          | document `-L` / `--no-dereference`, the broken-symlink output, and that a broken link trips `--strict`                                                                                                                                                                                                                                                                                                                                                                      |
| `docs/src/cli-reference.md`      | same (mdbook copy)                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `docs/src/cli-usage.md`          | same (mdbook usage page)                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `docs/src/security-assurance.md` | extend §7.2 (TOCTOU) with the new CLI-layer `lstat` -> `read_link` -> `stat` sequence; note the new stdout disclosure of raw `read_link` text; record `--no-dereference` as the caller mitigation for untrusted trees                                                                                                                                                                                                                                                       |
| `GOTCHAS.md`                     | new section per Phase 3 item 16                                                                                                                                                                                                                                                                                                                                                                                                                                             |

There are **three** CLI documentation files, not two — `docs/CLI_REFERENCE.md`, `docs/src/cli-reference.md`, and `docs/src/cli-usage.md` — and all three describe flags. Missing one leaves the mdbook and the top-level reference disagreeing about the CLI surface.

**The documentation task is purely additive.** `docs/CLI_REFERENCE.md:70` and `docs/src/cli-reference.md:65` each carry the row `` | `-h, --help` | Print help information | ``. Because `-h` keeps its help binding, those rows stay **correct** and must be left alone. (An earlier revision of this spec rebound `-h` to `--no-dereference`, which would have made both rows actively false and required editing them; that is no longer the case.)

`AGENTS.md` needs **no** change: it has no CLI-behavior section (verified — its only CLI-adjacent heading is `### Standard Commands`, which documents the dev-loop commands, not the `rmagic` flag surface). Shell completions need no regeneration either: they are **not** checked in (verified — no `completions/` directory and no completion files tracked by git); `--generate-completion` emits them from the clap definition at runtime, so the new flags appear automatically.

## Files to Create

| File                               | Purpose                                         |
| ---------------------------------- | ----------------------------------------------- |
| `src/cli/symlink.rs` (conditional) | only if Phase 3 item 15's size threshold is hit |

## Success Criteria

Strings below are copied from measured `file-5.41` output, not from the issue body.

01. `rmagic -m /usr/share/file/magic /usr/lib/libz.1.2.12.dylib` prints `/usr/lib/libz.1.2.12.dylib: broken symbolic link to libz.1.dylib` to **stdout** and exits **0**, with stderr empty. Like criterion 2, this is a **manual verification oracle, not a checked-in assertion** — it names a macOS- and version-specific path. Automated coverage of this shape is Test Plan case #1 (TempDir-created broken symlink).

02. All 6 broken `/usr/lib/*.dylib` symlinks produce a `broken symbolic link to <target>` line instead of no output. Enumerated on this machine 2026-07-26 (macOS, `file-5.41`) — this is the concrete oracle list, not a count taken from the issue body:

    | Link                   | Stored target                                                                      |
    | ---------------------- | ---------------------------------------------------------------------------------- |
    | `libipconfig.dylib`    | `../../System/Library/PrivateFrameworks/IPConfiguration.framework/IPConfiguration` |
    | `libnetquality.dylib`  | `../../System/Library/PrivateFrameworks/NetworkQuality.framework/NetworkQuality`   |
    | `libnetwork.dylib`     | `../../System/Library/Frameworks/Network.framework/Network`                        |
    | `libpcre2-8.dylib`     | `libpcre2-8.0.dylib`                                                               |
    | `libpcre2-posix.dylib` | `libpcre2-posix.3.dylib`                                                           |
    | `libz.1.2.12.dylib`    | `libz.1.dylib`                                                                     |

    This list is macOS- and version-specific, so it is a **manual verification oracle**, not a checked-in assertion. Regenerate with: `for f in /usr/lib/*.dylib; do [ -L "$f" ] && [ ! -e "$f" ] && echo "$f -> $(readlink "$f")"; done`

03. The target is rendered verbatim as stored in the link. Confirmed against `file` for the multi-level relative case: `/usr/lib/libipconfig.dylib` prints `broken symbolic link to ../../System/Library/PrivateFrameworks/IPConfiguration.framework/IPConfiguration` — the `../../` prefix survives, proving no canonicalization or parent-joining.

04. A symlink cycle produces `broken symbolic link to <target>`, not an ELOOP error.

05. `--strict` on a broken symlink prints the classification to stdout, writes nothing to stderr, and exits **non-zero**. Without `--strict` the same input exits **0**. 5b. `--strict` with `--no-dereference` on a *valid* symlink exits **0** — declining to follow a readable target is not an I/O failure.

06. `--no-dereference` on a valid symlink prints `symbolic link to <target>`; on a broken one, `broken symbolic link to <target>`.

07. `-L` on a valid symlink is indistinguishable from the default.

08. `--no-dereference -L` follows and `-L --no-dereference` does not (last-flag-wins), both exit 0.

09. `rmagic -h` and `rmagic --help` both print usage — the existing help binding is unchanged by this feature.

10. Valid symlinks and regular files are behaviorally unchanged. Real directories change **deliberately** (Phase 1b: `directory` on stdout instead of an error on stderr); everything else about non-symlink paths is untouched.

11. `--json` on a broken symlink emits a coherent object whose `matches[0].text` carries the full `broken symbolic link to <target>` string (mirrors Test Plan case #6).

12. A symlink-to-directory under `--no-dereference` prints `symbolic link to <target>`, while the same link under default/`-L` prints `directory` — proving the precheck runs ahead of the `is_dir()` check (mirrors Test Plan cases #9 and #15d).

13. `docs/src/security-assurance.md` §7.2 documents the new CLI-layer symlink precheck and names `--no-dereference` as the caller mitigation for untrusted trees.

14. `rmagic --no-dereference` on an EACCES symlink (target inside an unreadable directory) prints `broken symbolic link to <target>` — matching `file`, not a permission-specific message.

15. `rmagic` on an empty-target symlink does **not** print `broken symbolic link to ` with an empty target (binding — that would be a wrong detection). The diagnostic wording itself is rmagic's choice; matching `file`'s `` unreadable symlink `<path>' (No such file or directory) `` is the recommended default.

16. `rmagic <dir>` prints `<dir>: directory` on stdout and exits 0, matching `file` byte-for-byte — the sole ADR-0001 detection gap, closed in this pass rather than deferred.

17. `just ci-check` passes: `cargo fmt`, `cargo clippy -- -D warnings`, full test suite.

18. Tests are portable — the symlink helper compiles on all platforms and skips at runtime when privileges are missing (not `#[cfg(unix)]`-gated).

## Out of Scope

- **Magic rule evaluation.** No parser, evaluator, or AST changes.
- **The `metadata.is_symlink()` branch in `check_metadata`** (`src/io/mod.rs:259`) is unreachable: both callers supply metadata from `fstat` or from a following `fs::metadata`, neither of which can report a symlink. Leave it.

### ADR-0001 contract gaps — fixed here, not deferred

Applying the ADR's boundary test to the divergences this work surfaced leaves exactly **one** detection gap, and it is fixed in this pass (Phase 1b) rather than filed as a follow-up issue. The other three are not detection results and need no issue:

| Divergence                                                                                 | Class                                                                                                                                          | Disposition                  |
| ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| `directory` — `file` prints `directory`, rmagic errors `Path is a directory, not a file`   | **Detection** — `directory` describes what the path *is*                                                                                       | **Fixed in Phase 1b**        |
| Filename column padding in multi-file output (`file` pads to align, rmagic uses one space) | Formatting around the result                                                                                                                   | No change                    |
| Nonexistent non-symlink paths (`cannot open '<p>' (No such file or directory)`)            | Diagnostic                                                                                                                                     | rmagic keeps its own wording |
| Unsanitized `read_link` text reaching stdout                                               | Detection — but passing bytes through is precisely what *matches* `file`; sanitizing would break parity. A security question, not a parity gap | No change                    |

### Genuinely out of scope

- **`src/main.rs` size.** At 1148 lines it is already over the AGENTS.md 500-600 guideline. Phase 3 item 15 keeps this change from making it worse but does not undertake the broader split.
- **`--mime` / `!:mime` interaction with symlinks.** MIME output is gated on issue #51.
- **Library API parity.** `MagicDatabase::evaluate_file` continues to return an I/O error for a broken symlink; only the CLI gains classification behavior. The fix lives in `process_file` precisely to leave `src/lib.rs` and `src/io/` untouched. This is not an ADR-0001 gap — the ADR's contract is on rmagic's *detection results*, and the library returning a typed error to a Rust caller is API surface, governed instead by the v1.0.0 "Stable API" milestone. Still worth revisiting there, because a library consumer and the CLI currently disagree about what a broken symlink is. File as its own issue rather than widening this one.
- **Non-UTF-8 symlink targets.** `fs::read_link` returns a `PathBuf` whose `OsString` is not guaranteed to be UTF-8. Rendering uses `Path::display()`, which is lossy for invalid sequences — matching how every other path in the CLI is already printed (`src/main.rs:469` uses `file_path.display()`). Whether `file` renders such a target byte-identically was **not measured**; if a differential later shows it does not, that becomes an ADR-0001 tracked gap.
- **Wrapper scripts cannot force `--no-dereference`.** Last-flag-wins means a caller-supplied `-L` overrides a wrapper's earlier `--no-dereference`. This matches GNU `file`'s flag behavior, and a "sticky" safe mode would be a new ergonomic design decision (ADR-0001 leaves that free) that no current consumer needs.
