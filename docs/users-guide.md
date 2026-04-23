# User guide

## Table reflow

`mdtablefix` reformats Markdown pipe tables so each column is aligned to a
uniform width. The formatter measures each cell using Unicode display width,
which means accented characters, CJK glyphs, and emoji stay visually aligned
after reflow.

Continuation rows are preserved during reflow. When a row starts with empty
leading cells because its content continues from the previous row, those empty
cells keep their original column positions instead of collapsing into the first
non-empty cell.

Literal pipe characters inside cells must be written as `\|`. `mdtablefix`
preserves that escaping during reflow, so a literal pipe remains part of the
cell content rather than being interpreted as a column boundary.

## Paragraph wrapping

The `--wrap` flag reflows paragraphs and list items to a target column width
(default 80). The wrap engine uses the `textwrap` crate for greedy line fitting
and `unicode-width` for display-column measurements, so accented characters,
CJK glyphs, and emoji are counted by their rendered width rather than their
byte length.

### Hyphenation

Hyphenated words are treated as indivisible during wrapping. A compound such as
`very-long-word` moves to the next line intact rather than being split at the
hyphen.

### Inline code and links

Inline code spans (`` `like this` ``) and Markdown links (`[text](url)`) are
kept as atomic units and are never broken across lines. Trailing punctuation
such as `.` or `,` that immediately follows a code span stays attached to it on
the same line.

### Task list indentation

Checkbox list items (`- [ ]` and `- [x]`) keep their continuation lines aligned
with the text column that follows the checkbox marker, not the leading dash.

### Fenced code blocks

Lines inside fenced code blocks (`` ``` `` or `~~~` delimiters) are always
preserved verbatim. The wrap engine does not attempt to reflow indented code
blocks either.

### Unicode display-width awareness

All width calculations use the Unicode display width of each character, which
means multi-column characters such as full-width CJK glyphs and emoji are
measured correctly when deciding where to break a line.

## Ellipsis handling

The `--ellipsis` flag replaces `...` inside table cells with the Unicode
ellipsis character `…` before the table is reflowed. This ensures column widths
are computed from the final emitted glyph rather than from the three-dot source
sequence.