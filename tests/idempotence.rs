//! Idempotence regression corpus for issue #468.
//!
//! Every fixture under `tests/data/idempotence/` is formatted twice through the
//! real binary with its recorded flag set. A case passes when the second pass
//! is byte-identical to the first, and when a thematic break recorded with the
//! case survives as a standalone line instead of being absorbed into a
//! paragraph.
//!
//! The corpus covers both defect classes from the issue. Class A normalises a
//! thematic break and then re-wraps it on the next pass; class B re-wraps the
//! tail of a list item whose first line ends with a parenthesised inline code
//! span. `R1` and `R2` were already fixed points; they are here because a fix
//! that absorbed breaks on the first pass instead of the second would satisfy
//! the idempotence cases for the wrong reason.
//!
//! Case `H1` reaches the same invariant through `--headings`, which the
//! `make fmt` flag set does not enable: a candidate line that is itself a block
//! start must not swallow the underline below it. It is recorded separately
//! because the first defect classes were all reachable without `--headings`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use mdtablefix::THEMATIC_BREAK_LEN;
use tempfile::TempDir;

/// Flag set recorded for the class B, `--wrap`-only cases.
const WRAP: &[&str] = &["--wrap"];
/// Flag set recorded for the class A cases that need break normalisation.
const WRAP_BREAKS: &[&str] = &["--wrap", "--breaks"];
/// Flag set `make fmt` runs through `mdformat-all`.
const FULL: &[&str] = &["--wrap", "--renumber", "--breaks", "--ellipsis", "--fences"];
/// Flag set recorded for the heading cases, which `make fmt` does not enable.
const HEADINGS: &[&str] = &["--footnotes", "--code-emphasis", "--headings"];

/// Line that a case must keep standalone in the formatted output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Standalone {
    /// The 70-underscore line `--breaks` writes for a thematic break.
    NormalisedBreak,
    /// A thematic break that must survive spelling-for-spelling.
    Literal(&'static str),
}

impl Standalone {
    /// Returns the exact output line this expectation requires.
    fn line(self) -> String {
        match self {
            Self::NormalisedBreak => "_".repeat(THEMATIC_BREAK_LEN),
            Self::Literal(break_line) => break_line.to_string(),
        }
    }
}

/// One case in the reproduction corpus.
struct IdempotenceCase {
    /// Stable identifier matching the corpus recorded in the issue.
    id: &'static str,
    /// Fixture filename under `tests/data/idempotence/`.
    fixture: &'static str,
    /// Flags recorded for this case.
    flags: &'static [&'static str],
    /// Break that must stay on its own line, when the case has one.
    standalone: Option<Standalone>,
}

/// The reproduction corpus, one fixture per case.
const CASES: &[IdempotenceCase] = &[
    IdempotenceCase {
        id: "A1_hyphen_break_unterminated",
        fixture: "A1_hyphen_break_unterminated.dat",
        flags: WRAP_BREAKS,
        standalone: Some(Standalone::NormalisedBreak),
    },
    IdempotenceCase {
        id: "A2_hyphen_break_terminated",
        fixture: "A2_hyphen_break_terminated.dat",
        flags: WRAP_BREAKS,
        standalone: Some(Standalone::NormalisedBreak),
    },
    IdempotenceCase {
        id: "A3_setext_underline",
        fixture: "A3_setext_underline.dat",
        flags: WRAP_BREAKS,
        standalone: Some(Standalone::NormalisedBreak),
    },
    IdempotenceCase {
        id: "A4_frontmatter_fixture",
        fixture: "A4_frontmatter_fixture.dat",
        flags: FULL,
        standalone: Some(Standalone::NormalisedBreak),
    },
    IdempotenceCase {
        id: "B1_code_span_tail",
        fixture: "B1_code_span_tail.dat",
        flags: WRAP,
        standalone: None,
    },
    IdempotenceCase {
        id: "B2_code_span_tail_full_flags",
        fixture: "B2_code_span_tail_full_flags.dat",
        flags: FULL,
        standalone: None,
    },
    IdempotenceCase {
        id: "R1_asterisk_break_absorbed",
        fixture: "R1_asterisk_break_absorbed.dat",
        flags: WRAP,
        standalone: Some(Standalone::Literal("***")),
    },
    IdempotenceCase {
        id: "R2_underscore_break_absorbed",
        fixture: "R2_underscore_break_absorbed.dat",
        flags: WRAP,
        standalone: Some(Standalone::Literal("___")),
    },
    IdempotenceCase {
        id: "H1_atx_heading_above_break",
        fixture: "H1_atx_heading_above_break.dat",
        flags: HEADINGS,
        standalone: Some(Standalone::Literal("---")),
    },
];

