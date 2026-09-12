# Developers guide

## Frontmatter module visibility

The `frontmatter` module is an internal implementation detail of `mdtablefix`.
Its visibility is restricted to `pub(crate)` in the library so the crate does
not expose YAML frontmatter parsing as part of its supported public API.

This boundary matches the role of the module. The helper exists to shield a
leading YAML frontmatter block from Markdown transforms, including CLI-only
operations such as list renumbering and thematic break normalization. External
callers interact with that behaviour through higher-level formatting entry
points rather than by calling the frontmatter helper directly.

### Rationale

- Keep the public API focused on stable formatting operations rather than
  document pre-processing internals.
- Reduce the risk of accidental API commitments around a narrowly scoped
  helper that may need to change with the processing pipeline.
- Make the intended layering explicit: frontmatter detection supports stream
  processing, but it is not a general-purpose parsing API.

### Canonical frontmatter boundary

[`process_with_frontmatter`](../src/process.rs) is the single authoritative
boundary for splitting and rejoining leading YAML frontmatter. It never passes
the frontmatter prefix to `body_fn`, and it prepends that prefix verbatim to
the closure's output.

Callers must not split, transform, or restore frontmatter outside this
boundary. Both [`process_stream_opts`](../src/process.rs) in the library and
`process_lines` in the package binary route through it. All body transforms
must run inside the closure, including CLI-only transforms such as
`renumber_lists` and `format_breaks`.

When working in this area:

- Prefer wiring new behaviour through `process_with_frontmatter` rather than
  exporting frontmatter helpers.
- Treat changes to frontmatter parsing rules as internal architectural changes
  that should update this guide and any affected behaviour documentation.
- Keep the split/rejoin contract and its tests in sync with changes to
  `src/frontmatter.rs`.

## Table reflow architecture

The table reflow pipeline is split into small stages so continuation rows and
separator rows can be handled without losing column structure.

`protect_leading_empty_cells` rewrites leading empty continuation cells to a
marker before parsing. `parse_rows` preserves physical source-line boundaries
and delegates logical-row recovery to `src/reflow/row_parsing.rs`. That module
infers the expected table width, recognizes complete legacy rows concatenated
on one physical line, and retains padded or trailing empty cells as cell data.
`clean_rows` restores the markers to empty strings and removes rows that are
entirely empty.

`calculate_widths` measures each column using Unicode display width so the
formatter sizes columns according to the glyphs that will actually be emitted.
`format_rows` applies escaping and padding to each cell, and `insert_separator`
restores the separator row with widths derived from the final table body.

## Internal API reference

`Makefile`:

- `check-static-regexes`: Runs before Clippy as part of `make lint` and uses
  ripgrep (`rg`) to reject hand-rolled static regular expression declarations.
  The scan lives in `scripts/check-static-regexes.sh` and rejects any `static`
  that wraps `Regex::new` directly in a supported lazy-wrapper constructor —
  `std::sync::LazyLock::new` or `once_cell::sync::Lazy::new`, whether spelled
  directly or fully qualified — so `lazy_regex!` remains the sole sanctioned
  idiom. `tests/static_regex_lint.rs` exercises every supported form.
  Contributors must install ripgrep locally; Continuous Integration (CI)
  installs the pinned version before running the lint gate.

`src/lib.rs`:

- `lazy_regex!`: Declares every static regular expression through a single
  `LazyLock<Regex>` initialization idiom. New static regular expressions must
  use this macro and supply a descriptive expect message that identifies the
  pattern whose compilation failed.

`src/textproc.rs`:

- `leading_indent(s: &str) -> &str`: Returns the leading whitespace prefix of
  `s` as a borrowed slice. Whitespace follows `char::is_whitespace`, so Unicode
  whitespace and tabs are included. It returns an empty string when `s` has no
  leading whitespace and returns all of `s` when every character is whitespace.
  External callers may rely on this public helper for that Unicode-prefix
  contract, composing with its borrowed result and calling `to_string()` only
  when storage or an owned return is required. New indent extraction must reuse
  this helper rather than duplicating its logic, except Markdown
  block-indentation checks: `src/wrap/block.rs` owns those checks with its
  crate-private `leading_indent`, which recognizes only spaces and tabs and
  returns both column width and byte offset.

`src/command.rs` command-line surface and formatting:

- `Cli` and `FormatOpts` are the parsed command line and the formatting
  switches it carries; `process_lines(lines, opts)` runs the shared pipeline
  over them.
- `formatting_closure(opts) -> impl Fn(&SourceDocument<'_>) -> String + Sync`
  builds the one formatter every mode shares. It renders the document's body
  through the pipeline and re-attaches the mark and the selected line ending,
  so standard output, `--check`, `--diff`, and `--in-place` agree byte for
  byte.
- `format_lines(content, opts) -> Vec<String>` is the pure half of the
  boundary: it splits the body into lines and runs the transforms, leaving the
  terminator to the caller.

`src/main.rs` file-output functions:

- `open_file_parent(path) -> anyhow::Result<(Dir, Utf8PathBuf)>` is the CLI's
  sole ambient filesystem boundary. It opens the selected file's parent
  directory and returns a relative UTF-8 path for capability-scoped handling.
- `analyse_one(mode, path, format)` names the file in any error and delegates
  the work to `driver::analyse` through the capability `open_file_parent`
  returned. `run_stdin` and `run_files` are the two command boundaries built on
  those pieces.

`src/io/line_endings.rs` line-ending policy:

- `LineEnding` is the closed set of terminators the formatter can emit;
  `as_str` returns the characters written between lines.
- `detect_line_ending(text) -> LineEnding` counts carriage return and line feed
  (CRLF) pairs, subtracts them from the total line feed count to obtain the
  lone line feeds, and selects CRLF only when it strictly outnumbers them. An
  exact tie, and a document with no line endings at all, select LF, so the
  result is deterministic.
- `count_line_endings(text) -> LineEndingCounts` returns the selection together
  with the `crlf_count` and `lone_lf_count` that decided it.
  `LineEndingCounts::ending` is the selected style, and `detect_line_ending` is
  the selection-only form of the same query, so the command boundaries can
  report the vote without restating the counting rule.
- The counting query is pure and emits nothing. The report lives in the
  boundary that acts on it: a private `report_line_endings(counts, operation,
  path)` in `src/io/replace.rs`, called by `rewrite_with` with an `operation` of
  `"rewrite"` or `"rewrite_no_wrap"` and always with the file's path; and,
  because the binary is a separate crate and cannot reach that private helper,
  `driver::report_line_endings(counts, operation, path)` in `src/driver.rs`,
  called by `driver::analyse` with `"file"` and the display path — the path the
  user wrote, not the bare name the capability reads by — and by `format_stdin`
  with `"stdin"` and `None`, which the report renders as `<stdin>`. The message
  shape is identical across boundaries, so one filter finds them all.
- `serialize_lines(lines, ending) -> String` joins lines with the selected
  terminator and appends one further terminator, yielding an empty string for
  no lines.
- `detect_line_ending` is a pure query and emits no events. The rewrite
  helpers report the selected ending at `debug` level, with the `crlf_count`,
  `lone_lf_count`, and `selected_ending` fields, and also name the entry point
  and the file, so the decision is traceable without the query becoming
  side-effecting.

Detection runs on the raw document at each input boundary: `rewrite_with` in
`src/io/replace.rs`, `driver::analyse` in `src/driver.rs`, and `format_stdin`
in `src/main.rs`, each reporting through its own `report_line_endings` with the
same message shape. The internal pipeline stays LF-only —
`str::lines` strips each line's terminator before a transform sees it — and
only the serializer re-applies the detected style. Standard input keeps its
historical contract of printing one terminator even when it produces no lines,
which `tests/parallel.rs` pins.

The mode selects behaviour rather than a formatting variant: every mode shares
the one closure `formatting_closure` builds, and `driver::analyse` chooses what
to do with its result, so standard output and `--in-place` cannot disagree
about the formatted text. New file-output call sites must receive a directory
capability and relative `camino::Utf8Path` rather than performing ambient
filesystem access themselves.

`src/io/replace.rs`:

- `replace_file(directory, path, contents) -> std::io::Result<()>` performs the
  shared atomic replacement. It declines a symlinked target, creates a
  `create_new` temporary file in the same directory, writes, flushes and syncs
  the contents, then calls `swap_into_place` in `src/io/swap.rs`, which applies
  the target's permissions to the temporary file before the rename and clears a
  Windows destination's read-only attribute first, because that attribute
  blocks the rename. It attempts to remove the temporary file when a later step
  fails. The CLI and `rewrite`/`rewrite_no_wrap` all call it, so the sequence
  has one implementation.
- `open_parent(path) -> std::io::Result<(Dir, Utf8PathBuf)>` is the library's
  only ambient filesystem boundary. It opens a directory capability for the
  target's parent and returns the target's file name relative to that
  capability.

`src/reflow.rs`:

- `parse_rows`: Parses trimmed table lines into row vectors while preserving
  continuation-row boundaries.
- `clean_rows`: Restores continuation markers to empty strings and drops rows
  that contain no cell content.
- `calculate_widths`: Computes the emitted display width required for each
  output column.
- `format_rows`: Escapes literal pipes, pads cells to the computed widths, and
  emits aligned table lines.
- `insert_separator`: Reinserts a formatted separator row after the header when
  one was parsed or promoted.
- `detect_separator`: Chooses the separator source, preferring an explicit
  separator line and otherwise promoting the second parsed row when valid.

`src/reflow/row_parsing.rs`:

- `cell_is_semantically_empty`: Treats both an empty string and the private
  leading-cell marker as empty parser content.
- `split_physical_rows`: Recovers complete logical rows from each physical row
  using the inferred width, without treating padded cell delimiters as row
  boundaries.
- `infer_expected_width`: Derives the logical column count from the first row
  and complete legacy concatenations, including embedded separator rows.

`src/table.rs`:

- `format_separator_cells`: Expands separator cells to the target widths while
  preserving Markdown alignment markers.

`src/process/buffer.rs`:

- `ProcessBuffer`: Owns the stream-processing output buffer, the pending table
  run, and the table-mode state for `process_stream_inner`. The parent process
  module is responsible for orchestration; the buffer owns the boundary rules
  that decide when fence lines pass through verbatim, when a pipe-led row
  starts table mode, and when a new Markdown block flushes a pending table
  before the block line is processed. Debug instrumentation in this module must
  not log raw Markdown lines; use bounded fields such as line lengths and
  buffer counts.

