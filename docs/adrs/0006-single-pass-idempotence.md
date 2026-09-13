# Architecture Decision Record (ADR) 0006: Make the formatter a fixed point in one pass

- Status: Accepted
- Date: 2026-09-10

## Context

`mdtablefix` was not idempotent: `format(format(x)) != format(x)` held for
inputs reachable under the `make fmt` flag set (`--wrap`, `--renumber`,
`--breaks`, `--ellipsis`, `--fences`), and for further inputs once `--headings`
or `--code-emphasis` was added, so a check-after-fix gate could never converge:
one `--in-place` pass left a file that the next pass rewrote again. Fourteen
defect classes contributed:

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
- Code-emphasis repair ran after table reflow. Removing emphasis markers around
  inline code shortened a cell after its column width had been measured, so a
  second pass narrowed the table columns.
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
- Footnote labels were renumbered after the passes that measure text. A label is
  numbered by first encounter, so it narrows as the document is rewritten:
  `[^10]` becomes `[^1]`, and a definition header is rewritten from the same
  mapping. A wrap that measured the longer label broke a line the next pass
  joined, once the shorter label left the line within the width.
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
- A lazy continuation line below a deferred block's hard break was emitted
  flush-left on the first pass and indented on the second. A list item whose
  first line spills past the width is deferred so its tail reflows with the
  lines below it, and the flush that honours the hard break remembers the
  item's continuation indent; the flush-left line below the break dropped it,
  while the next pass re-read the indented tail above and applied the indent to
  everything after it: `- alpha … beta` over `delta epsilon` (a line ending in
  the two-space hard break) over `zeta eta`, ended at column one on one pass and
  two columns in on the pass after.