/// Returns the fixture directory for the corpus.
fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("idempotence")
}

/// Returns the path of `fixture` in the corpus directory.
fn corpus_path(fixture: &str) -> PathBuf { corpus_dir().join(fixture) }

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

/// Returns the output lines of `bytes`, for standalone-line assertions.
fn output_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn corpus_case_ids_and_fixtures_are_unique_and_present() {
    let mut fixtures = std::collections::BTreeSet::new();

    for case in CASES {
        assert!(
            fixtures.insert(case.fixture),
            "duplicate fixture {} in case {}",
            case.fixture,
            case.id,
        );
        let path = corpus_path(case.fixture);
        assert!(path.exists(), "missing fixture {}", path.display());
        assert_eq!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("dat"),
            "fixture {} must be a .dat file",
            case.fixture,
        );
    }

    let on_disk: std::collections::BTreeSet<String> = fs::read_dir(corpus_dir())
        .expect("reading the corpus directory")
        .map(|entry| {
            entry
                .expect("reading a corpus entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let recorded: std::collections::BTreeSet<String> =
        CASES.iter().map(|case| case.fixture.to_string()).collect();

    assert_eq!(
        on_disk, recorded,
        "every fixture on disk must be recorded in the case table",
    );
}

#[test]
fn corpus_records_a_flag_set_for_every_case() {
    for case in CASES {
        assert!(!case.flags.is_empty(), "case {} records no flags", case.id);
        assert!(
            case.flags.iter().all(|flag| flag.starts_with("--")),
            "case {} records a non-flag argument",
            case.id,
        );
    }
}

#[test]
fn every_corpus_case_is_a_fixed_point() -> Result<(), Box<dyn std::error::Error>> {
    for case in CASES {
        let original = fs::read(corpus_path(case.fixture))?;
        let directory = TempDir::new()?;

        let once = format_once(&directory, case.fixture, &original, case.flags)?;
        let twice = format_once(&directory, case.fixture, &once, case.flags)?;

        assert_eq!(
            String::from_utf8_lossy(&twice),
            String::from_utf8_lossy(&once),
            "case {} is not a fixed point under {:?}",
            case.id,
            case.flags,
        );

        if let Some(standalone) = case.standalone {
            let expected = standalone.line();
            let lines = output_lines(&once);
            assert!(
                lines.iter().any(|line| line == &expected),
                "case {} lost the standalone break line {expected:?}; output was {lines:?}",
                case.id,
            );
        }
    }

    Ok(())
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

/// Asserts that no repository document drifts on a second formatting pass.
///
/// The check is deliberately independent of the current on-disk formatting: it
/// copies each file, formats the copy twice with the `make fmt` flag set, and
/// compares the two passes. A file that is already formatted simply produces
/// its own bytes twice.
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

    let directory = TempDir::new()?;
    let mut checked = 0_usize;
    for file in files {
        let Ok(original) = fs::read(&file) else {
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

        let once = format_once(&directory, &name, &original, FULL)?;
        let twice = format_once(&directory, &name, &once, FULL)?;

        assert_eq!(
            String::from_utf8_lossy(&twice),
            String::from_utf8_lossy(&once),
            "{} is not a fixed point under the full flag set",
            file.display(),
        );
        checked += 1;
    }

    assert!(
        checked > 100,
        "expected the whole corpus, checked {checked}"
    );
    Ok(())
}