`src/footnotes/renumber/definitions.rs`:

- `collect_definition_updates`: Scans lines for footnote definitions and
  promotable numeric-list candidates, assigns sequential numbers, and rewrites
  inline references within definition bodies to produce the definition rewrite
  plan.
- `definition_segment_end`: Returns the exclusive end row of a definition
  segment, absorbing continuation and connector-blank lines up to the block
  end. The sibling `reorder` module shares this boundary primitive so scanning
  and reordering compute definition boundaries identically.

`src/footnotes/renumber/reorder.rs`:

- `reorder_definition_block`: Reorders the final footnote-definition block
  according to the numbering plan built by the sibling `definitions` module. It
  keeps continuation lines attached to their definition, preserves block
  prefixes, migrates leading separator blanks at the first segment boundary,
  and skips mutation with a warning if the composed block would change row
  count.

The `reorder` module was extracted from `definitions` during the module-size
refactoring audit covering issues `#357`–`#367` in PR `#368`.

`src/wrap/tokenize/scanning.rs`:

- `scan_code_suffix_end(text: &str, start: usize) -> usize` advances `start`
  past any recognized inflectional or possessive suffix that is directly
  attached to a closing inline-code fence at that position. Recognized shapes
  are: a hyphenated compound where the word after the hyphen starts with a
  lowercase letter (e.g. `-style`), a possessive `'s`, and bare alphabetic
  suffixes (`s`, `ed`, `ing`, and any other run of ASCII letters). Returns the
  original `start` when no suffix is recognized.

`src/wrap/inline/fragment.rs`:

- `has_inline_code_structure(text: &str) -> bool` returns `true` when `text`
  begins with a backtick fence (optionally preceded by an opening bracket or
  punctuation) and contains a corresponding closing fence, with or without a
  trailing inflectional suffix. Used by `classify_fragment` and `inline.rs` to
  identify combined code+suffix tokens as atomic inline code.

## CLI driver and reporting architecture

### Read-only by type

`ReadOnlyDir` in `src/driver.rs` is a newtype over `cap_std::fs_utf8::Dir`. It
exposes only `new` and `read`, so the reporting modes receive a capability that
cannot write: read-only access is a property of the type rather than of a
convention or of a test double that declines to write.

Re-use policy:

- `driver::analyse` clones the caller's writable `Dir` capability with
  `try_clone` and wraps the clone as a `ReadOnlyDir`, so every mode reads
  through one type and only `Mode::InPlace` holds a capability that can write.
  The clone is how the same directory handle serves both the read and the
  write.
- Keep the read-only view on the type system. A convention can be broken by a
  later edit that never read the convention, and a runtime flag records a
  decision that a wrong branch can still make; a type with no write method
  cannot. A test double proves only that the double declined, not that the
  production path cannot write.

### One formatter, built once

`Formatter` in `src/driver.rs` is:

```rust
pub type Formatter = dyn Fn(&SourceDocument<'_>) -> String + Sync;
```

`formatting_closure(opts)` in `src/command.rs` builds the closure once per run,
and `run_files` passes it by reference to every file and every mode. `assess`
is the only place it is called. A mode that built its own formatting path is
the defect this design exists to prevent.

One closure behind both reporting modes and the writer is what makes `--check`
and `--in-place` structurally unable to disagree: they assess the same bytes
with the same formatter, so neither can report drift the other would not
write, or leave a file the other reported. A mode with a second formatting
route would drift from the writer by edits rather than by construction, which
is the divergence the shared closure removes. See
`docs/adrs/0009-check-and-diff-reporting.md`.

The structure is necessary but not sufficient, and it is tested as well as
argued. `tests/check_prediction.rs` runs the reporting and writing modes over
byte-identical copies of one document and compares the report against the
writer's bytes. Those two runs alone would be vacuous, because both modes
consult one shared change decision: a decision that answered wrongly would move
them together and the comparison would still agree. Each case therefore runs
the document a third time in print mode, which renders the formatter's output
without consulting that decision, and measures both sides against it. Its
sixteen-document corpus is in `tests/check_prediction/corpus.rs`, one document
per transform flag, each measured to drift under the flag it is paired with.

### Explicit argument order

`driver::in_argument_order(Vec<(usize, T)>) -> Vec<T>` sorts indexed results
by their recorded index. `rayon`'s `ParallelIterator::collect` into a `Vec` is
not documented to preserve input order, so a report list that relied on it
could follow completion order instead of argument order, and the difference
would be invisible on a machine that happened to finish in order. The index is
captured by `par_iter().enumerate()` in `src/main.rs`'s `run_files`, and
`in_argument_order` restores the sequence explicitly. Because each index is
unique, sorting on it is deterministic whatever order the workers produced.

`tests/cli_check/ordering.rs`'s `reports_every_file_in_order` pins the
sequence: its batch of eight files is in neither alphabetical nor size order,
so a report list that came back sorted by file name, by file size, or by
completion order is rejected rather than passing by luck. The plan records
the reasoning as `AX-4`.

## The binary's private driver

`src/driver.rs` is declared `mod driver;` in `src/main.rs`, so it is not part
of the published library. The library's entry points stay infallible and free
of filesystem policy, while the CLI's exit-status contract, its directory
capabilities, and its `Mode`, `Inputs`, and `ExitStatus` types live in the
binary.

This placement is why the module can hold `anyhow` error types: its callers
are the binary's own, so the module fails with context-rich errors and reports
them at the command boundary. The public library keeps the formatting entry
points and the report types (`mdtablefix::report` is public so a host can
render a `FileReport` itself) and returns `std::io::Result` from its
filesystem entry points rather than the binary's diagnostic error type.

The unit tests live in three modules beside it: `src/driver_contract_tests.rs`
covers the exit-status contract, `Inputs::resolve`, and `in_argument_order`;
`src/driver_report_tests.rs` covers the assessment and both reporting renders;
and `src/driver_in_place_tests.rs` covers the write-back. Their fixtures — the
two tables, the identity and aligning formatters, and the capability-scoped
temporary directory — are shared through `src/driver_test_support.rs`. The same
contract is exercised end to end through the built binary by
`tests/cli_check.rs`, `tests/cli_diff.rs`, and the BDD scenarios in
`tests/features/`.

### File selection is a second private tree

`--git` adds two more modules to the binary and no library surface.
`src/command.rs` holds the clap surface, including the `ArgGroup`s and the
post-parse check that the four git-only flags arrive with `--git`.
`src/git_inputs.rs` is the composition root: it turns the command line into a
selection question and the answer back into the paths `run_files` receives,
and it is the one module that knows the whole selection tree at once.
`src/select.rs` and its submodules state and answer that question.

```text
src/select.rs               the tree root: module list and dependency rule
src/select/policy.rs        select_files, FileIdentity, PathKind, PathProbe
src/select/extensions.rs    --md-exts parsing and matching
src/select/fs_probe.rs      AmbientPathProbe, the working-tree adapter
src/select/git_ls_files.rs  the git subprocess, its framing and diagnostics
src/select/conflict.rs      operation_in_progress and ConflictGuard
```

Dependencies point inwards, in one direction only. `policy` names the
`PathProbe` port and imports neither `std::fs`, `std::process`, nor `cap_std`,
so the rule it states can be read and tested without a filesystem; the adapters
and the composition root depend on the policy, never the reverse. Nothing in
the tree holds a directory capability: the paths it returns are relative to the
working directory, and `main` opens each file's parent as it does for a path the
user typed, so a `--git` run reaches the same capability-scoped writer as every
other run.

The selection is tested at three levels. The sibling `*_tests.rs` files beside
each module cover the policy and its adapters as unit tests, and the boundary
test in `src/select/git_ls_files_tests.rs` runs the real `git`, because the
framing the adapter depends on — NUL-terminated, unquoted, verbatim paths
relative to the process working directory — is exactly the part a fake would
assume. The scenarios in `tests/features/git_file_selection.feature`, bound by
`tests/git_file_selection.rs` to the steps in `tests/steps/git_selection.rs`,
build real repositories and drive the built binary as a user would.
`tests/cli_git.rs` covers what a scenario cannot state as behaviour: the
grammar of the flags, the `--help` rendering, and the properties a reader of a
terminal depends on.

Both fixture sets neutralize the ambient Git configuration
(`GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_GLOBAL`, and `HOME`), so the developer's own
`core.excludesFile` cannot change what is selected, and they supply the identity
that `GIT_CONFIG_GLOBAL=/dev/null` would otherwise remove. No assertion quotes
Git's own wording: this tool's text is the part under test, and Git's is relayed
beside it rather than folded into it.

## HTML parser dependency coupling

HTML table conversion uses `html5ever` for parsing and `markup5ever_rcdom` for
the temporary DOM sink. These crates must stay on the same `markup5ever` parser
stack because `RcDom` implements the `TreeSink` trait from that shared
dependency line. If `html5ever` is upgraded, update `markup5ever_rcdom` in the
same change and run the compile-time parser integration test before merging.

The manifest uses caret requirements rather than exact pins, so compatible
patch updates remain available. The lockfile records the concrete crate release
selected for the branch.

## Fence normalization module

`src/fences.rs` exposes the preprocessing helpers used by the `--fences` option.

- `compress_fences(lines: &[String]) -> Vec<String>` performs
  `FenceTracker`-driven conditional rewriting. For each matched fenced block,
  it determines whether normalizing the outer delimiter would make an inner
  fence line structural. This covers same-marker inner fences and the
  cross-marker case where an inner backtick fence would become structural after
  an outer tilde fence is converted to backticks. If so, it preserves the
  original outer delimiter width and marker family. Unmatched or malformed
  delimiter runs fall through the legacy stateless normalization path.
- `attach_orphan_specifiers(lines: &[String]) -> Vec<String>` attaches a lone
  language identifier line to the following unlabelled fence, but only when the
  scanner is outside any active fenced block. It uses `FenceTracker` to skip
  attachment for specifier-like lines and target fences that appear inside an
  open block, including nested cases preserved by `compress_fences`.

Both functions reuse `FenceTracker` from
[src/wrap/fence.rs](../src/wrap/fence.rs) for structural fence detection. This
keeps preprocessing semantics consistent with the wrapping pipeline. See
[docs/architecture.md](architecture.md) for the processing pipeline context.

## Design decisions

The rationale for the staged table reflow pipeline is recorded in
`docs/adrs/0001-table-reflow-pipeline.md`. Refer to that ADR when changing the
parse, width-calculation, or separator-handling flow so implementation changes
stay aligned with the documented design constraints.

