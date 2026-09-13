//! Corpus-wide idempotence sweeps over the repository's own files.
//!
//! `tests/idempotence.rs` pins the reproduction corpus under
//! `tests/data/idempotence/`. The tests here widen the same invariant to
//! every fixture under `tests/data/` and every document under `docs/`: each
//! file is formatted twice through the real binary and the two passes must be
//! byte-identical, so a check-after-fix gate cannot report drift indefinitely
//! on the same file.
//!
//! The `--headings` sweep is the check-after-fix gate for the class `T`
//! defect. That shape was reachable only through the Setext pass, which the
//! `make fmt` flag set does not enable, so the gate has to hold for the output
//! `--headings` produces from the fixtures already in `tests/data/`, not only
//! from a purpose-built reproduction.
//!
//! The `--code-emphasis` sweep protects table reflow from substitutions that
//! shorten a cell after its widths have been measured.

use std::{
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use tempfile::TempDir;

/// Flag set `make fmt` runs through `mdformat-all`.
const FULL: &[&str] = &["--wrap", "--renumber", "--breaks", "--ellipsis", "--fences"];
/// The `make fmt` flag set plus `--headings`.
///
/// The class `T` defect needs the Setext pass, which `make fmt` does not
/// enable, so a drift check over the repository fixtures has to add the flag to
/// reach it.
const FULL_HEADINGS: &[&str] = &[
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
    "--headings",
];

/// The `make fmt` flag set plus `--code-emphasis`.
const FULL_CODE_EMPHASIS: &[&str] = &[
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
    "--code-emphasis",
];

/// Formats `text` once with `flags` through the real binary.
///
/// The input is written to `directory`, formatted in place, and read back. The
/// binary must succeed with an empty stdout and stderr.
fn format_once(
    directory: &TempDir,
    name: &str,
    text: &[u8],
    flags: &[&str],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let path = directory.path().join(name);
    fs::write(&path, text)?;

    Command::cargo_bin("mdtablefix")?
        .args(flags)
        .arg("--in-place")
        .arg(&path)
        .assert()
        .success()
        .stdout("")
        .stderr("");

    Ok(fs::read(&path)?)
}

/// Returns every Markdown-like file under `root`, recursively.
fn markdown_files(root: &Path) -> Vec<PathBuf> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            {
                found.push(path);
            }
        }
    }

    let mut found = Vec::new();
    walk(root, &mut found);
    found
}

/// Returns every fixture file under `root`, recursively.
fn data_files(root: &Path) -> Vec<PathBuf> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else {
                found.push(path);
            }
        }
    }

    let mut found = Vec::new();
    walk(root, &mut found);
    found
}

/// Asserts that no file in `files` drifts on a second formatting pass.
///
/// The check is deliberately independent of the current on-disk formatting: it
/// formats a copy twice with `flags` and compares the two passes, so a file
/// that is already formatted simply produces its own bytes twice.
///
/// Returns the number of files checked. Files that cannot be read, or that are
/// not UTF-8, are skipped rather than failing the check.
fn assert_no_drift(
    files: &[PathBuf],
    flags: &[&str],
    description: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let directory = TempDir::new()?;
    let mut checked = 0_usize;
    for file in files {
        let Ok(original) = fs::read(file) else {
            continue;
        };
        if String::from_utf8(original.clone()).is_err() {
            continue;
        }
        let name = file
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .into_owned();

        let once = format_once(&directory, &name, &original, flags)?;
        let twice = format_once(&directory, &name, &once, flags)?;

        assert_eq!(
            String::from_utf8_lossy(&twice),
            String::from_utf8_lossy(&once),
            "{} is not a fixed point under {description}",
            file.display(),
        );
        checked += 1;
    }

    Ok(checked)
}

#[test]
fn repository_documents_do_not_drift_on_a_second_pass() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = data_files(&root.join("tests").join("data"));
    files.extend(markdown_files(&root.join("docs")));
    assert!(!files.is_empty(), "found no files to check for drift");
    assert!(
        files
            .iter()
            .any(|path| path.ends_with("docs/developers-guide.md")),
        "the drift check must cover docs/developers-guide.md",
    );

    let checked = assert_no_drift(&files, FULL, "the full flag set")?;

    assert!(
        checked > 100,
        "expected the whole corpus, checked {checked}"
    );
    Ok(())
}

/// Asserts that no repository fixture drifts under `--headings`.
///
/// The reported defect was reachable only through the Setext pass, so a
/// check-after-fix gate has to hold for the output `--headings` produces from
/// the fixtures already in `tests/data/`. The class `T` fixtures are the
/// reproduction: before the delimiter-row guard, the second pass restructured
/// the table above the row and the output never settled.
#[test]
fn repository_fixtures_do_not_drift_under_headings() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = data_files(&root.join("tests").join("data"));
    assert!(!files.is_empty(), "found no fixtures to check for drift");
    assert!(
        files
            .iter()
            .any(|path| path.ends_with("tests/data/idempotence/T1_delimiter_then_break.dat")),
        "the drift check must cover the class `T` reproduction",
    );

    let checked = assert_no_drift(
        &files,
        FULL_HEADINGS,
        "the `make fmt` flag set with --headings",
    )?;

    assert!(
        checked > 100,
        "expected the whole fixture corpus, checked {checked}"
    );
    Ok(())
}

/// Asserts that no repository fixture drifts under `--code-emphasis`.
///
/// Code-emphasis can remove markers from a table cell. The reflow must measure
/// the shortened cell on the first pass so a check-after-fix gate sees the
/// output as clean without requiring a second rewrite.
#[test]
fn repository_fixtures_do_not_drift_under_code_emphasis() -> Result<(), Box<dyn std::error::Error>>
{
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = data_files(&root.join("tests").join("data"));
    assert!(!files.is_empty(), "found no fixtures to check for drift");
    assert!(
        files
            .iter()
            .any(|path| path.ends_with("tests/data/cli-matrix/table-prose.dat")),
        "the drift check must cover the code-emphasis table reproduction",
    );

    let checked = assert_no_drift(
        &files,
        FULL_CODE_EMPHASIS,
        "the `make fmt` flag set with --code-emphasis",
    )?;

    assert!(
        checked > 100,
        "expected the whole fixture corpus, checked {checked}"
    );
    Ok(())
}
