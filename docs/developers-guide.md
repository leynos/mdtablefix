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

## Fence normalisation module

`src/fences.rs` exposes the preprocessing helpers used by the `--fences` option.

- `compress_fences(lines: &[String]) -> Vec<String>` performs
  `FenceTracker`-driven conditional rewriting. For each matched fenced block,
  it determines whether compressing the outer delimiter would make a
  same-marker inner fence line structural. If so, it preserves the original
  outer delimiter width and marker family. Unmatched or malformed delimiter
  runs fall through the legacy stateless normalisation path.
- `attach_orphan_specifiers(lines: &[String]) -> Vec<String>` attaches a lone
  language identifier line to the following unlabelled fence, but only when the
  scanner is outside any active fenced block. It uses `FenceTracker` to skip
  attachment for specifier-like lines and target fences that appear inside an
  open block.

Both functions reuse `FenceTracker` from
[src/wrap/fence.rs](../src/wrap/fence.rs) for structural fence detection. This
keeps preprocessing semantics consistent with the wrapping pipeline. See
[docs/architecture.md](architecture.md) for the processing pipeline context.

## Design decisions

The rationale for the staged table reflow pipeline is recorded in
`docs/adrs/0001-table-reflow-pipeline.md`. Refer to that ADR when changing the
parse, width-calculation, or separator-handling flow so implementation changes
stay aligned with the documented design constraints.

## Wrap module architecture

The wrapping pipeline for `--wrap` is:

1. **Block classification.** `classify_block` in `src/wrap.rs` inspects each
   input line and decides whether it should pass through verbatim or enter the
   paragraph wrapper. Fenced code blocks, indented code blocks, headings,
   tables, directives, and blank lines stop paragraph accumulation.

2. **Prefix-aware paragraph handling.** `ParagraphWriter` in
   `src/wrap/paragraph.rs` is the single entry point for prefix-aware wrapping.
   `wrap_with_prefix` computes the available content width once from the
   Unicode display width of the first-line prefix, then feeds the paragraph
   text into `wrap_preserving_code`.

3. **Fragment construction and line fitting.** `wrap_preserving_code` in
   `src/wrap/inline.rs` tokenizes prose with `tokenize::segment_inline`, groups
   the tokens into `InlineFragment` values, and calls
   `textwrap::wrap_algorithms::wrap_first_fit` over the accumulated fragment
   buffer.

4. **Post-processing and rendering.** The `postprocess` module applies
   `merge_whitespace_only_lines` and then `rebalance_atomic_tails` so
   whitespace-only wrap artefacts and isolated tails are normalized before the
   fragments are rendered back into output lines.

`InlineFragment` carries the rendered fragment text, its precomputed display
width, and a `FragmentKind` tag. That construction-time classification lets the
`is_whitespace`, `is_atomic`, and `is_plain` predicates answer all later
questions without repeating ad hoc string inspection in the post-processing
passes.

The `postprocess` module exists because greedy line fitting alone does not
reproduce the repository's historical whitespace semantics. The first pass
merges whitespace-only wrap lines into adjacent content, and the second pass
rebalances a trailing atomic or plain fragment only when the destination line
still fits within the configured width.

### Key types and functions

Table: Key types and functions.

| Symbol                                                  | File                             |
| ------------------------------------------------------- | -------------------------------- |
| `FragmentKind`, `InlineFragment`, `classify_fragment`   | `src/wrap/inline.rs`             |
| `build_fragments`, `wrap_preserving_code`               | `src/wrap/inline.rs`             |
| `merge_whitespace_only_lines`, `rebalance_atomic_tails` | `src/wrap/inline/postprocess.rs` |
| `ParagraphWriter`, `wrap_with_prefix`                   | `src/wrap/paragraph.rs`          |
| `ParagraphState`, `PrefixLine`                          | `src/wrap/paragraph.rs`          |

### Design constraints

- **Public API stability.** `mdtablefix::wrap::wrap_text`, `Token`, and
  `tokenize_markdown` must not change their signatures or observable behaviour.
- **Atomic fragments.** Inline code spans and Markdown links are never split
  across lines; they move as a unit when they would overflow the target width.
- **Hard breaks.** Trailing two-space hard breaks must survive on the emitted
  line where they occur.
- **Verbatim blocks.** Fenced code blocks must pass through unchanged, along
  with the other non-paragraph block kinds detected by `classify_block`.
- **Prefix width.** The visual width of every prefix string is measured with
  `UnicodeWidthStr::width` before the available text width is computed, so
  non-ASCII prefix characters (e.g. `「` in CJK blockquotes) are accounted for
  correctly.

Refer to `docs/adrs/0002-textwrap-wrapping-engine.md` for the rationale behind
replacing `LineBuffer` with `textwrap`.