The rationale for treating date-like prose sequences as atomic inline fragments
is recorded in `docs/adrs/0003-date-sequences-as-inline-fragments.md`. Refer to
that ADR before adding new date forms or changing the span-grouping boundary.

## Wrap module architecture

The wrapping pipeline for `--wrap` is:

`wrap_text` parses each leading blockquote with `BlockquotePrefix` before any
block classification. The abstraction exposes the source prefix, nesting depth,
and stripped inner content: downstream classification and prefix-aware wrapping
receive the inner content, while emitted lines retain the source prefix.
`FenceTracker` receives the same inner content and depth. An open fence closes
when a compatible marker is observed at its opening depth, or implicitly when
the current depth drops below that opening depth (the `depth < open_depth`
contract). A transition from depth 3 to depth 2 therefore retains a fence
opened at depth 2; it closes only once the depth falls below 2. Processing
stages that loop over raw Markdown use the crate-private `observe_source_line`
helper. It parses `BlockquotePrefix` once and returns the fence state before
observation, whether the line is a fence marker, and the resulting state. The
public `observe_line` and `in_fence_for_line` compatibility helpers remain for
callers that need one of those individual operations; they apply the same
depth-aware tracking.

1. **Block classification.** `classify_block` in `src/wrap/block.rs` inspects
   each stripped inner line and decides whether it should pass through verbatim
   or enter the paragraph wrapper. `wrap_text` injects a shared
   [`LinkReferenceMatcher`] into each call. Fenced code blocks, indented code
   blocks, headings, tables, directives, thematic breaks, link reference
   definitions, and blank lines stop paragraph accumulation.

2. **Prefix-aware paragraph handling.** `ParagraphWriter` in
   `src/wrap/paragraph.rs` is the single entry point for prefix-aware wrapping.
   `wrap_with_prefix` computes the available content width once from the
   Unicode display width of the first-line prefix, then feeds the paragraph
   text into `wrap_preserving_code`. `ParagraphState` owns the ordinary
   paragraph segments, their shared indentation, any remembered continuation
   indentation, and an optional deferred `PendingPrefix`.

   **Pending prefix deferral.** When `handle_prefix_line` processes a line
   whose text contains an unclosed inline code span (checked by
   `has_unclosed_code_span`), it clears the current paragraph buffer and saves
   the prefix, rest text, available width, `repeat_prefix`, and `hard_break`
   flag into `ParagraphState::pending_prefix` as a `PendingPrefix` value rather
   than wrapping immediately. `PendingPrefix` also records original source
   lines for ambiguity-preserving passthrough, and `synthetic_join_spaces`
   stores byte offsets for spaces inserted by continuation joining so only
   formatter-created code-span edge spaces are trimmed later. Subsequent source
   lines are routed via `handle_pending_continuation` (in `src/wrap/pending.rs`)
   instead of the normal wrapping path. `handle_pending_continuation`
   classifies the line and delegates each soft-wrapped continuation chunk to
   `apply_continuation_chunk` in `src/wrap/continuation.rs`, the module that
   owns the join/update/dispatch state machine. Each continuation is joined onto
   `pending_prefix.rest` via `join_pending_continuation`, which inserts a
   space unless the continuation begins with the exact matching closing fence
   (detected by `continuation_begins_with_closing_fence`). Blockquote
   continuations are only joined when their prefix exactly matches the pending
   prefix. After joining, `apply_continuation_chunk` consults
   `update_span_state` to drive a `SpanStateUpdate` (`StillOpen`,
   `ClosedAndReopened`, or `Flush`); when the same chunk both closes the
   pre-existing span and opens a new one, the helper emits the closed prefix
   segment and keeps the new span pending rather than inventing a closing fence.
   `ParagraphState::drain_pending_prefix` takes that pending segment and
   clears the regular paragraph buffers before final emission.
   `PendingPrefix::used_prefix` tracks whether the original prefix has already
   been emitted, and `pending_prefix_for_next_segment` uses it to give the
   first split segment the original prefix and later split segments the
   continuation indent. `TailReflow` records whether prose after a resolved
   span may remain buffered for greedy reflow or must flush after an ambiguous
   close-and-reopen transition. If the opener is at or near the end of its
   source line, `PendingPrefix` marks subsequent continuations as verbatim, so
   joining does not create leading or trailing spaces inside the code span.
   When the projected join would exceed the available content width, the
   pending line and continuation retain conforming authored boundaries inside
   that span rather than forming a Markdownlint-invalid overlong line.
   Otherwise, `flush_paragraph` passes resolved content through the ordinary
   wrapper so prose after the span is reflowed in the same pass. The exception
   is `ContinuationMode::VerbatimFlush`: when the scanner sees a closing fence
   immediately followed by a word character, `flush_paragraph` emits
   `pending.original_lines` verbatim instead of rewrapping the buffer. When
   `hard_break` is set, two trailing spaces are appended to the last emitted
   line. `clear()` on `ParagraphState` also resets `pending_prefix` to `None`.

   `ContinuationMode` records how pending continuations are handled:
   `Normalize` uses ordinary Markdown soft-break spacing, `TightCodeSpan`
   suppresses synthetic spaces after an opener at end-of-line, and
   `VerbatimFlush` preserves `pending.original_lines` for ambiguous close and
   reopen sequences. The `code_span_trim` module contains
   `trim_code_span_edge_spaces`, which matches code spans by exact fence length
   and removes only spaces whose byte offsets appear in
   `synthetic_join_spaces`. The `spanning_code` module handles ordinary
   paragraphs whose code span crosses a source boundary: when the joined span
   is overlong but every source line conforms, it retains only boundaries
   inside that span, restores the paragraph indentation, and leaves surrounding
   prose to the greedy wrapper. Trace events record width-triggered
   preservation, prefix-mismatch flushes, and `TailReflow` transitions at these
   non-obvious decision boundaries.

3. **Fragment construction and line fitting.** `wrap_preserving_code` in
   `src/wrap/inline.rs` tokenizes prose with `tokenize::segment_inline`, groups
   the tokens into `InlineFragment` values via `determine_token_span`, and calls
    `textwrap::wrap_algorithms::wrap_first_fit` over the accumulated fragment
   buffer. Token predicates in `src/wrap/inline/predicates.rs` classify
   punctuation, links, code spans, and footnote markers. Span grouping helpers
   in `src/wrap/inline/span_helpers.rs` extend grouped spans over trailing
   punctuation, couple adjacent footnote references, and merge chained inline
   code or link tokens. `determine_token_span` forward-couples opening
   punctuation tokens (`(`, `[`, and CJK openers) and hyphen-prefix tokens to
   the next inline code span or Markdown link so wrapping never leaves a lone
   opener or prefix at the end of a line. `try_couple_inline_link_after_opener`
   applies the same rule to parenthesized inline citation links such as
   `([1](url))`, grouping the opener and link as one `SpanKind::Link` so
   adjacent citations like `([1](url))([2](url2))` do not split at the
   boundary. At the tokeniser level, `segment_inline` also stops
   trailing-punctuation and plain-text scans at an unescaped `([` boundary via
   `scan_trailing_punctuation_end` and `scan_plain_text_end`, both using
   `starts_inline_citation`, so the citation opener `(` is emitted as its own
   token instead of being swallowed into the preceding token's punctuation
   cluster. That boundary gives `determine_token_span` and
   `try_couple_inline_link_after_opener` a clean opener token to couple with
   the following inline link, making the full `([n](url))` span atomic, while
   escaped sequences such as `\([` bypass the early exit and remain plain text.
   `normalize_footnote_ref_spacing` in `src/wrap/inline/normalize.rs` then
   removes whitespace tokens between trailing punctuation and inline GFM
   footnote references before fragment construction, while leaving footnote
   definition starts (`[^label]:`) untouched. Trailing punctuation after atomic
   spans is grouped in the same pass, and GFM footnote references that
   immediately follow inline code or links (including opener-coupled spans)
   stay attached to the preceding punctuation cluster. Date-component
   predicates are applied by `try_match_date_sequence` in `span_helpers.rs`
   before `determine_token_span` performs the standard punctuation and link
   grouping pass.

4. **Post-processing and rendering.** The `postprocess` module applies
   `merge_whitespace_only_lines` and then `rebalance_atomic_tails` so
   whitespace-only wrap artefacts and isolated tails are normalized before the
   fragments are rendered back into output lines. `wrap_preserving_code` passes
   its configured wrap width into `merge_whitespace_only_lines`; that pass must
   compare any projected inline-code tail carry against the same width before
   moving an atomic code span onto a following content line. `render_line` in
   `src/wrap/inline.rs` converts each finished fragment line into Markdown
   text. Its `strip_leading_carry_whitespace` flag removes carry whitespace
   that the fitter attaches to the start of wrapped continuation lines; it is
   set only when `wrap_preserving_code` has already emitted at least one line,
   so intentional leading whitespace on the first output line is preserved.
   Non-final lines may also drop a single trailing space unless the line ends
   with a hard-break double space.

### Block classification

**`BlockKind::ThematicBreak`**

Classified by `classify_block` when the stripped line matches
`crate::breaks::THEMATIC_BREAK_RE`: three or more `-`, `*`, or `_` characters,
including spaced runs such as `- - -`. The check outranks bullet
classification because `BULLET_RE` also matches spaced runs. A thematic break
passes through wrapping on its own line and never enters paragraph
accumulation. Table separator rows such as `|---|` contain pipes and remain
table rows.

**`BlockKind::LinkReferenceDefinition`**

Classified when indentation is fewer than four columns and
`LinkReferenceMatcher::is_definition` matches the line.

**`LinkReferenceMatcher`**

Centralizes link reference regex access. `production()` returns the workspace
matcher; callers inject `&LinkReferenceMatcher` (or a copy) into query methods
rather than reading global statics directly. `is_definition(line)` classifies
complete link reference definitions. `is_bare_label_only(line)` classifies
split definitions whose destination may follow on the next line.
`is_url_continuation_line(line)` accepts indented destination continuation
lines with an optional inline title, but rejects indented Markdown-prefixed
blocks so lists and blockquotes still use normal block handling.
`standalone_title_need(line)` returns `None` when the line is not a definition,
`Some(true)` when no inline title is present, and `Some(false)` when a title is
already on the same line. `is_standalone_title_line(line)` matches title
continuation lines per `CommonMark` spec §4.7 (at most three leading spaces,
title in `"…"`, `'…'`, or `(…)`). Known limitation: nested or escaped brackets
inside link labels are not supported (for example, `[label [nested]]` or
`[\[escaped\]]`).

