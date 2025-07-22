# mdtablefix

`mdtablefix` unb0rks and reflows Markdown tables so that each column has a
uniform width. When the `--wrap` option is used, it also wraps paragraphs and
list items to 80 columns.

Hyphenated words are treated as indivisible during wrapping, so
`very-long-word` will move to the next line intact rather than split at the
hyphen. The tool ignores fenced code blocks and respects escaped pipes (`\|`),
making it safe to use on Markdown with mixed content.

## Installation

Install via Cargo:

```bash
cargo install mdtablefix
```

Or clone the repository and build from source:

```bash
cargo install --path .
```

## Command-line usage

```bash
mdtablefix [--wrap] [--renumber] [--breaks] [--ellipsis] [--fences] [--footnotes] [--in-place] [FILE...]
```

- When one or more file paths are provided, the corrected tables are printed to
  stdout.

- Use `--wrap` to reflow paragraphs and list items to 80 columns.

- Use `--renumber` to rewrite ordered lists with consistent sequential
  numbering. The renumbering logic correctly handles nested lists by tracking
  indentation (tabs are interpreted as four spaces) and restarts numbering
  after a list is interrupted by other content, such as a paragraph at a lower
  indentation level, a thematic break, or a heading. Blank lines between items
  are ignored, so numbering continues uninterrupted.

- Use `--breaks` to standardize thematic breaks to a line of 70 underscores
  (configurable via the `THEMATIC_BREAK_LEN` constant).

- Use `--ellipsis` to replace groups of three dots (`...`) with the ellipsis
  character (`…`). Longer runs are processed left-to-right, so any leftover
  dots are preserved.

- Use `--fences` to normalize code block delimiters to three backticks before
  other processing.

- Use `--footnotes` to convert bare numeric references and the final numbered
  list into GitHub-flavoured footnote links.

- Use `--in-place` to modify files in-place.

- If no files are specified, input is read from stdin and output is written to
  stdout.

## Concurrency

When multiple file paths are supplied the tool processes them in parallel using
the [`rayon`](https://docs.rs/rayon) crate. Results are buffered so they can be
printed in the original order. This coordination uses extra memory and can
outweigh the speed gains when each file is small.

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
        ..Default::default()
    };
    let fixed = process_stream_opts(&lines, opts);
    println!("{}", fixed.join("\n"));
    rewrite(Path::new("table.md"))?;
    Ok(())
}
```

- `process_stream_opts(lines: &[String], opts: Options) -> Vec<String>`
  rewrites tables in memory. The options enable paragraph wrapping, ellipsis
  substitution, fence normalization and footnote conversion when `footnotes` is
  set to `true`.

- `rewrite(path: &Path) -> std::io::Result<()>` modifies a Markdown file on
  disk in-place.

## HTML table support

`mdtablefix` recognizes basic HTML `<table>` elements embedded in Markdown.
These are converted to Markdown in a preprocessing stage using
`convert_html_tables`, prior to reflow.

Only simple tables composed of `<tr>`, `<th>`, and `<td>` tags are supported.
Tag case and attributes are ignored. After conversion, they are reformatted
alongside regular Markdown tables.

See
[HTML table support for more details](docs/architecture.md#html-table-support-in-mdtablefix)
 .

## Module structure

For an overview of how the crate's internal modules relate to each other, see
[Module relationships](docs/architecture.md#module-relationships).

## Testing

The test suite is structured using the `rstest` crate. See
[Rust testing with rstest fixtures](docs/rust-testing-with-rstest-fixtures.md)
for details.

## License

This project is licensed under the ISC License. See the [LICENSE](LICENSE) file
for full details.
