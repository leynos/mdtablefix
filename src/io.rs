//! File helpers for rewriting Markdown documents.

use std::{fs, path::Path};

use crate::process::{process_stream, process_stream_no_wrap};

/// Rewrite a file in place with wrapped tables.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite(path: &Path) -> std::io::Result<()> {
    let text = fs::read_to_string(path)?;
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let fixed = process_stream(&lines);
    fs::write(path, fixed.join("\n") + "\n")
}

/// Rewrite a file in place without wrapping text.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite_no_wrap(path: &Path) -> std::io::Result<()> {
    let text = fs::read_to_string(path)?;
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let fixed = process_stream_no_wrap(&lines);
    fs::write(path, fixed.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn rewrite_roundtrip() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("sample.md");
        fs::write(&file, "|A|B|\n|1|2|").unwrap();
        rewrite(&file).unwrap();
        let out = fs::read_to_string(&file).unwrap();
        assert!(out.contains("| A | B |"));
    }

    #[test]
    fn rewrite_no_wrap_roundtrip() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("sample.md");
        fs::write(&file, "|A|B|\n|1|2|").unwrap();
        rewrite_no_wrap(&file).unwrap();
        let out = fs::read_to_string(&file).unwrap();
        assert_eq!(out, "| A | B |\n| 1 | 2 |\n");
    }
}
