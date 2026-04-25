# Preserve nested fenced blocks during fence normalization

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
 `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

Issue: <https://github.com/leynos/mdtablefix/issues/262>

## Purpose / big picture

After this change, `mdtablefix --fences` must normalize only real fenced code
block delimiters, not fence-like lines that appear as literal content inside a
larger fenced block. A user should be able to format Markdown that uses four or
more backticks, or longer tilde fences, to contain inner triple-backtick or
triple-tilde examples without the inner examples being rewritten or treated as
attachment targets for orphaned language specifiers.

The observable success case is a document such as:

    ```markdown
    ```rust
    fn main() {}
    ```
    ```

which must become:

    ```markdown
    ```rust
    fn main() {}
    ```
    ```

The outer opening and closing delimiters stay at four backticks because
compressing them to three would let the inner triple-backtick close the outer
block. The same preservation rule must apply to literal inner tilde fences such
as `~~~rust` inside an outer four-backtick block, or `~~~` inside an outer
`~~~~` block.

## Relevant references and skills

Read these repository documents before touching the implementation:

- [`docs/architecture.md`](../architecture.md) for the processing pipeline and
  the relationship between `process_stream_inner`, fence normalization, and the
  shared `FenceTracker`.
- [`docs/developers-guide.md`](../developers-guide.md) for the internal module
  boundaries and the wrap pipeline notes.
- [`docs/documentation-style-guide.md`](../documentation-style-guide.md) for
  Markdown and wording rules if this plan or architecture notes are updated
  during delivery.

Use these skills while implementing the plan:

- `/root/.codex/skills/execplans/SKILL.md` to keep this document current as a
  living ExecPlan.
- `/root/.codex/skills/leta/SKILL.md` for symbol-aware navigation of
  `compress_fences`, `attach_orphan_specifiers`, `FenceTracker`, and their
  callers.
- `/root/.codex/skills/rust-router/SKILL.md` if implementation work expands
  beyond a local fix and needs deeper Rust-specific guidance.

## Context and orientation

The current fence preprocessing logic lives in
[`src/fences.rs`](../../src/fences.rs). Two functions matter:
`compress_fences(lines)` compresses any matching backtick or tilde fence to
exactly three backticks, and `attach_orphan_specifiers(lines)` rewrites a lone
language line such as `Rust` onto the following unlabeled fence.

The shared fence-state logic already exists in
[`src/wrap/fence.rs`](../../src/wrap/fence.rs). `FenceTracker::observe(line)`
opens a fence when it sees three or more backticks or tildes, closes only when
the marker character matches and the closing run is at least as long as the
opening run, and otherwise leaves the tracker inside the current fence. That is
the exact behaviour needed for nested-fence handling.

`process_stream_inner` in [`src/process.rs`](../../src/process.rs) runs fence
normalization first when `Options { fences: true, .. }` is enabled. That means
the bug is not in paragraph wrapping; it is in the preprocessing step that runs
before the rest of the formatting pipeline sees the lines.

The active regression targets are:

- [`tests/fences.rs`](../../tests/fences.rs) for fence normalization unit and
  integration-style behaviour.
- [`tests/cli.rs`](../../tests/cli.rs) for end-to-end CLI behaviour with
  `--fences`.
- [`tests/cli_fences.rs`](../../tests/cli_fences.rs) for additional
  end-to-end CLI fence regressions, because `tests/cli.rs` is already close to
  the repository's 400-line limit.
- [`src/wrap/tests/fence_tracker.rs`](../../src/wrap/tests/fence_tracker.rs)
  for the shared tracker semantics.
- [`tests/wrap_unit.rs`](../../tests/wrap_unit.rs) if one additional wrapping
  regression is needed in an active top-level target.

Do not plan new coverage in `tests/wrap/fence_behaviour.rs` alone. That file is
not an active Cargo integration target in this repository.

## Constraints

- Do not add new dependencies. The fix must reuse existing regex and fence
  tracking code.
- Do not change the public API exported from `src/lib.rs`.
- Keep fence normalization semantics stable for existing supported cases:
  backtick fences, tilde fences, null-language normalization, and orphan
  specifier attachment outside fenced blocks.
- Preserve literal nested fence content for both marker families. Inner
  backtick fences and inner tilde fences must survive unchanged unless they are
  the real closing delimiter for the currently open outer fence.
- Preserve the original outer fence length when shortening it would make a
  same-marker inner fence line terminate the outer block early. In particular,
  a four-character outer fence that wraps a three-character inner fence of the
  same type must remain four characters wide.
- Reuse the shared fence-state semantics from `FenceTracker` instead of
  creating a second independent nested-fence state machine in `src/fences.rs`.
- Keep code files under the repository's 400-line limit. If new helper logic
  would push a file past that limit, extract a small helper module or helper
  functions instead of growing the file unchecked.
- Add regression tests that fail before the fix and pass after it.
- Run the repository quality gates relevant to the touched files before ending
  the work.

## Tolerances (exception triggers)

- Scope: if the smallest correct fix requires changing more than 6 source files
  or more than roughly 200 net lines of Rust, stop and reassess whether the
  tracker logic should be extracted into a shared helper instead.
- Interfaces: if reusing `FenceTracker` cleanly requires changing public
  exports or widening module visibility beyond internal use, stop and escalate.
- Semantics: if the current tracker cannot model the preprocessing needs
  without changing its observable behaviour for wrapping, stop and document the
  conflict before proceeding.
- Compression rule: if the fix requires abandoning fence compression entirely
  instead of making it conditional on safety, stop and confirm that scope
  change before proceeding.
- Tests: if focused regressions still fail after 3 fix iterations, stop and
  record which input shape is still ambiguous.
- Documentation: if the implementation reveals that
  `docs/architecture.md` materially misdescribes fence normalization, update it
  in the same change; do not leave the architecture notes stale.

## Risks

- Risk: `src/fences.rs` and `src/wrap/fence.rs` currently use similar but not
  identical regex logic, so naïvely mixing them could change accepted info
  strings or indentation handling.
  - Severity: high
  - Likelihood: medium
  - Mitigation: decide early whether `src/fences.rs` should call
    `wrap::is_fence`, reuse only `FenceTracker`, or extract one shared internal
    parser that preserves current `compress_fences` output formatting.

- Risk: orphan specifier attachment could still look through an open outer fence
  and incorrectly attach a language line that is meant to be literal code block
  content.
  - Severity: high
  - Likelihood: high
  - Mitigation: make fence-state tracking explicit in
    `attach_orphan_specifiers`, and add a regression where a specifier-like
    line appears inside an outer fence before an inner triple-backtick or
    triple-tilde example.

- Risk: literal nested tilde fences could still be over-normalized into
  backticks because `compress_fences` currently rewrites real tilde delimiters
  to backticks.
  - Severity: high
  - Likelihood: medium
  - Mitigation: add explicit regressions for `~~~` content nested inside outer
    backtick fences and for shorter `~~~` content nested inside longer tilde
    fences such as `~~~~`.

- Risk: even a real outer fence delimiter can be over-compressed.
  If an outer ````` ```` ````` block containing an inner ```` ``` ```` line is
  rewritten to ```` ``` ````, the inner close will terminate the outer block
  and corrupt the document structure.
  - Severity: high
  - Likelihood: high
  - Mitigation: add a regression that asserts the outer four-character opening
    and closing fences remain four characters wide when they contain a
    same-marker inner triple fence. Mirror the same check for `~~~~` wrapping
    `~~~`.

- Risk: new regressions might be added in inactive test locations and give
  false confidence.
  - Severity: medium
  - Likelihood: medium
  - Mitigation: keep new behavioural coverage in `tests/fences.rs`,
    `tests/cli.rs`, `tests/cli_fences.rs`, `tests/wrap_unit.rs`, or another
    top-level `tests/*.rs` target only.

- Risk: `src/fences.rs` is already moderately sized, and the extra stateful
  logic could make it harder to read.
  - Severity: medium
  - Likelihood: medium
  - Mitigation: prefer small helpers such as `compress_fence_line` or
    `can_attach_orphan_specifier` rather than one large loop body.

## Progress

- [x] (2026-04-23 00:00Z) Reviewed the current fence preprocessing code, the
  shared `FenceTracker`, and the active test targets.
- [x] (2026-04-24 00:00Z) Began implementation after explicit approval to
  proceed, with the plan status updated to `IN PROGRESS`.
- [x] (2026-04-24 00:00Z) Added failing regressions for nested backticks,
  nested tildes, mixed marker nesting, orphan specifier attachment inside an
  outer fence, and CLI `--fences` behaviour. The new red tests failed before
  the implementation in `tests/fences.rs` and `tests/cli_fences.rs`.
- [x] (2026-04-24 00:00Z) Refactored `compress_fences` to analyse matched
  fenced blocks with `FenceTracker`, preserve nested literal fence content, and
  keep unsafe outer delimiter widths and marker families unchanged.
- [x] (2026-04-24 00:00Z) Refactored `attach_orphan_specifiers` to track active
  fenced blocks and avoid attaching specifier-like lines inside them.
- [x] (2026-04-24 00:00Z) Extended `FenceTracker` tests with explicit
  same-marker and mixed-marker nested-fence expectations.
- [x] (2026-04-24 00:00Z) Focused validation passed:
  `cargo test --test fences`, `cargo test --test cli_fences`,
  `cargo test --test cli`, and `cargo test fence_tracker`.
- [x] (2026-04-24 00:00Z) Ran the full required quality gates:
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie`.
- [x] (2026-04-24 00:00Z) Updated `docs/architecture.md` to state that fence
  normalization now shares `FenceTracker` semantics and only compresses outer
  delimiters when doing so is structurally safe.
- [x] (2026-04-24 00:00Z) Addressed code review feedback by replacing the
  repeated `analyze_opening` forward scan with a single-pass pending-block
  implementation, consolidating marker rewriting behind one helper, and adding
  deeper orphan specifier, symmetric tracker, and broader CLI regressions.
- [x] (2026-04-24 00:00Z) Ran review follow-up validation:
  `cargo test --test fences`, `cargo test --test cli_fences`,
  `cargo test fence_tracker`, `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie`.
- [x] (2026-04-24 00:00Z) Verified and addressed follow-up documentation review
  comments in `docs/architecture.md`,
  `docs/execplans/issue-262-nested-code-block-handling.md`, and `src/fences.rs`.
- [x] (2026-04-25 00:00Z) Verified and addressed follow-up review comments:
  `compress_fences` now enters fenced-state tracking for openings recognised by
  `wrap::is_fence` even when they are outside `FENCE_RE`, including spaced info
  strings; nested `FenceTracker` tests are parameterized with `rstest`; and the
  CLI validation path consistently points at `tests/cli_fences.rs`.

## Surprises & discoveries

- Discovery: the bug is earlier than the main streaming processor.
  `process_stream_inner` only consumes the already-normalized output from
  `compress_fences` and `attach_orphan_specifiers`, so fixing wrapping alone
  cannot solve issue `#262`.

- Discovery: the repository already contains the correct closing-fence rule in
  `FenceTracker::observe`: only the same fence marker character can close a
  block, and the closing run must be at least as long as the opening run.

- Discovery: preserving nested literal content is not sufficient on its own.
  A real outer four-character fence must also be preserved when it wraps a
  three-character inner fence of the same marker type, or the inner close will
  become a structural close for the outer block after compression.

- Discovery: `tests/wrap/fence_behaviour.rs` looks relevant but is not run by
  Cargo in this repository. New regressions must live in active top-level test
  targets.

- Discovery: `tests/cli.rs` was already 392 lines before this work, so adding
  the CLI regression there would have pushed it over the repository's 400-line
  code-file limit. The end-to-end nested fence case now lives in the active
  top-level integration target `tests/cli_fences.rs`.

- Discovery: the existing mixed-marker regression covers malformed input where
  a tilde opening is followed by a backtick delimiter, or vice versa, without a
  matching same-marker close. To keep that legacy behaviour stable,
  `compress_fences` only enables stateful shielding after confirming the
  opening delimiter has a real matching close in the original input.

- Discovery: a fully linear implementation still needs to delay emitting a
  matched fenced block until its closing delimiter is seen, because the opening
  delimiter rewrite depends on whether later literal fence-like lines make
  compression unsafe. The follow-up implementation buffers only the currently
  open block and flushes unmatched blocks through the legacy stateless
  normalization path at end of input.

## Decision log

- Decision: plan around shared fence-state semantics instead of duplicating the
  nested-fence rule in a new ad hoc parser. Rationale: the repository already
  depends on `FenceTracker` to keep headings, breaks, wrapping, and other logic
  aligned on fence boundaries. Date/Author: 2026-04-23 / Codex.

- Decision: treat `compress_fences` and `attach_orphan_specifiers` as one
  behavioural unit for this issue. Rationale: both functions currently inspect
  lines without persistent fence state, so fixing only one would leave the
  other able to mis-handle literal content inside an outer fence. Date/Author:
  2026-04-23 / Codex.

- Decision: define fence compression as a safety-preserving rewrite, not an
  unconditional reduction to three markers. Rationale: same-marker nested
  examples require the outer delimiter width to remain greater than the inner
  literal fence width, otherwise the formatted output becomes structurally
  incorrect. Date/Author: 2026-04-24 / Codex.

- Decision: keep unmatched delimiter runs on the legacy stateless normalization
  path. Rationale: existing tests rely on mixed-marker malformed input being
  normalized line by line, while valid matched fenced blocks need stateful
  shielding to preserve literal nested examples. Date/Author: 2026-04-24 /
  Codex.

- Decision: add the new CLI regression in `tests/cli_fences.rs` instead of
  `tests/cli.rs`. Rationale: `tests/cli.rs` was too close to the 400-line file
  limit, and a separate active integration target preserves coverage without
  creating a file-size violation. Date/Author: 2026-04-24 / Codex.

- Decision: use a single-pass pending-block implementation instead of caching
  lookahead results. Rationale: each input line is visited once, the current
  open block carries the only needed safety state, and unmatched malformed
  inputs can still be flushed with the previous line-by-line normalization
  behaviour. Date/Author: 2026-04-24 / Codex.

- Decision: keep `FENCE_RE` as the normalization grammar and use
  `wrap::is_fence`/`FenceTracker` only for structural Markdown fence state.
  Rationale: preprocessing has narrower language-normalization rules than
  wrapping, but closure and marker-family semantics must stay shared.
  Date/Author: 2026-04-24 / Codex.

## Plan of work

Stage A is a regression-first pass. Add failing examples to `tests/fences.rs`
that prove the current bug in both reported shapes: four-backtick outer fences
containing inner triple-backtick lines, and triple-tilde outer fences
containing inner triple-backtick lines. Those same-marker cases must assert
that the outer four-character opening and closing fences remain four characters
wide after formatting. Extend that matrix with explicit tilde preservation
cases: a four-backtick outer fence containing literal `~~~` content, and a
longer tilde outer fence such as `~~~~` containing a shorter literal `~~~`
block that must remain unchanged because it does not close the outer fence. Add
one case that shows `attach_orphan_specifiers` must not attach a specifier-like
line when it appears inside an already open outer fence. Add a CLI regression in
 `tests/cli_fences.rs` that exercises `--fences` on one of these documents so
the user-visible behaviour is covered end to end.

Stage B is the implementation pass in `src/fences.rs`. Refactor
`compress_fences` from a stateless `map` into a line-by-line loop that keeps a
`FenceTracker` for the original input lines. For each line, determine whether
the line is a real fence delimiter in the current state. Compress only those
delimiters that are safe to shorten. If an outer fence uses four or more
markers and the block contains a same-marker literal fence line that would
become a closing delimiter after compression, preserve the original outer
delimiter width on both the opening and closing lines. If the tracker is
already inside a fence and the current line does not close it, preserve the
line exactly as written even if it matches the old regex. This is what keeps
nested literal triple-backtick and triple-tilde examples intact without making
the formatted output structurally invalid.

Stage C is the orphan-specifier pass. Update `attach_orphan_specifiers` to
track fence state while scanning. Candidate orphan specifiers should only be
considered when the scanner is outside any active fence, and the target fence
search must likewise stay outside open blocks. Keep the existing attachment
semantics for indentation and null-language fences once a legitimate outside
fence is found.

Stage D is the shared-semantics lock-in. Extend
`src/wrap/tests/fence_tracker.rs` with explicit cases that demonstrate the rule
preprocessing now depends on: an outer four-backtick fence stays open when an
inner triple-backtick line appears, an outer fence ignores the other marker
family entirely, and a longer tilde fence stays open when a shorter inner
triple-tilde line appears. These tests are not the main user regressions, but
they document the shared contract that now drives both wrapping and
preprocessing, including the requirement that outer width remains greater than
same-marker inner literal fence width.

Stage E is the validation and documentation pass. Run focused fence tests
first, then the broader repository gates required for the touched files. If the
implementation changes the architecture story in a meaningful way, add a short
note to `docs/architecture.md` stating that fence normalization now shares the
same nested-fence tracking semantics as the wrapping pipeline.

## Concrete steps

Work from the repository root:

    pwd

Expected:

    /home/user/project

Make the bug visible with focused tests first:

    set -o pipefail && cargo test --test fences 2>&1 | tee /tmp/issue-262-fences.log

Expected before the fix: at least one new nested-fence regression fails.

After adding the CLI regression:

    set -o pipefail && cargo test --test cli_fences nested 2>&1 | tee /tmp/issue-262-cli-fences.log

Expected before the fix: the new `--fences` nested-fence case in
`tests/cli_fences.rs` fails.

Once the implementation is in place, rerun the focused suites:

    set -o pipefail && cargo test --test fences 2>&1 | tee /tmp/issue-262-fences.log
    set -o pipefail && cargo test --test cli_fences 2>&1 | tee /tmp/issue-262-cli-fences.log
    set -o pipefail && cargo test fence_tracker 2>&1 | tee /tmp/issue-262-tracker.log

Expected after the fix:

    test result: ok. <N> passed; 0 failed

Run the repository gates required for Rust code changes:

    set -o pipefail && make check-fmt 2>&1 | tee /tmp/issue-262-check-fmt.log
    set -o pipefail && make lint 2>&1 | tee /tmp/issue-262-lint.log
    set -o pipefail && make test 2>&1 | tee /tmp/issue-262-test.log

Expected:

    All commands exit with status 0.

If `docs/architecture.md` is updated, run the documentation gates as well:

    set -o pipefail && make fmt 2>&1 | tee /tmp/issue-262-doc-fmt.log
    set -o pipefail && make markdownlint 2>&1 | tee /tmp/issue-262-markdownlint.log
    set -o pipefail && make nixie 2>&1 | tee /tmp/issue-262-nixie.log

Expected:

    All commands exit with status 0.

## Acceptance criteria

- `compress_fences` compresses only actual outer delimiters and leaves nested
  fence-like content unchanged, and preserves outer delimiter width when
  shortening it would let a same-marker inner fence close the block early.
- Literal nested tilde fences such as `~~~` inside outer backtick fences or
  inside longer tilde fences remain unchanged.
- A four-character outer backtick fence wrapping an inner triple-backtick fence
  remains four backticks wide after formatting. The same safety rule applies to
  `~~~~` wrapping `~~~`.
- `attach_orphan_specifiers` does not attach specifiers that appear inside an
  already open fenced block.
- `FenceTracker` tests explicitly cover the outer-four-backticks and
  outer-tildes cases, including shorter inner tilde runs, that govern the new
  preprocessing behaviour.
- `cargo test --test fences`, `cargo test --test cli_fences` for
  `tests/cli_fences.rs`, `make check-fmt`, `make lint`, and `make test` all
  pass.
- If architecture documentation is updated, `make fmt`, `make markdownlint`,
  and `make nixie` also pass.

## Outcomes & retrospective

Implemented nested-fence-safe normalization for `mdtablefix --fences`.
`compress_fences` now analyses matched fenced blocks with `FenceTracker`,
preserves literal nested fence-like lines, and keeps outer delimiters unchanged
when shortening or changing their marker family would make nested literal
content structural. `attach_orphan_specifiers` now tracks active fenced blocks
so specifier-like lines inside an outer fence remain content.

The implementation kept existing legacy behaviour for unmatched mixed-marker
input by using the stateful shielding path only when a matching close exists in
the original input. This preserved the existing regression expectations while
fixing the valid nested-fence cases from issue `#262`.

Validation passed with `make check-fmt`, `make lint`, `make test`,
`make markdownlint`, and `make nixie`.

Code review follow-up replaced repeated forward scans with a linear
pending-block pass and added regressions for deeper orphan specifier content,
outer tilde fences containing inner backticks, and CLI nested-fence variants.
The follow-up validation passed with the focused tests and full repository
gates listed in `Progress`.