**`LinkTitleWindow`**

Explicit state for link reference continuations in `wrap_text` and
`replace_ellipsis`. Starts `Closed`. Callers pass each classified definition to
`observe_definition(line, matcher)`, which opens `AwaitingStandaloneTitle` for
a bare definition or `AwaitingUrlContinuation` for a label-only definition.
While a title is expected, `observe_next_line(line, matcher)` returns
`Some(EmitVerbatim)` for blank or title lines (and closes the window), or
`Some(Reprocess)` when the line is ordinary prose (closing the window, so the
caller processes it normally).

After a label-only reference definition, `observe_bare_label()` opens
`AwaitingUrlContinuation`. A valid indented destination emits verbatim; if the
destination does not include an inline title, the window advances to
`AwaitingStandaloneTitle` so the following line may still be a standalone
title. Blank lines and inline-title destinations close the window. Markdown
prefixed blocks return `Reprocess`, close the window, and allow normal block
classification to handle the line. Fence entry calls `observe_fence_context()`
to reset the window.

`InlineFragment` carries the rendered fragment text, its precomputed display
width, and a `FragmentKind` tag. That construction-time classification lets the
`is_whitespace`, `is_atomic`, and `is_plain` predicates answer all later
questions without repeating ad hoc string inspection in the post-processing
passes. Inline code spans, Markdown links, and GFM footnote references use
atomic fragment kinds, so the wrapper never inserts a break inside their
Markdown syntax.

The inline span builder uses the private `is_trailing_punctuation_token`
helper, via `extend_punctuation`, to keep trailing punctuation attached to
links and code spans while token groups are being formed. Markdown delimiters
that open syntax are not treated as trailing punctuation, which avoids
classifying arbitrary ASCII punctuation as link suffixes.

The `postprocess` module exists because greedy line fitting alone does not
reproduce the repository's historical whitespace semantics. The first pass
merges whitespace-only wrap lines into adjacent content, and the second pass
rebalances a trailing atomic or plain fragment only when the destination line
still fits within the configured width.

### Key types and functions

Table: Key types and functions.

<!-- markdownlint-disable MD013 MD055 MD056 MD060 -->
| Symbol                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | File                                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------- |
| `LinkReferenceMatcher`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `src/wrap/link_reference.rs`          |
| `LinkTitleWindow`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | `src/wrap/link_reference.rs`          |
| `classify_block`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `src/wrap/block.rs`                   |
| `FragmentKind`, `InlineFragment`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `src/wrap/inline/fragment.rs`         |
| `classify_fragment`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `src/wrap/inline/fragment.rs`         |
| Character and fragment predicates (`is_inline_code_token`, `looks_like_link`, `looks_like_footnote_ref`, `is_month_name`, `is_ordinal_day`, `is_numeric_day`, `is_year`, …)                                                                                                                                                                                                                                                                                                                                                                                                                                  | `src/wrap/inline/predicates.rs`       |
| `SpanKind`, span grouping helpers (`merge_code_span`, `try_couple_footnote_reference`, `try_match_date_sequence`, …)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | `src/wrap/inline/span_helpers.rs`     |
| `try_couple_inline_link_after_opener`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | `src/wrap/inline/span_helpers.rs`     |
| `normalize_footnote_ref_spacing`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `src/wrap/inline/normalize.rs`        |
| `build_fragments`, `wrap_preserving_code`, `render_line`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `src/wrap/inline.rs`                  |
| `determine_token_span`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `src/wrap/inline.rs`                  |
| `merge_whitespace_only_lines`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | `src/wrap/inline/postprocess.rs`      |
| `rebalance_atomic_tails`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `src/wrap/inline/postprocess.rs`      |
| `ParagraphWriter`, `wrap_with_prefix`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | `src/wrap/paragraph.rs`               |
| `ParagraphState`, `PrefixLine`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | `src/wrap/paragraph.rs`               |
| `PendingPrefix`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | `src/wrap/paragraph/pending.rs`       |
| `ContinuationMode`, `TailReflow`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `src/wrap/paragraph/pending.rs`       |
| `conforming_source_lines_for_overlong_span` — Retains conforming authored boundaries only inside a cross-line code span that would be overlong when joined.                                                                                                                                                                                                                                                                                                                                                                                                                                                  | `src/wrap/paragraph/spanning_code.rs` |
| `emit_pending_with_verbatim_continuation` — Emits a pending prefix plus raw continuation for ambiguous inline-code source.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | `src/wrap/paragraph.rs`               |
| `drain_pending_prefix` — Takes the deferred prefixed segment and clears plain paragraph buffers before final emission.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `src/wrap/paragraph.rs`               |
| `pending_prefix_for_next_segment` — Selects the original pending prefix for the first deferred segment, then the continuation indent for later segments.                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `src/wrap/paragraph/pending.rs`       |
| `apply_continuation_chunk` — Centralized join/update/dispatch entry point that reconciles a single continuation chunk with the active `PendingPrefix` buffer.                                                                                                                                                                                                                                                                                                                                                                                                                                                | `src/wrap/continuation.rs`            |
| `join_pending_continuation`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | `src/wrap/continuation.rs`            |
| `starts_inline_citation`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `src/wrap/tokenize/mod.rs`            |
| `opening_fence_run_len` — Measures the length of an unescaped backtick run at the start of a byte slice; used to identify opening code-span fences.                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `src/wrap/tokenize/scanning.rs`       |
| `position_after_close(text, search_start, fence_len)` — Walks `text` from absolute byte offset `search_start` to find the first closing backtick fence of exactly `fence_len` characters. Escaped closing candidates (backtick runs preceded by an odd number of backslashes) are recorded but skipped; a literal (unescaped) closing fence is returned unless an iterative forward look-ahead confirms that fence is itself the opener of a subsequent balanced span, in which case the earlier escaped candidate is returned instead. Returns `None` when no matching close exists or `fence_len` is zero. | `src/wrap/tokenize/scanning.rs`       |
| `scan_continuation_span_state` — Incrementally scans a continuation string given a known open fence length, returning the remaining open fence length or `None` when all spans are balanced; used to avoid O(N²) rescanning of the accumulated pending text.                                                                                                                                                                                                                                                                                                                                                 | `src/wrap/tokenize/scanning.rs`       |
| `handle_backtick_fence` — Tokenizes an inline code span from the opening fence byte offset and delegates closing-fence detection to `position_after_close`.                                                                                                                                                                                                                                                                                                                                                                                                                                                  | `src/wrap/tokenize/parsing.rs`        |
| `handle_pending_continuation`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | `src/wrap/pending.rs`                 |
| `scan_code_suffix_end`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `src/wrap/tokenize/scanning.rs`       |
| `has_inline_code_structure`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | `src/wrap/inline/fragment.rs`         |
<!-- markdownlint-enable MD013 MD055 MD056 MD060 -->

`ContinuationMode` in `src/wrap/paragraph/pending.rs` selects normal joining,
opener-at-EOL tight joining, or original-line verbatim flushing for
`PendingPrefix`. The private `code_span_trim` module provides
`trim_code_span_edge_spaces` for metadata-guided trimming of synthetic
code-span boundary spaces.

`SpanKind` in `src/wrap/inline/span_helpers.rs` records how a grouped token
span behaves while `determine_token_span` walks the stream: `General` for
ordinary prose, `Code` and `Link` for atomic inline spans, and `FootnoteRef`
when a footnote marker has been promoted or grouped with preceding punctuation.

### Design constraints

- **Public API stability.** `mdtablefix::wrap::wrap_text`, `Token`, and
  `tokenize_markdown` must not change their signatures or observable behaviour.
- **Shared fence tracking.** `tokenize_markdown()` in
  `src/wrap/tokenize/mod.rs` uses the same `FenceTracker` implementation as
  `wrap_text` and `src/wrap/fence.rs`, rather than a local boolean, to track
  whether the tokenizer is inside a fenced code block. Once a structural
  opening fence is observed, the tokenizer emits the opener, every interior
  line, and the matching closer as `Token::Fence`, then resumes inline
  tokenization for following prose. Every line inside an open fence preserves
  its byte content verbatim, so post-wrap transforms such as `--ellipsis`,
  `--renumber`, `--breaks`, and `--fences` cannot mutate fenced code block
  bodies. This behaviour was introduced for issue `#329` in PR `#343`,
  including nested literal fences whose marker run is shorter than the active
  outer fence.
- **Atomic fragments.** The inline fitter never introduces a new break inside
  inline code spans, Markdown links, or GFM footnote references. These
  fragments move as a unit when they would overflow the target width. A
  conforming source boundary already inside an overlong cross-line code span
  may instead be retained; Markdown renders that soft break as a space. Opening
  punctuation that immediately precedes an inline code span or link is grouped
  with that span during token grouping so the opener is not left on the
  previous line. Trailing punctuation after those spans follows the same
  grouping rules. GFM footnote references that immediately follow inline code
  or link spans without intervening whitespace are coupled to the preceding
  punctuation cluster, so the marker is not wrapped onto the next line alone.
  Inflectional affixes (`s`, `'s`, `ed`, `ing`) and hyphenated compounds that
  immediately follow a closed backtick fence are absorbed into the code token by
  `scan_code_suffix_end` in `src/wrap/tokenize/scanning.rs`; the combined
  token is recognized as atomic by `has_inline_code_structure` in
  `src/wrap/inline/fragment.rs`, so wrapping treats the full string as one
  unit. Leading-hyphen compounds — a token that ends with a hyphen and contains
  at least one alphabetic character (for example `pre-`, `LLM-`, `(API-`) — are
  coupled forward to the next inline code span during span grouping by the
  `ends_with_hyphen_prefix` predicate in `src/wrap/inline/predicates.rs`,
  applied in `determine_token_span` in `src/wrap/inline.rs`. The coupling
  mirrors the existing opening-punctuation pattern, so compounds such as
  `` pre-`LLMPort` `` and `` (API-`Foo`) `` remain atomic during wrapping.
  Internal hyphen chains (e.g. `state-of-the-art-`) are accepted by design;
  bare dash runs such as `-` or `---` are rejected. Unicode alphabetic
  characters (e.g. `pré-`, `字-`) are intentionally supported.
- **Hard breaks.** Trailing two-space hard breaks must survive on the emitted
  line where they occur.
- **Verbatim blocks.** Fenced code blocks must pass through unchanged, along
  with the other non-paragraph block kinds detected by `classify_block`.
