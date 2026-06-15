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

### Implications for internal code organization

The package binary in [src/main.rs](../src/main.rs) is compiled as a separate
crate from the library, so it cannot use library items marked `pub(crate)`. To
preserve CLI access without reopening the library surface, the binary includes
[src/frontmatter.rs](../src/frontmatter.rs) privately via:

```rust
#[path = "frontmatter.rs"]
mod frontmatter;
```

This arrangement keeps the helper available to both the library and the CLI
while maintaining a closed external API boundary.

When working in this area:

- Prefer wiring new behaviour through `process_stream_inner` or other public
  formatting APIs instead of exporting frontmatter helpers.
- Treat changes to frontmatter parsing rules as internal architectural changes
  that should update this guide and any affected behaviour documentation.
- Keep module documentation in sync in both `src/frontmatter.rs` and the
  private module declaration used by the binary.

## Table reflow architecture

The table reflow pipeline is split into small stages so continuation rows and
separator rows can be handled without losing column structure.

`protect_leading_empty_cells` rewrites leading empty continuation cells to a
marker before parsing. `parse_rows` then performs the sentinel-based split that
turns the buffered table text into row vectors. `clean_rows` restores the
markers to empty strings and removes rows that are entirely empty.

`calculate_widths` measures each column using Unicode display width so the
formatter sizes columns according to the glyphs that will actually be emitted.
`format_rows` applies escaping and padding to each cell, and `insert_separator`
restores the separator row with widths derived from the final table body.

## Internal API reference

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

`src/footnotes/renumber/reorder.rs`:

- `reorder_definition_block`: Reorders the final footnote-definition block
  according to the numbering plan built by the sibling `definitions` module. It
  keeps continuation lines attached to their definition, preserves block
  prefixes, migrates leading separator blanks at the first segment boundary,
  and skips mutation with a warning if the composed block would change row
  count.

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

1. **Block classification.** `classify_block` in `src/wrap/block.rs` inspects
   each input line and decides whether it should pass through verbatim or enter
   the paragraph wrapper. `wrap_text` injects a shared [`LinkReferenceMatcher`]
   into each call. Fenced code blocks, indented code blocks, headings, tables,
   directives, link reference definitions, and blank lines stop paragraph
   accumulation.

2. **Prefix-aware paragraph handling.** `ParagraphWriter` in
   `src/wrap/paragraph.rs` is the single entry point for prefix-aware wrapping.
   `wrap_with_prefix` computes the available content width once from the
   Unicode display width of the first-line prefix, then feeds the paragraph
   text into `wrap_preserving_code`.

   **Pending prefix deferral.** When `handle_prefix_line` processes a line
   whose text contains an unclosed inline code span (checked by
   `has_unclosed_code_span`), it clears the current paragraph buffer and saves
   the prefix, rest text, available width, `repeat_prefix`, and `hard_break`
   flag into `ParagraphState::pending_prefix` as a `PendingPrefix` value rather
   than wrapping immediately. `PendingPrefix` also records original source
   lines for ambiguity-preserving passthrough, and `synthetic_join_spaces`
   stores byte offsets for spaces inserted by continuation joining so only
   formatter-created code-span edge spaces are trimmed later. Subsequent source
   lines are routed through `handle_pending_continuation` (in `src/wrap.rs`)
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
   continuation indent. If the opener is at or near the end of its source line,
   `PendingPrefix` marks subsequent continuations as verbatim, so joining does
   not create leading or trailing spaces inside the code span. When the
   projected join would exceed the available content width, the pending line
   and continuation are emitted verbatim rather than joined into a
   Markdownlint-invalid overlong line. When the scanner reports no open span
   and no close/reopen boundary exists, `flush_paragraph` emits the buffered
   segment atomically using `append_wrapped_with_prefix_width`. The exception is
   `ContinuationMode::VerbatimFlush`: when the scanner sees a closing fence
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
   and removes only spaces whose byte offsets appear in `synthetic_join_spaces`.

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

