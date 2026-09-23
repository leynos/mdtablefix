//! Unit tests for `--in-place`, the one mode that writes.
//!
//! Its payload is empty and its effect is on the file, so these tests read the
//! fixture back through the capability that wrote it. The two replacement
//! tests are Unix-only: they observe the write through the inode, which is not
//! a portable thing to assert.

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use camino::Utf8Path;
use mdtablefix::io::SourceDocument;

// `identity` is used by an inode-observing test, which is Unix-only, so the
// import has to be Unix-only as well: on Windows it would be an unused import,
// and this repository's test targets deny warnings.
#[cfg(unix)]
use super::test_support::identity;
use super::{
    ConflictGuard,
    Mode,
    analyse,
    test_support::{ALIGNED, RAGGED, align, fixture, read},
};

/// The text another writer leaves behind, distinct from both fixtures.
const INTRUDER: &str = "|X|Y|\n|---|---|\n|3|4|\n";

/// `--in-place` is the one mode that writes, and its payload is empty: the
/// formatted text goes to the file, not to standard output.
#[test]
fn in_place_writes_the_formatted_text() -> anyhow::Result<()> {
    let (_dir, directory) = fixture("ragged.md", RAGGED)?;

    let (_report, payload) = analyse(
        Mode::InPlace,
        &ConflictGuard::unguarded(),
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &align,
    )
    .expect("analyse fixture");

    assert_eq!(payload, "");
    assert_eq!(read(&directory, "ragged.md")?, ALIGNED);
    Ok(())
}

/// An ordinary changed document never needs repository state to be read.
///
/// A regular file deliberately stands in for the Git directory here: opening
/// it as a directory would fail, so successful formatting proves that the
/// marker scan kept the guarded state probe out of this path.
#[test]
fn in_place_formats_an_unmarked_file_without_reading_repository_state() -> anyhow::Result<()> {
    let (dir, directory) = fixture("ragged.md", RAGGED)?;
    let regular_file = dir.path().join("ragged.md");
    let git_dir = Utf8Path::from_path(&regular_file).expect("the fixture path is UTF-8");

    let (report, payload) = analyse(
        Mode::InPlace,
        &ConflictGuard::guarded(git_dir),
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &align,
    )
    .expect("an unmarked file does not query repository state");

    assert!(report.is_changed);
    assert_eq!(payload, "");
    assert_eq!(read(&directory, "ragged.md")?, ALIGNED);
    Ok(())
}

/// A drifting file is replaced, not edited in place.
///
/// The positive control for [`in_place_leaves_a_clean_file_untouched`]:
/// `replace_file` renames a temporary over the target, so a file that really is
/// rewritten must come back with a different inode. Without this, an
/// implementation that never wrote anything would satisfy the invariance test.
#[cfg(unix)]
#[test]
fn in_place_replaces_a_drifting_file() -> anyhow::Result<()> {
    let (dir, directory) = fixture("ragged.md", RAGGED)?;
    let target = dir.path().join("ragged.md");
    let before = fs::metadata(&target).expect("read the metadata before the write");

    analyse(
        Mode::InPlace,
        &ConflictGuard::unguarded(),
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &align,
    )
    .expect("analyse fixture");

    let after = fs::metadata(&target).expect("read the metadata after the write");
    assert_ne!(
        before.ino(),
        after.ino(),
        "a drifting file must be replaced through a temporary"
    );
    assert_eq!(read(&directory, "ragged.md")?, ALIGNED);
    Ok(())
}

/// A file another writer changed while it was being formatted is not
/// overwritten.
///
/// The seam is the formatter itself, which runs between the read that produced
/// the assessment and the replacement that would act on it — the whole window
/// the conditional write exists to close. The other writer is an ambient one,
/// as a concurrent writer would be: it holds no capability of this run's, and
/// the run's own read is what goes stale.
#[test]
fn in_place_declines_a_file_that_changed_under_it() -> anyhow::Result<()> {
    let (dir, directory) = fixture("ragged.md", RAGGED)?;
    let intruder_path = dir.path().join("ragged.md");
    let intruder = move |document: &SourceDocument<'_>| {
        std::fs::write(&intruder_path, INTRUDER).expect("write the concurrent change");
        align(document)
    };

    let error = analyse(
        Mode::InPlace,
        &ConflictGuard::unguarded(),
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &intruder,
    )
    .expect_err("a file that changed under the run must not be overwritten");

    assert!(
        error
            .to_string()
            .contains("changed while it was being formatted"),
        "the error must say why the file was left alone: {error}"
    );
    assert_eq!(
        read(&directory, "ragged.md")?,
        INTRUDER,
        "the other writer's text must survive the run"
    );
    assert_eq!(
        directory
            .read_dir(".")
            .expect("read the fixture directory")
            .count(),
        1,
        "a declined write must leave no temporary file behind"
    );
    Ok(())
}

/// A clean file is left alone byte for byte, and observably so.
///
/// The write would be invisible in the text — the bytes written would be the
/// bytes already there — but not in the file: `replace_file` swaps the inode and
/// the modification time of a file it did not change, so a staleness check
/// downstream would see a rebuild where there was nothing to rebuild.
#[cfg(unix)]
#[test]
fn in_place_leaves_a_clean_file_untouched() -> anyhow::Result<()> {
    let (dir, directory) = fixture("clean.md", ALIGNED)?;
    let target = dir.path().join("clean.md");
    let before = fs::metadata(&target).expect("read the metadata before the analysis");

    let (report, payload) = analyse(
        Mode::InPlace,
        &ConflictGuard::unguarded(),
        &directory,
        Utf8Path::new("clean.md"),
        Utf8Path::new("clean.md"),
        &identity,
    )
    .expect("analyse fixture");

    let after = fs::metadata(&target).expect("read the metadata after the analysis");
    assert_eq!(payload, "");
    assert!(
        !report.is_changed,
        "the fixture is the formatter's own output"
    );
    assert_eq!(
        before.ino(),
        after.ino(),
        "a clean file must not be replaced by a temporary"
    );
    assert_eq!(
        before.mtime(),
        after.mtime(),
        "a clean file's modification time must not move"
    );
    assert_eq!(
        before.mtime_nsec(),
        after.mtime_nsec(),
        "a clean file's modification time must not move"
    );
    assert_eq!(read(&directory, "clean.md")?, ALIGNED);
    Ok(())
}
