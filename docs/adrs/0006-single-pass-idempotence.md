# Architecture Decision Record (ADR) 0006: Make the formatter a fixed point in one pass

- Status: Accepted
- Date: 2026-09-10

## Context

`mdtablefix` was not idempotent: `format(format(x)) != format(x)` held for
inputs reachable under the `make fmt` flag set (`--wrap`, `--renumber`,
`--breaks`, `--ellipsis`, `--fences`), and for further inputs once `--headings`
was added, so a check-after-fix gate could never converge: one `--in-place` pass
left a file that the next pass rewrote again. Ten defect classes contributed:

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
- Setext conversion also consumed the break below a table delimiter row. A
  table whose delimiter row is the last line before a thematic break was
  restructured on every pass: the delimiter row `| --- | --- |` above `---` was
  rewritten as `## | --- | --- |`, which left the table above it without a
  delimiter row, and the orphaned header row was then padded to the minimal
  width on the next pass. The class was found by the property suite after the
  fifth class was fixed, and is reachable under `--headings`, which the
  `make fmt` flag set does not enable.
- An unmatched code fence was rewritten to its opener's run length on every
  line of the block. A block whose interior held a shorter fence-shaped run —
  three backticks inside a four-backtick block — had that interior line widened
  with the rest, so the next pass read a different interior run and rewrote the
  block again.
- Footnote references were converted after the passes that measure text. A
  reference such as `docs.1` grows into `docs.[^1]`, so the table pass laid out
  a cell from the shorter source text, and the pass after it measured the same
  cell one character longer: the delimiter row below `| a | see docs.1 |` was
  ten dashes wide on the first pass and thirteen on the second, and the two
  never agreed.
- A table whose header row was empty had that header taken for the delimiter
  row. `SEP_RE` matches a row made only of pipes and spaces as readily as one
  made of dashes, so `|  |  |` was read as the alignment row, the genuine
  delimiter row was demoted to a data row, and the replacement the table pass
  synthesized left two delimiter-shaped rows for later passes to consume in
  turn.
- Setext conversion consumed a table *body* row above a thematic break, not
  only a delimiter row. The row the pass takes is often the table's widest, so
  once it had become a heading the rows above were measured without it and
  every one of them was padded a column narrower on the next pass: `| a | b |`
  over `| --- | --- |` over `| ccccc | d |` over `---` reflowed to a
  five-column first row on one pass and a three-column one on the pass after.

The unmatched-fence class was reported separately, in issue #480, and reached
the suite through the corpus rather than through a generator. The footnote,
empty-header, and table-row classes were found by the widened generators of
issue #493, which the suite had been unable to reach: it wrote only balanced
three-character fences, used `1.` as its sole ordered-list marker, left the
sentinel characters out of its cells, and put neither a hard break nor an
overlong code span in a paragraph.

## Decision

The formatter is a fixed point: `format(format(x)) == format(x)` for every flag
set the CLI exposes. Nine rules enforce the invariant:

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
  reparse as a different block: a tail indented by four or more columns
  (indented code), a tail that repeats its blockquote marker, and a footnote
  definition tail stay separate.
- `--ellipsis` runs before the wrap. `process_stream_inner` converts inline
  footnote references, lays out tables, converts Setext headings, repairs code
  emphasis, replaces ellipses, wraps paragraphs, and folds footnote definitions,
  in that order; replacing `...` with `…` shortens a line by two display
  columns, so a wrap that measured the source dots emitted a break that the next
  pass joined.
- Setext conversion accepts only paragraph candidates. `is_setext_text` in
  `src/headings.rs` measures the candidate after the indentation or blockquote
  prefix it shares with the underline has been removed, so a quoted heading
  such as `> Title` above `> -----` still converts. A candidate indented by
  four or more columns is also refused, because the pair is then an indented
  code block rather than a heading. The indentation width is measured on the
  whole line before the shared prefix is removed: the prefix would otherwise
  swallow the very columns that mark the code block. Blockquote markers and
  their optional single space are consumed before measuring, and tabs count as
  four columns.
  A candidate that is itself a block start keeps its underline: an ATX
  heading, a thematic break, a list item, a blockquote, a footnote definition,
  a link reference definition, a markdownlint directive, or a fence marker.
  The kinds are the ones `wrap::classify_block` already reports, so the heading
  pass and the wrapper agree on what a block start is. A digit-prefixed
  candidate stays eligible, because `BlockKind::DigitPrefix` marks a line the
  wrapper measures specially rather than a block. The check is limited to
  the grammar this formatter supports and is not a CommonMark block parser:
  HTML blocks other than the `<table>` conversion in `src/html.rs` remain
  outside it.