Explicit state for standalone title continuation in `wrap_text`. Starts
`Closed`. After a bare link reference definition, `observe_bare_definition()`
opens `AwaitingStandaloneTitle`. While open, `observe_next_line(line, matcher)`
returns `Some(EmitVerbatim)` for blank or title lines (and closes the window),
or `Some(Reprocess)` when the line is ordinary prose (closing the window, so
the caller reflows it).

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
| Symbol                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | File                              |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------- |
| `LinkReferenceMatcher`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `src/wrap/link_reference.rs`      |
| `LinkTitleWindow`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | `src/wrap/link_reference.rs`      |
| `classify_block`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `src/wrap/block.rs`               |
| `FragmentKind`, `InlineFragment`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `src/wrap/inline/fragment.rs`     |
| `classify_fragment`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `src/wrap/inline/fragment.rs`     |
| Character and fragment predicates (`is_inline_code_token`, `looks_like_link`, `looks_like_footnote_ref`, `is_month_name`, `is_ordinal_day`, `is_numeric_day`, `is_year`, …)                                                                                                                                                                                                                                                                                                                                                                                                                                  | `src/wrap/inline/predicates.rs`   |
| `SpanKind`, span grouping helpers (`merge_code_span`, `try_couple_footnote_reference`, `try_match_date_sequence`, …)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | `src/wrap/inline/span_helpers.rs` |
| `try_couple_inline_link_after_opener`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | `src/wrap/inline/span_helpers.rs` |
| `normalize_footnote_ref_spacing`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `src/wrap/inline/normalize.rs`    |
| `build_fragments`, `wrap_preserving_code`, `render_line`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `src/wrap/inline.rs`              |
| `determine_token_span`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `src/wrap/inline.rs`              |
| `merge_whitespace_only_lines`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | `src/wrap/inline/postprocess.rs`  |
| `rebalance_atomic_tails`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `src/wrap/inline/postprocess.rs`  |
| `ParagraphWriter`, `wrap_with_prefix`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | `src/wrap/paragraph.rs`           |
| `ParagraphState`, `PrefixLine`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | `src/wrap/paragraph.rs`           |
| `PendingPrefix`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | `src/wrap/paragraph.rs`           |
| `emit_pending_with_verbatim_continuation` — Emits a pending prefix plus raw continuation for ambiguous inline-code source.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | `src/wrap/paragraph.rs`           |
| `drain_pending_prefix` — Takes the deferred prefixed segment and clears plain paragraph buffers before final emission.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `src/wrap/paragraph.rs`           |
| `pending_prefix_for_next_segment` — Selects the original pending prefix for the first deferred segment, then the continuation indent for later segments.                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `src/wrap/paragraph.rs`           |
| `apply_continuation_chunk` — Centralized join/update/dispatch entry point that reconciles a single continuation chunk with the active `PendingPrefix` buffer.                                                                                                                                                                                                                                                                                                                                                                                                                                                | `src/wrap/continuation.rs`        |
| `join_pending_continuation`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | `src/wrap/continuation.rs`        |
| `starts_inline_citation`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `src/wrap/tokenize/mod.rs`        |
| `opening_fence_run_len` — Measures the length of an unescaped backtick run at the start of a byte slice; used to identify opening code-span fences.                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `src/wrap/tokenize/scanning.rs`   |
| `position_after_close(text, search_start, fence_len)` — Walks `text` from absolute byte offset `search_start` to find the first closing backtick fence of exactly `fence_len` characters. Escaped closing candidates (backtick runs preceded by an odd number of backslashes) are recorded but skipped; a literal (unescaped) closing fence is returned unless an iterative forward look-ahead confirms that fence is itself the opener of a subsequent balanced span, in which case the earlier escaped candidate is returned instead. Returns `None` when no matching close exists or `fence_len` is zero. | `src/wrap/tokenize/scanning.rs`   |
| `scan_continuation_span_state` — Incrementally scans a continuation string given a known open fence length, returning the remaining open fence length or `None` when all spans are balanced; used to avoid O(N²) rescanning of the accumulated pending text.                                                                                                                                                                                                                                                                                                                                                 | `src/wrap/tokenize/scanning.rs`   |
| `handle_backtick_fence` — Tokenizes an inline code span from the opening fence byte offset and delegates closing-fence detection to `position_after_close`.                                                                                                                                                                                                                                                                                                                                                                                                                                                  | `src/wrap/tokenize/parsing.rs`    |
| `handle_pending_continuation`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | `src/wrap.rs`                     |
| `scan_code_suffix_end`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `src/wrap/tokenize/scanning.rs`   |
| `has_inline_code_structure`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | `src/wrap/inline/fragment.rs`     |
<!-- markdownlint-enable MD013 MD055 MD056 MD060 -->

