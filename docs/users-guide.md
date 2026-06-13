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

When a table run is followed by a line that opens a new Markdown block — such
as a bullet list item, blockquote, link reference definition, or footnote
definition — `mdtablefix` flushes and reflows the buffered table before the new
block is processed. This applies even when the block-opening line itself
contains a pipe character; it is not treated as a table continuation row.
For example, after a table, `> quote | with pipe` starts a blockquote rather
than extending the table.

Pipe-looking lines indented by four or more columns are preserved as indented
code blocks. For example, a source line with four leading spaces before
`| not | a table |` is emitted verbatim rather than being table-reflowed.

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

Inline code spans (`` `…` ``), Markdown links (`[text](url)`), and inline GFM
footnote references (`[^label]`) are treated as unbreakable units. A span is
never split across lines; it moves as a whole to the next line when it would
otherwise exceed the target width.

Common English prose dates, such as `25th December 2025`, `19 March 2018`, and
`July 4, 2008`, are also treated as unbreakable inline fragments. This applies
to ordinal-day, numeric-day, and month-name-first forms with full or
abbreviated month names. If a date is wider than the configured wrap width, the
existing long-token fallback behaviour applies.

Parenthesized inline citations such as `pattern([1](url))` are also treated as
unbreakable units, keeping the citation link and its surrounding parentheses
together during wrapping.

When an inline code span is split across two or more soft-wrapped source lines,
`--wrap` first joins the continuation lines into a single span before applying
any line-length limit. The joined span is then treated as an indivisible unit:
no line break is inserted inside it, and the closing backtick always remains on
the same line as the span content. This behaviour applies in all prefixed
contexts — bulleted lists, ordered lists, blockquotes, and footnote definitions
— as well as in plain paragraphs.

An inline code span may itself contain backslash-escaped backticks — for
example `` `pass \`--file\` to the tool` ``. `--wrap` keeps the whole span,
including its escaped inner backticks, as a single atomic unit: it is never
split across lines, and the escaped backticks are preserved verbatim.

For list items, deferred inline code continuations use continuation indentation
rather than repeating the original list marker. This prevents a wrapped
checklist item from being reformatted as several independent checklist entries.

If joining a split inline code span would exceed the configured wrap width,
`mdtablefix` preserves the existing multi-line shape instead of emitting an
overlong line. Ambiguous close-and-reopen patterns are also preserved verbatim,
so the formatter does not introduce Markdownlint MD038 spacing violations or
change the intended code-span boundaries.

When `--wrap` is combined with `--renumber`, ordered list item boundaries are
preserved even when a list item contains a long inline code span. The wrapper
may leave the span on its existing continuation line, but it does not split a
single list item into new numbered steps or strand code-span fragments as
separate list items.

When a footnote reference immediately follows an inline code span or Markdown
link without intervening whitespace—for example `` `code`.[^ref] `` or
`[text](url).[^ref]`—the reference stays on the same line as the preceding
punctuation during wrapping. The same rule applies when opening punctuation is
coupled to the span, such as `` (`code`).[^ref] ``.

Inline GFM footnote references that immediately follow sentence punctuation are
also kept attached as unbreakable units. For example, `Sentence.[^ref]`,
`Sentence,[^ref]`, `Sentence?[^ref]`, and `Sentence"[^ref]` remain attached
during wrapping, and previously split paragraph text such as `Sentence. [^ref]`
or `Sentence.` followed by `[^ref]` is normalized back to `Sentence.[^ref]`.
Footnote definition lines such as `[^ref]: note text` remain definitions and
are not joined to preceding prose.

Opening brackets and other opening punctuation (`(`, `[`, and CJK openers such
as `（` and `「`) that immediately precede an inline code span or Markdown link
stay coupled to that span during wrapping. This prevents a lone opener from
being stranded at the end of a line before the code or link that follows it.

Inflectional affixes and possessives that appear immediately after a closing
inline code fence — for example `` `VarGuard`s ``, `` `class`'s ``,
`` `fetch`ed ``, or `` `run`ning `` — are kept on the same line as the code
span during wrapping. The tokenizer treats the backtick fence together with its
directly attached suffix as a single unbreakable unit, so no line break is ever
inserted between the closing backtick and the following letters.

