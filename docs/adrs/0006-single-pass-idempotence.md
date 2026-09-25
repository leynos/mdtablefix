# Architecture Decision Record (ADR) 0006: Make the formatter a fixed point in one pass

- Status: Accepted
- Date: 2026-09-10

## Context

`mdtablefix` was not idempotent: `format(format(x)) != format(x)` held for
inputs reachable under the `make fmt` flag set (`--wrap`, `--renumber`,
`--breaks`, `--ellipsis`, `--fences`), and for further inputs once `--headings`
or `--code-emphasis` was added, so a check-after-fix gate could never converge:
one `--in-place` pass left a file that the next pass rewrote again. Seven
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

## Decision

The formatter is a fixed point, `format(format(x)) == format(x)`, for the flag
sets and documents with recorded evidence: the `make fmt` flag set (`--wrap`,
`--renumber`, `--breaks`, `--ellipsis`, `--fences`), that set with
`--headings`, and that set with `--code-emphasis`. The guarantee is not
universal over the inputs the formatter accepts: the one measured exception, a
bracket reference the wrapper splits across lines, is recorded in the addendum
below and tracked as issue #504. Seven rules enforce the invariant where it
holds:

- Thematic breaks are a block-level pass-through. `BlockKind::ThematicBreak` in
  `src/wrap/block.rs` recognizes a break through the shared classifier, which
  reports `LineClass::ThematicBreak` for three or more `-`, `*`, or `_`
  characters, including spaced runs such as `- - -`, and the seventy-underscore
  line that `--breaks` writes. Such a line is emitted on its own line and never
  absorbed into the surrounding paragraph, with or without `--breaks`; table
  separator rows keep their pipes and remain table rows.
- `--breaks` output is itself a fixed point. A seventy-underscore line is not
  absorbed into a paragraph by a later `--wrap`, and
  `fences::attach_orphan_specifiers` refuses to attach it to a following fence
  as an orphaned info string. The guard is `is_thematic_break`, which now takes
  its decision from the shared line classifier, as the break pass does. The two
  predicates are no longer identical: the break pass also preserves Setext
  underlines in their preceding-line context, so a line this guard declines is
  not necessarily one `format_breaks` rewrites.
- Prefixed blocks that wrap reflow with their continuation lines.
  `ParagraphWriter` defers an overlong first line through `PendingPrefix` so
  the continuation and lazy continuation lines below it are folded into the
  same wrapped output; `wraps_to_tail(text, available)` and
  `continuation_folds_tail(continuation_prefix)` in `src/wrap/paragraph.rs`
  decide whether folding is safe. Folding is skipped when the tail would
  reparse as a different block: a tail indented by four or more columns
  (indented code), a tail that repeats its blockquote marker, and a footnote
  definition tail stay separate.
- Content normalizers consumed by layout run before the layout they affect.
  After fence processing and HTML-table conversion, footnote conversion runs
  over the complete normalized stream before Markdown table buffering. This
  preserves document-wide reference-definition mapping and gives table cells
  their final footnote labels before layout. For buffered tables, the
  table-substitution stage retains its order: code-emphasis repair first, then
  ellipsis replacement, before `reflow_table` measures column widths. For the
  remaining content, `process_stream_inner` performs, in order, Setext heading
  conversion, code-emphasis repair, ordered-list renumbering, ellipsis
  replacement, and paragraph wrapping. Footnotes, list renumbering, code
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
- Table delimiter rows are refused separately from the block kinds.
  `is_setext_text_line` in `src/classify.rs` refuses a candidate unless the
  shared classifier reports paragraph text for it, and a delimiter row carries
  the classifier's `LineClass::TableDelimiter` class, the table-delimiter
  decision that refuses it, so the heading pass and the table pass agree on
  what one is. A delimiter row is table syntax rather than paragraph text, and
  `BlockKind` does not model it: `classify_block` reports `None` for a
  pipe-prefixed line, because such a line is part of a table. The `|` is
  required, so a bare `---` stays a thematic break and a break above another
  break stays two breaks, while a paragraph that merely contains a pipe still
  converts. Refusing the candidate keeps the `---` below it as a thematic
  break, which is what the no-flags run already produces.