`ContinuationMode` in `src/wrap/paragraph.rs` selects normal joining,
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
- **Atomic fragments.** Inline code spans, Markdown links, and GFM footnote
  references are never split across lines; they move as a unit when they would
  overflow the target width. Opening punctuation that immediately precedes an
  inline code span or link is grouped with that span during token grouping so
  the opener is not left on the previous line. Trailing punctuation after those
  spans follows the same grouping rules. GFM footnote references that
  immediately follow inline code or link spans without intervening whitespace
  are coupled to the preceding punctuation cluster, so the marker is not
  wrapped onto the next line alone. Inflectional affixes (`s`, `'s`, `ed`,
  `ing`) and hyphenated compounds that immediately follow a closed backtick
  fence are absorbed into the code token by `scan_code_suffix_end` in
  `src/wrap/tokenize/scanning.rs`; the combined token is recognized as atomic by
  `has_inline_code_structure` in `src/wrap/inline/fragment.rs`, so wrapping
  treats the full string as one unit. Leading-hyphen compounds — a token that
  ends with a hyphen and contains at least one alphabetic character (for example
  `pre-`, `LLM-`, `(API-`) — are coupled forward to the next inline code span
  during span grouping by the `ends_with_hyphen_prefix` predicate in
  `src/wrap/inline/predicates.rs`, applied in `determine_token_span` in
  `src/wrap/inline.rs`. The coupling mirrors the existing opening-punctuation
  pattern, so compounds such as `` pre-`LLMPort` `` and `` (API-`Foo`) ``
  remain atomic during wrapping. Internal hyphen chains (e.g.
  `state-of-the-art-`) are accepted by design; bare dash runs such as `-` or
  `---` are rejected. Unicode alphabetic characters (e.g. `pré-`, `字-`) are
  intentionally supported.
- **Hard breaks.** Trailing two-space hard breaks must survive on the emitted
  line where they occur.
- **Verbatim blocks.** Fenced code blocks must pass through unchanged, along
  with the other non-paragraph block kinds detected by `classify_block`.
- **Prefix width.** The visual width of every prefix string is measured with
  `UnicodeWidthStr::width` before the available text width is computed, so
  non-ASCII prefix characters (e.g. `「` in CJK blockquotes) are accounted for
  correctly.
- **Cross-line code spans.** When a prefixed line contains an unclosed inline
  code span, the entire continuation is buffered in `PendingPrefix` and emitted
  atomically once the span closes. No line break may be inserted inside the
  span, and the closing backtick must remain on the same line as the span
  content.
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

`tracing = "0.1"` is the runtime observability dependency, used by both the
library and executables. `tracing-test = "0.2"` is a test-only dev-dependency;
use it only in tests (e.g. `#[traced_test]`). The crate does not install a
global subscriber or metrics recorder. Executables and test harnesses that want
log output must install their own subscriber (e.g.
`tracing_subscriber::fmt::init()` in `main`).

### Log levels

