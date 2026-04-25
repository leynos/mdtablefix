# Replace bespoke wrapping with `textwrap` and `unicode-width`

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
 `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETED

## Purpose / big picture

After this change, `mdtablefix --wrap` must keep the same observable wrapping
behaviour that users already rely on for paragraphs, list items, blockquotes,
footnotes, hard line breaks, and fenced or indented code blocks, but the
line-breaking engine should be driven by the `textwrap` crate instead of the
current bespoke span buffer in `src/wrap/inline.rs` and the duplicated prefix
logic in `src/wrap/paragraph.rs`.

Observable success means three things. First, the existing active wrap-related
tests still pass unchanged. Second, the implementation no longer depends on the
custom `LineBuffer`-driven wrapping loop or the duplicated
`append_wrapped_with_prefix` and `handle_prefix_line` split. Third, the
documentation explains that wrapping now delegates width-sensitive line
breaking to `textwrap` together with `unicode-width`.

This issue is a maintainability refactor, not a user-facing bug fix. The plan
therefore prioritizes behaviour preservation over aggressive deletion. If a
proposed simplification would change a public API or visible wrapping result,
the plan prefers a smaller, safer first delivery and a follow-up issue.

## Constraints

- Keep public CLI flags and the public `mdtablefix::wrap::wrap_text` entry
  point stable.
- Treat `mdtablefix::wrap::Token` and
  `mdtablefix::wrap::tokenize_markdown` as stable unless the user explicitly
  approves a public API change. They are re-exported from `src/lib.rs` and are
  used by `src/code_emphasis.rs`, `src/footnotes/mod.rs`,
  `src/footnotes/renumber.rs`, and `src/textproc.rs`.
- Preserve the current observable behaviour covered by active tests in
  `src/wrap/tests.rs`, `tests/wrap_unit.rs`, `tests/wrap_cli.rs`,
  `tests/wrap_renumber.rs`, `tests/cli.rs`, `tests/cli_frontmatter.rs`,
  `tests/code_emphasis.rs`, and `tests/markdownlint.rs`.
- Do not rely on `tests/wrap/*.rs` for safety. Those files are currently
  orphaned from Cargo test discovery and do not run unless they are promoted
  into an active top-level target.
- Keep Unicode display-width handling based on `unicode-width`. Any helper that
  computes indentation or available columns must use display width rather than
  byte length.
- Add the `textwrap` dependency to `Cargo.toml` using implicit semver (no
  leading `^`) that is compatible with the repository's Rust `1.89` minimum
  version and existing 2024 edition build.
- Keep touched source files below the repository's 400-line soft ceiling. If
  the refactor would make a file exceed that limit, split the code into a small
  new module instead of extending the file.
- Update documentation that currently describes the bespoke wrapping engine:
  at minimum `README.md`, `docs/architecture.md`, and `docs/trailing-spaces.md`
  if the named helper or explanation changes.

## Tolerances (exception triggers)

- Scope: if the implementation requires changes to more than 12 files or
  roughly 500 net lines of code after cleanup, stop and re-evaluate whether the
  work has grown beyond a maintainability refactor.
- Interfaces: if removing bespoke wrapping would require changing the public
  `Token` type, `tokenize_markdown`, or the signature of `wrap_text`, stop and
  escalate rather than smuggling in an API break.
- Behaviour: if a naive `textwrap` prototype fails more than three existing
  active wrap regressions that cannot be fixed with a small shielding layer,
  stop and document the cases before proceeding.
- Dependencies: if the chosen `textwrap` release requires a Rust version newer
  than `1.89`, stop and choose a compatible release or ask for approval to
  raise the minimum supported version.
- Complexity: if preserving inline code spans and Markdown links appears to
  require rebuilding a custom tokenizer and buffer that is materially similar
  to the current one, stop and reframe the work as a narrower prefix-handling
  simplification plus a separate design review for the inline engine.
- Validation: if `make lint` or `make test` still fail after three focused fix
  cycles, stop and capture the failing cases in `Decision Log`.

## Risks

- Risk: plain `textwrap::wrap` is likely to split Markdown inline code spans or
  links that contain spaces, which would regress tests such as
  `wrap_text_preserves_code_spans`, `wrap_text_multiple_code_spans`, and
  `wrap_text_preserves_links`. Severity: high Likelihood: high Mitigation:
  begin with a red test baseline and a deliberate prototype. If the prototype
  fails, add a minimal shielding layer for inline code spans and links before
  invoking `textwrap`, rather than reviving the full custom `LineBuffer` loop.

- Risk: the issue text suggests removing `tokenize_markdown`, but that helper
  is public and is used outside wrapping. Severity: high Likelihood: high
  Mitigation: treat public tokenization as out of scope for the first delivery.
  Remove only wrapping-specific internals that become dead after the refactor.
  Record any remaining token API cleanup as a follow-up issue.

- Risk: `src/wrap/paragraph.rs` currently mixes byte-length and display-width
  calculations. A rushed refactor could accidentally preserve that bug or
  introduce a different indentation drift for bullets, blockquotes, or
  footnotes. Severity: medium Likelihood: medium Mitigation: centralize prefix
  wrapping in one helper that computes prefix and indent widths once with
  `UnicodeWidthStr::width`, then add focused tests for repeated prefixes and
  continuation indentation.

- Risk: the repository contains many wrap-focused tests under `tests/wrap/`,
  but they are not active Cargo targets. Severity: medium Likelihood: high
  Mitigation: do not assume they provide safety. Promote any critical cases
  needed for the refactor into `src/wrap/tests.rs`, `tests/wrap_unit.rs`, or
  another active top-level `tests/*.rs` target before changing the engine.

- Risk: the architecture docs currently describe the bespoke tokenizer flow and
  module relationships in detail. Severity: low Likelihood: high Mitigation:
  update the docs as part of the same change, so the repository does not
  advertise an implementation that no longer exists.

## Progress

- [x] (2026-04-22 00:00Z) Reviewed the current wrapping pipeline in
  `src/wrap.rs`, `src/wrap/inline.rs`, `src/wrap/paragraph.rs`, and
  `src/wrap/tokenize/`.
- [x] (2026-04-22 00:00Z) Confirmed that `unicode-width` is already present in
  `Cargo.toml` and that `textwrap` is not yet a dependency.
- [x] (2026-04-22 00:00Z) Confirmed that `tokenize_markdown` is public and is
  used by footnotes, text processing, and code-emphasis logic outside the wrap
  engine.
- [x] (2026-04-22 00:00Z) Confirmed that `tests/wrap/*.rs` are orphaned from
  Cargo integration discovery and must not be relied upon as active coverage.
- [x] (2026-04-22 00:00Z) Added active regressions for nested blockquote
  prefix repetition, footnote continuation alignment, and checkbox indentation
  in `src/wrap/tests.rs`.
- [x] (2026-04-22 00:00Z) Introduced `textwrap` and routed inline line fitting
  through `textwrap::wrap_algorithms::wrap_first_fit`.
- [x] (2026-04-22 00:00Z) Replaced the bespoke line-buffer loop with
  fragment-based wrapping and consolidated prefix handling in
  `src/wrap/paragraph.rs`.
- [x] (2026-04-22 00:00Z) Removed the dead `src/wrap/line_buffer.rs` module
  from the active wrap path.
- [x] (2026-04-22 00:00Z) Updated the affected documentation and passed
  `make fmt`, `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
  and `make nixie`.
- [x] (2026-04-23 00:00Z) Tightened `rebalance_atomic_tails` so post-wrap
  fragment moves no longer create overflow lines, reused the trailing buffer in
  `wrap_preserving_code` instead of cloning the full prefix per fragment, and
  added an active regression for the `a four five` / width `6` case.

## Surprises & Discoveries

- Observation: the user-facing issue text names `tokenize_markdown` as a
  candidate for removal, but in this repository it is not just an internal wrap
  helper. It is re-exported publicly from `src/lib.rs` and also feeds
  `src/textproc.rs`, `src/footnotes/mod.rs`, `src/footnotes/renumber.rs`, and
  `src/code_emphasis.rs`. Evidence: `src/lib.rs`, `src/textproc.rs`,
  `src/footnotes/mod.rs`, `src/footnotes/renumber.rs`, and
  `src/code_emphasis.rs`. Impact: the first implementation milestone should
  replace the bespoke wrap engine without promising removal of the public token
  API.

- Observation: `tests/wrap/mod.rs` and the other files under `tests/wrap/` are
  not active integration tests. Evidence: Cargo only discovers top-level
  `tests/*.rs` files, and the active wrap targets are instead
  `tests/wrap_unit.rs`, `tests/wrap_cli.rs`, and the library tests in
  `src/wrap/tests.rs`. Impact: any critical regression that currently exists
  only in the nested directory must be promoted to an active harness before the
  refactor.

- Observation: `docs/architecture.md` explicitly documents the bespoke
  tokenizer flow and module relationships, and `docs/trailing-spaces.md`
  mentions `wrap_preserving_code` by name. Evidence: `docs/architecture.md`
  sections "Module relationships", "Tokenizer flow", and "Unicode width
  handling", plus `docs/trailing-spaces.md`. Impact: documentation updates are
  part of the required work, not optional cleanup.

- Observation: `textwrap`'s low-level `wrap_first_fit` API works better for
  this migration than `textwrap::wrap` because it accepts pre-grouped fragments
  with caller-defined widths. Evidence: `textwrap::core::Fragment` and
  `textwrap::wrap_algorithms::wrap_first_fit`. Impact: the repository can keep
  its existing Markdown-aware token grouping and whitespace preservation
  without reviving the old mutable `LineBuffer`.

- Observation: the post-processing pass that rebalances trailing atomic or
  plain fragments can invalidate `wrap_first_fit`'s width guarantee if it moves
  a fragment without rechecking the destination line width. Evidence:
  `rebalance_atomic_tails` on 2026-04-23 could turn `a four` / `five` into `a` /
   `four five` at width `6`. Impact: any heuristic that mutates fitted lines
  after wrapping must be width-aware, or it can regress downstream layout
  assumptions.

## Decision Log

- Decision: scope the first delivery to replacing the bespoke wrapping engine,
  not the public token API. Rationale: `tokenize_markdown` is public and has
  non-wrap consumers. Keeping that boundary stable preserves safety and lets
  this issue land as the low-risk refactor it was labelled to be. Date/Author:
  2026-04-22 / Codex.

- Decision: use `textwrap` only for line-breaking and keep the existing
  high-level block classification in `wrap_text`. Rationale: fences, indented
  code blocks, headings, tables, and hard line breaks are repository-specific
  policy decisions that already live in `src/wrap.rs` and do not need to move
  into a third-party crate. Date/Author: 2026-04-22 / Codex.

- Decision: prove behaviour with active tests before deleting old internals.
  Rationale: the repository contains inactive wrap tests, so moving directly to
  clean up would create a false sense of safety. Date/Author: 2026-04-22 /
  Codex.

- Decision: use `textwrap::wrap_algorithms::wrap_first_fit` with custom
  Markdown-aware fragments instead of `textwrap::wrap`. Rationale:
  `wrap_first_fit` delegates line fitting to `textwrap` while allowing
  `mdtablefix` to preserve leading carry whitespace, atomic code spans, and
  display-width measurements on grouped fragments. That keeps the refactor
  small without losing the behaviours guarded by the current tests.
  Date/Author: 2026-04-22 / Codex.

- Decision: treat post-wrap fragment rebalancing as width-constrained, and keep
  fragment classification explicit on `InlineFragment`. Rationale: the
  migration's shielding layer is still part of the line-fitting contract, so it
  must not create lines that `wrap_first_fit` would have rejected. Recording
  fragment kind once also makes the whitespace-carry and tail-rebalance passes
  easier to audit than repeating ad hoc string classification in each branch.
  Date/Author: 2026-04-23 / Codex.

## Outcomes & Retrospective

The implementation kept `tokenize_markdown` and the existing block
classification intact, but replaced the bespoke `LineBuffer` loop with a
fragment-based adapter over `textwrap::wrap_algorithms::wrap_first_fit`. Prefix
handling now flows through one helper in `src/wrap/paragraph.rs`, and active
regressions cover nested blockquotes, footnote continuation alignment, and
checkbox indentation.

The key lesson from the migration is that `textwrap` alone is not a drop-in
replacement for the repository's historic whitespace semantics. Preserving the
old behaviour required a thin adaptation layer that merges whitespace-only
overflow lines forward and sometimes rebalances the trailing fragment when the
correct break only becomes obvious after the following separator arrives.

That adaptation layer still needs the same width discipline as the underlying
wrapper. The 2026-04-23 follow-up fix confirmed that a seemingly harmless
tail-rebalancing heuristic can reintroduce overflow unless it rechecks the
destination line's display width before moving a fragment.

## Context and orientation

The top-level wrapping entry point is `wrap_text(lines, width)` in
`src/wrap.rs`. It streams input line by line, preserving fenced code blocks,
indented code blocks, tables, headings, Markdownlint directives, and blank
lines. Only ordinary paragraph text and prefixed lines such as bullets,
blockquotes, and footnotes reach the wrapping helpers.

Prefix-aware wrapping currently lives in `src/wrap/paragraph.rs`.
`ParagraphWriter::handle_prefix_line` flushes any active paragraph and
delegates to `append_wrapped_with_prefix`. That helper computes the available
width for the content after the prefix, wraps the content, and then prefixes
the first and continuation lines manually. This is the best place to
consolidate the prefix logic into one helper such as `wrap_with_prefix`.

Inline line-breaking currently lives in `src/wrap/inline.rs`.
`wrap_preserving_code` tokenizes the paragraph into inline fragments, groups
them into spans, and then walks them through a custom `LineBuffer`. The custom
logic exists to preserve inline code spans, links, adjacent punctuation,
consecutive whitespace, and trailing spaces. This is the core complexity that
the refactor is meant to remove.

The `src/wrap/tokenize/` directory serves two separate roles today. One role is
the inline segmentation used only by `wrap_preserving_code`. The other role is
the public `tokenize_markdown` API used by other modules. These roles must be
treated separately during implementation; deleting the wrapping-specific
segmenter does not automatically justify removing the public token API.

Active tests that currently exercise wrapping behaviour live in
`src/wrap/tests.rs`, `tests/wrap_unit.rs`, `tests/wrap_cli.rs`,
`tests/wrap_renumber.rs`, and wrap-related cases inside `tests/cli.rs`,
`tests/cli_frontmatter.rs`, `tests/code_emphasis.rs`, and
`tests/markdownlint.rs`. The nested `tests/wrap/*.rs` tree is useful as a
source of examples, but not as active protection until its cases are promoted
to top-level targets.

The documentation that will need review lives in `README.md`,
`docs/architecture.md`, and `docs/trailing-spaces.md`. `docs/architecture.md`
currently includes a tokenizer-flow diagram that will become misleading once
the bespoke span buffer is removed.

## Plan of work

Stage A is a baseline and red-test stage. Start by identifying which current
behaviours the refactor must preserve. The minimum list is: inline code spans
remain intact, Markdown links remain intact, hyphenated words are not split,
trailing punctuation stays attached to code spans or links, trailing spaces on
the final wrapped line survive, bullets and footnotes indent continuation lines
correctly, blockquotes repeat the quote prefix, and fenced or indented code
blocks remain byte-identical. Promote any missing critical cases from the
inactive `tests/wrap/*.rs` tree into active harnesses before changing the wrap
engine. Prefer `src/wrap/tests.rs` for unit-level behaviour and
`tests/wrap_cli.rs` or `tests/wrap_unit.rs` for end-to-end regressions.

Stage B is a prototype that proves whether `textwrap` can replace the custom
buffer without hidden regressions. Add `textwrap` to `Cargo.toml` and create a
temporary adapter in `src/wrap/inline.rs` that keeps the current
`wrap_preserving_code` call site stable while routing line breaking through
`textwrap::wrap` plus explicit `Options`. Start with `break_words(false)` and
Unicode-aware word separation. Run only the active wrap tests added in Stage A.
If the naive adapter breaks code spans or links, do not continue deleting old
logic. Instead, implement a small shielding layer that marks inline code spans,
links, and images as atomic fragments before calling `textwrap`, then restore
them after wrapping. The key goal of this stage is to prove that the new engine
can satisfy current tests without recreating the whole old buffer.

Stage C simplifies prefix handling. Replace the current
`append_wrapped_with_prefix` plus `handle_prefix_line` split with a single
helper in `src/wrap/paragraph.rs` that receives a prefix, the remaining text,
the available width, and whether the prefix repeats on continuation lines.
Compute the display width of the prefix once with `UnicodeWidthStr::width`,
compute the wrapped continuation prefix once, and reuse that value for all
continuation lines. Apply the same display-width approach to ordinary paragraph
indentation so the module no longer mixes byte length and visible column width.

Stage D removes dead internals. After the `textwrap`-backed path passes the
full active test suite, delete any no-longer-used custom buffering helpers such
as `src/wrap/line_buffer.rs` and any wrapping-only inline tokenization helpers
that are left unused. Do not remove `tokenize_markdown`, `Token`, or the
modules that still serve non-wrap consumers unless the user explicitly expands
the scope and the public API question is resolved.

Stage E updates the docs. Rewrite the wrapping description in `README.md` only
where user-visible semantics changed or need clarification. In
`docs/architecture.md`, replace the bespoke tokenizer-flow explanation with a
short description of the new split: repository-owned block classification and
prefix handling on one side, `textwrap` plus `unicode-width` for line breaking
on the other. Update `docs/trailing-spaces.md` if `wrap_preserving_code`
changes name, visibility, or implementation details.

Stage F is final validation and cleanup. Run formatting, lint, tests, and the
documentation gates through `tee` so the logs remain inspectable in this
environment. Review the diff to confirm that the result is a real
simplification and not a hidden rewrite of the bespoke engine under new names.

## Concrete steps

Work from the repository root:

```bash
pwd
```

Expected:

```plaintext
/home/user/project
```

List the currently active wrap-related tests before editing so the red/green
cycle is grounded in real targets:

```bash
cargo test -- --list | rg "wrap|tokenize_markdown|frontmatter|markdownlint"
```

Expected:

```plaintext
...wrap_text_preserves_code_spans
...wrap_text_multiple_code_spans
...cli_wrap_in_place_preserves_shell_block_verbatim
...
```

After adding any missing active regressions, prove they fail against a naive
`textwrap` prototype before investing in cleanup. For example, run the focused
unit and CLI suites that exercise the wrapping path:

```bash
cargo test wrap_text_preserves_code_spans --lib
cargo test wrap_text_multiple_code_spans --lib
cargo test --test wrap_unit
cargo test --test wrap_cli
```

Expected:

```plaintext
running ...
test ... ok
```

Once the prototype is passing, run the wider wrap-adjacent suites that can
catch unintended interactions:

```bash
cargo test --test wrap_renumber
cargo test --test cli
cargo test --test cli_frontmatter
cargo test --test code_emphasis
cargo test --test markdownlint
```

Manually verify an end-to-end wrap example that covers a blockquote, inline
code, and a Markdown link:

```bash
printf '%s\n' \
  '> A deliberately long blockquote line that mentions' \
  '`cargo test --test wrap_cli` and the' \
  '[project README](https://example.invalid/readme) so the wrap' \
  'engine must preserve both atomic spans while continuing the quote' \
  'prefix correctly.' \
  | cargo run -- --wrap
```

Expected:

```plaintext
> A deliberately long blockquote line that mentions
> `cargo test --test wrap_cli` and the
> [project README](https://example.invalid/readme) so the wrap
> engine must preserve both atomic spans while continuing the quote
> prefix correctly.
```

Finish with the full repository quality gates. Use `tee` so the logs survive
output truncation and can be inspected afterward:

```bash
set -o pipefail
make fmt 2>&1 | tee /tmp/issue-80-fmt.log
set -o pipefail
make check-fmt 2>&1 | tee /tmp/issue-80-check-fmt.log
set -o pipefail
make lint 2>&1 | tee /tmp/issue-80-lint.log
set -o pipefail
make test 2>&1 | tee /tmp/issue-80-test.log
set -o pipefail
make markdownlint 2>&1 | tee /tmp/issue-80-markdownlint.log
set -o pipefail
make nixie 2>&1 | tee /tmp/issue-80-nixie.log
```

Expected:

```plaintext
... finished successfully ...
```

If any command fails, inspect the tail of the corresponding log before making
another change:

```bash
tail -n 40 /tmp/issue-80-test.log
tail -n 40 /tmp/issue-80-lint.log
```

## Acceptance criteria

The change is complete when all of the following statements are true:

1. `wrap_text` still preserves the behaviour covered by the active wrap tests,
   including code spans, links, prefixes, fences, hard breaks, and trailing
   spaces.
2. `src/wrap/paragraph.rs` uses one display-width-aware helper for prefix
   wrapping instead of the current two-step helper split.
3. The bespoke `LineBuffer`-driven wrapping loop is gone from the active wrap
   path, and the implementation delegates line breaking to `textwrap`.
4. `tokenize_markdown` either remains intact and documented as out of scope, or
   any broader removal is explicitly approved and covered by updated tests and
   documentation.
5. `README.md`, `docs/architecture.md`, and any other affected docs describe
   the new implementation accurately.
6. `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
   `make nixie` all pass.
