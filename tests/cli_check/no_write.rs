//! The read-only guarantee, asserted against a directory snapshot.
//!
//! The second test is the positive control the first needs: a snapshot helper
//! that could not observe a write would make "nothing changed" vacuous, so one
//! mode that must write is checked against the same helper.

use std::fs;

use tempfile::tempdir;

use super::{CLEAN, RAGGED, run_in, snapshot, status_of, stderr_of};

#[test]
fn directory_snapshot_unchanged() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("clean.md"), CLEAN).expect("write fixture");
    fs::write(dir.path().join("ragged.md"), RAGGED).expect("write fixture");
    fs::write(dir.path().join("empty.md"), "").expect("write fixture");
    fs::write(dir.path().join("bom.md"), format!("\u{FEFF}{RAGGED}")).expect("write fixture");
    let before = snapshot(dir.path());

    let output = run_in(
        dir.path(),
        &[
            "--check",
            "clean.md",
            "ragged.md",
            "empty.md",
            "bom.md",
            "missing.md",
        ],
    );

    assert_eq!(status_of(&output), 2, "an unreadable file must exit 2");
    assert!(
        stderr_of(&output).contains("missing.md"),
        "the run must name the unreadable file, which proves it reached the filesystem rather \
         than failing while parsing arguments: {}",
        stderr_of(&output)
    );
    assert_eq!(
        snapshot(dir.path()),
        before,
        "--check must leave entry set, lengths, and modification times untouched"
    );
}

#[test]
fn in_place_over_a_drifting_file_does_change_the_directory() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("ragged.md"), RAGGED).expect("write fixture");
    let before = snapshot(dir.path());

    let output = run_in(dir.path(), &["--in-place", "ragged.md"]);

    assert_eq!(status_of(&output), 0, "a successful rewrite exits 0");
    assert_ne!(
        snapshot(dir.path()),
        before,
        "the snapshot helper must detect a write, or the read-only assertion is vacuous"
    );
}