- **Prefix width.** The visual width of every prefix string is measured with
  `UnicodeWidthStr::width` before the available text width is computed, so
  non-ASCII prefix characters (e.g. `「` in CJK blockquotes) are accounted for
  correctly.
- **Cross-line code spans.** When a prefixed line contains an unclosed inline
  code span, `PendingPrefix` buffers the continuation until the span closes.
  The wrapper does not introduce a new break inside the span. If the joined
  span exceeds the width, it may preserve conforming authored boundaries inside
  the span, including the correct repeated or indented continuation prefix,
  while prose outside the span remains eligible for ordinary greedy reflow.
- **Prefixed tail deferral.** When the first line of a prefixed block exceeds
  the available width, `ParagraphWriter` defers it through `PendingPrefix` so
  the continuation and lazy continuation lines below it are folded into the
  same wrapped output. `wraps_to_tail` and `continuation_folds_tail` in
  `src/wrap/paragraph.rs` decide whether folding is safe: a tail that would
  re-parse as a different block stays separate, which covers a tail indented by
  four or more columns (indented code), a tail that repeats its blockquote
  marker, and a footnote definition tail. Emitting a bare tail instead would
  let the next pass fold the lines below into it, so the formatter would never
  converge.
- **Closing fence detection.** Backslash escape checks apply only while
  detecting opening backtick fences in ordinary Markdown text. Once a code span
  is open, backslashes in the span content are literal bytes and must not make
  a matching closing fence invisible. All tokenizer entry points that close
  code spans use `position_after_close` so they also reject candidate closers
  embedded in a longer backtick run.
- **Width-aware inline-code carries.** `merge_whitespace_only_lines` receives
  the active wrap width from `wrap_preserving_code`. Before carrying a previous
  inline-code tail across a single-space wrap artefact, it must compute the
  projected destination line width and skip the carry when that projection
  would exceed the configured width.
- **`WRAP_COLS` public constant.** `mdtablefix::process::WRAP_COLS` is
  exported as `pub` so that integration tests can reference the production wrap
  width instead of hard-coding `80`. When writing tests that depend on the
  column boundary (for example, wrap-boundary edge-case tests), import and use
  `WRAP_COLS` as the single source of truth. Do not duplicate the literal value
  `80` in test code.

Refer to `docs/adrs/0002-textwrap-wrapping-engine.md` for the rationale behind
replacing `LineBuffer` with `textwrap`.

## Observability

### Dependency

`tracing = "0.1"` and `metrics = "0.24"` are the runtime observability
dependencies, used by the library and the executables. `tracing-test = "0.2"`
and `metrics-util = "0.20"` are test-only dev-dependencies; use them only in
tests (e.g. `DebuggingRecorder`). Traced tests should use the in-repo
`test_macros::traced_test` rather than `tracing_test::traced_test` directly,
because it rebuilds the `tracing` interest cache after the subscriber is
installed, so a callsite first used before that install cannot remain cached
as `Interest::never()` and lose the test's log lines (see
`test-macros/src/lib.rs` for the full rationale). The crate does not install a
global subscriber or metrics recorder. Executables and test harnesses that want
log output must install their own subscriber (e.g.
`tracing_subscriber::fmt::init()` in `main`).