## Consequences

- A single `--in-place` run reaches the formatter's final output, so a
  check-after-fix gate cannot report drift indefinitely on the same file.
- Changed output is confined to thematic breaks that are now preserved instead
  of consumed, to prefixed blocks that now reflow with their continuation lines
  in one pass, and to candidates that are themselves block starts or table
  delimiter rows, which no longer convert, so the line below them survives as a
  block of its own. Tables with code-emphasis repairs also receive their final
  column widths in the first pass.
- `tests/idempotence.rs` formats the fixture corpus under
  `tests/data/idempotence/` twice through the real binary and asserts
  byte-identical output; the class `T` fixtures pin the delimiter-row adjacency
  and assert that the row survives as table syntax. Its repository-wide drift
  sweeps live in `tests/idempotence_drift.rs`. The drift sweeps include tables
  processed with `--code-emphasis`, including the fixture that previously
  required a second pass. `tests/idempotence_properties.rs` is a `proptest!`
  property over generated documents and a sampled eight-flag powerset, while
  `tests/idempotence_adjacencies.rs` holds the structural-adjacency property
  and its coverage sweep; the generator both suites share lives in
  `tests/support/idempotence_harness.rs`. The property generates structural
  adjacencies — a candidate directly above a thematic break — with `--headings`
  forced on, since the `make fmt` flag set does not enable it. The delimiter
  row is generated both alone and below a header row, and a deterministic sweep
  asserts the shape is reached and its row survives, so removing the generator
  branch fails the sweep rather than leaving the guard unexercised. Together
  they guard the invariant against regression.

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
closing shape the tokenizer emits, `try_couple_bracketed_reference` sums the
two widths and absorbs trailing punctuation, and `FragmentKind::BracketedRef`
keeps the merged span atomic so the post-wrap rebalancing pass cannot separate
it again. Labels are digit-only by design; the residue is that a short
alphabetic label such as `[a]` is still ordinary prose the wrapper may break
at, which is unchanged behaviour rather than a regression.

The evidence for the shape recorded above is the corpus fixture
`E1_bracket_after_bold_code` under `tests/data/idempotence/`, which is that
reproduction formatted twice through the real binary under `--wrap` and
asserted to be byte-identical. The fixture records `[1]` as its structural
expectation, so a fix that settled the output by dropping the reference fails
rather than passes. `tests/idempotence_properties.rs` reaches the same shape
from generated documents: `bracket_reference_seam_strategy` grows the head to
one column short of the wrap width, which puts the wrap boundary before the
reference, and a coverage test asserts the shape is actually generated so
removing the strategy cannot leave the property vacuous. The wrap suites pin
the break itself in `src/wrap/tests/inline_wrapping.rs`,
`tests/wrap_unit/stream.rs` and `tests/wrap/lists.rs`, and `tests/wrap/cli.rs`
formats twice through the CLI and compares the passes. Each of those cases
fails against the parent commit.

The narrowing in the previous addendum therefore no longer applies to this
shape, and `--git --check` is a sound one-pass drift check for documents that
contain it.

The findings of issue #493 are recorded below: the defect classes its widened
generators reached, the rules they produced, and the evidence for them. The
accepted body above is left as accepted, and the wording this part supersedes
is listed under "Superseded wording" below.

### Expanded defect classes

The accepted Context records seven defect classes; the work of issue #493
reached fourteen, and the bracket-reference seam recorded above is a fifteenth,
fixed under issue #504. The seven added classes are:

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
- Footnote labels were renumbered after the passes that measure text. A label
  is numbered by first encounter, so it narrows as the document is rewritten:
  `[^10]` becomes `[^1]`, and a definition header is rewritten from the same
  mapping. A wrap that measured the longer label broke a line the next pass
  joined, once the shorter label left the line within the width.
- A table whose header row was empty had that header taken for the delimiter
  row. `SEP_RE` matches a row made only of pipes and spaces as readily as one
  made of dashes, so `|  |  |` was read as the alignment row, the genuine
  delimiter row was demoted to a data row, and the replacement the table pass
  synthesized left two delimiter-shaped rows for later passes to consume in
  turn.
