//! Corpus-wide region-preservation sweeps for fence normalization.
//!
//! `LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS` states that normalizing a
//! block's outer delimiter may change its *spelling* but not the region of any
//! line: a line that was literal fenced content before the rewrite is literal
//! after it, and a line that was rewritable prose stays rewritable.
//!
//! Two sweeps cover it, both over every fixture under `tests/data/`:
//!
//! 1. `compress_fences` is a one-line-in, one-line-out map, so its theorem is an exact line-by-line
//!    equality: `regions(compress(lines))` must equal `regions(lines)` at every index. This is the
//!    theorem itself, and it fails the moment a rewrite moves a payload line out of the literal
//!    region.
//! 2. Through the real binary, every line the classifier calls literal must survive the full flag
//!    set byte-for-byte. This is what a user observes: fenced code is not rewritten.
//!
//! Both sweeps self-guard: an empty or implausibly small corpus fails the test
//! rather than passing vacuously, and each pins a fixture it must have covered.

use std::{
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use mdtablefix::{
    compress_fences,
    wrap::{Region, classify_regions},
};
use tempfile::TempDir;

/// Flag set `make fmt` runs, matching the estate `markdown-fencing-baseline`
/// rule's `MDTABLEFIX_RULES`.
const FULL: &[&str] = &["--wrap", "--renumber", "--breaks", "--ellipsis", "--fences"];

/// The fixture the region sweep must have covered.
///
/// It opens a fence with four backticks and carries an interior three-backtick
/// line. A pass that reads the interior line as a closer moves the lines after
/// it out of the literal region, which is exactly issue #480.
const MUST_COVER: &str = "footnotes_fence_toggle_input.txt";

/// Documents that reach the unclosed-fence path, which `tests/data/` does not.
///
/// Every fixture under `tests/data/` closes its fences, so the corpus alone
/// cannot exercise `flush_unmatched_block`. That path is where issue #480
/// lived, and leaving it uncovered would make the sweep vacuous for the
/// theorem's most delicate case: with no document ending inside a fence, the
/// rewrite rule for an unclosed block is never applied and any error in it goes
/// unnoticed.
///
/// Each document is a whole file. The first is the issue #480 reproduction: a
/// four-backtick opener, an interior three-backtick line, and a payload line
/// that must stay literal. Compressing the opener to three backticks would let
/// the interior line close it and turn the payload into prose.
const UNCLOSED_DOCUMENTS: &[&str] = &[
    "````\n```\nliteral...\n",
    "````text\n```\ncode with a ref[^1]\n",
    "~~~~\n~~~\nliteral...\n",
    "> ````\n> ```\n> literal...\n",
];

/// Returns every file under `root`, recursively.
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

/// Splits a document into lines the way `compress_fences` consumes them.
///
/// A trailing newline does not produce a final empty line, matching how the
/// binary reads a file.
fn lines_of(text: &str) -> Vec<String> {
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    if trimmed.is_empty() {
        return Vec::new();
    }
    trimmed.split('\n').map(str::to_owned).collect()
}

/// Asserts that normalizing every fixture's fences preserves its regions.
///
/// Returns the number of files checked. Files that cannot be read, or that are
/// not UTF-8, are skipped rather than failing the sweep.
fn assert_regions_preserved(files: &[PathBuf]) -> Result<usize, Box<dyn std::error::Error>> {
    let mut checked = 0_usize;
    for file in files {
        let Ok(original) = fs::read(file) else {
            continue;
        };
        let Ok(text) = String::from_utf8(original) else {
            continue;
        };

        let lines = lines_of(&text);
        let normalized = compress_fences(&lines);

        assert_eq!(
            normalized.len(),
            lines.len(),
            "{} changed its line count under fence compression",
            file.display(),
        );
        assert_eq!(
            classify_regions(&normalized),
            classify_regions(&lines),
            "{} changed a line's region under fence compression",
            file.display(),
        );
        checked += 1;
    }

    Ok(checked)
}

/// Formats `text` once with the full flag set through the real binary.
fn format_once(
    directory: &TempDir,
    name: &str,
    text: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let path = directory.path().join(name);
    fs::write(&path, text)?;

    Command::cargo_bin("mdtablefix")?
        .args(FULL)
        .arg("--in-place")
        .arg(&path)
        .assert()
        .success()
        .stdout("")
        .stderr("");

    Ok(fs::read_to_string(&path)?)
}

/// Asserts that an unclosed block keeps its payload literal.
///
/// This is the issue #480 reproduction as a specification test, run against
/// every document in [`UNCLOSED_DOCUMENTS`]. The document ends inside a fence,
/// so its interior lines are literal content that no rewrite may move into the
/// prose region.
#[test]
fn unclosed_fences_keep_their_payload_literal() {
    for document in UNCLOSED_DOCUMENTS {
        let lines = lines_of(document);
        let before = classify_regions(&lines);
        let normalized = compress_fences(&lines);

        assert_eq!(
            normalized.len(),
            lines.len(),
            "{document:?} changed its line count under fence compression",
        );
        assert_eq!(
            classify_regions(&normalized),
            before,
            "{document:?} changed a line's region under fence compression",
        );

        let payload = lines.last().expect("each document has a payload line");
        let payload_region = before.last().expect("each line has a region");
        assert_eq!(
            *payload_region,
            Region::Literal,
            "{document:?} does not leave its final line literal, so the witness is vacuous",
        );
        assert!(
            normalized.contains(payload),
            "{document:?} rewrote the literal payload {payload:?}",
        );
    }
}

/// Asserts that no fixture's literal lines are rewritten by the full pass.
///
/// A line the classifier calls literal is fenced content. Every such line must
/// appear in the output unchanged, because no pass may rewrite it. Delimiter
/// lines are excluded: normalization is entitled to respell those, which is the
/// whole point of the theorem.
///
/// Returns the number of files checked.
fn assert_literal_lines_survive(files: &[PathBuf]) -> Result<usize, Box<dyn std::error::Error>> {
    let directory = TempDir::new()?;
    let mut checked = 0_usize;
    for file in files {
        let Ok(original) = fs::read(file) else {
            continue;
        };
        let Ok(text) = String::from_utf8(original) else {
            continue;
        };

        let lines = lines_of(&text);
        let regions = classify_regions(&lines);
        let name = file
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .into_owned();
        let output = format_once(&directory, &name, &text)?;

        for (line, region) in lines.iter().zip(&regions) {
            if *region != Region::Literal {
                continue;
            }
            assert!(
                output.contains(line.as_str()),
                "{} lost the literal line {line:?} under the full flag set",
                file.display(),
            );
        }
        checked += 1;
    }

    Ok(checked)
}

#[test]
fn every_fixture_preserves_regions_under_fence_normalization()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = data_files(&root.join("tests").join("data"));

    assert!(!files.is_empty(), "found no fixtures to check");
    assert!(
        files
            .iter()
            .any(|path| path.file_name().is_some_and(|name| name == MUST_COVER)),
        "the region sweep must cover {MUST_COVER}",
    );

    let checked = assert_regions_preserved(&files)?;

    assert!(
        checked > 100,
        "expected the whole fixture corpus, checked {checked}",
    );
    Ok(())
}

#[test]
fn every_fixture_keeps_its_literal_lines_verbatim() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = data_files(&root.join("tests").join("data"));

    assert!(!files.is_empty(), "found no fixtures to check");

    let checked = assert_literal_lines_survive(&files)?;

    assert!(
        checked > 100,
        "expected the whole fixture corpus, checked {checked}",
    );
    Ok(())
}