Hyphenated compounds where a word ends with a hyphen immediately before an
inline code span — for example `` pre-`LLMPort` ``, `` LLM-`Port` ``, or
`` (API-`Foo`) `` — are also kept on the same line during wrapping. The
hyphen-prefix token is coupled to the following code span, so wrapping never
strands the prefix at the end of a line or the code span at the start of the
next. This mirrors the opening-punctuation coupling rule but applies to any
token that ends with a hyphen and contains at least one alphabetic character,
including Unicode alphabetic characters such as `` pré-`code` `` or
`` 字-`code` ``. If the compound alone exceeds the target width, it may be
broken. Trailing-hyphen compounds such as `` `code`-style `` continue to be
absorbed by the tokenizer at the closing fence.

When a Markdown link or inline code span is followed by trailing punctuation,
such as a full stop or comma, `mdtablefix` keeps that punctuation attached to
the same wrapped unit. It does not leave the punctuation orphaned on a line by
itself after wrapping.

Blockquote prefixes (`>`), task-list item markers (`- [ ]`, `- [x]`), and
footnote definition labels (`[^n]:`) are detected automatically. The first
wrapped line carries the original prefix; subsequent wrapped lines are indented
to the same visual column, so the text stays aligned.

Fenced code blocks, HTML blocks, indented code blocks (four or more leading
spaces or a leading tab), and table rows are passed through unchanged. Wrapping
is applied only to prose paragraphs and prefixed lines.

**Link reference definitions** — lines of the form `[label]: <URL>` or
`[label]: URL` (with an optional inline title) are left untouched by the reflow
pass. The definition line is preserved verbatim; when a valid standalone title
continuation line follows (a separate line containing only the title in quotes
or parentheses), that line is also preserved verbatim. Collapsed definitions
that place the label on one line and an indented destination on the next line
are also preserved, including the destination indentation:

```markdown
[users-guide]:
  docs/users-guide.md
```

The indented destination continuation is distinct from a standalone title
continuation. It must look like a link destination, so indented Markdown blocks
such as lists, blockquotes, and headings are still routed through normal block
wrapping.

Two trailing spaces at the end of a line produce a hard line break in rendered
Markdown. `mdtablefix --wrap` preserves those trailing spaces on the final
wrapped line, so hard-break semantics are not lost after reformatting.

Lines that consist entirely of whitespace — spaces, tabs, or any mixture — are
normalized to empty strings during wrapping. Such lines act as paragraph
boundaries and are never passed through with their original whitespace content,
so the output uses a single uniform separator between paragraphs regardless of
the input's incidental indentation.

When computing the indentation width for continuation lines in prefixed
contexts (blockquotes, lists, and footnote definitions), `mdtablefix` measures
the prefix using Unicode display width (`UnicodeWidthStr::width`) rather than
byte or character count. Continuation lines therefore stay correctly aligned
when the prefix contains full-width characters such as ideographic spaces or
CJK punctuation.

## HTML table conversion

`mdtablefix` converts `<table>…</table>` blocks that span multiple lines and
carry leading indentation into Markdown pipe tables. The leading indentation is
preserved on every emitted row, so the converted table sits at the same
indentation level as the original HTML. Surrounding non-table lines at that
same indentation level are passed through unchanged. Nested `<table>` tags are
tracked by depth, so the buffered structure is converted only once the outermost
`</table>` is reached and never split into two separate conversions.

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
and drops the blank lines when attachment succeeds. Specifiers that follow
prose or other content are intentionally not attached. If no suitable fence
follows, the specifier line and any intervening blank lines are left unchanged,
preserving document spacing. Orphan-specifier attachment only happens when the
identifier line starts a block and both the identifier line and the target
fence are outside any already-open fenced block.

Before:

`````markdown
````markdown
```rust
fn main() {}
```
````
`````

After running `mdtablefix --fences`:

`````markdown
````markdown
```rust
fn main() {}
```
````
`````

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

## Library API notes

### `format_breaks` return type

`format_breaks` returns `Vec<Cow<'_, str>>` rather than `Vec<String>`. Lines
that are not thematic breaks are returned as `Cow::Borrowed` slices into the
input, avoiding heap allocations for unchanged content. Synthesized
thematic-break lines are also `Cow::Borrowed`, borrowing from a shared static
buffer.

Callers that need owned `String` values must call `.into_owned()`:

<!-- markdownlint-disable-next-line MD046 -->
```rust
use mdtablefix::format_breaks;

let lines = vec!["some text".to_string(), "---".to_string()];
let owned: Vec<String> = format_breaks(&lines)
    .into_iter()
    .map(|c| c.into_owned())
    .collect();
```
