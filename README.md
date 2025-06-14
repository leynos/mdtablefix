# mdtablefix

`mdtablefix` reflows Markdown tables so that each column has a uniform width.
It ignores fenced code blocks and respects escaped pipes (`\|`),
making it safe for mixed content.

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
mdtablefix [--in-place] [FILE...]
```

- With file paths provided, the corrected tables are printed to stdout.
- Use `--in-place` to overwrite files.
- If no files are supplied, input is read from stdin and results are written to stdout.

### Example

Before:

```markdown
| A|B |  C |
|---|--|---|
|1|22|333 |
```

After running `mdtablefix`:

```markdown
| A | B  | C   |
| --- | --- | --- |
| 1 | 22 | 333 |
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

`mdtablefix` recognises simple `<table>` elements embedded in Markdown.
Those tables are converted to Markdown before the regular reflow logic so
Markdown and HTML tables are formatted consistently.

The crate relies on `html5ever` and `markup5ever_rcdom` to parse the table
structure. Only basic tables using `<tr>`, `<th>` and `<td>` tags are
supported, and attributes or tag casing do not affect detection.

## Testing

See `docs/rust-testing-with-rstest-fixtures.md` for notes on how the test suite
is organised using the [`rstest`](https://crates.io/crates/rstest) crate.

## License

This project is licensed under the ISC license.
See the [LICENSE](LICENSE) file for details.

