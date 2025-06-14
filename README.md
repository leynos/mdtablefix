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
Use `--in-place` to overwrite files.
- If no files are supplied, input is read from stdin and results are written to stdout.

## Library usage

The crate exposes helper functions so you can integrate the table reflow logic
in your own project.

```rust
use mdtablefix::{process_stream, rewrite};
```

- `process_stream(&[String]) -> Vec<String>` rewrites tables in memory.
- `rewrite(Path)` updates a Markdown file on disk.

## Testing

See `docs/rust-testing-with-rstest-fixtures.md` for notes on how the test suite
is organised using the [`rstest`](https://crates.io/crates/rstest) crate.

## License

This project is licensed under the ISC license.
