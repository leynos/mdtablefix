# Architecture Decision Record (ADR) 0006: Make the formatter a fixed point in one pass

- Status: Accepted
- Date: 2026-09-10

## Context

`mdtablefix` was not idempotent: `format(format(x)) != format(x)` held for
inputs reachable under the `make fmt` flag set (`--wrap`, `--renumber`,
`--breaks`, `--ellipsis`, `--fences`), and for further inputs once `--headings`
was added, so a check-after-fix gate could never converge: one `--in-place` pass
left a file that the next pass rewrote again. Five defect classes contributed:

- A normalized thematic break was absorbed into the following paragraph
  instead of passed through on its own line.
- A prefixed block whose tail spilled past the target width reflowed
  differently on a second pass, because the emitted tail was folded with the
  lines below it.
- An ellipsis was replaced after the wrap had measured the longer three-dot
  source text, so a break computed from the dots was joined once the glyph
  shortened the line.
- A seventy-underscore break written by `--breaks` was attached to the next
  fence as an info string.
- Setext conversion consumed the line below a candidate that was already a
  block of its own. `## aa` above `---` became the single line `## ## aa`, and
  because the first pass left the break behind and the second pass consumed it,
  the output never settled.

## Decision

The formatter is a fixed point: `format(format(x)) == format(x)` for every flag
set the CLI exposes. Five rules enforce the invariant:

- Thematic breaks are a block-level pass-through. `BlockKind::ThematicBreak` in
  `src/wrap/block.rs` recognizes a break with
  `crate::breaks::THEMATIC_BREAK_RE`, which covers three or more `-`, `*`, or
  `_` characters, including spaced runs such as `- - -`, and the
  seventy-underscore line that `--breaks` writes. Such a line is emitted on its
  own line and never absorbed into the surrounding paragraph, with or without
  `--breaks`; table separator rows keep their pipes and remain table rows.
- `--breaks` output is itself a fixed point. A seventy-underscore line is not
  absorbed into a paragraph by a later `--wrap`, and
  `fences::attach_orphan_specifiers` refuses to attach it to a following fence
  as an orphaned info string. The guard is `is_thematic_break`, whose predicate
  is deliberately identical to the one `format_breaks` uses.
- Prefixed blocks that wrap reflow with their continuation lines.
  `ParagraphWriter` defers an overlong first line through `PendingPrefix` so
  the continuation and lazy continuation lines below it are folded into the
  same wrapped output; `wraps_to_tail(text, available)` and
  `continuation_folds_tail(continuation_prefix)` in `src/wrap/paragraph.rs`
  decide whether folding is safe. Folding is skipped when the tail would
  re-parse as a different block: a tail indented by four or more columns
  (indented code), a tail that repeats its blockquote marker, and a footnote
  definition tail stay separate.
- `--ellipsis` runs before the wrap. `process_stream_inner` performs, in
  order, Setext heading conversion, code-emphasis repair, ellipsis
  replacement, paragraph wrapping, and footnote conversion; replacing `...`
  with `…` shortens a line by two display columns, so a wrap that measured the
  source dots emitted a break that the next pass joined.
- Setext conversion accepts only paragraph candidates. `is_setext_text` in
  `src/headings.rs` measures the candidate after the indentation or blockquote
  prefix it shares with the underline has been removed, so a quoted heading
  such as `> Title` above `> -----` still converts. A candidate that is itself
  a block start keeps its underline: an ATX heading, a thematic break, a list
  item, a blockquote, a footnote definition, a link reference definition, a
  markdownlint directive, or a fence marker. The kinds are the ones
  `wrap::classify_block` already reports, so the heading pass and the wrapper
  agree on what a block start is. A digit-prefixed candidate stays eligible,
  because `BlockKind::DigitPrefix` marks a line the wrapper measures specially
  rather than a block. The check is limited to the grammar this formatter
  supports and is not a CommonMark block parser: HTML blocks other than the
  `<table>` conversion in `src/html.rs` remain outside it.

## Consequences

- A single `--in-place` run reaches the formatter's final output, so a
  check-after-fix gate cannot report drift indefinitely on the same file.
- Changed output is confined to thematic breaks that are now preserved instead
  of consumed, to prefixed blocks that now reflow with their continuation lines
  in one pass, and to candidates that are themselves block starts, which no
  longer convert, so the line below them survives as a block of its own.
- `tests/idempotence.rs` formats the fixture corpus under
  `tests/data/idempotence/` twice through the real binary and asserts
  byte-identical output; `tests/idempotence_properties.rs` is a `proptest!`
  property over generated documents and a sampled eight-flag powerset, and
  generates structural adjacencies — a candidate directly above a thematic
  break — with `--headings` forced on, since the `make fmt` flag set does not
  enable it. Together they guard the invariant against regression.
