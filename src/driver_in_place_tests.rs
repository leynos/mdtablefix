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

use super::{
    Mode,
    analyse,
    test_support::{ALIGNED, RAGGED, align, fixture, identity, read},
};

/// `--in-place` is the one mode that writes, and its payload is empty: the
/// formatted text goes to the file, not to standard output.
#[test]
fn in_place_writes_the_formatted_text() {
    let (_dir, directory) = fixture("ragged.md", RAGGED);

    let (_report, payload) = analyse(
        Mode::InPlace,
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &align,
    )
    .expect("analyse fixture");

    assert_eq!(payload, "");
    assert_eq!(read(&directory, "ragged.md"), ALIGNED);
}

/// A drifting file is replaced, not edited in place.
///
/// The positive control for [`in_place_leaves_a_clean_file_untouched`]:
/// `replace_file` renames a temporary over the target, so a file that really is
/// rewritten must come back with a different inode. Without this, an
/// implementation that never wrote anything would satisfy the invariance test.
#[cfg(unix)]
#[test]
fn in_place_replaces_a_drifting_file() {
    let (dir, directory) = fixture("ragged.md", RAGGED);
    let target = dir.path().join("ragged.md");
    let before = fs::metadata(&target).expect("read the metadata before the write");

    analyse(
        Mode::InPlace,
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
    assert_eq!(read(&directory, "ragged.md"), ALIGNED);
}

/// A clean file is left alone byte for byte, and observably so.
///
/// The write would be invisible in the text — the bytes written would be the
/// bytes already there — but not in the file: `replace_file` swaps the inode and
/// the modification time of a file it did not change, so a staleness check
/// downstream would see a rebuild where there was nothing to rebuild.
#[cfg(unix)]
#[test]
fn in_place_leaves_a_clean_file_untouched() {
    let (dir, directory) = fixture("clean.md", ALIGNED);
    let target = dir.path().join("clean.md");
    let before = fs::metadata(&target).expect("read the metadata before the analysis");

    let (report, payload) = analyse(
        Mode::InPlace,
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
    assert_eq!(read(&directory, "clean.md"), ALIGNED);
}
