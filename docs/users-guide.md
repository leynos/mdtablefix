# User guide

## Command-line usage

```bash
mdtablefix [--wrap] [--renumber] [--breaks] [--ellipsis] [--fences]
          [--footnotes] [--code-emphasis] [--headings]
          [--in-place | --check | --diff] [FILE...]
```

Every named file is formatted, and the result is printed to standard output. If
no file is named, the document is read from standard input and the formatted
text is written to standard output. The behaviour of each formatting flag is
described in the sections that follow.

| Flag               | Effect                                                    |
| ------------------ | --------------------------------------------------------- |
| `--wrap`           | Reflow paragraphs and list items to 80 columns.           |
| `--renumber`       | Renumber ordered lists sequentially.                      |
| `--breaks`         | Rewrite thematic breaks as a line of 70 underscores.      |
| `--ellipsis`       | Replace `...` with the ellipsis character.                |
| `--fences`         | Normalize fenced code blocks where compression is safe.   |
| `--footnotes`      | Convert bare numeric references into footnote links.      |
| `--code-emphasis`  | Repair emphasis markers that adjoin inline code.          |
| `--headings`       | Convert Setext headings to hash-prefixed headings.        |
| `--in-place`       | Rewrite each named file instead of printing it.           |
| `--check`          | Report each file that would be reformatted, without       |
|                    | writing.                                                  |
| `--diff`           | Print a unified diff for each file that would be          |
|                    | reformatted.                                              |
| `--version`        | Print the version and exit.                               |

_Table 1: The command-line flags._

### The three file modes

`--in-place`, `--check`, and `--diff` act on the files named on the command
line, and at most one of them may be given. Each requires at least one file:
`mdtablefix --check` on its own is a usage error, because there is no file for
the mode to act on. With no mode flag the tool prints the formatted text, which
is what makes `mdtablefix FILE` and `cat FILE | mdtablefix` interchangeable.

