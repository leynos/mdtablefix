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

## Wrap module architecture

The `--wrap` pipeline is spread across four modules under `src/wrap/`:

- `src/wrap.rs` — entry point; classifies each incoming line (fenced code,
  table, heading, blank, or paragraph) and routes it to `ParagraphWriter`.
- `src/wrap/paragraph.rs` — `ParagraphWriter` and `ParagraphState`; buffers
  paragraph lines and applies prefix-aware wrapping via `wrap_with_prefix`.
- `src/wrap/inline.rs` — `wrap_preserving_code`; converts a paragraph string
  into `InlineFragment` values, feeds them to `textwrap::wrap_first_fit`, and
  post-processes the result.
- `src/wrap/inline/postprocess.rs` — `merge_whitespace_only_lines` and
  `rebalance_atomic_tails`; normalises the raw fit output before rendering.

### Key types

`InlineFragment` (in `src/wrap/inline.rs`) implements
`textwrap::core::Fragment`. Each fragment stores the rendered text, its
precomputed display-column width, and a `FragmentKind` discriminant:

| `FragmentKind` | Description |
| :-- | :-- |
| `Whitespace` | Fragment composed entirely of whitespace characters. |
| `InlineCode` | Backtick-delimited code span, optionally with trailing punctuation. |
| `Link` | Markdown inline link or image reference. |
| `Plain` | Ordinary prose text. |

`PrefixLine` (in `src/wrap/paragraph.rs`) carries a prefix string (e.g. `"> "`
for blockquotes, `"- "` for bullets), the content after the prefix, and a
`repeat_prefix` flag that controls whether continuation lines repeat the prefix
verbatim (blockquotes) or use a space-padded equivalent (bullets, footnotes,
checkboxes).

### Width accounting

All display-width measurements use `UnicodeWidthStr::width` from the
`unicode-width` crate. The `textwrap` call receives widths expressed as `f64`
values through `InlineFragment::width()` so that `wrap_first_fit` never
measures the byte length of a fragment.

Prefix width is computed once in `wrap_with_prefix` before calling
`wrap_preserving_code`, ensuring that available content width is derived from
the rendered prefix width rather than its byte count.

### Post-processing passes

After `wrap_first_fit` assigns fragments to lines, two passes correct edge
cases that greedy fitting cannot anticipate:

1. `merge_whitespace_only_lines` — absorbs whitespace-only separator lines back
   into adjacent content lines so that rendered output does not gain spurious
   blank entries.
2. `rebalance_atomic_tails` — moves a trailing atomic or plain fragment from
   one line to the start of the next when the destination line can accommodate
   it within `width`, preventing orphaned code spans or punctuation.

Both passes are width-constrained: `rebalance_atomic_tails` rechecks the
destination line width before moving a fragment, so it cannot create a line
that `wrap_first_fit` would have rejected.

### Public API stability

`wrap_text`, `Token`, and `tokenize_markdown` are re-exported from `src/lib.rs`
and are used by `src/code_emphasis.rs`, `src/footnotes/mod.rs`,
`src/footnotes/renumber.rs`, and `src/textproc.rs`. These symbols must remain
stable across internal refactoring. Any change to their signatures or
observable behaviour requires an explicit approval step and updated tests.

### Design decisions

The rationale for adopting `textwrap` and the fragment-adapter approach is
recorded in `docs/adrs/0002-textwrap-inline-wrapping.md`. Refer to that ADR
when evaluating changes to the fragment model, post-processing passes, or the
`textwrap` dependency.

## Design decisions

The rationale for the staged table reflow pipeline is recorded in
`docs/adrs/0001-table-reflow-pipeline.md`. Refer to that ADR when changing the
parse, width-calculation, or separator-handling flow so implementation changes
stay aligned with the documented design constraints.