- A deferred block's tail was reflowed without the backslash that ends it. The
  tail of an overlong prefixed line is wrapped on its own and the hard-break
  marker is put back afterwards, which is correct for the two-space form — the
  next pass trims trailing spaces before measuring — but a backslash is content.
  It is glued to the last word of the source line, so wrapping the line without
  it spent the whole width and the marker then pushed the line one column past
  it. The next pass did measure the backslash and broke one word earlier: a list
  item ending `… bbbb bbbbb\` reflowed to two continuation lines on one pass and
  three on the pass after, the last of them carrying the marker alone.

The unmatched-fence class was reported separately, in issue #480, and reached
the suite through the corpus rather than through a generator. The
code-emphasis class was found by the property suite once the flag was added to
it. The footnote, empty-header, table-row, and lone-dash-header classes were
found by the widened generators of issue #493, which the suite had been unable
to reach: it wrote only balanced three-character fences, used `1.` as its sole
ordered-list marker, left the characters the parser used as placeholders out of
its cells, and put neither a hard break nor an overlong code span in a
paragraph. The
lazy-continuation class was found by those same generators, once their
paragraphs carried a hard break at all; a list item wide enough to defer, a
break inside it, and one prose line below the break reach it. The
backslash-tail class needed a longer sweep still — it appeared four thousand
cases in, where the tail's last wrapped line filled the width exactly and the
marker taken off it was the one column that did not fit.

## Decision

The formatter is a fixed point, `format(format(x)) == format(x)`, for the flag
sets and documents with recorded evidence: the `make fmt` flag set (`--wrap`,
`--renumber`, `--breaks`, `--ellipsis`, `--fences`), that set with `--headings`,
and that set with `--code-emphasis`. The property suites broaden the ground
within that claim — they force each flag alone and sample the eight-flag
powerset — but not outside it. Two exceptions are of record:

- `--headings` sits outside the `make fmt` flag set, so the everyday gate does
  not reach it; the property suites force it on for the structural adjacency
  shape instead, as the consequences below record.
- Under `--wrap` alone, a paragraph that wraps so that the opening bracket of a
  `[1]`-style reference is the last character on a line settles one pass later
  than the invariant allows: the first pass ends the line with `[` and leaves
  `1]` below it, the second rejoins the two with a space between them — `[ 1]`
  — and the third reproduces the second. The shape is a pre-existing defect,
  byte-identical on main, that this decision neither introduces nor fixes, and
  is recorded in the addendum below as issue #504.

Thirteen rules enforce the invariant where it holds:

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
- A lazy continuation below a deferred block keeps the block's indent.
  `ParagraphState::note_indent` in `src/wrap/paragraph.rs` prefers the indent a
  deferred prefix flush remembered over the line's own, and a flush-left line
  inherits it instead of clearing it: such a line is a lazy continuation of the
  block above, so emitting it at column one gave the same paragraph a second
  spelling, and the pass that re-read the indented tail above applied the indent
  to everything below the break. The remembered indent is consumed either way,
  and a flush-left line with none to inherit contributes no indent of its own.
- A deferred tail measures a backslash hard break as content.
  `ParagraphWriter::append_stable_pending_prefix` in
  `src/wrap/paragraph/tail_reflow.rs` leaves the backslash in the text it hands
  to `wrap_preserving_code`, and still strips and re-appends the two-space
  marker. The two forms do not re-parse alike: the next pass trims trailing
  spaces before measuring, so the whitespace marker is appended after the fit
  without changing it, while the backslash is read back as the final character
  of the last word and must be inside it. A line that ends
  `bbbbb bbbbb\` therefore wraps on the marker's column budget, not one column
  beyond it.
- Content normalizers consumed by layout run before the layout they affect.
  After fence processing and HTML-table conversion, the inline footnote stage
  runs over the complete normalized stream before Markdown table buffering, so
  a reference such as `docs.1` has already grown into `docs.[^1]` when the
  table pass measures the cell it sits in. For buffered tables, the
  table-substitution stage retains its order: code-emphasis repair first, then
  ellipsis replacement, before `reflow_table` measures column widths. For the
  remaining content, `process_stream_inner` performs, in order, Setext heading
  conversion, code-emphasis repair, ordered-list renumbering, ellipsis
  replacement, and paragraph wrapping, and folds the footnote definition block
  last, after the layout, because it appends lines and reads the heading
  structure the heading pass settled. Footnotes, list renumbering, code
  emphasis, and ellipsis are therefore consumed by layout; they have no
  downstream preservation obligation. Thematic-break normalization remains last
  because it is width-independent. Replacing `...` with `…` shortens a line by
  two display columns, while footnotes and list markers can lengthen text, so
  wrapping must measure each final form.
- `--code-emphasis` repairs table cells before reflow measures them. Removing
  emphasis markers around inline code shortens the cell, so applying the repair
  in the table-substitution stage lets the formatter calculate the final column
  widths. The later global code-emphasis pass handles non-table content; each
  table cell is repaired once.
- Setext conversion accepts only paragraph candidates. `is_setext_text` in
  `src/headings.rs` measures the candidate after the indentation or blockquote
  prefix it shares with the underline has been removed, so a quoted heading
  such as `> Title` above `> -----` still converts. A candidate indented by
  four or more columns is also refused, because the pair is then an indented
  code block rather than a heading. The indentation width is measured on the
  whole line before the shared prefix is removed: the prefix would otherwise
  swallow the very columns that mark the code block. Blockquote markers and
  their optional single space are consumed before measuring, and tabs count as
  four columns. A candidate that is itself a block start keeps its underline:
  an ATX heading, a thematic break, a list item, a blockquote, a footnote
  definition, a link reference definition, a markdownlint directive, or a fence
  marker. The kinds are the ones `wrap::classify_block` already reports, so the
  heading pass and the wrapper agree on what a block start is. A digit-prefixed
  candidate stays eligible, because `BlockKind::DigitPrefix` marks a line the
  wrapper measures specially rather than a block. The check is limited to the
  grammar this formatter supports and is not a CommonMark block parser: HTML
  blocks other than the `<table>` conversion in `src/html.rs` remain outside it.
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
  trailing list — converting a heading-led list that no reference reaches — and
  reorders the definitions. It settles the structure only, keeping the number
  each header already carries, because the label stage has already made the
  numbers final; a second scan would take fresh numbers from the free pool for
  the definitions no reference points at, in line order, which moves them behind
  one the label stage placed earlier. `convert_footnotes` remains as the
  composition of the three for callers that need them in one step.
  The label stage promotes a trailing list item that a reference *does* reach,
  and promotes it there rather than last: a bare reference and a list item are
  matched by the number they share — `error.3` and the item `3.` are one
  footnote — so the promotion has to happen in the scan that rewrites the
  reference, before it is rewritten, and the header it writes is a longer marker
  than the item's own, which puts it on the measuring side of the split as well.
- Footnote labels are renumbered before the passes that measure text.
  `footnotes::renumber_footnote_labels` rewrites both the references and the
  definition headers from the mapping numbered by first encounter, and it runs
  beside `convert_inline_footnotes`, ahead of the table pass and the wrap,
  because a label narrows as it is rewritten: `[^10]` becomes `[^1]`. A wrap
  that measured the longer label broke a line the next pass joined, once the
  shorter label left the line within the width.
- A delimiter cell is an optional colon, one or more dashes, and an optional
  trailing colon. `table::is_delimiter_cell` applies that grammar to a cell's
  payload, and `table::is_delimiter_row`, `reflow::second_row_is_separator`, and
  `reflow::row_parsing` each require every cell of the row to satisfy it, so the
  heading pass and the table pass agree on what table syntax is.
  `SEP_RE` alone also matches a cell that is empty, so a row of nothing but
  pipes and spaces — a table's empty header row — was read as the alignment row,
  and the genuine delimiter row was then demoted to a data row. A lone dash
  among empty cells — `|  | - |` above `| --- | --- |` — is the same trap one
  dash later, and is reachable with no flags at all: the header was taken for
  the delimiter row and the genuine delimiter row was laid out as a data row
  beside the synthesized one, so the pass after that read the data row as the
  delimiter row in turn and the first pass had no fixed point. A cell test that
  asked only for a dash was weaker still, because `SEP_RE` permits whitespace
  inside a cell: the malformed header `| - - |` was taken for the alignment row
  and rewritten into `| --- |`, which turned malformed source into table syntax
  instead of leaving it as data.

## Consequences

- A single `--in-place` run reaches the formatter's final output, so a
  check-after-fix gate cannot report drift indefinitely on the same file.
- Changed output is confined to thematic breaks that are now preserved instead
  of consumed, to prefixed blocks that now reflow with their continuation lines
  in one pass, to lazy continuation lines below such a block, which now carry
  the block's continuation indent rather than starting a column to the left, to
  deferred tails ending in a backslash hard break, which now wrap within the
  width instead of one column past it, to unmatched fences that are rewritten
  from the opener alone, to footnote references that are converted and footnote
  labels that are renumbered before the layout rather than after it, to
  delimiter rows that are only recognized when every cell is a well-formed
  delimiter cell, and to candidates that are themselves block starts or table
  rows, which no longer convert, so the line below them survives as a block of
  its own. Tables with
  code-emphasis repairs also receive their final column widths in the first
  pass. A footnote definition promoted from a trailing list item wraps its
  continuation lines to the definition body's indent rather than to the item's
  own, because the promotion now runs before the wrap measures the line; the
  text the two indents apply to is the same either way, and both spellings are
  fixed points.
- `tests/idempotence.rs` formats the fixture corpus under
  `tests/data/idempotence/` twice through the real binary and asserts
  byte-identical output; the class `T` fixtures pin the table-adjacency screens
  and assert that the row survives as table syntax, `T7` among them for the body
  row above a break. Its repository-wide drift sweeps live in
  `tests/idempotence_drift.rs`, and include tables processed with
  `--code-emphasis`, among them the fixture that previously required a second
  pass. `tests/idempotence_properties.rs` is a `proptest!` property over
  generated documents and a sampled eight-flag powerset, while
  `tests/idempotence_adjacencies.rs` holds the structural adjacency property and
  its coverage sweep. The generators the suites draw on live in
  `tests/support/idempotence_generators.rs` and the harness they share in
  `tests/support/idempotence_harness.rs`. The case count comes from
  `PROPTEST_CASES`, through the `proptest_config` helper in the harness, so the
  48 both suites run at is a default rather than a ceiling: a longer sweep
  raises it without a recompile. The generated domain covers the shapes the
  defects above were reachable through: fence openers of three to five
  characters in both marker characters, with shorter interior runs and blocks
  that are never closed; ordered-list markers beyond `1.`, including multi-digit
  numbers, restarts, and nesting; table cells drawn from the whole `char` range,
  the characters the parser once used as placeholders included; and paragraphs
  carrying hard breaks and code spans longer than the wrap width. The property
  generates structural adjacencies — a candidate directly above a thematic break
  — with `--headings` forced on, since the `make fmt` flag set does not enable
  it, and a fourth shape chains a converting paragraph, a thematic break, a
  table, and a delimiter row above the trailing break in one document, so each
  boundary's guard is exercised where its neighbours are guard cases too. The
  delimiter row is generated both alone and below a header row, and a
  deterministic sweep
  asserts the shape is reached and its row survives, so removing the generator
  branch fails the sweep rather than leaving the guard unexercised.
  `src/wrap/paragraph_tests.rs` pins the two documents that sweep shrank its
  drift to, along with a three-line item that reaches the class on its own, the
  two documents the backslash-tail overflow shrank to, and `src/table.rs` pins
  the lone-dash header row beside the empty one. Together they guard the
  invariant against regression.

## Addendum (2026-09-13)

The decision above is scoped to the flag sets and documents with recorded
evidence. That evidence is the corpus sweep in `tests/idempotence_drift.rs`,
which runs each of the three sets over every fixture under `tests/data/` and
asserts the two passes are byte-identical; the `--code-emphasis` set covers the
table case that once required a second pass, fixed in `b01b999`. It is not a
universal guarantee over the inputs the formatter accepts.

The measured exception is a bracket reference the wrapper splits across lines.
Under `--wrap` alone — and so under every set above, since each one contains it
— the input

```text
aaaaa aaaaaa aaaa aa aaaa aamw jkxf ht
abm iqy uxqdkre fz ioelg
**bold**`code`
[1]
```

ends its first pass with the opening bracket left dangling at the end of a line
and the rest of the reference on the next one:

```text
aaaaa aaaaaa aaaa aa aaaa aamw jkxf ht abm iqy uxqdkre fz ioelg **bold**`code` [
1]
```

Its second pass rejoins the bracket with its text and reflows that line:

```text
aaaaa aaaaaa aaaa aa aaaa aamw jkxf ht abm iqy uxqdkre fz ioelg **bold**`code`
[ 1]
```

The output settles there rather than growing, so it is a one-pass drift and not
a cycle. The reproduction is recorded as issue #504, and the split lives in the
inline-wrapping path, which the rules above do not cover.

The impact is that `--git --check` is a sound one-pass drift check for a
document that does not contain that shape, while a document that does is
reported as needing formatting again after an `--in-place` run has written the
first pass's output. This addendum narrows the operational guarantee until the
inline-wrapping path has evidence of the same fixed-point behaviour. That
evidence now exists for the shape recorded here; see the addendum below.

## Addendum (2026-09-14)

Issue #504 is fixed. The tokenizer emits `[` and `1]` as separate tokens, so
nothing bound them and the opener was free to end a line on its own. The inline
wrapping path now couples the pair the way it already couples footnote markers
and links to their openers: `looks_like_bracketed_reference` recognizes the
closing shape the tokenizer emits, `try_couple_bracketed_reference` sums the two
widths and absorbs trailing punctuation, and `FragmentKind::BracketedRef` keeps
the merged span atomic so the post-wrap rebalancing pass cannot separate it
again. Labels are digit-only by design; the residue is that a short alphabetic
label such as `[a]` is still ordinary prose the wrapper may break at, which is
unchanged behaviour rather than a regression.

The evidence for the shape recorded above is the corpus fixture
`E1_bracket_after_bold_code` under `tests/data/idempotence/`, which is that
reproduction formatted twice through the real binary under `--wrap` and asserted
to be byte-identical. The fixture records `[1]` as its structural expectation,
so a fix that settled the output by dropping the reference fails rather than
passes. `tests/idempotence_properties.rs` reaches the same shape from generated
documents: `bracket_reference_seam_strategy` grows the head to one column short
of the wrap width, which puts the wrap boundary before the reference, and a
coverage test asserts the shape is actually generated so removing the strategy
cannot leave the property vacuous. The wrap suites pin the break itself in
`src/wrap/tests/inline_wrapping.rs`, `tests/wrap_unit/stream.rs` and
`tests/wrap/lists.rs`, and `tests/wrap/cli.rs` formats twice through the CLI and
compares the passes. Each of those cases fails against the parent commit.

The narrowing in the previous addendum therefore no longer applies to this
shape, and `--git --check` is a sound one-pass drift check for documents that
contain it.
