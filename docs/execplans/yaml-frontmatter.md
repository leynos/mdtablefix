# Preserve leading YAML frontmatter while formatting Markdown

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DELIVERED

## Purpose / big picture

After this change, `mdtablefix` must accept a Markdown document that begins
with a YAML frontmatter block and leave that block byte-for-byte unchanged
while continuing to format the Markdown body normally. A user should be able
to run the formatter with flags such as `--wrap`, `--breaks`, or `--in-place`
and still see the opening delimiter, YAML keys, and closing delimiter exactly
as they were written.

The observable success case is a file that starts with:

```plaintext
---
name: weaver
description: Preserve this YAML metadata exactly.
---
```

and ends with the same frontmatter block unchanged after formatting, while the
Markdown that follows is still reflowed, renumbered, or otherwise rewritten
according to the selected options.

## Constraints

- Only a leading document block counts as YAML frontmatter. A `---` line later
  in the document must keep its existing Markdown meaning.
- The detected frontmatter block must be copied verbatim, including both
  delimiter lines and all interior lines.
- No new external dependencies may be introduced.
- Public CLI flags and existing library entry points in `src/lib.rs` must stay
  stable.
- The implementation must keep files below the repository's 400-line limit.
- The change must include both focused unit tests and behavioural CLI tests.
- The user guide must be updated. In this repository that means at minimum
  `README.md`, and `docs/architecture.md` should also be updated if the
  processing pipeline changes materially.

## Tolerances (exception triggers)

- Scope: if the work requires changes to more than 8 files or roughly 250 net
  lines of code, stop and re-evaluate the design.
- Interfaces: if preserving frontmatter requires changing a public function
  signature or adding a new CLI flag, stop and escalate.
- Dependencies: if a new crate seems necessary, stop and escalate.
- Ambiguity: if the required delimiter rules are not satisfied by the standard
  leading YAML forms (`---` opener with `---` or `...` closer), stop and ask
  for clarification before coding.
- Iterations: if the new or updated tests still fail after 3 focused fix
  cycles, stop and document the blocker.
- Time: if one milestone takes more than 2 hours, stop and record why.

## Risks

- Risk: frontmatter might still be modified by CLI-only transforms such as
  `renumber_lists` or `format_breaks` after the main stream processor returns.
  Severity: high
  Likelihood: medium
  Mitigation: protect the body split at the highest shared pipeline boundary
  and add a CLI regression that includes `--breaks`.

- Risk: delimiter detection can become too permissive and accidentally treat a
  thematic break or ordinary `---` block as frontmatter.
  Severity: medium
  Likelihood: medium
  Mitigation: only detect frontmatter when the very first line is a delimiter
  and require a matching closing delimiter before shielding the block.

- Risk: `src/process.rs` is already close to the repository's file-length
  ceiling.
  Severity: medium
  Likelihood: high
  Mitigation: place the detector and splitter logic in a new small module
  instead of extending `src/process.rs` significantly.

## Progress

- [x] (2026-04-05 22:45Z) Reviewed the current processing pipeline, test
  layout, and user-facing documentation surfaces.
- [x] (2026-04-09) Add a shared helper for detecting and splitting leading YAML
  frontmatter.
- [x] (2026-04-09) Thread the helper through the library and CLI formatting pipeline
  so all transforms skip the frontmatter prefix.
- [x] (2026-04-09) Add unit and behavioural regression tests covering detection,
  wrapping, and `--breaks`.
