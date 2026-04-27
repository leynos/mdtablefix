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

## Fences module

The `fences` module in [src/fences.rs](../src/fences.rs) is responsible for
normalizing fenced code blocks before later Markdown transforms run. It exposes
two public functions that are called in sequence:

- `compress_fences(&[String]) -> Vec<String>` conditionally compresses fence
  delimiters of three or more backticks or tildes to exactly three backticks
  when doing so preserves the structural interpretation of the inner content. If
  compression would make inner fence-like content become structural, the
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