- Table rows are refused separately from the block kinds. `is_table_syntax` in
  `src/headings.rs` refuses a candidate that starts with the `|` the table pass
  enters its table mode on, or that carries a `|` and matches
  `crate::table::SEP_RE`, the pattern the table parser already uses to find the
  delimiter row, so the heading pass and the table pass agree on what table
  syntax is. `BlockKind` does not model a table row: `classify_block` reports
  `None` for a pipe-prefixed line, because such a line is part of a table. The
  `|` is required, so a bare `---` stays a thematic break and a break above
  another break stays two breaks, while a paragraph that merely contains a pipe
  still converts. Refusing the candidate keeps the `---` below it as a thematic
  break, which is what the no-flags run already produces, and keeps the table
  above it whole rather than one row shorter.
- An unmatched code fence is rewritten from its opener alone. Every interior
  line of an unclosed fence is literal content of that fence, so
  `fences::flush_unmatched_block` rewrites the opener — the same way the matched
  path does, and by the same strategy, so an opener whose interior holds a
  fence-shaped run keeps its length rather than shortening into it — and emits
  the interior lines verbatim. Rewriting the interior as well ran both ways: the
  shorter interior run of a four-backtick block was widened with the opener, so
  the next pass no longer read it as a run at all and rewrote the block again.
- `--footnotes` is split around the passes that measure text.
  `footnotes::convert_inline_footnotes` rewrites bare numeric references as
  Markdown references before the table pass and the wrap, because a reference
  such as `docs.1` grows into `docs.[^1]` and every later pass lays out the line
  it sits in. `footnotes::convert_footnote_definitions` stays last, after the
  heading pass has settled, because it reads the block structure around the
  trailing list and appends definition lines. `convert_footnotes` remains as the
  composition of the two for callers that need them in one step.
- A delimiter row carries a dash in every cell. `table::extract_separator_line`
  and `reflow::second_row_is_separator` both require one, as
  `reflow::row_parsing` already did when it decides what a delimiter cell is.
  `SEP_RE` alone also matches a cell that is empty, so a row of nothing but
  pipes and spaces — a table's empty header row — was read as the alignment row,
  and the genuine delimiter row was then demoted to a data row.

## Consequences

- A single `--in-place` run reaches the formatter's final output, so a
  check-after-fix gate cannot report drift indefinitely on the same file.
- Changed output is confined to thematic breaks that are now preserved instead
  of consumed, to prefixed blocks that now reflow with their continuation lines
  in one pass, to unmatched fences that are rewritten from the opener alone, to
  footnote references that are converted before the layout rather than after it,
  to delimiter rows that are only recognized when every cell carries a dash, and
  to candidates that are themselves block starts or table rows, which no longer
  convert, so the line below them survives as a block of its own.
- `tests/idempotence.rs` formats the fixture corpus under
  `tests/data/idempotence/` twice through the real binary and asserts
  byte-identical output; the class `T` fixtures pin the table-adjacency
  screens and assert that the row survives as table syntax, `T7` among them for
  the body row above a break. Its repository-wide drift sweeps live in
  `tests/idempotence_drift.rs`. `tests/idempotence_properties.rs` is a
  `proptest!` property over generated documents and a sampled eight-flag
  powerset, while `tests/idempotence_adjacencies.rs` holds the structural
  adjacency property and its coverage sweep. The generators the suites draw on
  live in `tests/support/idempotence_generators.rs` and the harness they share
  in `tests/support/idempotence_harness.rs`. The case count comes from
  `PROPTEST_CASES`, through the `proptest_config` helper in the harness, so the
  48 both suites run at is a default rather than a ceiling: a longer sweep
  raises it without a recompile. The generated domain covers the shapes the
  defects above were reachable through: fence openers of three to five
  characters in both marker characters, with shorter interior runs and blocks
  that are never closed; ordered-list markers beyond `1.`, including multi-digit
  numbers, restarts, and nesting; table cells drawn from the whole `char` range,
  sentinel characters included; and paragraphs carrying hard breaks and code
  spans longer than the wrap width. The property generates structural
  adjacencies — a candidate directly above a thematic break — with `--headings`
  forced on, since the `make fmt` flag set does not enable it, and a fourth
  shape chains a converting paragraph, a thematic break, a table, and a
  delimiter row above the trailing break in one document, so each boundary's
  guard is exercised where its neighbours are guard cases too. The delimiter row
  is generated both alone and below a header row, and a deterministic sweep
  asserts the shape is reached and its row survives, so removing the generator
  branch fails the sweep rather than leaving the guard unexercised. Together
  they guard the invariant against regression.