- [x] (2026-04-09) Update `README.md` and `docs/architecture.md`.
- [x] (2026-04-09) Run `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
  and `make nixie` if Mermaid content changes.

## Surprises & discoveries

- Observation: `src/main.rs` applies `renumber_lists` and `format_breaks`
  after `process_stream_opts`, so shielding only `process_stream_inner` would
  still allow the frontmatter delimiters to be rewritten.
  Evidence: `process_lines` in `src/main.rs`.
  Impact: the plan must protect the body before or around CLI-only transforms,
  not just inside `src/process.rs`.

- Observation: `src/process.rs` is 343 lines before this feature.
  Evidence: `leta files` output for `src/process.rs`.
  Impact: new helper logic should live in its own module to stay within the
  repository limit and keep tests readable.

## Decision log

- Decision: use a shared internal splitter for leading YAML frontmatter rather
  than adding special cases separately in each transform.
  Rationale: one detector keeps the delimiter rules consistent and reduces the
  chance that a later pipeline stage mutates the protected prefix.
  Date/Author: 2026-04-05 22:45Z / Droid

- Decision: treat unmatched opening delimiters as ordinary Markdown instead of
  partially shielding the document.
  Rationale: this avoids swallowing the entire file into a special mode and
  preserves current behaviour for malformed input.
  Date/Author: 2026-04-05 22:45Z / Droid

## Outcomes & retrospective

The frontmatter splitter was successfully implemented in the `frontmatter`
module and integrated through both the `process` module and `main` module.
Test coverage was added covering detection, wrapping, and `--breaks` flags
for both library and CLI paths. All transforms now correctly skip the
frontmatter prefix, preserving the leading YAML block exactly while
formatting the Markdown body.

## Context and orientation

The main formatting pipeline lives in `src/process.rs`. It handles table
reflow, fence tracking, HTML table conversion, heading conversion, wrapping,
ellipsis replacement, and footnote conversion through
`process_stream_inner(lines, opts)`.

The CLI entry point lives in `src/main.rs`. Its `process_lines` function first
calls `process_stream_opts`, then applies ordered-list renumbering and
thematic-break formatting. This is important because the YAML delimiter `---`
looks like a thematic break, so protecting the frontmatter only in
`src/process.rs` is insufficient.

In-place file rewriting is handled by `src/io.rs`, which delegates to the
library functions and therefore benefits automatically once the shared library
pipeline preserves frontmatter correctly.

Focused library tests already live beside the implementation in
`src/process.rs` and `src/io.rs`. Behavioural CLI tests live in `tests/cli.rs`
and `tests/wrap/cli.rs`. The user-facing guide is `README.md`. The processing
pipeline is described in `docs/architecture.md`.

## Plan of work

Stage A is a small, isolated detector module. Add `src/frontmatter.rs` with a
module-level comment and a private helper that splits the input into an
unchanged frontmatter prefix and a Markdown body slice. The helper should only
match when `lines.first()` is exactly the YAML opener and a closing delimiter
is found before the body begins. If no valid closer exists, return an empty
prefix and the original input as the body.

Stage B wires the helper through the library pipeline. Update `src/lib.rs` to
declare the new module. In `src/process.rs`, split the input first, run the
existing processing logic only on the body slice, and then prepend the
unchanged prefix to the processed body. Keep the current ordering of fences,
HTML tables, wrapping, headings, ellipsis, and footnotes for the body.

Stage C wires the same protection through the CLI-only transforms. In
`src/main.rs`, split the original input once in `process_lines`, pass only the
body slice through `process_stream_opts`, `renumber_lists`, and `format_breaks`
as needed, then prepend the original prefix before returning the final lines.
This ensures `--breaks` cannot rewrite the `---` delimiters in the frontmatter
block.

Stage D adds regression coverage. Put detector-specific unit tests in
`src/frontmatter.rs` and a pipeline regression in `src/process.rs` or a small
new test module. Add at least one behavioural CLI test in `tests/cli.rs`
covering a document with leading frontmatter plus a paragraph or table body.
The CLI test should enable `--breaks` and one ordinary formatting option such
as `--wrap` so it proves both preservation and continued formatting.

Stage E updates the docs. Add a short YAML frontmatter note and example to
`README.md` so users know the block is preserved. Update
`docs/architecture.md` to describe the leading-frontmatter split before the
rest of the formatting pipeline.

Each stage ends with focused validation before moving on.

## Concrete steps

Work from the repository root:

```bash
pwd
```

Expected:

```plaintext
/home/leynos/Projects/mdtablefix.worktrees/yaml-frontmatter
```

Add the detector and its focused tests, then run the smallest relevant test
set first:

```bash
cargo test frontmatter --lib
```

Expected:

```plaintext
running <N> tests
test ...frontmatter... ok
```

After wiring the library and CLI paths, run focused regressions:

```bash
cargo test process::tests:: --lib
cargo test --test cli yaml_frontmatter
```

Manually verify the user-visible behaviour with a CLI example:

```bash
printf '%s\n' \
  '---' \
  'name: weaver' \
  'description: short example' \
  '---' \
  '' \
  '|A|B|' \
  '|1|2|' | cargo run -- --wrap --breaks
