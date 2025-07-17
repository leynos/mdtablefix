# mdtablefix

`mdtablefix` unb0rks and reflows Markdown tables so that each column has a
uniform width. It can wrap paragraphs and list items to 80 columns when the
`--wrap` option is used. Hyphenated words are treated as single units during
wrapping, so `very-long-word` moves to the next line rather than splitting at
the hyphen. The tool ignores fenced code blocks and respects escaped pipes (
`\|`), making it safe for mixed content.

## Installation

```bash
cargo install mdtablefix
```

or clone the repository and build from source:

```bash
cargo install --path .
```

## Command-line usage

```bash
mdtablefix [--wrap] [--renumber] [--breaks] [--in-place] [FILE...]
```

- With file paths provided, the corrected tables are printed to stdout.
- Use `--wrap` to also reflow paragraphs and list items to 80 columns.
- Use `--renumber` to rewrite ordered lists with sequential numbering.
- Tabs are interpreted as four spaces when counting indentation for
  `--renumber`.
- Use `--breaks` to normalize thematic breaks to a line of 70 underscores
  (configurable via the `THEMATIC_BREAK_LEN` constant).
- Use `--in-place` to overwrite files.
- If no files are supplied, input is read from stdin and results are written
  to stdout.

### Example

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

## Library usage

The crate exposes helper functions so you can integrate the table reflow logic
in your own project.

```rust
use mdtablefix::{process_stream, rewrite};
use std::path::Path;

fn main() -> std::io::Result<()> {
    let lines = vec!["|A|B|".to_string(), "|1|2|".to_string()];
    let fixed = process_stream(&lines);
    println!("{}", fixed.join("\n"));
    rewrite(Path::new("table.md"))?;
    Ok(())
}
```

- `process_stream(&[String]) -> Vec<String>` rewrites tables in memory.
- `rewrite(&Path) -> std::io::Result<()>` updates a Markdown file on disk.

## HTML table support

`mdtablefix` recognises simple `<table>` elements embedded in Markdown. Before
the main table reflow runs these HTML tables are converted to Markdown in a
preprocessing stage handled by `convert_html_tables`.

Only basic tables composed of `<tr>`, `<th>` and `<td>` tags are detected, and
attributes or tag casing do not matter. After conversion the regular reflow
logic aligns them alongside Markdown tables. See [`docs/html-table-support.md`]
(docs/html-table-support.md) for details.

## Testing

See `docs/rust-testing-with-rstest-fixtures.md` for notes on how the test suite
is organised using the [`rstest`](https://crates.io/crates/rstest) crate.

## License

This project is licensed under the ISC license. See the [LICENSE](LICENSE) file
for details.
