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

## Ellipsis handling

The `--ellipsis` flag replaces `...` inside table cells with the Unicode
ellipsis character `…` before the table is reflowed. This ensures column widths
are computed from the final emitted glyph rather than from the three-dot source
sequence.

## Paragraph wrapping

Pass `--wrap <width>` to reflow prose paragraphs so that every output line fits
within `<width>` display columns. The width argument is measured in terminal
columns, not bytes, so it accounts correctly for CJK glyphs, emoji, and
accented characters.

Line fitting is delegated to the `textwrap` crate using a greedy first-fit
algorithm: each word is placed on the current line if it fits, and a new line
is started otherwise. This produces predictable, diff-friendly output.

Inline code spans (`` `…` ``) and Markdown links (`[text](url)`) are treated as
unbreakable units. A span is never split across lines; it moves as a whole to
the next line when it would otherwise exceed the target width.

Blockquote prefixes (`>`), task-list item markers (`- [ ]`, `- [x]`), and
footnote definition labels (`[^n]:`) are detected automatically. The first
wrapped line carries the original prefix; subsequent wrapped lines are indented
to the same visual column, so the text stays aligned.

Fenced code blocks, HTML blocks, indented code blocks (four or more leading
spaces or a leading tab), and table rows are passed through unchanged. Wrapping
is applied only to prose paragraphs and prefixed lines.

Two trailing spaces at the end of a line produce a hard line break in rendered
Markdown. `mdtablefix --wrap` preserves those trailing spaces on the final
wrapped line, so hard-break semantics are not lost after reformatting.

## Fence normalization

Pass `--fences` to normalize fenced code blocks before later processing. Safe
outer fences are compressed to three backticks, which keeps simple code blocks
consistent before later formatting steps run. Indentation and any language
identifiers are preserved.

Outer delimiters are compressed only when doing so is structurally safe. If
normalization would turn an inner literal fence into a structural close, the
outer fence is kept, so the inner content remains literal. Preservation applies
when the inner fence uses the same marker character as the outer fence, or when
a tilde outer fence wraps a literal inner backtick fence.

If a language specifier starts a block, either at the start of the file or
immediately after a blank line, and appears before the next unlabelled opening
fence with only blank lines in between, `mdtablefix` attaches it to that fence
and drops the blank lines when attachment succeeds. Specifiers that follow prose
or other content are intentionally not attached. If no suitable fence follows,
the specifier line and any intervening blank lines are left unchanged,
preserving document spacing. Orphan-specifier attachment only happens when the
identifier line starts a block and both the identifier line and the target fence
are outside any already-open fenced block.

Before:

    ````markdown
    ```rust
    fn main() {}
    ```
    ````

After running `mdtablefix --fences`:

    ````markdown
    ```rust
    fn main() {}
    ```
    ````

Before:

    ````rust
    fn main() {}
    ````

After running `mdtablefix --fences`:

    ```rust
    fn main() {}
    ```