`Cargo.toml` also declares `googletest = "0.14"` and `pretty_assertions = "1"`
as dev-dependencies, though neither has a use site in the tree today. They are
retained because the ExecPlan's Decision log records them as accepted on
explicit owner instruction, with the narrowing of `EP-M5` as the mitigation
rather than removal (see `docs/execplans/check-option.md`, the "keep
`googletest`, `pretty_assertions`, and `rstest-bdd` despite the review's
objection" entry).

### Log levels

Use `debug!` for high-value classification outcomes: fragment kind, parsed
token length, span promotion result, parsed blockquote prefix, and fence-state
transitions. Use `trace!` for branch-level checks: predicate matched, prefix
mismatch, unterminated bracket, rejected blockquote prefix, and incompatible
fence marker. Never emit at `info!` or above from library code.

### Field naming

Use the stable structured field names `token_length`, `kind`, `start`, `end`,
`width`, `reason`, `is_image`, `row_index`, `cell_count`, and `error_category`.
Blockquote and fence events additionally use `line_len`, `prefix_len`, `depth`,
`inner_len`, `open_depth`, `marker_len`, `open_marker_len`, and `transition`.
Line-ending events use `crlf_count`, `lone_lf_count`, and `selected_ending`,
and every reporting boundary adds `operation` and, for a file, `path` (the
library rewrite reports both; standard input has no path). The binary's
per-file analysis span adds `mode`, `outcome`, and `elapsed_seconds`.
These events are content-free: never include raw Markdown, blockquote
prefixes, fence info strings, or other document content. Executables remain
responsible for installing subscribers.

Blockquote parsing emits `blockquote prefix parsed` or
`blockquote prefix rejected`. Fence tracking emits `fence state changed` with
the `open`, `matching_close`, or `implicit_close` transition, and
`fence marker did not change state` with the `unchanged` transition. The
corresponding `reason` values are `no_blockquote_prefix`,
`blockquote_depth_decreased`, and `incompatible_active_opener`.

The in-place rewrite in `src/io/replace.rs` follows the same discipline.
`replace_file` carries a `debug` span whose only field is the target `path`.
The `path` field is span metadata rather than a metric label, and the
replacement path's metrics use only fixed label values, so target paths cannot
create unbounded metric cardinality; the crate installs no recorder. Inside it,
`target metadata read` (trace), `temporary file created` (debug, with
`attempt`), `temporary file written` (debug, with `bytes`),
`temporary file synced` (debug), `destination read-only attribute cleared`
(debug, on a read-only Windows destination, immediately before the rename),
`target mode applied` (debug, on the temporary file and before the rename on
every platform), and `target replaced` (debug) mark the success path;
`temporary name rejected` (trace, with `attempt` and
`reason = "already_exists"`) marks the retry path;
`temporary file removed after failure` (trace), `temporary file cleanup failed`
(debug, with `error_category` from `io::ErrorKind`), and
`temporary file mode could not be cleared` (debug, with
`error_category` from `io::ErrorKind`, on Windows, where a read-only
temporary file has its attribute cleared before it can be deleted)
mark the cleanup path; and `rewrite declined` (debug, with
`error_category = "symlink_target"`) marks a symbolic-link target.
`replacement failed` (debug, with `error_category` from `io::ErrorKind`)
marks a failed metadata read, temporary-file creation, or write/swap.
`destination mode restore failed` (debug, with `error_category` from
`io::ErrorKind`) marks a swap that failed after the destination's read-only
attribute had been cleared and whose original attribute could not be put back;
that restoration is best effort, so a failure to restore never masks the reason
the swap failed. None of these events carry file content.

The binary's own boundary follows the same discipline. `record_analysis` in
`src/metrics.rs` opens a `debug` span named after it around one file's
analysis, carrying `mode` and the display `path` on entry, and `outcome` and
`elapsed_seconds` recorded once the analysis has run — the same names and
values the per-file counters use, so a trace and a metric select the same
files, and a subscriber can tell a `--check` analysis from an `--in-place` one.
A failed analysis also emits `analysis failed` (debug, with `error_category`
from the same bounded `category` the error counter labels with: `not_found`,
`permission_denied`, `declined`, or `other`). The `path` is the display path
the user wrote and is span metadata rather than a metric label, so two files
called `a.md` in different directories stay distinct in a trace and still cost
no cardinality. The `operation` field already reaches the same trace: the
line-ending report this analysis emits carries `"file"`.

Table: Structured field names emitted by tracing instrumentation.

| Field             | Type            | Used in                                                 | Meaning                                                     |
| ----------------- | --------------- | ------------------------------------------------------- | ----------------------------------------------------------- |
| `token_length`    | `usize`         | fragment, link, footnote events                         | Character count of the text that was classified or parsed   |
| `kind`            | `?FragmentKind` | `fragment classified`                                   | The computed fragment classification                        |
| `start`           | `usize`         | span events                                             | Byte offset where the span begins                           |
| `end`             | `usize`         | span events                                             | Byte offset where the span ends (exclusive)                 |
| `width`           | `usize`         | span events                                             | Display-column width of the span                            |
| `reason`          | `&str`          | rejected, unchanged, or fence-state decisions           | Stable diagnostic category for any decision                 |
| `is_image`        | `bool`          | `link or image parsed`                                  | `true` when the link token is an image literal (`![]()`)    |
| `row_index`       | `usize`         | table-row events                                        | Zero-based index of the parsed logical row                  |
| `cell_count`      | `usize`         | table-row events                                        | Number of cells in the parsed logical row                   |
| `error_category`  | `&str`, Debug   | declined, discarded, replacement, and analysis failures | Stable category or I/O error kind for a failure             |
| `attempt`         | `u32`           | `replace_file` events                                   | Zero-based index of the temporary-file creation attempt     |
| `bytes`           | `usize`         | `replace_file` events                                   | Byte length of the formatted replacement that was written   |
| `line_len`        | `usize`         | blockquote-prefix events                                | Byte length of the examined source line                     |
| `prefix_len`      | `usize`         | blockquote-prefix events                                | Byte length of the recognized blockquote prefix             |
| `depth`           | `usize`         | blockquote and fence events                             | Current blockquote nesting depth                            |
| `inner_len`       | `usize`         | blockquote-prefix events                                | Byte length after removing the blockquote prefix            |
| `open_depth`      | `usize`         | fence-state events                                      | Blockquote depth of the active fence opener                 |
| `marker_len`      | `usize`         | fence-state events                                      | Length of the currently recognized fence marker             |
| `open_marker_len` | `usize`         | fence-state events                                      | Length of the active opening fence marker                   |
| `transition`      | `&str`          | fence-state events                                      | Stable fence-state transition category                      |

For example:

```rust
debug!(token_length = token.chars().count(), kind = ?kind, "fragment classified");
```

### Metrics

The in-place replacement in `src/io/replace.rs` emits three counters and one
histogram through the `metrics` façade. `describe_metrics` registers their
descriptions exactly once per process behind a `std::sync::OnceLock`.

- `mdtablefix_io_replace_total` increments once per `replace_file` call and
  carries one label, `outcome`, with the value `success` or `failure`.
- `mdtablefix_io_replace_duration_seconds` is a histogram of replacement
  durations in seconds, with the unit declared by `metrics::Unit::Seconds`. It
  is recorded once per `replace_file` call with the same `outcome` label as
  `mdtablefix_io_replace_total`. Failures are recorded too, so a replacement
  that stalls before it fails is visible rather than missing from the
  distribution.
- `mdtablefix_io_temporary_name_collisions_total` counts each candidate
  temporary name rejected because it was already taken. It carries no labels.
- `mdtablefix_io_temporary_name_exhausted_total` counts each replacement
  abandoned when all 16 candidate names are taken. It carries no labels.

Metric cardinality is bounded by construction: every metric name and every
label value is a compile-time constant. Target paths, file names, and error
text are never labels. The target `path` appears only as a tracing span field.

The library emits metrics but never installs a recorder, in line with
`AGENTS.md`. A host application installs one once, as early as practical in
startup, for example `metrics::set_global_recorder(...)` or an exporter such as
`metrics_exporter_prometheus::PrometheusBuilder::install()`. With no recorder
installed the emission macros are no-ops, so the `mdtablefix` CLI stays silent
unless a host wires one in.

`src/io_metrics_tests.rs` uses `metrics_util::debugging::DebuggingRecorder`
through `metrics::with_local_recorder` on the test thread and asserts the
emitted metric names, the counts for a success, for an occupied candidate
name, and for an exhausted name space, plus the bounded label set: only the
`outcome` key, with only the values `success` and `failure`. The tests also
assert that the histogram's declared unit is seconds and that exactly one
sample is recorded per replacement for both the `success` and `failure`
outcomes.

#### Binary metrics

The binary is a separate crate, so it cannot reuse the library's declarations;
`src/metrics.rs` declares its own under the same convention. The names describe
what a run did rather than what one replacement did, and the label set stays
bounded for the same reason: `mode` is one of `print`, `in_place`, `check`, or
`diff`, `outcome` is one of a fixed set per instrument, and no path is ever a
label.

- `mdtablefix_run_total` counts runs by `mode` and `outcome`, where the outcome
  is the exit status as a user sees it: `success`, `drift`, or `error`.
- `mdtablefix_file_total` counts analysed files by `mode` and `outcome`, where
  the outcome is `changed`, `unchanged`, or `error`.
- `mdtablefix_file_duration_seconds` is a histogram of one file's analysis in
  seconds, with the unit declared by `metrics::Unit::Seconds`, carrying the
  same two labels. The duration is measured around the analysis alone, so a
  file that waited behind another on the parallel analysis pool does not carry
  that wait in the distribution.
- `mdtablefix_file_error_total` counts failed analyses by `category`, a fixed
  name derived from the `io::ErrorKind` in the chain — `not_found`,
  `permission_denied`, `declined` for this tool's own refusal to replace a
  symlink, and `other` for everything else, including an error with no
  `io::Error` at all.

The changed-or-unchanged distinction is the byte comparison the analysis itself
made, so the label a host aggregates and the exit status the run reports cannot
disagree about whether a file drifted. `record_analysis` records the same
distinction on the span it opens, described under field naming above.

`src/metrics_tests.rs` and `src/metrics_file_tests.rs` install a local recorder
through `metrics::with_local_recorder` and assert the declared names, units,
and descriptions, the label sets, and the counts for each outcome; the runner
tests do the same for `mdtablefix_run_total`.

### Performance discipline

Guard any expression that performs non-trivial work with
`tracing::enabled!(Level::DEBUG)` or `tracing::enabled!(Level::TRACE)` before
computing the value.

### Security considerations

Tracing events must not include raw document content. Record bounded metadata
such as indices, lengths, fragment kinds, and stable error categories instead.
In particular, do not rely on downstream subscribers to redact link, footnote,
table-row, or token text.

### Instrumented functions

Functions decorated with `#[tracing::instrument]` are listed below with their
level and notable fields. Update this list when adding new instrumented entry
points.

Table: Instrumented functions and their logging levels and fields.

| Function                  | Level        | Fields                                                                                            |
| ------------------------- | ------------ | ------------------------------------------------------------------------------------------------- |
| `looks_like_footnote_ref` | trace        | `skip(token)`, return value (out)                                                                 |
| `ends_with_footnote_ref`  | trace        | `skip(token)`, return value (out)                                                                 |
| `ends_with_hyphen_prefix` | trace        | `skip(token)`, return value (out)                                                                 |
| `is_month_name`           | trace        | `skip(token)`, return value (out)                                                                 |
| `is_ordinal_day`          | trace        | `skip(token)`, return value (out)                                                                 |
| `is_numeric_day`          | trace        | `skip(token)`, return value (out)                                                                 |
| `is_year`                 | trace        | `skip(token)`, return value (out)                                                                 |
| `try_match_date_sequence` | trace, debug | `start` (in), `skip(tokens)`, return value (out); matched date pattern                            |
| `date_token_span`         | trace        | `start` (in), `skip(tokens)`, return value (out); over-width date fallback remains behaviour-only |
| `parse_link_or_image`     | debug        | `idx` (in), `skip(text)`; `token_length` and `is_image` events                                    |
| `find_footnote_end`       | trace        | `idx` (in), `skip(text)`, return value (out)                                                      |
| `parse_rows`              | trace, debug | `skip(trimmed)`; `row_index`, `cell_count`, and `error_category` events                           |
| `replace_file`            | debug        | `path` (in); emits the in-place rewrite events                                                    |

### Tracing-event snapshot tests

The stable structured fields above are pinned by `insta` snapshots so that
accidental changes to a tracing event's level, target, message, or field set
are caught in review. These tests live next to the instrumented code:

- `src/wrap/inline/fragment_tracing_snapshots.rs` – `fragment classified`.
- `src/wrap/inline/span_helper_tracing_tests.rs` – date-span events.
- `src/wrap/tokenize/parsing_tracing_snapshots.rs` – link, image, and footnote
  events.
- `src/io_tracing_tests.rs` – in-place rewrite events: `temporary file written`,
  `target replaced`, and `rewrite declined`; `replacement failed` is covered by
  a presence assertion rather than a snapshot.

Each is wired into its owning module as a `#[cfg(test)]` `#[path = "…"]`
submodule so the snapshot test sits beside the code it pins while keeping the
production module within the 400-line limit. The `.snap` fixtures live under the
neighbouring `snapshots/` directory.

A test captures events with the in-repo `test_macros::traced_test` attribute,
then normalizes the captured lines through the shared
`crate::wrap::tracing_snapshot_support::normalise_event_lines` helper before
asserting the snapshot.

#### `normalise_event_lines` helper

`tracing-test` prefixes every captured line with a volatile timestamp and span
context, which would make raw snapshots non-deterministic.
`normalise_event_lines(lines, message)` retains only the lines containing
`message`, strips the volatile prefix up to the event level (`TRACE`/`DEBUG`),
trims trailing whitespace, and joins the survivors. The level, target, message,
and structured fields are preserved verbatim, so the snapshot still fails if any
of those change.

Re-use policy for this helper:

- **Ownership.** Owned by the wrap module (`src/wrap/tracing_snapshot_support.rs`)
  and gated behind `#[cfg(test)]`; it is `pub(crate)` test-support code, not part
  of any public or runtime API.
- **Permitted call-sites.** Only tracing-event snapshot tests. Call it from
  inside a `test_macros::traced_test` function through the injected
  `logs_assert` closure, copying the normalized result into an owned buffer
  before asserting the snapshot after the closure returns (see the modules
  listed above for the canonical shape).
- **Composition.** Do not layer additional normalization on top; if a new event
  needs different masking, extend the helper (and this section) rather than
  post-processing its output at the call-site, so every snapshot shares one
  normalization contract.

## Fences module

The `fences` module in [src/fences.rs](../src/fences.rs) is responsible for
normalizing fenced code blocks before later Markdown transforms run. It exposes
two public functions that are called in sequence:

- `compress_fences(&[String]) -> Vec<String>` conditionally compresses fence
  delimiters of three or more backticks or tildes to exactly three backticks
  when doing so preserves the structural interpretation of the inner content.
  If compression would make inner fence-like content become structural, the
  original outer delimiters are preserved.
- `attach_orphan_specifiers(&[String]) -> Vec<String>` reattaches language
  specifier lines that appear on a separate line before an unlabelled opening
  fence.

Together, these helpers make the rest of the processing pipeline deal with a
single normalized fence, and avoid carrying separate logic for detached
specifier lines.

### Strategy enum

`Strategy` is the canonical private choice for fence-marker rewriting. Its
variants have the following effects:

- `Compress` rewrites the marker to exactly three backticks while preserving
  indentation and the language specifier.
- `Preserve` retains the original marker character and run length when an
  interior fence would otherwise become structural.

All fence-marker rewriting must dispatch on `Strategy` through `rewrite_marker`.
`flush_matched_block` selects the matched-block strategy, while
`rewrite_fence_line` only dispatches it and falls back to the original line.

### Architecture

The earlier implementation used two slice-and-index helpers,
`orphan_specifier_target` and `orphan_specifier_target_without_language`, to
search forward from the current position. That approach worked, but it split
the lookahead rules across multiple helpers and kept the main loop tied to
manual index management.

The current implementation converts slice traversal into a `Peekable` iterator
and centralizes the forward scan in one private helper, `attach_to_next_fence`.
`attach_orphan_specifiers` now acts as the coordinator: it identifies a
candidate orphan specifier, delegates lookahead to the helper, and otherwise
just pushes unchanged lines into the output.

### `attach_to_next_fence` semantics

`attach_to_next_fence` receives a `Peekable` iterator positioned immediately
after the current orphan specifier line.

It follows these rules:

1. Peek at the next line. If it is blank, consume it, buffer it, and continue
   scanning.
2. If the next non-blank line is a fence whose language is absent, meaning the
   language is empty or the case-insensitive string `null`, consume that fence,
   rewrite it with the normalized specifier and selected indentation, push the
   rewritten fence into the output buffer, and drop the buffered blank lines.
   This preserves the historical skip-blank-lines semantics, so no intervening
   blank lines appear in the output when attachment succeeds.
3. If the next non-blank line is not an attachable fence, stop scanning, push
   the original specifier line to the output, then extend the output with the
   buffered blank lines verbatim.
4. A thematic-break line is never treated as an attachable specifier, even
   though a run of underscores or hyphens matches the specifier pattern. The
   guard is `is_thematic_break`, whose predicate is deliberately identical to
   the one `format_breaks` uses, so a line the fences pass declines to attach
   is exactly a line `format_breaks` rewrites. The break line and any
   buffered blank lines are emitted unchanged.

This structure keeps the one non-trivial lookahead path local to the helper
instead of spreading it between the main loop and several index-based search
utilities.

### Indentation selection

`attach_specifier_to_fence` controls which indentation is retained on the
rewritten opening fence.

- The fence's own indentation is preferred by default.
- If the fence has no indentation, the specifier's indentation is used.
- If `spec_indent.starts_with(fence_indent)`, the implementation treats the
  specifier's indentation as extending, or matching, the fence's indentation
  and uses it. Equality is allowed, so `starts_with` covers both exact matches
  and deeper indentations.

This rule keeps existing fenced block indentation stable while still handling
the common case where the detached specifier line carried the indentation that
should apply to the fence.

## CLI matrix harness

The CLI matrix harness in [tests/cli_matrix.rs](../tests/cli_matrix.rs) checks
that important option combinations keep working through the real `mdtablefix`
binary. It uses `assert_cmd` to run the command and `insta` to snapshot a
labelled envelope containing the case identifier, execution mode, arguments,
exit status, stdout, stderr, and rewritten file content. Every case runs in a
fresh temporary directory with its fixture staged as `input.dat`, so a mode
that echoes the file it reports echoes the same name in every snapshot. The
`[file]` block is recorded as `<not applicable>` for the read-only modes, which
is what keeps a reporting snapshot free to claim the file was not written.

The base catalogue lives in
[tests/cli_matrix/cases.rs](../tests/cli_matrix/cases.rs). It covers the
seven non-wrap transform flags:

- `--renumber`
- `--breaks`
- `--ellipsis`
- `--fences`
- `--footnotes`
- `--code-emphasis`
- `--headings`

The harness expands every base row into both `--wrap` and no-`--wrap` variants.
It then runs each logical variant as file-to-stdout formatting and with
`--in-place` against an equivalent temporary file. The snapshot test also
asserts that stdout output and the `--in-place` rewritten file are identical
for the same logical case.

### Curated reporting coverage

`--check` and `--diff` report what the printing mode would write, so the
harness exercises them over a curated subset of base rows rather than doubling
every snapshot. A row that joins the subset declares both reporting modes and
therefore runs them in both wrap variants; the self-test
`matrix_reporting_rows_are_curated` holds that declaration to those rules, and
`matrix_cases_expand_to_their_declared_modes` checks that every logical case
runs exactly the modes it declares. The subset is `row_000` (the plain table
case), `row_010` (whose unwrapped variant is already a fixed point, so the
subset covers a file that needs no change as well as one that drifts), and
`row_111` (the frontmatter document boundary).

The reporting modes are measured against the printing mode's own output for the
same document rather than against counts written into the test. Each reporting
run must leave the file unwritten, exit `0` when the printed document already
matches the file and `1` when it does not, and report exactly the edit that
status implies: `--check` prints one `<path> +<n> -<m>` line, and `--diff`
prints a unified diff whose marked lines are those same counts.
`matrix_reporting_modes_agree` additionally requires the two modes to agree on
the counts for the same document. The diff body itself is checked by applying
its hunks to the file, copying the lines no hunk covers as a patch would, and
asserting that the result is exactly the printed document.

Matrix input fixtures live under `tests/data/cli-matrix/` and must use the
`.dat` extension. Do not use `.md` or `.txt` for these fixtures because
`make fmt` runs Markdown formatting and must not rewrite matrix inputs. The
harness has a self-test that rejects non-`.dat` fixtures, and the staging name
above is why: a reporting mode echoes the name the file was given, so only a
`.dat` fixture keeps that name stable in a snapshot.

`make typecheck` runs `cargo check --all-targets --all-features` to verify
type-correctness without running tests. Use it for rapid feedback during
development before moving on to the full lint and test gates.

Before changing snapshots, run the harness self-tests:

```bash
cargo test --test cli_matrix matrix_case_ids_are_unique
cargo test --test cli_matrix matrix_cases_expand_to_their_declared_modes
cargo test --test cli_matrix matrix_cases_expand_to_wrapped_and_unwrapped
cargo test --test cli_matrix matrix_cases_cover_all_transform_pairs
cargo test --test cli_matrix matrix_reporting_rows_are_curated
cargo test --test cli_matrix matrix_reporting_modes_agree
```

Create or update snapshots only when the behaviour change is intentional:

```bash
INSTA_UPDATE=always cargo test --test cli_matrix cli_matrix_snapshots
cargo test --test cli_matrix
```

Review the generated `tests/snapshots/cli_matrix__*.snap` files before
committing. Snapshot churn across many cases usually means the fixture is too
broad or a shared transform changed behaviour; inspect the labelled case, mode,
and arguments before accepting the new output.

## 1. Stateful pipeline helpers

Internal state carriers centralize the buffered state used by the conversion
pipeline. Each owns one slice of pipeline behaviour, so the surrounding
functions remain focused on traversal. Keep new parser and wrapping state
machines explicit unless they meet the adoption threshold in
[ADR 0004](adrs/0004-state-machine-abstractions.md). That decision records the
crate research and the local pattern maintainers should follow when changing
stateful helpers.

### 1.1. State-machine adoption checklist

Keep an explicit Rust struct, enum, and event-shaped helper unless every item
below is true. If every item is true, build a small comparison spike before
proposing a dependency:

- the transition graph is larger or more important than its Markdown parsing
  predicates;
- explicit events, rather than source-line predicates, drive transitions;
- lifecycle hooks, recoverable transition errors, or introspection would
  otherwise be duplicated across modules; and
- the spike demonstrates that generated or framework-driven code is shorter
  and more legible than equivalent explicit Rust.

Evaluate `statig` first for a dynamic event-driven machine and `smlang` second
for a compact transition table. Record graph size, event mapping, lifecycle and
error semantics, observability, generated-code legibility, and dependency
adoption risk in an ADR update before adding a crate.

### 1.2. Wrapping continuation state (`src/wrap/paragraph.rs`)

`ParagraphState` owns buffered prose, shared indentation, remembered
continuation indentation, and an optional `PendingPrefix`. `PendingPrefix`
carries the prefix, source lines, synthetic join offsets, available width, and
open-fence metadata while an inline-code span crosses source lines.
`ContinuationMode` selects normalized, tight, or verbatim emission, while
`TailReflow` decides whether prose after a resolved span may remain buffered.
The private `wrap::paragraph::spanning_code` helper preserves a conforming
authored boundary when joining its overlong span would exceed the width; it
restores stored indentation and returns prose outside that span to normal
wrapping. Emit stable `trace!` fields at verbatim-preservation,
prefix-mismatch, and tail-reflow transitions so maintainers can inspect why
output changed.

### 1.3. `HtmlTableState` (`src/html.rs`)

`HtmlTableState` buffers the lines belonging to an HTML `<table>…</table>`
block and tracks the current nesting depth. `in_html()` returns `true` whenever
the buffer is non-empty, so the caller knows a table is still being accumulated.
`push_html_line` appends the supplied line, increments `depth` once for every
`<table>` start tag found on the trimmed line, and decrements it once for every
`</table>` end tag on the same trimmed line. When `depth` returns to zero, the
buffered lines are converted by `table_lines_to_markdown` and the buffer is
cleared. `flush_raw` exists for the fenced-block escape path: it emits the
buffered lines verbatim without conversion, so raw HTML inside a fenced code
block is preserved unchanged.

### 1.4. `DefinitionScanState` (`src/footnotes/renumber/definitions.rs`)

`DefinitionScanState` accumulates the footnote-definition rewrite plan during a
single scan over the input. It borrows the shared `(original → new)` mapping
and the `next_number` counter so renumbering decisions stay consistent with
explicit reference rewrites. Explicit `[^n]:` headers are appended to
`definitions` as soon as they are encountered, producing a `DefinitionLine` per
header in scan order. Ordered-list items that look like candidate footnote
definitions are buffered as `NumericCandidate` entries during the scan and
finalized at the end via `finalize_numeric_candidates`, which drains the buffer
in reverse, so the assigned numbers reflect bottom-up ordering rather than the
order in which the candidates were discovered.

### 1.5. `ListState` (`src/lists.rs`)

`ListState` maintains an indent stack and a per-indent counter map for
ordered-list renumbering. `next_number(indent)` first prunes indent levels
deeper than `indent` (their counters disappear so a future deeper level
restarts at 1), pushes `indent` onto the stack if it is new, and returns the
next sequential number for that level — incrementing the counter, so the next
call at the same indent receives the following integer. `reset()` clears both
the stack and the counter map; the renumbering pass invokes it when a heading
or thematic break is encountered, so the next list starts numbering from 1
again.

## 2. Test infrastructure

### 2.1. `tests/support/` module

Integration-test helpers are organized under `tests/support/`:

Table: Integration-test support modules and their purposes.

| Module                   | Purpose                                                               |
| ------------------------ | --------------------------------------------------------------------- |
| `cli_args.rs`            | `run_cli_with_args` — invokes the binary with argument-only tests     |
| `cli_stdin.rs`           | `run_cli_with_stdin` — invokes the binary feeding stdin               |
| `fixtures.rs`            | Shared rstest fixtures (e.g. `broken_table`)                          |
| `wrap_assertions.rs`     | Higher-level assertions for wrapping output                           |
| `idempotence_harness.rs` | Shared proptest generators and CLI harness for the idempotence suites |

Each integration-test file declares the modules it needs via explicit
`#[path = "support/…"]` attributes, keeping inter-test coupling minimal.

### 2.2. Exported test macros (`tests/common/mod.rs`)

`tests/common/mod.rs` exports two `#[macro_export]` macros available to all
integration-test crates:

Table: Macros for building `Vec<String>` from literals and file lines.

| Macro                    | Purpose                                         |
| ------------------------ | ----------------------------------------------- |
| `lines_vec![…]`          | Builds a `Vec<String>` from string-like values. |
| `include_lines!("path")` | Builds a `Vec<String>` from file lines.         |

`lines_vec![…]` reduces boilerplate when constructing fixture inputs.
`include_lines!("path")` uses `include_str!` at compile time and returns one
`String` per line of the referenced file.

Both macros are exported rather than kept private because Rust's macro scoping
rules require `#[macro_export]` for macros to be visible across
integration-test binary crates. The `#[expect(unused_macros)]` suppressions
that previously guarded them were replaced by the export attribute when it
became clear that multiple test binaries depend on them.

### 2.3. `test-macros` crate

The `test-macros` workspace crate provides the `allow_fixture_expansion_lints`
proc-macro attribute. It suppresses the `unused_braces` lint that `rstest`
fixture expansion triggers when `fn_single_line = true` is set in
`rustfmt.toml`.

The macro emits `#[allow(unused_braces, …)]` rather than `#[expect(…)]` because
the Rust proc-macro API delivers a pre-parsed token stream; the emitted lint
attribute applies to code that the compiler has not yet expanded, making
`#[expect]` semantically unusable at that site. This is a known consequence of
the `rstest` fixture expansion and is not a lint-integrity violation.

Apply it to any fixture function whose single-expression body triggers the lint:

```rust
#[test_macros::allow_fixture_expansion_lints]
#[rstest::fixture]
pub fn broken_table() -> Vec<String> { … }
```

The same crate provides `test_macros::traced_test`, the attribute all traced
tests in this repository use in place of `tracing_test::traced_test`. The
wrapper exists because `tracing-test` installs its global subscriber lazily,
from whichever traced test the harness reaches first. `tracing` decides once,
when a callsite is first used, whether that callsite can ever be dispatched; a
callsite first used before that install finds no subscriber to ask and caches
`Interest::never()` for the life of the process, because installing a global
subscriber does not recompute the cache. The callsite then stays silent, so a
test that asserts on its own log lines fails intermittently, on a schedule set
by which other tests the harness happens to run alongside it.

The wrapper prepends `::tracing::callsite::rebuild_interest_cache();` to the
function body and re-emits `#[::tracing_test::traced_test]`. `tracing-test`
prepends its own initialization to whatever body it is given, so the rebuild
always runs after the install. The ordering is therefore structural rather
than dependent on the test author writing calls in the right order. The full
rationale is in the macro's doc comment in `test-macros/src/lib.rs`.

Apply it to any test that asserts on its own log lines:

```rust
#[test_macros::traced_test]
#[test]
fn a_traced_case() { … }
```

### 2.4. Inline unit-test modules

AGENTS.md caps a source file at 400 lines, so a `#[cfg(test)] mod tests` block
that pushes its production module over the limit moves into a sibling file
wired back in with `#[path]`:

```rust
#[cfg(test)]
#[path = "io_tests.rs"]
mod tests;
```

`src/headings.rs`, `src/io.rs`, and `src/main.rs` use this shape, as do the
tracing-snapshot modules listed under
[Tracing-event snapshot tests](#tracing-event-snapshot-tests). The
moved tests keep their original paths (`io::tests::…`), and `super` still
resolves to the owning module, so unqualified access to its items is unchanged.

### 2.5. Platform portability of the test suite

Every tracked file is LF in the repository and checks out as LF on every
platform, because `.gitattributes` pins `* text=auto eol=lf`. The CLI suites
compare fixture and snapshot bytes against output the tool writes with `\n`, so
a CRLF checkout — the default for Git for Windows — would fail those
comparisons for reasons unrelated to the change under test.

Snapshot content has to be platform-independent as well. The CLI matrix
envelope records a process-result *value*, not diagnostic wording:
`tests/cli_matrix/support.rs` renders `ExitStatus` through the private
`status_text` helper, which reports `code: 0`, `code: <n>`, or `no exit code`
for a process that was killed by a signal. `ExitStatus`'s own `Display` is not
portable — an ordinary exit reads `exit status: 0` on Unix and `exit code: 0`
on Windows — so snapshotting it directly would make every envelope a
Windows-only failure. The committed snapshots under `tests/snapshots/`
therefore carry `status: code: 0`, or `status: code: 1` where a reporting mode
is asked about a file that drifts.

`tests/static_regex_lint.rs` is gated whole-file with `#![cfg(unix)]`. The guard
it drives is a `bash` script that shells out to ripgrep, and the tests stand in
for ripgrep with stub scripts that have to carry the executable bit; on Windows
the target compiles to an empty binary rather than failing. Nothing goes
unguarded on that account: the Linux lint job runs the same script over the same
sources through the `check-static-regexes` Makefile target.

### 2.6. Behaviour-driven scenario tests

`rstest-bdd` features live in `tests/features/`. The scenario bindings are in
`tests/bdd_reporting.rs`, which is its own test binary, and the step
definitions are in `tests/steps/reporting.rs`, declared by the bindings as a
`#[path]` submodule.

Conventions:

- The step definitions must be declared before the bindings. The step registry
  is populated as macros expand, so a binding that expanded first would not
  yet see them, and `strict-compile-time-validation` would report every step
  as missing.
- Both feature files share one set of steps because they describe one analysis
  with two renderings. A step that differed between them would be exactly the
  place the two modes could silently diverge.
- Each scenario has its own `ReportingState` fixture, built by the `state`
  fixture in `tests/bdd_reporting.rs` and injected through `#[from(state)]`.
  The state holds `Slot` fields, so a step borrows the whole state immutably
  and fills one slot, which is what lets `Given`, `When`, and `Then` share
  data without a mutable borrow crossing a step boundary. The state covers the
  scenario's temporary directory, the files as named in argument order, and the
  directory fingerprint from before a run through to the most recent `Run`, or
  the outputs when a scenario repeats the run.
- Every step drives the real binary through `assert_cmd`'s
  `Command::cargo_bin`, so the features specify the command-line contract
  rather than a reimplementation of it. There is no external fixture-file
  directory: the steps create their fixtures on demand with `std::fs::write`
  inside a `tempfile::tempdir()`, writing the ragged or already-formatted
  content and recording the name in argument order. The read-only scenarios
  capture a fingerprint of entry names, bytes, and modification times before
  the run and compare it afterwards.
- [docs/rstest-bdd-users-guide.md](rstest-bdd-users-guide.md) is vendored in
  this repository. It records the framework conventions and the
  `strict-compile-time-validation` feature that
  `rstest-bdd-macros` is pinned with in `Cargo.toml`.

### 2.7. Build and test requirements

`make test` is two `cargo test` invocations, both with `RUSTFLAGS` set so that
warnings are errors:

Table: The `make test` recipe, verbatim from the Makefile.

| Step | Command                                                           |
| ---- | ----------------------------------------------------------------- |
| 1    | `RUSTFLAGS="-D warnings" cargo test --all-targets --all-features` |
| 2    | `RUSTFLAGS="-D warnings" cargo test --doc --all-features`         |

The first step compiles and runs every test target — the unit tests in `src/`,
the integration binaries under `tests/`, and the compile fixtures driven by
`tests/compile.rs` — with every feature enabled. The second is not redundant:
documentation tests are not part of `--all-targets`, so `--doc` is the only
invocation that compiles and runs the examples in doc comments, including those
on `mdtablefix::report` and `mdtablefix::io::SourceDocument`. Because both
carry `-D warnings`, a warning raised while compiling a test target or a
doctest fails the gate rather than scrolling past.

The other two commit gates are `make check-fmt` (`cargo fmt --all -- --check`)
and `make lint` (`cargo clippy --all-targets --all-features -- -D warnings`).
All three run before a commit. `make markdownlint` covers the documentation
changes that none of the Rust gates see.

#### `make mutants`

`make mutants` runs mutation testing over the selection tree with
`cargo-mutants`, which is not a manifest dependency: install it as a Cargo
subcommand before the target can run, for example with
`cargo install cargo-mutants`. The run reads `.cargo/mutants.toml` from this
checkout:

- `examine_globs = ["src/select/**"]` restricts mutation to the selection
  tree, which the configuration chooses because its correctness rests on
  properties rather than on a transcript.
- `all_features = true` builds each mutant as `make test` does, with every
  feature enabled. The crate declares no features today; the setting means a
  feature added later cannot quietly narrow the run.
- No `additional_cargo_test_args` is set, so the whole suite is the oracle,
  exactly as `make test` runs it.
- `output = "target"` puts the results in `target/mutants.out`, under the
  already-ignored `target/` directory rather than at the repository root.
- `copy_target = false` rebuilds the scratch tree from source rather than
  copying the `target/` directory, which the configuration records as 15 GB
  here. The scratch tree is reused within a run, so the cold dependency build
  is paid once rather than once per mutant.

Two Makefile variables can be overridden on the command line:

- `MUTANTS_JOBS`, default `3`, is passed to `cargo-mutants` as `-j`.
- `MUTANTS_TMPDIR`, default
  `$(HOME)/.cache/mdtablefix/mutants/$(notdir $(CURDIR))`, is created and
  exported as `TMPDIR` for the run. The default is absolute and outside the
  tree under test, and it names the worktree so two runs cannot collide:
  `cargo-mutants`' child processes run inside the scratch copy, where a
  relative path would not resolve, and a suite whose temporary directories
  landed inside this repository would fail the `--git` scenarios that assert
  on being outside one.

The tool reports each mutant as caught, missed, or unviable. A `caught` mutant
is one that made the suite fail, which is the wanted outcome. A `missed` mutant
compiled and survived the suite: the tests do not observe the behaviour the
mutation changed, and the mutant is listed in
`target/mutants.out/missed.txt`. An `unviable` mutant did not build, so no
test could have caught it; the tool counts those apart from the survivors
rather than among them. The selection tree's acceptance criterion is an empty
`missed.txt`: a surviving mutant in `src/select/**` is a gate failure, not a
warning.

#### `similar`

`similar` is a runtime dependency of the library, not a dev-dependency. It
serves the reporting domain in `src/report/`: `LineDelta::between` counts
insertions and deletions by iterating `TextDiff::from_lines`, and
`write_unified_diff` renders a unified diff from a `TextDiff::configure()`
value, choosing between Myers and Patience from the `DiffOptions` the caller
supplied. The requirement is `similar = "2.7"` — a caret requirement, so the
resolved version is at least 2.7 and below 3.0 — and both call sites are
written against the 2.x API: the `ChangeTag` variants, the configured algorithm
selection, and the `iter_all_changes` iterator. Widening the requirement to a
3.x line means re-checking both.

`DiffOptions` fields are this crate's own policy rather than `similar`'s. The
context radius is fixed at three lines, matching `git diff`, and the patience
threshold switches algorithm above a line count so that diffing a very large
file stays bounded without a wall-clock cut-off.

## 3. Breaks module – Cow allocation strategy

`format_breaks` in [src/breaks.rs](../src/breaks.rs) returns
`Vec<Cow<'_, str>>` so unchanged lines can be forwarded without allocating.
Lines that do not match a thematic break are emitted as `Cow::Borrowed` slices
into the input `&[String]`. Synthesized thematic-break lines are also emitted as
`Cow::Borrowed`, pointing to the shared `LazyLock<String>` static
`THEMATIC_BREAK_LINE`. Callers that need owned `String` values must call
`.into_owned()` on each item.