```

Expected:

```plaintext
---
name: weaver
description: short example
---

| A | B |
| 1 | 2 |
```

Finish with repository validators, run sequentially:

```bash
make check-fmt
make lint
make test
make markdownlint
make nixie
```

If `docs/architecture.md` does not change any Mermaid content, `make nixie`
may be skipped.

## Validation and acceptance

Acceptance means all of the following are true:

- A document that starts with a valid YAML frontmatter block keeps that block
  exactly unchanged after formatting.
- The Markdown body that follows is still formatted normally, including table
  reflow and optional wrapping.
- `--breaks` does not rewrite the frontmatter delimiters.
- The new detector rejects malformed or non-leading cases without changing
  existing behaviour elsewhere.
- The README explains the feature clearly enough for a user to discover it.

Quality criteria:

- Tests: the new unit tests and CLI regression tests pass, and `make test`
  passes for the full suite.
- Lint: `make lint` passes with no warnings.
- Formatting: `make check-fmt` passes.
- Docs: `make markdownlint` passes, and `make nixie` passes if Mermaid content
  changed.

## Idempotence and recovery

The planned edits are safe to repeat because the detector only changes control
flow, not persisted state outside the repository. If a step goes wrong, revert
the affected file and rerun the focused tests before continuing. The manual
CLI example is read-only and may be rerun as many times as needed.

## Artifacts and notes

Key repository evidence gathered before implementation:

```plaintext
src/process.rs  -> core stream processor, already 343 lines
src/main.rs     -> CLI-only post-processing for renumbering and thematic breaks
tests/cli.rs    -> behavioural CLI coverage
README.md       -> current user guide
docs/architecture.md -> pipeline description
```

The most failure-prone path is `--breaks`, because it can legally rewrite a
plain `---` line outside frontmatter. The tests must therefore include that
flag.

## Interfaces and dependencies

Do not add dependencies.

Add a new internal module at `src/frontmatter.rs` with a helper shaped like:

```rust
#[doc(hidden)]
pub mod frontmatter;
#[doc(hidden)]
pub use frontmatter::split_leading_yaml_frontmatter;
```

The helper `split_leading_yaml_frontmatter` returns `(prefix, body)`, where
`prefix` is the untouched leading YAML block, or an empty slice if no valid
block exists. The module and helper are marked `#[doc(hidden)]` to keep them
out of the public API documentation while remaining accessible to the binary
crate.

`src/process.rs` calls the helper in `process_stream`, `process_stream_no_wrap`,
and `process_stream_opts` before existing body processing. `src/main.rs` calls
the same helper in `process_lines` before CLI-only transforms (`--renumber`,
`--breaks`).

Interface note: The `frontmatter` module is exported as `pub` with
`#[doc(hidden)]` rather than `pub(crate)` because the binary crate (`main.rs`)
requires access to `split_leading_yaml_frontmatter`. The binary and library are
separate crate targets, so `pub(crate)` would not allow the binary to access
the symbol. Using `#[doc(hidden)]` prevents the API from appearing in docs
while maintaining the necessary visibility.

Revision note: Delivered. The implementation follows the plan with the
visibility adjustment noted above. All tests pass and the feature is ready
for use.
