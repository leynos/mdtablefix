//! Corpus-wide region-preservation sweeps for fence normalization.
//!
//! `LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS` states that normalizing a
//! block's outer delimiter may change its *spelling* but not the region of any
//! line: a line that was literal fenced content before the rewrite is literal
//! after it, and a line that was rewritable prose stays rewritable.
//!
//! Two sweeps cover it, both over every fixture under `tests/data/`:
//!
//! 1. `compress_fences` is a one-line-in, one-line-out map, so the theorem takes the shape of an
//!    exact line-by-line equality: `regions(compress(lines))` must equal `regions(lines)` at every
//!    index. This sweep checks that shape over the corpus and fails the moment a rewrite moves a
//!    payload line out of the literal region. It is evidence, not the theorem: a corpus says
//!    nothing about a document no fixture holds. The induction that generalizes it is
//!    `lemma_document_regions_preserved`, which builds its chain of preserved spans from the fold
//!    itself (`docs/verification.md`).
//! 2. Through the real binary, every line the classifier calls literal must survive the full flag
//!    set byte-for-byte. This is what a user observes: fenced code is not rewritten.
//!
//! Both sweeps self-guard: an empty or implausibly small corpus fails the test
//! rather than passing vacuously, and each pins a fixture it must have covered.
//!
//! Filesystem access follows the integration-test convention in
//! `docs/developers-guide.md`: `CARGO_MANIFEST_DIR` is opened once as the sole
//! ambient boundary, and every read, walk, and write below it goes through a
//! `cap_std` directory capability on `camino` UTF-8 paths. An absolute path is
//! derived only where a child process needs one.

use anyhow::{Context as _, Result};
use assert_cmd::Command;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use mdtablefix::{
    compress_fences,
    wrap::{Region, classify_regions},
};

#[path = "common/fs.rs"]
mod test_fs;
use test_fs::TestDir;

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
/// cannot exercise the unclosed-flush path, `flush_unclosed`. That is where
/// issue #480
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

/// Opens the repository root, the suite's one ambient filesystem boundary.
fn manifest_dir() -> Result<Dir> {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Dir::open_ambient_dir(&manifest, ambient_authority())
        .with_context(|| format!("open repository root {manifest}"))
}

/// The fixture corpus, opened beneath the repository capability.
fn fixture_root(manifest: &Dir) -> Result<Dir> {
    manifest
        .open_dir("tests/data")
        .context("open the fixture corpus at tests/data")
}

/// Returns every file under `root`, recursively, as paths relative to it.
///
/// A directory that cannot be listed is an error, not an empty result: a walk
/// that silently returns fewer files would let the sweep pass over a corpus it
/// never read.
fn data_files(root: &Dir) -> Result<Vec<Utf8PathBuf>> {
    fn walk(directory: &Dir, prefix: &Utf8Path, found: &mut Vec<Utf8PathBuf>) -> Result<()> {
        for entry in directory.entries().context("list a fixture directory")? {
            let entry = entry.context("read a fixture directory entry")?;
            let name = entry.file_name().context("read a fixture entry's name")?;
            let relative = prefix.join(name);
            if entry
                .file_type()
                .context("read a fixture entry's file type")?
                .is_dir()
            {
                walk(
                    &entry.open_dir().context("open a fixture subdirectory")?,
                    &relative,
                    found,
                )?;
            } else {
                found.push(relative);
            }
        }
        Ok(())
    }

    let mut found = Vec::new();
    walk(root, Utf8Path::new(""), &mut found)?;
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
fn read_fixture(root: &Dir, file: &Utf8Path) -> Result<String> {
    root.read_to_string(file)
        .with_context(|| format!("read fixture {file} as UTF-8"))
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
fn assert_regions_preserved(root: &Dir, files: &[Utf8PathBuf]) -> Result<usize> {
    let mut checked = 0_usize;
    for file in files {
        let text = read_fixture(root, file)?;

        let lines = lines_of(&text);
        let normalized = compress_fences(&lines);

        assert_eq!(
            normalized.len(),
            lines.len(),
            "{file} changed its line count under fence compression",
        );
        assert_eq!(
            classify_regions(&normalized),
            classify_regions(&lines),
            "{file} changed a line's region under fence compression",
        );
        checked += 1;
    }

    Ok(checked)
}

/// Formats `text` once with the full flag set through the real binary.
fn format_once(directory: &TestDir, name: &str, text: &str) -> Result<String> {
    directory
        .directory()
        .write(name, text)
        .with_context(|| format!("write temporary fixture {name}"))?;

    Command::cargo_bin("mdtablefix")?
        .args(FULL)
        .arg("--in-place")
        .arg(directory.path().join(name))
        .assert()
        .success()
        .stdout("")
        .stderr("");

    directory
        .directory()
        .read_to_string(name)
        .with_context(|| format!("read formatted fixture {name}"))
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
fn assert_literal_lines_survive(root: &Dir, files: &[Utf8PathBuf]) -> Result<usize> {
    let directory = TestDir::new().context("create the sweep's output directory")?;
    let mut checked = 0_usize;
    for file in files {
        let text = read_fixture(root, file)?;

        let lines = lines_of(&text);
        let regions = classify_regions(&lines);
        let name = file.file_name().expect("a fixture path has a file name");
        let output = format_once(&directory, name, &text)?;
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
                "{file} changed or lost the literal line {line:?} under the full flag set",
            );
        }
        assert!(
            output_literal_lines.next().is_none(),
            "{file} gained an extra literal line under the full flag set",
        );
        checked += 1;
    }

    Ok(checked)
}

#[test]
fn every_fixture_preserves_regions_under_fence_normalization() -> Result<()> {
    let root = fixture_root(&manifest_dir()?)?;
    let files = data_files(&root)?;

    assert!(!files.is_empty(), "found no fixtures to check");
    assert!(
        files
            .iter()
            .any(|path| path.file_name() == Some(MUST_COVER)),
        "the region sweep must cover {MUST_COVER}",
    );

    let checked = assert_regions_preserved(&root, &files)?;

    assert!(
        checked > 100,
        "expected the whole fixture corpus, checked {checked}",
    );
    Ok(())
}

#[test]
fn every_fixture_keeps_its_literal_lines_verbatim() -> Result<()> {
    let root = fixture_root(&manifest_dir()?)?;
    let files = data_files(&root)?;

    assert!(!files.is_empty(), "found no fixtures to check");

    let checked = assert_literal_lines_survive(&root, &files)?;

    assert!(
        checked > 100,
        "expected the whole fixture corpus, checked {checked}",
    );
    Ok(())
}