Use `debug!` for high-value classification outcomes: fragment kind, parsed
token, span promotion result. Use `trace!` for branch-level checks: predicate
matched, prefix mismatch, unterminated bracket. Never emit at `info!` or above
from library code.

### Field naming

Use the stable structured field names `token`, `kind`, `start`, `end`, `width`,
`truncated`, `reason`, and `is_image`.

Table: Structured field names emitted by tracing instrumentation.

| Field       | Type            | Used in                         | Meaning                                                         |
| ----------- | --------------- | ------------------------------- | --------------------------------------------------------------- |
| `token`     | `%str`          | fragment, link, footnote events | The text slice that was classified or parsed                    |
| `kind`      | `?FragmentKind` | `fragment classified`           | The computed fragment classification                            |
| `start`     | `usize`         | span events                     | Byte offset where the span begins                               |
| `end`       | `usize`         | span events                     | Byte offset where the span ends (exclusive)                     |
| `width`     | `usize`         | span events                     | Display-column width of the span                                |
| `truncated` | `bool`          | `fragment classified`           | Whether `token` was shortened to <= 80 bytes                    |
| `reason`    | `&str`          | `footnote end not found`        | Diagnostic tag: `"prefix_mismatch"` or `"unterminated_bracket"` |
| `is_image`  | `bool`          | `link or image parsed`          | `true` when the link token is an image literal (`![]()`)        |

For example:

```rust
debug!(token = %token, kind = ?kind, "fragment classified");
```

### Performance discipline

Guard any expression that allocates (e.g. `String` truncation) with
`tracing::enabled!(Level::DEBUG)` or `tracing::enabled!(Level::TRACE)` before
computing the value.

### Security considerations

The `token` field records raw text slices from the input document, including
URL tokens parsed by `parse_link_or_image`. URLs may embed API keys, session
identifiers, or other sensitive values.

When enabling DEBUG or TRACE logging from this library in a production
environment, configure the subscriber to redact or drop the `token` field
before writing to any persistent sink. For example, with `tracing-subscriber`:

```rust
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

tracing_subscriber::registry()
    .with(
        tracing_subscriber::fmt::layer()
            .with_span_events(FmtSpan::NONE)
            // Add a field-filtering layer here to drop `token` in production.
    )
    .init();
```

Do not enable DEBUG or TRACE logging from this library without a redacting
subscriber in any environment where the input documents may contain
confidential URLs.

### Instrumented functions

Functions decorated with `#[tracing::instrument]` are listed below with their
level and notable fields. Update this list when adding new instrumented entry
points.

Table: Instrumented functions and their logging levels and fields.

| Function                  | Level        | Fields                                                                                            |
| ------------------------- | ------------ | ------------------------------------------------------------------------------------------------- |
| `looks_like_footnote_ref` | trace        | `token` (in), return value (out)                                                                  |
| `ends_with_footnote_ref`  | trace        | `token` (in), return value (out)                                                                  |
| `ends_with_hyphen_prefix` | trace        | `token` (in), return value (out)                                                                  |
| `is_month_name`           | trace        | `token` (in), return value (out)                                                                  |
| `is_ordinal_day`          | trace        | `token` (in), return value (out)                                                                  |
| `is_numeric_day`          | trace        | `token` (in), return value (out)                                                                  |
| `is_year`                 | trace        | `token` (in), return value (out)                                                                  |
| `try_match_date_sequence` | trace, debug | `start` (in), `skip(tokens)`, return value (out); matched date pattern                            |
| `date_token_span`         | trace        | `start` (in), `skip(tokens)`, return value (out); over-width date fallback remains behaviour-only |
| `parse_link_or_image`     | debug        | `idx` (in), `skip(text)`, return value (out)                                                      |
| `find_footnote_end`       | trace        | `idx` (in), `skip(text)`, return value (out)                                                      |

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
exit status, stdout, stderr, and rewritten file content where relevant.

