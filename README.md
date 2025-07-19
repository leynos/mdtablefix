# mdtablefix

`mdtablefix` unb0rks and reflows Markdown tables so that each column has a uniform width. When the `--wrap` option is used, it also wraps paragraphs and list items to 80 columns.

Hyphenated words are treated as indivisible during wrapping, so `very-long-word` will move to the next line intact rather than split at the hyphen. The tool ignores fenced code blocks and respects escaped pipes (`\|`), making it safe to use on Markdown with mixed content.

## Installation

Install via Cargo:

Bash

```
cargo install mdtablefix
```

Or clone the repository and build from source:

Bash

```
cargo install --path .
```

## Command-line usage

Bash

```
mdtablefix [--wrap] [--renumber] [--breaks] [--ellipsis] [--in-place] [FILE...]
```

- When one or more file paths are provided, the corrected tables are printed to stdout.

- Use `--wrap` to reflow paragraphs and list items to 80 columns.

- Use `--renumber` to rewrite ordered lists with consistent sequential numbering. The renumbering logic correctly handles nested lists by tracking indentation (tabs are interpreted as four spaces) and restarts numbering after a list is interrupted by other content, such as a paragraph at a lower indentation level.

- Use `--breaks` to standardise thematic breaks to a line of 70 underscores (configurable via the `THEMATIC_BREAK_LEN` constant).

- Use `--ellipsis` to replace groups of three dots (`...`) with the ellipsis character (`…`). Longer runs are processed left-to-right, so any leftover dots are preserved.

- Use `--in-place` to modify files in-place.

- If no files are specified, input is read from stdin and output is written to stdout.

### Example: Table Reflowing

Before:

Markdown

```
|Character|Catchphrase|Pizza count| |---|---|---| |Speedy Cerviche|Here
come the Samurai Pizza Cats!|lots| |Guido Anchovy|Slice and dice!|tons|
|Polly Esther|Cat fight!|many|
```

After running `mdtablefix`:

Markdown

```
| Character       | Catchphrase                       | Pizza count |
| --------------- | --------------------------------- | ----------- |
| Speedy Cerviche | Here come the Samurai Pizza Cats! | lots        |
| Guido Anchovy   | Slice and dice!                   | tons        |
| Polly Esther    | Cat fight!                        | many        |
```

### Example: List Renumbering

Before:

Markdown

```
1. The Big Cheese's evil plans.
4. Jerry Atric's schemes.

A brief intermission for pizza.

9. Bad Bird's ninja crows.
    1. Crow #1
    5. Crow #2
12. Miscellaneous robotic mayhem.
```

After running `mdtablefix --renumber`:

Markdown

```
1. The Big Cheese's evil plans.
2. Jerry Atric's schemes.

A brief intermission for pizza.

1. Bad Bird's ninja crows.
    1. Crow #1
    2. Crow #2
2. Miscellaneous robotic mayhem.
```

## Library usage

The crate provides helper functions for embedding the table reflow logic in your own Rust project:

Rust

```
use mdtablefix::{process_stream_opts, rewrite};
use std::path::Path;

fn main() -> std::io::Result<()> {
    let lines = vec!["|A|B|".to_string(), "|1|2|".to_string()];
    let fixed = process_stream_opts(
        &lines,
        /* wrap = */ true,
        /* ellipsis = */ true,
    );
    println!("{}", fixed.join("\n"));
    rewrite(Path::new("table.md"))?;
    Ok(())
}
```

- `process_stream_opts(lines: &[String], wrap: bool, ellipsis: bool) -> Vec<String>` rewrites tables in memory, with optional paragraph wrapping and ellipsis substitution.

- `rewrite(path: &Path) -> std::io::Result<()>` modifies a Markdown file on disk in-place.

## HTML table support

`mdtablefix` recognises basic HTML `<table>` elements embedded in Markdown. These are converted to Markdown in a preprocessing stage using `convert_html_tables`, prior to reflow.

Only simple tables composed of `<tr>`, `<th>`, and `<td>` tags are supported. Tag case and attributes are ignored. After conversion, they are reformatted alongside regular Markdown tables.

See [HTML table support for more details](docs/html-table-support.md).

## Module structure

For an overview of how the crate's internal modules relate to each other, see [Module relationships](docs/module-relationships.md).

## Testing

The test suite is structured using the `rstest` crate. See [Rust testing with rstest fixtures](docs/rust-testing-with-rstest-fixtures.md) for details.

## License

This project is licensed under the ISC License. See the [LICENSE](LICENSE) file for full details.