`--check` prints one line per file that would be reformatted, and `--diff`
prints a unified diff per file that would be reformatted. Neither writes
anything: a clean file prints nothing at all. `--in-place` rewrites only the
files whose bytes would change, so a file that is already formatted keeps its
inode and its modification time. See
[In-place editing](#in-place-editing) for the replacement guarantees.

Standard output is the machine-readable half of the contract: report lines for
`--check`, diffs for `--diff`, formatted text otherwise. The summary
(`2 files would be reformatted, 1 file left unchanged.`) and every error go to
standard error, so standard output can be piped or captured on its own.

### Exit status

| Status | Meaning                                                            |
| ------ | ------------------------------------------------------------------ |
| `0`    | Every file was analysed, and no reporting mode found drift.        |
| `1`    | `--check` or `--diff` found a file that would be reformatted.      |
| `2`    | A file could not be read or rewritten, or the command line was     |
|        | rejected.                                                          |

_Table 2: The exit statuses._

An error outranks drift: a run that could not read one file exits `2` even when
another file drifted, so a gate never reports a clean tree from an incomplete
analysis. Drift is a failure only under `--check` and `--diff`; `--in-place`
over drifting files exits `0`, because the drift was the work it was asked to
do. A run whose standard output is closed early — the shape produced by
`mdtablefix --check *.md | head` — exits `0` rather than failing, because the
reader stopped early and that says nothing about the files.

### Reading a report line

A report line is the file's path followed by the line delta of the rewrite, as
two further space-separated fields:

```text
docs/users-guide.md +25 -25
```

Read it from the right: the last field is the deletion count with a `-` prefix,
the field before it is the insertion count with a `+` prefix, and the path is
everything before those two, so a path containing spaces needs no quoting. The
counts come from a line diff between the file's bytes and the text a rewrite
would write, so they count lines replaced rather than edits a reviewer would
tally by hand.

### Line endings and byte-order marks

`mdtablefix` writes the line-ending style that holds the majority of the
document's line endings, and preserves a leading byte-order mark (BOM) only
when the file already has one. Both rules apply in every mode, so `--check`,
`--diff`, and `--in-place` agree on the bytes a file would end up with. See
[Line endings](#line-endings) for the detection rule and its tie-breaks.

Two consequences are worth knowing before running the tool over a repository.

- Detection covers the whole document, including fenced code blocks. A
  predominantly CRLF document whose code samples use LF endings has those
  samples rewritten to CRLF, which is a change to the content of the code
  rather than to its formatting.
- A lone carriage return is content, not a line ending. A file that separates
  its lines with `\r` alone is therefore treated as a single line.

### Trailing newlines

A non-empty result always ends with one line ending, so a file whose last line
is unterminated gains a terminator and is reported as drift, as `+1 -1`. The
rule applies to every mode, including standard input, which prints the bare
terminator even when the input produces no lines; an empty file stays empty.

### Paths that match nothing

`mdtablefix` takes file paths, not patterns, and does no discovery of its own.
A shell expands a glob before the tool sees it, so `mdtablefix --check *.md` in
a directory with no Markdown files passes the literal pattern `*.md`, which is
then reported as an unreadable path and exits `2`. Shells differ in whether
that happens: `sh` and `bash` pass an unmatched pattern through literally,
while `zsh` — and `bash` under `failglob` — refuse to run the command at all,
so the exit `2` above belongs to the shells that let the pattern through. The
behaviour is deliberate either way: a run asked to check a set of files and
checking none of them has not earned a clean tree. A gate should expand the
list in a way that runs the tool only when there is something to check:

```bash
fd -e md -X mdtablefix --check
find . -name '*.md' -print0 | xargs -0 -r mdtablefix --check
```

Both run the tool once with every match, and run nothing at all — exiting `0` —
when there are no matches.

### Symbolic links

A read follows a symbolic link, so `--check` and `--diff` report a link's target
like any other file. A write declines the link rather than replacing it,
because the rename would turn the link into a regular file; the run reports the
declined link, fails that file, and exits `2`. A link to a file that needs no
changes is not written at all, so it succeeds. See
[In-place editing](#in-place-editing) for the full replacement contract.

## Footnote conversion

`mdtablefix --footnotes` converts eligible prose outside fenced code blocks,
including bare numeric candidates and the final numbered list, into Markdown
footnote links. Footnote references (`[^n]`), definitions (`[^n]:`), and
numeric candidates inside fenced code blocks remain unchanged. Fenced regions
are recognised using matching marker characters and run lengths; a closer must
also have no info string. A fence opened inside a blockquote ends when the
blockquote depth drops below its opening depth.

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
contains a pipe character; it is not treated as a table continuation row. For
example, after a table, `> quote | with pipe` starts a blockquote rather than
extending the table.

Pipe-looking lines indented by four or more columns are preserved as indented
code blocks. For example, a source line with four leading spaces before
`| not | a table |` is emitted verbatim rather than being table-reflowed.

Content inside fenced code blocks is treated the same way. A line beginning with
`|` inside such a block — for example a shell pipeline continuation such as
`| tee /tmp/test.log` — is never treated as a Markdown table row and never
gains an appended trailing `|`.

Literal pipe characters inside cells must be written as `\|`. `mdtablefix`
preserves that escaping during reflow, so a literal pipe remains part of the
cell content rather than being interpreted as a column boundary.

Table-cell payloads preserve arbitrary Unicode characters, including U+001D and
U+001F, during reflow.

## YAML frontmatter

When a document begins with a valid YAML frontmatter block, `mdtablefix`
preserves the entire block verbatim and formats only the Markdown body that
follows it. A frontmatter block begins with `---` as the first line and ends
with `---` or `...`.

This applies to standard CLI formatting as well as options such as `--wrap`,
`--renumber`, and `--breaks`; in particular, `--breaks` never rewrites the
frontmatter delimiters. The library stream functions provide the same
preservation behaviour. Advanced library callers can use
`process_with_frontmatter` to run their own body transform without receiving
the frontmatter lines.

## Ellipsis handling

The `--ellipsis` flag replaces `...` inside table cells with the Unicode
ellipsis character `…` before the table is reflowed. This ensures column widths
are computed from the final emitted glyph rather than from the three-dot source
sequence.

Replacement also runs before paragraph wrapping, so `--wrap` measures the
emitted `…` glyph rather than the three-dot source sequence. A paragraph near
the wrap boundary therefore breaks in the same place on every run.

Literal dot sequences in inline code, fenced code blocks, and four-space or
tab-indented code blocks remain unchanged. An indented code block must start at
the document boundary or after a blank line, heading, closed fenced block, link
reference definition, or markdownlint directive. This matches Markdown's rule
that an indented code block cannot interrupt a paragraph.

The flag also preserves `...` where the dots are semantically significant:
inline links and images, autolinks, bare URLs, link reference definitions and
their destination or title continuation lines, and filesystem-like tokens are
passed through unchanged. For example, a GitHub compare destination containing
`v1...v2` remains a valid URL even when it follows its reference label on the
next line.

## Code-emphasis handling

The `--code-emphasis` flag repairs emphasis markers adjoining inline code
spans. Inside buffered table cells, the repair runs before column widths are
measured, so the reflow uses the final cell text and produces aligned output in
the same pass.

For non-table content, `--code-emphasis` retains its existing behaviour and
repairs the same inline-code and emphasis-marker combinations as before.

## Paragraph wrapping

Pass `--wrap` to reflow prose paragraphs so that every output line fits within
80 display columns. Width is measured in terminal columns, not bytes, so the
wrapper accounts correctly for CJK glyphs, emoji, and accented characters. The
flag does not accept a width parameter.

Line fitting is delegated to the `textwrap` crate using a greedy first-fit
algorithm: each word is placed on the current line if it fits, and a new line
is started otherwise. This produces predictable, diff-friendly output.

Inline code spans (`` `…` ``), Markdown links (`[text](url)`), and inline GFM
footnote references (`[^label]`) are treated as unbreakable units during normal
line fitting. `mdtablefix --wrap` never introduces a line break within an
inline code span; the span otherwise moves as a whole to the next line when it
would exceed the target width. The exception for an already cross-line overlong
span is described below.

Reference-style links such as `[text][reference]` are likewise unbreakable. The
opening `[` always stays with the link label, avoiding leading whitespace
inside link text after continuation indentation is applied.

A colon-suffixed footnote reference in running prose, such as
`subcategories [^96]:`, remains attached to the preceding word. The wrapper
therefore cannot move `[^96]:` to column one, where Markdown would reinterpret
the reference as a footnote definition.

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
the line-length limit. When the joined span fits, the remainder of the
paragraph is greedily reflowed during the same pass, including later
continuation lines in a list item. Running `--wrap` again therefore produces no
further changes.

Thematic breaks act as block boundaries. A line of three or more `-`, `*`, or
`_` characters is passed through on its own line and never absorbed into the
surrounding paragraph, with or without `--breaks`. This includes spaced runs
such as `- - -` and the seventy-underscore line that `--breaks` writes. A
table separator row such as `| --- | --- |` still contains pipes and is
reflowed with its table rather than treated as a break.

When the first line of a prefixed block spills past the target width, the
wrapper keeps that block open so its continuation and lazy continuation lines
reflow with the tail in the same pass rather than being joined to it on a later
run. The prefixed forms are list items, task items, ordered items, blockquotes,
and footnote definitions. Folding only happens when the tail would reparse as
paragraph text, so a tail indented by four or more columns (indented code), a
tail that repeats its blockquote marker, and a footnote definition tail stay
separate blocks. The formatter therefore reaches its final form in one pass, so
a second run makes no further changes.

The wrapper never introduces a new line break inside an inline-code span. When
joining the span would exceed the configured width and each authored line
already fits, however, it may retain the authored boundaries inside that span.
Markdown renders those retained soft breaks as spaces, while the physical lines
remain within the limit. These rules apply in all prefixed contexts — bulleted
lists, ordered lists, blockquotes, and footnote definitions — as well as in
plain paragraphs.

An inline code span may itself contain backslash-escaped backticks — for example
`` `pass \`--file\` to the tool` ``. `--wrap` keeps the whole span, including
its escaped inner backticks, opaque to ordinary line fitting. It does not
introduce a split inside the span, and the escaped backticks are preserved
verbatim; an already conforming authored boundary may still be retained under
the overlong-span rule above.

For list items, deferred inline code continuations use continuation indentation
rather than repeating the original list marker. This prevents a wrapped
checklist item from being reformatted as several independent checklist entries.

Ambiguous close-and-reopen patterns are preserved verbatim, so the formatter
does not introduce Markdownlint MD038 spacing violations or change the intended
code-span boundaries.

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

Blockquote prefixes (`>`) are parsed before their inner content at every
nesting depth, including compact (`>>`) and spaced (`> >`) forms. Fenced code
blocks, inline code spans, task-list item markers (`- [ ]`, `- [x]`), ordinary
list markers, and footnote definition labels (`[^n]:`) therefore retain their
usual meaning inside blockquotes. Wrapped prose repeats the complete blockquote
prefix. For a list inside a blockquote, continuations repeat the blockquote
portion and replace only the list marker with an alignment indent.

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

## Line endings

`mdtablefix` preserves the line-ending style of the document it formats. It
counts carriage return and line feed (CRLF) (`\r\n`) and lone line feed (`\n`)
endings in the input and terminates every output line with whichever style
holds the strict majority. A file authored with Windows line endings therefore
stays CRLF, and a file authored with Unix line endings stays LF; formatting
never converts a consistently ended file to the other style.

When the two styles occur equally often, and when a non-empty input contains no
line ending at all, `mdtablefix` emits LF. That tie-break is deterministic: it
does not depend on which style appears first.

Detection covers the whole document, including fenced code blocks. A document
whose endings are predominantly CRLF is emitted entirely as CRLF, so a code
sample authored with LF endings inside such a document is rewritten to CRLF.

Standard input is treated the same way: the style detected on standard input
selects the endings written to standard output. Empty standard input still
prints a single line ending, as it always has, while an empty file still
produces empty output.

## Heading conversion

Pass `--headings` to convert Setext headings into ATX headings. A Setext
heading is a paragraph line followed by an underline of three or more identical
`=` or `-` characters; the pair becomes one ATX line, `#` for an `=` underline
and `##` for a `-` underline. The flag is off unless it is passed and is not
part of the `make fmt` flag set.

A candidate line is converted only when it is paragraph text. A line that is
already a Markdown block start keeps its underline, so the line below it
survives: an ATX heading, a thematic break, a list item, a blockquote, a
footnote or link reference definition, a markdownlint directive, or a
fenced-code marker.

A table delimiter row is refused too. It is table syntax rather than paragraph
text, so the `---` beneath it is a thematic break and not an underline, and the
table above it keeps its alignment row.

Indentation and blockquote markers shared by the heading and its underline are
preserved, so `> Title` above `> -----` becomes `> ## Title`.

A candidate indented by four or more columns, or one inside a blockquote and
indented four or more columns after the marker, is an indented code block: it
is left untouched, as is the line beneath it. Three columns or fewer still
convert. Tabs count as four columns.

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

A fence closes only on a bare marker line (`` ``` `` or `~~~`) optionally
followed by ASCII spaces or tabs. A same-marker line that also carries an info
string — trailing non-whitespace text — is literal content rather than a
closing fence, per CommonMark. This keeps pipe lines and other content inside
such blocks verbatim.

If a language specifier starts a block, either at the start of the file or
immediately after a blank line, and appears before the next unlabelled opening
fence with only blank lines in between, `mdtablefix` attaches it to that fence
and drops the blank lines when attachment succeeds. Specifiers that follow
prose or other content are intentionally not attached. If no suitable fence
follows, the specifier line and any intervening blank lines are left unchanged,
preserving document spacing. Orphan-specifier attachment only happens when the
identifier line starts a block and both the identifier line and the target
fence are outside any already-open fenced block.

A thematic break is never attached as a specifier, even though a run of
underscores or hyphens matches the specifier pattern. The break line and the
blank lines after it are preserved, so a break directly above a code fence is
not merged into that fence on a later pass.

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

## In-place editing

Pass `--in-place` to rewrite each named file with the formatted result. The
formatted output is written to a temporary file in the same directory as the
target and then renamed over it, so the replacement is atomic on POSIX
filesystems: a reader sees either the whole original file or the whole
replacement, never a partial write. When the run fails before the rename — a
full disk, a permission error, or a declined target — the original file is left
byte-identical and any temporary file is removed on a best-effort basis, so the
run can be retried safely. A run killed abruptly, by `SIGKILL` or a power loss,
can leave a stale temporary file beside the target; the original is still
intact, and the next run retries past the stale name rather than reusing it.
Stale files are named `<target>.mdtablefix-<pid>-<n>.tmp`. Delete them once no
run is in progress.

When an in-place rewrite fails, `mdtablefix` reports the full error chain on
standard error: first the file context, naming the path exactly as given on the
command line, then the underlying cause beneath a `Caused by:` heading. Every
failing file is reported this way, and the run then exits with a non-zero
status. Scripts that match exact standard-error text should expect the chain
and its multi-line form; matching the file name or the cause is more robust.

The original file mode is preserved. A freshly created temporary file does not
inherit the target's permissions, so `mdtablefix` copies them to the temporary
file before the rename: a file with mode `0640` still has mode `0640`
afterwards. Because the replacement is a rename, it needs write permission on
the containing directory rather than on the file itself, so a read-only file in
a writable directory is replaced successfully, and the replacement takes over
the read-only state rather than losing it.

Windows needs one step more than that. There, read-only is a file attribute,
`FILE_ATTRIBUTE_READONLY`, and the rename cannot replace a destination that
carries it. `mdtablefix` therefore clears that attribute on the destination
through its directory capability immediately before the rename. The temporary
file still carries the original read-only attribute, so the file that takes over
the target's name is read-only as soon as the rename lands. If the swap does not
complete, the original attribute is put back on a best-effort basis: a run
interrupted between those two steps, or a restoration that itself fails, can
leave the target's read-only attribute cleared. The contents are unaffected,
because a swap that does not complete leaves the original file byte-identical.

Symbolic links are declined rather than replaced. The read follows the link, but
the rename swaps the link entry itself, which would turn the symlink into a
regular file while leaving the real file untouched. The run reports the declined
link on standard error; rewrite the link's target directly instead. A link whose
target resolves outside the file's directory is refused by the directory
capability before the rewrite begins.

Two limitations apply. On Windows the replacement can fail if another process
holds the destination open without delete sharing, because the rename cannot
displace an open handle. Atomicity is also not durability: the new contents are
flushed to storage before the rename, but the rename itself is not, so a power
loss immediately afterwards can revert the directory entry to the original
file.

## Library API notes

### Atomic in-place rewrites

`rewrite(path)` and `rewrite_no_wrap(path)` give library callers the same
guarantee as `--in-place`: the replacement is written to a temporary file beside
the target, flushed, and renamed over it, with the original file mode preserved.
The temporary file receives the target's permissions before the rename, so a
read-only target is replaced by a read-only file rather than by a writable one.
On Windows, where the destination's `FILE_ATTRIBUTE_READONLY` blocks the rename
outright, that attribute is cleared immediately before the rename and put back
if the swap does not complete. Symbolic links are declined, as described in
[In-place editing](#in-place-editing).

Callers that already hold a `cap_std::fs_utf8::Dir` capability can use
`mdtablefix::io::replace_file(directory, path, contents)` instead. It performs
the same temporary-file-and-rename sequence relative to the supplied directory,
so no ambient filesystem access is needed. The CLI and the two path helpers all
call it, so the sequence has one implementation.

### Replacement metrics

The library emits six metrics for its replacement path: five counters and one
histogram. It installs no recorder or subscriber of its own; a host
application installs one to collect these metrics, and a host that installs
none sees no behavioural change.

- `mdtablefix_io_replace_total` — replacements attempted, labelled by
  `outcome` (`success` or `failure`).
- `mdtablefix_io_replace_duration_seconds` — histogram of replacement
  durations in seconds, labelled by `outcome`.
- `mdtablefix_io_temporary_name_collisions_total` — candidate temporary names
  rejected because they were already taken.
- `mdtablefix_io_temporary_name_exhausted_total` — replacements abandoned
  because every candidate temporary name was taken.
- `mdtablefix_io_temporary_cleanup_failures_total` — temporary files a failed
  replacement could not remove.
- `mdtablefix_io_symlink_declined_total` — symbolic-link targets declined with
  `InvalidInput` rather than replaced.

Cleanup is best effort, so the caller still sees the failure that prompted it;
the cleanup counter is the only signal that a stale temporary file was left
beside the target. No temporary file is created for a declined link.

Metric names are stable, and labels are bounded: `outcome` is the only label
key, carried only by the two replacement metrics above. No path, file name, or
error text is ever used as a label, so a recorder's cardinality stays bounded.

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

### Line-ending helpers

`LineEnding` is the closed set of terminators the formatter emits.
`LineEnding::Lf` is `\n` and `LineEnding::Crlf` is `\r\n`. The `const fn`
`LineEnding::as_str` returns the characters written between lines.

`detect_line_ending(text) -> LineEnding` selects the style holding the strict
majority of the text's line endings. CRLF pairs are counted first and
subtracted from the line-feed count to obtain the lone line feeds; CRLF wins
only when it strictly outnumbers lone line feeds. An exact tie, and a non-empty
input with no line endings at all, select `LineEnding::Lf`. Only CRLF and lone
LF are recognized: a lone carriage return is content, matching the `str::lines`
split.

`count_line_endings(text) -> LineEndingCounts` returns the selection together
with the counts that decided it. `LineEndingCounts::ending` is the selected
style, `crlf_count` counts CRLF pairs, and `lone_lf_count` counts lone line
feeds, so a caller that reports or acts on the vote does not restate the
counting rule.

The boundaries that act on the decision (`rewrite`, `rewrite_no_wrap`, and the
CLI's file and standard-input boundaries) each emit one `debug` event,
`selected the majority line ending`, with the fields `operation`, `path`,
`crlf_count`, `lone_lf_count`, and `selected_ending`. The event is visible to
anyone who enables `debug` logging.

`serialize_lines(lines, ending) -> String` joins the processed lines with the
selected terminator and appends one further terminator, so a non-empty result
always ends with a line ending; an empty slice yields an empty string.

See [Line endings](#line-endings) for the user-facing behaviour.

<!-- markdownlint-disable-next-line MD046 -->
```rust
use mdtablefix::{
    LineEnding, count_line_endings, detect_line_ending, serialize_lines,
};

let counts = count_line_endings("alpha\r\nbeta\r\ngamma\n");
assert_eq!(counts.ending, LineEnding::Crlf);
assert_eq!(counts.crlf_count, 2);
assert_eq!(counts.lone_lf_count, 1);

let ending = detect_line_ending("alpha\r\nbeta\r\n");
assert_eq!(ending, LineEnding::Crlf);

let lines = vec!["| A |".to_string(), "| 1 |".to_string()];
assert_eq!(serialize_lines(&lines, ending), "| A |\r\n| 1 |\r\n");
assert!(serialize_lines(&[], ending).is_empty());
```

### Reporting: line deltas and unified diffs

`mdtablefix::report` is the pure half of `--check` and `--diff`. Nothing in it
opens a path, chooses an output stream, or defines an error type: it counts
what changed and renders it into a writer the caller supplies, so a library
caller produces exactly what the command line reports without restating the
counting rule.

`LineDelta::between(original, formatted)` counts the lines that turning
`original` into `formatted` would insert and delete. A modified line counts as
one insertion and one deletion, matching `git diff --numstat`, and the counts
are read back with `insertions()`, `deletions()`, and `has_changes()`. The
count comes from the same tokenizer that renders the diff, so the two cannot
disagree about where a line ends. Callers compare bytes first: equal texts need
no diff work, and `between` is not written to be called on them.

`render_report_line(display_path, delta)` renders `--check`'s one-line form,
for example `docs/a.md +12 -8`. Consumers parse it by taking the final two
whitespace-separated fields as the counts and everything before them as the
path, as [Reading a report line](#reading-a-report-line) describes.
`render_summary(changed, unchanged, errored)` renders the trailing summary, for
example `2 files would be reformatted, 1 file left unchanged.`; it belongs on
standard error, so standard output stays a machine contract.

`write_unified_diff(out, display_path, original, formatted, options)` streams
`--diff`'s output to any `io::Write`. Both headers name `display_path` with
directory separators normalized to `/` and carry no timestamps, so the same
input renders the same bytes on every platform. The
`\ No newline at end of file` marker is preserved, and can only appear on the
`-` side because the formatter always terminates the lines it emits. Output is
never colourized. `DiffOptions` fixes the context radius and the line count
above which rendering switches from Myers to Patience, so diffing a very large
file stays bounded without a wall-clock cut-off.

`FileReport` is one file's analysis as a value — its display path, whether the
formatter would change it, and its `LineDelta` — so a caller can render or
aggregate it rather than parsing rendered text back apart.

<!-- markdownlint-disable-next-line MD046 -->
```rust
use camino::Utf8Path;
use mdtablefix::report::{
    DiffOptions, LineDelta, render_report_line, write_unified_diff,
};

let original = "|A|B|\n";
let formatted = "| A | B |\n";

let delta = LineDelta::between(original, formatted);
assert_eq!((delta.insertions(), delta.deletions()), (1, 1));
assert!(delta.has_changes());
assert_eq!(
    render_report_line(Utf8Path::new("ragged.md"), delta),
    "ragged.md +1 -1"
);

let options = DiffOptions {
    context_radius: 3,
    patience_threshold: 1000,
};
let mut diff = Vec::new();
write_unified_diff(
    &mut diff,
    Utf8Path::new("ragged.md"),
    original,
    formatted,
    options,
)
.expect("writing to a Vec cannot fail");
assert_eq!(
    String::from_utf8(diff).expect("the diff is UTF-8"),
    "--- ragged.md\n+++ ragged.md\n@@ -1 +1 @@\n-|A|B|\n+| A | B |\n"
);
```

### Byte-order marks and the document boundary

`mdtablefix::io::SourceDocument` binds a document's boundary concerns to the
text they were read from. A byte-order mark and the line-ending style are
boundary concerns rather than content concerns: both are split off before
formatting and restored afterwards, so every content transform sees the same
lines however the file was authored, and a Windows-authored file is not
silently rewritten to line feeds.

`SourceDocument::parse(content)` strips one leading `U+FEFF` and counts the
line endings of what remains. Splitting the mark off matters beyond fidelity:
left attached to the first line it defeats every content transform, which would
make `--check` report a ragged file as clean. Counting over the body rather
than over the whole input is deliberate — the mark is not a line ending, and on
a document that is only a mark it would otherwise be the sole reason a majority
existed.

`body()` borrows that text: the mark gone, the endings intact. It is what a
content transform is given. `counts()` returns the `LineEndingCounts` that
decided the style, and `ending()` returns the selection alone, so a caller that
reports the vote and a caller that only needs the terminator read the same
answer.

`render(lines)` writes `lines` back with the mark and the selected terminator.
It is a method rather than a free function so a caller cannot render one
document's lines with another document's style. An empty slice renders the mark
alone, or nothing when the document had none, so a document that formats to no
lines stays byte-identical to its input: `--check` remains a fixed point and
`--in-place` cannot destroy the mark.

<!-- markdownlint-disable-next-line MD046 -->
```rust
use mdtablefix::io::SourceDocument;

let document = SourceDocument::parse("\u{FEFF}|A|B|\r\n|1|2|\r\n");
assert_eq!(document.body(), "|A|B|\r\n|1|2|\r\n");
assert_eq!(document.ending().as_str(), "\r\n");
assert_eq!(document.counts().crlf_count, 2);

let formatted = ["| A   | B   |".to_string(), "| --- | --- |".to_string()];
assert_eq!(
    document.render(&formatted),
    "\u{FEFF}| A   | B   |\r\n| --- | --- |\r\n"
);

// A document that formats to no lines renders as nothing, not as a bare
// terminator, so a mark-only document survives unchanged.
assert!(SourceDocument::parse("").render(&[]).is_empty());
```
