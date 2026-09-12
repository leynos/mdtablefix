# mdtablefix

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](
https://deepwiki.com/leynos/mdtablefix)

`mdtablefix` unb0rks and reflows Markdown tables so that each column has a
uniform width. When the `--wrap` option is used, it also wraps paragraphs and
list items to 80 columns.

Hyphenated words are treated as indivisible during wrapping, so
`very-long-word` will move to the next line intact rather than split at the
hyphen. The wrap engine now delegates line fitting to the `textwrap` crate
while preserving Markdown-aware token grouping for inline code, links, GFM
footnote references, parenthesized inline citations such as
`pattern([1](url))`, common English date sequences such as
`25th December 2025`, and hard breaks. The tool ignores fenced code blocks and
respects escaped pipes (`\|`), making it safe to use on Markdown with mixed
content.

## Installation

Install via Cargo:

```bash
cargo install mdtablefix
```

Install a prebuilt release archive with `cargo-binstall`, which never compiles
the crate:

```bash
cargo binstall --disable-strategies compile mdtablefix
```

Prebuilt archives are published for these targets:

| Platform | Targets                                                 |
| -------- | ------------------------------------------------------- |
| Linux    | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` |
| macOS    | `x86_64-apple-darwin`, `aarch64-apple-darwin`           |
| Windows  | `x86_64-pc-windows-msvc`                                |
| FreeBSD  | `x86_64-unknown-freebsd`                                |

Every archive is accompanied by a `.sha256` sidecar in `sha256sum` format.

Or clone the repository and build from source:

```bash
cargo install --path .
```

## Command-line usage

```bash
mdtablefix [--wrap] [--renumber] [--breaks] [--ellipsis] [--fences]
          [--footnotes] [--code-emphasis] [--headings]
          [--in-place | --check | --diff | --list-files] [FILE...]
          [--git [--include-untracked] [--md-exts EXT[,EXT...]]
                 [--allow-conflicted]]
```

One or more file paths are formatted and printed to standard output; with no
file path at all, the document is read from standard input instead. The
formatting flags are independent and may be combined.

Four flags select what happens to the files. `--in-place` rewrites each file
through a temporary file and a rename, preserving its mode. `--check` reports
every file that would be reformatted, `--diff` prints a unified diff for each
of them, and `--list-files` prints each selected path without opening it; none
of the three writes anything. Every mode exits `0` when no file would change,
`--check` and `--diff` exit `1` when a file would, and every mode exits `2`
when a file could not be read or rewritten or the selection itself failed.

With `--git`, the files are the ones Git reports beneath the current directory
rather than the ones named on the command line: the tracked Markdown files in
the index, plus the untracked ones with `--include-untracked`. The selection is
sorted and deduplicated, and `--list-files` reports it without touching a file,
so a repository-wide run is one invocation with one summary and one exit
status:

```bash
mdtablefix --git --in-place --wrap
```

The files that Git reports but that cannot safely be rewritten — a symbolic
link, or a document carrying conflict markers while a merge, rebase, or
cherry-pick is in progress — are skipped or refused rather than rewritten. See
[Git file selection](docs/architecture.md#git-file-selection) for how the
selection is made.

See the [user's guide](docs/users-guide.md#command-line-usage) for every flag,
the exit-status contract, line-ending and byte-order-mark behaviour, and the
limitations worth knowing before running the tool over a repository.

## YAML frontmatter

Documents that begin with a YAML frontmatter block have that block preserved
exactly while the remainder of the document is formatted. A frontmatter block
starts with a line containing exactly `---` and ends with a line containing
exactly `---` or `...`. Only a block at the very beginning of the document is
recognized as frontmatter.

Before:

```markdown
---
title: My Document
author: Jane Doe
---
|Character|Catchphrase|
|---|---|
|Speedy|Here come the cats!|
```

After running `mdtablefix`:

```markdown
---
title: My Document
author: Jane Doe
---
| Character | Catchphrase         |
| --------- | ------------------- |
| Speedy    | Here come the cats! |
```

## Concurrency

When multiple file paths are supplied, `mdtablefix` processes them in parallel
using the [`rayon`](https://docs.rs/rayon) crate. The CLI buffers each result,
so it can print them in the original order. This buffering uses extra memory.
It might outweigh the speed gains for small files.

### Example: Table Reflowing

Before:

```markdown
|Character|Catchphrase|Pizza count| |---|---|---| |Speedy Cerviche|Here
come the Samurai Pizza Cats!|lots| |Guido Anchovy|Slice and dice!|tons|
|Polly Esther|Cat fight!|many|
```

After running `mdtablefix`:

```markdown
| Character       | Catchphrase                       | Pizza count |
| --------------- | --------------------------------- | ----------- |
| Speedy Cerviche | Here come the Samurai Pizza Cats! | lots        |
| Guido Anchovy   | Slice and dice!                   | tons        |
| Polly Esther    | Cat fight!                        | many        |
```

### Example: List Renumbering

Before:

```markdown
1. The Big Cheese's evil plans.
4. Jerry Atric's schemes.

A brief intermission for pizza.

9. Bad Bird's ninja crows.
    1. Crow #1
    5. Crow #2
12. Miscellaneous robotic mayhem.
```

After running `mdtablefix --renumber`:

```markdown
1. The Big Cheese's evil plans.
2. Jerry Atric's schemes.

A brief intermission for pizza.

1. Bad Bird's ninja crows.
    1. Crow #1
    2. Crow #2
2. Miscellaneous robotic mayhem.
```

## Library usage

The crate exposes helper functions for embedding the table-reflow logic in Rust
projects:

```rust
use mdtablefix::{process_stream_opts, rewrite, Options};
use std::path::Path;

fn main() -> std::io::Result<()> {
    let lines = vec!["|A|B|".to_string(), "|1|2|".to_string()];
    let opts = Options {
        wrap: true,
        ellipsis: true,
        fences: true,
        footnotes: true,
        headings: true,
        ..Default::default()
    };
    let fixed = process_stream_opts(&lines, opts);
    println!("{}", fixed.join("\n"));
    rewrite(Path::new("table.md"))?;
    Ok(())
}
```

The `footnotes` option also rewrites bare numeric references:

```rust
use mdtablefix::{process_stream_opts, Options};

let lines = vec![
    "A tip.1".to_string(),
    "",
    "1. Footnote text".to_string(),
];
let opts = Options { footnotes: true, ..Default::default() };
let out = process_stream_opts(&lines, opts);
assert_eq!(out[0], "A tip.[^1]");
```

It converts a trailing numbered list into footnote definitions:

```rust
use mdtablefix::{process_stream_opts, Options};

let lines = vec![
    "More text.".to_string(),
    "".to_string(),
    "1. First note".to_string(),
    "2. Second note".to_string(),
];
let opts = Options { footnotes: true, ..Default::default() };
let out = process_stream_opts(&lines, opts);
assert_eq!(out[2], "[^1]: First note");
```

- `process_stream_opts(lines: &[String], opts: Options) -> Vec<String>`
  rewrites tables in memory. The options enable paragraph wrapping, ellipsis
  substitution, fence normalization, Setext-to-ATX heading conversion, and
  footnote conversion when `footnotes` is set to `true`. The flags are `false`
  by default.

- `rewrite(path: &Path) -> std::io::Result<()>` modifies a Markdown file on
  disk in-place, wrapping paragraphs and list items as it reflows.
- `rewrite_no_wrap(path: &Path) -> std::io::Result<()>` does the same without
  wrapping text.

Both helpers write the replacement to a temporary file in the same directory
and rename it over the target, so the swap is atomic on POSIX filesystems and a
failure before the rename leaves the original file intact. The target's
permissions are copied to the temporary file before the rename, so the original
file mode is preserved and a read-only target is replaced by a read-only file.
On Windows, where a destination carrying `FILE_ATTRIBUTE_READONLY` cannot be
renamed over at all, that attribute is cleared immediately before the rename and
put back if the swap does not complete, though an abrupt interruption or a
failed restore can leave it cleared. A symbolic link is declined rather than
replaced. Callers that already hold a `cap_std::fs_utf8::Dir` capability can
call `mdtablefix::io::replace_file(directory, path, contents)` for the same
behaviour without ambient filesystem access.

> **Breaking change:** `format_breaks` now returns
> `Vec<Cow<'_, str>>` instead of `Vec<String>` so unchanged lines stay
> borrowed from the input rather than forcing heap allocations.
> Synthesized thematic-break lines borrow from a shared static. Callers
> that need owned `String` values should map `.into_owned()` over the
> returned `Cow`s:
>
> ```rust
> use mdtablefix::format_breaks;
>
> let input = vec!["foo".to_string(), "---".to_string()];
> let owned: Vec<String> = format_breaks(&input)
>     .into_iter()
>     .map(std::borrow::Cow::into_owned)
>     .collect();
> ```

## HTML table support

`mdtablefix` recognizes basic HTML `<table>` elements embedded in Markdown.
These are converted to Markdown in a preprocessing stage using
`convert_html_tables`, prior to reflow.

Only simple tables composed of `<tr>`, `<th>`, and `<td>` tags are supported.
Tag case and attributes are ignored. After conversion, they are reformatted
alongside regular Markdown tables.

See
[HTML table support for more details](docs/architecture.md#html-table-support-in-mdtablefix).

## Module structure

For an overview of how the crate's internal modules relate to each other, see \
[Module relationships](docs/architecture.md#module-relationships).

## Testing

The test suite is structured using the `rstest` crate. See
[Rust testing with rstest fixtures](docs/rust-testing-with-rstest-fixtures.md)
for details.

## License

This project is licensed under the ISC License. See the [LICENSE](LICENSE) file
for full details.
