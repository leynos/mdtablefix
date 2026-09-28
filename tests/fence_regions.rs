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
///
/// A directory that cannot be listed is an error, not an empty result: a walk
/// that silently returns fewer files would let the sweep pass over a corpus it
/// never read.
fn data_files(root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
        for entry in
            fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?
        {
            let path = entry
                .map_err(|error| format!("{}: {error}", directory.display()))?
                .path();
            if path.is_dir() {
                walk(&path, found)?;
            } else {
                found.push(path);
            }
        }
        Ok(())
    }

    let mut found = Vec::new();
    walk(root, &mut found)?;
    Ok(found)
}

/// Reads a fixture as UTF-8 text, naming the file in any failure.
///
/// The sweeps must not treat an unreadable or non-UTF-8 fixture as one that
/// passed. Skipping it would leave the guard counting only the files that
/// happened to be readable, so a corpus read in part could pass while the file
/// that would have failed went unchecked. Failing here makes a non-UTF-8
/// fixture a deliberate addition that updates this test rather than a silent
/// gap.
fn read_fixture(file: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = fs::read(file).map_err(|error| format!("{}: {error}", file.display()))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("{}: not valid UTF-8: {error}", file.display()))?;
    Ok(text)
}

/// Splits a document into the lines the product actually sees.
///
/// The product parses a file into a `SourceDocument`, which strips a leading
/// byte-order mark, and then splits with `str::lines` — so a line never carries
/// its terminator. Splitting on `\n` and keeping the `\r` would hand the
/// classifier a line the product never produces, and the error is not benign:
/// `FENCE_RE` excludes carriage returns from its info capture, so a delimiter
/// carrying a stray `\r` is not recognised as a fence at all, and the document
/// would classify as unbroken prose. Both sides of every comparison here use
/// this split, so the test classifies the same lines the binary does.
fn lines_of(text: &str) -> Vec<String> {
    text.strip_prefix('\u{FEFF}')
        .unwrap_or(text)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Asserts that normalizing every fixture's fences preserves its regions.
///
/// Returns the number of files checked. Every fixture must be readable as
/// UTF-8; an unreadable one fails the sweep rather than being skipped.
fn assert_regions_preserved(files: &[PathBuf]) -> Result<usize, Box<dyn std::error::Error>> {
    let mut checked = 0_usize;
    for file in files {
        let text = read_fixture(file)?;

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
/// A line the classifier calls literal is fenced content. Every such line's
/// text must appear in the output unchanged, because no pass may rewrite it.
/// Delimiter lines are excluded from the sweep entirely: normalization is
/// entitled to respell those, which is the whole point of the theorem.
///
/// The comparison walks the two literal subsequences in step rather than asking
/// whether each source literal line appears *anywhere* in the output. Membership
/// is too weak: a blank prose line, or a second block that happens to repeat the
/// text, would satisfy it after the real line had been merged or reordered. The
/// output's own region classification decides which lines to compare, and the
/// walk rejects a missing, changed, or extra literal line alike.
///
/// Both sides are split by [`lines_of`], so the comparison is between the same
/// terminator-free lines the product itself works with.
///
/// Returns the number of files checked. Every fixture must be readable as
/// UTF-8; an unreadable one fails the sweep rather than being skipped.
fn assert_literal_lines_survive(files: &[PathBuf]) -> Result<usize, Box<dyn std::error::Error>> {
    let directory = TempDir::new()?;
    let mut checked = 0_usize;
    for file in files {
        let text = read_fixture(file)?;

        let lines = lines_of(&text);
        let regions = classify_regions(&lines);
        let name = file
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .into_owned();
        let output = format_once(&directory, &name, &text)?;
        let output_text = lines_of(&output);
        let output_regions = classify_regions(&output_text);
        let mut output_literal_lines = output_text
            .iter()
            .zip(&output_regions)
            .filter(|(_line, region)| **region == Region::Literal)
            .map(|(line, _region)| line.as_str());

        for (line, region) in lines.iter().zip(&regions) {
            if *region != Region::Literal {
                continue;
            }
            assert_eq!(
                output_literal_lines.next(),
                Some(line.as_str()),
                "{} changed or lost the literal line {line:?} under the full flag set",
                file.display(),
            );
        }
        assert!(
            output_literal_lines.next().is_none(),
            "{} gained an extra literal line under the full flag set",
            file.display(),
        );
        checked += 1;
    }

    Ok(checked)
}

#[test]
fn every_fixture_preserves_regions_under_fence_normalization()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = data_files(&root.join("tests").join("data"))?;

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
    let files = data_files(&root.join("tests").join("data"))?;

    assert!(!files.is_empty(), "found no fixtures to check");

    let checked = assert_literal_lines_survive(&files)?;

    assert!(
        checked > 100,
        "expected the whole fixture corpus, checked {checked}",
    );
    Ok(())
}