The base catalogue lives in
[tests/cli_matrix/support.rs](../tests/cli_matrix/support.rs). It covers the
seven non-wrap transform flags:

- `--renumber`
- `--breaks`
- `--ellipsis`
- `--fences`
- `--footnotes`
- `--code-emphasis`
- `--headings`

The harness expands every base row into both `--wrap` and no-`--wrap` variants.
It then runs each logical variant twice: once as file-to-stdout formatting and
once with `--in-place` against an equivalent temporary file. The snapshot test
also asserts that stdout output and the `--in-place` rewritten file are
identical for the same logical case.

Matrix input fixtures live under `tests/data/cli-matrix/` and must use the
`.dat` extension. Do not use `.md` or `.txt` for these fixtures because
`make fmt` runs Markdown formatting and must not rewrite matrix inputs. The
harness has a self-test that rejects non-`.dat` fixtures.

`make typecheck` runs `cargo check --all-targets --all-features` to verify
type-correctness without running tests. Use it for rapid feedback during
development before moving on to the full lint and test gates.

Before changing snapshots, run the harness self-tests:

```bash
cargo test --test cli_matrix matrix_case_ids_are_unique
cargo test --test cli_matrix matrix_cases_expand_to_stdout_and_in_place
cargo test --test cli_matrix matrix_cases_expand_to_wrapped_and_unwrapped
cargo test --test cli_matrix matrix_cases_cover_all_transform_pairs
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

## Stateful pipeline helpers

Three internal types centralize the buffered state used by the conversion
pipeline. Each owns one slice of pipeline behaviour, so the surrounding
functions remain focused on traversal. Keep new parser and wrapping state
machines explicit unless they meet the adoption threshold in
[ADR 0004](adrs/0004-state-machine-abstractions.md). That decision records the
crate research and the local pattern maintainers should follow when changing
stateful helpers.

### `HtmlTableState` (`src/html.rs`)

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

### `DefinitionScanState` (`src/footnotes/renumber/definitions.rs`)

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

### `ListState` (`src/lists.rs`)

`ListState` maintains an indent stack and a per-indent counter map for
ordered-list renumbering. `next_number(indent)` first prunes indent levels
deeper than `indent` (their counters disappear so a future deeper level
restarts at 1), pushes `indent` onto the stack if it is new, and returns the
next sequential number for that level — incrementing the counter, so the next
call at the same indent receives the following integer. `reset()` clears both
the stack and the counter map; the renumbering pass invokes it when a heading
or thematic break is encountered, so the next list starts numbering from 1
again.

## Test infrastructure

### `tests/support/` module

Integration-test helpers are organized under `tests/support/`:

Table: Integration-test support modules and their purposes.

| Module               | Purpose                                                           |
| -------------------- | ----------------------------------------------------------------- |
| `cli_args.rs`        | `run_cli_with_args` — invokes the binary with argument-only tests |
| `cli_stdin.rs`       | `run_cli_with_stdin` — invokes the binary feeding stdin           |
| `fixtures.rs`        | Shared rstest fixtures (e.g. `broken_table`)                      |
| `wrap_assertions.rs` | Higher-level assertions for wrapping output                       |

Each integration-test file declares the modules it needs via explicit
`#[path = "support/…"]` attributes, keeping inter-test coupling minimal.

### Exported test macros (`tests/common/mod.rs`)

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

### `test-macros` crate

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

## Breaks module – Cow allocation strategy

`format_breaks` in [src/breaks.rs](../src/breaks.rs) returns
`Vec<Cow<'_, str>>` so unchanged lines can be forwarded without allocating.
Lines that do not match a thematic break are emitted as `Cow::Borrowed` slices
into the input `&[String]`. Synthesized thematic-break lines are also emitted as
`Cow::Borrowed`, pointing to the shared `LazyLock<String>` static
`THEMATIC_BREAK_LINE`. Callers that need owned `String` values must call
`.into_owned()` on each item.