- Setext conversion consumed a table body row above a thematic break, not only
  a delimiter row. The row the pass takes is often the table's widest, so once
  it had become a heading the rows above were measured without it and every one
  of them was padded a column narrower on the next pass: `| a | b |` over
  `| --- | --- |` over `| ccccc | d |` over `---` reflowed to a five-column
  first row on one pass and a three-column one on the pass after.
- A lazy continuation line below a deferred block's hard break was emitted
  flush-left on the first pass and indented on the second. A list item whose
  first line spills past the width is deferred so its tail reflows with the
  lines below it, and the flush that honours the hard break remembers the
  item's continuation indent; the flush-left line below the break dropped it,
  while the next pass re-read the indented tail above and applied the indent to
  everything after it: `- alpha … beta` over `delta epsilon` (a line ending in
  the two-space hard break) over `zeta eta`, ended at column one on one pass
  and two columns in on the pass after.
- A deferred block's tail was reflowed without the backslash that ends it. The
  tail of an overlong prefixed line is wrapped on its own and the hard-break
  marker is put back afterwards, which is correct for the two-space form — the
  next pass trims trailing spaces before measuring — but a backslash is
  content. It is glued to the last word of the source line, so wrapping the
  line without it spent the whole width and the marker then pushed the line one
  column past it. The next pass did measure the backslash and broke one word
  earlier: a list item ending `… bbbb bbbbb\` reflowed to two continuation
  lines on one pass and three on the pass after, the last of them carrying the
  marker alone.

The unmatched-fence class was reported separately, in issue #480, and reached
the suite through the corpus rather than through a generator. The code-emphasis
class was found by the property suite once the flag was added to it. The two
footnote classes, the empty-header class, and the Setext table-row class were
found by the widened generators of issue #493, which the suite had been unable
to reach: it wrote only balanced three-character fences, used `1.` as its sole
ordered-list marker, left the characters the parser used as placeholders out of
its cells, and put neither a hard break nor an overlong code span in a
paragraph. The lazy-continuation class was found by those same generators, once
their paragraphs carried a hard break at all; a list item wide enough to defer,
a break inside it, and one prose line below the break reach it. The
backslash-tail class needed a longer sweep still — it appeared four thousand
cases in, where the tail's last wrapped line filled the width exactly and the
marker taken off it was the one column that did not fit.

### New and revised rules

The accepted Decision states seven rules; the work records thirteen: six added
and two revised. The six added rules are:

- A lazy continuation below a deferred block keeps the block's indent.
  `ParagraphState::note_indent` in `src/wrap/paragraph.rs` prefers the indent a
  deferred prefix flush remembered over the line's own, so a flush-left line
  below the break inherits it rather than clearing it.
- A deferred tail measures a backslash hard break as content.
  `ParagraphWriter::append_stable_pending_prefix` in
  `src/wrap/paragraph/tail_reflow.rs` keeps the backslash inside the text it
  hands to `wrap_preserving_code`, and still strips and re-appends the
  two-space marker around the fit.
- An unmatched code fence is rewritten from its opener alone.
  `fences::flush_unmatched_block` rewrites the opener and emits the interior
  lines verbatim, so a shorter fence-shaped run inside the block survives.
- `--footnotes` is split around the passes that measure text: the inline
  reference and label stages run before the table pass and the wrap, and the
  definition stage runs last; the Footnote stages section below sets out the
  reasons.
- Footnote labels are renumbered before the passes that measure text.
  `footnotes::renumber_footnote_labels` rewrites references and definition
  headers from the mapping numbered by first encounter, ahead of the table pass
  and the wrap, because a label narrows as it is rewritten.
- The delimiter-cell grammar is its own rule. `table::is_delimiter_cell`
  applies it, and `table::is_delimiter_row`, `reflow::second_row_is_separator`
  and `reflow::row_parsing` require every cell of a row to satisfy it; the
  grammar is set out under the Delimiter-cell grammar section below.

The two revised rules are:

- The normalizer-ordering rule, extended by the split footnote stage and the
  definition fold: the inline footnote stage runs before the table pass and the
  wrap, and the definition stage runs last, after the layout.
- The table-row refusal rule, now `is_setext_text_line` in `src/classify.rs`
  through the shared classifier's table decisions: a candidate is refused when
  the classifier reports `LineClass::TableDelimiter` or `LineClass::TableRow`
  for it, so a body row above a break is refused as well as a delimiter row.

### Footnote stages

The footnote conversion is split into three stages rather than one.
`footnotes::convert_inline_footnotes` and `footnotes::renumber_footnote_labels`
run before the table pass and the wrap, because a reference grows — `docs.1`
becomes `docs.[^1]` — and a label narrows — `[^10]` becomes `[^1]` — so both
change the text a later pass measures.
`footnotes::convert_footnote_definitions` stays last: it appends lines and
reads the heading structure the heading pass settled, and it settles the
structure only, keeping the number each header already carries, because a
second numbering scan would take fresh numbers from the free pool and move
definitions behind ones the label stage placed earlier.
`footnotes::convert_footnotes` remains the composition of the three for callers
that need them in one step.

The label stage also promotes a trailing list item that a reference reaches,
and it promotes it in that scan rather than last: a bare reference and a list
item are matched by the number they share — `error.3` and the item `3.` are one
footnote — so the promotion has to happen before the reference is rewritten,
and the header it writes is a longer marker than the item's own, which puts it
on the measuring side of the split as well.

### Delimiter-cell grammar

A delimiter cell is an optional colon, one or more dashes, and an optional
trailing colon. `table::is_delimiter_cell` applies that grammar to a trimmed
cell payload, and `table::is_delimiter_row`, `reflow::second_row_is_separator`
and `reflow::row_parsing` each require every cell of the row to satisfy it, so
the heading pass and the table pass agree on what table syntax is.

The weaker tests admitted three shapes that are not delimiter cells. `SEP_RE`
alone also matches an empty cell, so a row of nothing but pipes and spaces — a
table's empty header row — was read as the alignment row, and the genuine
delimiter row was then demoted to a data row. A lone dash among empty cells —
`|  | - |` above `| --- | --- |` — is the same trap one dash later, and is
reachable with no flags at all: the header was taken for the delimiter row and
the genuine delimiter row was laid out as a data row beside the synthesized
one, so the pass after that read the data row as the delimiter row in turn and
the first pass had no fixed point. A cell test that asked only for a dash was
weaker still, because `SEP_RE` permits whitespace inside a cell: the malformed
header `| - - |` was taken for the alignment row and rewritten into `| --- |`,
which turned malformed source into table syntax instead of leaving it as data.

### Fixes recorded

Four fixes carry the added classes into the rule set:

- The unmatched fence: a four-backtick block whose interior held a
  three-backtick run had that interior line widened to four backticks on the
  first pass, so the next pass read a run that was no longer there and rewrote
  the block again; `fences::flush_unmatched_block` now rewrites the opener
  alone and emits the interior lines verbatim.
- The Setext table row: `| a | b |` over `| --- | --- |` over `| ccccc | d |`
  over `---` had the body row converted, leaving a five-column first row on one
  pass and a three-column one on the next; `is_setext_text_line` refuses the
  row, and the table keeps its delimiter row and its widths.
- The lazy continuation: `- alpha … beta` over a hard-broken `delta epsilon`
  over `zeta eta` ended at column one on the first pass and two columns in on
  the second; `ParagraphState::note_indent` now carries the block's
  continuation indent across the flush, so both passes agree.
- The backslash hard break: a list item ending `… bbbb bbbbb\` wrapped to two
  continuation lines on one pass and three on the next, the last carrying the
  marker alone; `ParagraphWriter::append_stable_pending_prefix` measures the
  backslash as content, so the tail wraps on the marker's column budget.

### Evidence

The evidence for the classes and rules above is the tests the accepted
Consequences record, extended by this work.

- The fixture corpus under `tests/data/idempotence/` is formatted twice
  through the real binary by `tests/idempotence.rs` and asserted
  byte-identical, and `tests/idempotence_drift.rs` runs the three flag sets
  over every fixture under `tests/data/`, tables processed with
  `--code-emphasis` included.
- `tests/idempotence_properties.rs` is a `proptest!` property over generated
  documents and a sampled eight-flag powerset, and
  `tests/idempotence_adjacencies.rs` holds the structural-adjacency property
  and its coverage sweep. The generators both suites draw on live in
  `tests/support/idempotence_generators.rs` and the harness they share in
  `tests/support/idempotence_harness.rs`.
- The case count comes from `PROPTEST_CASES`, through the `proptest_config`
  helper in that harness, so the 48 both suites run at is a default rather than
  a ceiling: a longer sweep raises it without a recompile.
- The generated domain covers the shapes the added classes were reachable
  through: fence openers of three to five characters in both marker characters,
  with shorter interior runs and blocks that are never closed; ordered-list
  markers beyond `1.`, including multi-digit numbers, restarts, and nesting;
  table cells drawn from the whole `char` range, the characters the parser once
  used as placeholders included; and paragraphs carrying hard breaks and code
  spans longer than the wrap width.
- The structural-adjacency property generates a candidate directly above a
  thematic break with `--headings` forced on, and a fourth shape chains a
  converting paragraph, a thematic break, a table, and a delimiter row above
  the trailing break in one document, so each boundary's guard is exercised
  where its neighbours are guard cases too. The delimiter row is generated both
  alone and below a header row, and a deterministic sweep asserts the shape is
  reached and its row survives, so removing the generator branch fails the
  sweep rather than leaving the guard unexercised.
- Three regression pins record the shapes the sweeps shrank the classes to:
  `T7` in the fixture corpus, for the body row above a break;
  `src/wrap/paragraph_tests.rs`, for the lazy-continuation and backslash-tail
  documents the sweeps shrank to alongside a three-line item that reaches the
  lazy-continuation class on its own; and `src/table.rs`, for the lone-dash
  header row beside the empty one.

### Scope and exceptions

The guarantee remains scoped to the flag sets with recorded evidence: the
`make fmt` flag set (`--wrap`, `--renumber`, `--breaks`, `--ellipsis`,
`--fences`), that set with `--headings`, and that set with `--code-emphasis`.
The property suites broaden the ground within that claim — they force each flag
alone and sample the eight-flag powerset — but not outside it. Two exceptions
are of record:

- `--headings` sits outside the `make fmt` flag set, so the everyday gate does
  not reach it; the property suites force it on for the structural-adjacency
  shape instead.
- Under `--wrap` alone, a short alphabetic label such as `[a]` is not a
  reference at all: only ASCII digits couple to their opener, so the wrapper
  may still break between the bracket and the label. Such a document reaches
  the invariant one pass later than it should: the first pass ends the line with
  `[`, the second rejoins the two as `[ a]`, and the third reproduces the
  second. The bracket-reference fix recorded at the top of this addendum covers
  the digit-only shape; this residue pre-dates it and is tracked as issue #507.

### Superseded wording

The accepted-body wording this addendum replaces is:

- The defect-class count, "Seven defect classes contributed": there are now
  fifteen, the fourteen the Expanded defect classes section above records and
  the bracket-reference seam recorded at the top of this addendum.
- The rule count, "Seven rules enforce the invariant where it holds": there
  are now thirteen, as the New and revised rules section above records.
- The rule title "Table delimiter rows are refused separately from the block
  kinds": the rule is now "Table rows are refused separately from the block
  kinds", resting on `is_setext_text_line` and the shared classifier's
  table-delimiter decision.
- The rule opening "Content normalizers consumed by layout run before the
  layout they affect", in so far as it describes footnote conversion as one
  stage: the conversion is split, and the definition stage runs last.
- The Consequences sentence "Changed output is confined to thematic breaks
  that are now preserved instead of consumed": the list of changed shapes is
  now the longer one above, which also names the lazy continuation lines, the
  deferred tails ending in a backslash hard break, the unmatched fences, the
  footnote stages, and the delimiter-cell grammar.
- The Consequences sentences that name the fixture classes, the generator
  modules, and the sweep evidence: "the class `T` fixtures pin the
  delimiter-row adjacency", "Its repository-wide drift sweeps live in
  `tests/idempotence_drift.rs`", and "the generator both suites share lives in
  `tests/support/idempotence_harness.rs`". The Evidence section above replaces
  these with the `T7` pin, the two generator and harness modules, and the
  widened sweeps.
