//! Metrics tests for the replacements that do not complete.
//!
//! Each case here drives the replacement to a failure the boundary reports —
//! every candidate temporary name taken, or a cleanup that cannot remove the
//! file a failed swap left behind — and asserts the counter that failure emits.
//! A recorder installed with `metrics::with_local_recorder` captures what the
//! replacement emits on this thread, as in the parent module, whose harness
//! these tests share. The cases live in a child module so that each file stays
//! inside the project's line-count guideline.

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::*;
use crate::io::{cleanup_failure_seam, remove_failed_temporary_file, rename_failure_seam};

#[test]
fn an_exhausted_name_space_is_counted() {
    let dir = tempdir().expect("create temporary directory");
    let file = fixture(&dir);
    for attempt in 0..TEMP_FILE_ATTEMPTS {
        let candidate = temporary_path(camino::Utf8Path::new("sample.md"), attempt);
        fs::write(
            dir.path()
                .join(candidate.file_name().expect("candidate name")),
            "",
        )
        .expect("occupy the candidate name");
    }

    let (result, recorded) = recorded(|| rewrite(&file));

    let error = result.expect_err("every candidate name is occupied");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_labels_are_bounded(&recorded);
    assert_eq!(
        count(&recorded, COLLISIONS, &[]),
        u64::from(TEMP_FILE_ATTEMPTS),
        "every occupied candidate is a collision: {recorded:?}"
    );
    assert_eq!(
        count(&recorded, EXHAUSTED, &[]),
        1,
        "exhausting the name space is counted once: {recorded:?}"
    );
    assert_eq!(
        outcome_count(&recorded, "failure"),
        1,
        "an abandoned replacement is a failure: {recorded:?}"
    );
    assert_eq!(
        outcome_samples(&recorded, "failure").len(),
        1,
        "a failed replacement is timed too, so stalls before failure are visible: {recorded:?}"
    );
    assert_eq!(
        count(&recorded, CLEANUP_FAILURES, &[]),
        0,
        // `concat!` expands the format string, which rules out an implicit
        // capture, so the argument is named.
        concat!(
            "a replacement abandoned before its temporary file existed has none to clean up: ",
            "{recorded:?}"
        ),
        recorded = recorded
    );
}

/// A cleanup that does not complete is counted, because the failure that
/// prompted it is the one the caller sees: the counter is the only signal that
/// a stale temporary file was left beside the target.
#[test]
fn a_cleanup_that_cannot_remove_the_temporary_file_is_counted() {
    let dir = tempdir().expect("create temporary directory");
    let root = Utf8Path::from_path(dir.path()).expect("the temporary directory is UTF-8");
    let directory =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");
    // Removing a path that points to a directory is documented to fail on every
    // platform, so the cleanup fails without the test depending on a permission
    // bit, which a run as root would ignore.
    directory
        .create_dir("taken.tmp")
        .expect("create the entry the cleanup cannot remove");

    let ((), recorded) =
        recorded(|| remove_failed_temporary_file(&directory, Utf8Path::new("taken.tmp")));

    assert_labels_are_bounded(&recorded);
    assert_eq!(
        count(&recorded, CLEANUP_FAILURES, &[]),
        1,
        "a cleanup that did not complete is counted once: {recorded:?}"
    );
    assert!(
        is_described(&recorded, CLEANUP_FAILURES, &[]),
        "the counter must carry a description: {recorded:?}"
    );
}

/// The cleanup counter is emitted where the replacement boundary cleans up
/// after a failed swap, not only by the helper driven on its own, and the
/// caller still sees the failure that prompted the cleanup.
#[test]
fn a_cleanup_failure_at_the_replacement_boundary_is_counted() {
    let dir = tempdir().expect("create temporary directory");
    let file = fixture(&dir);
    let original = fs::read_to_string(&file).expect("read the fixture");
    let _swap = rename_failure_seam::arm();
    let _cleanup = cleanup_failure_seam::arm();

    let (result, recorded) = recorded(|| rewrite(&file));

    let error = result.expect_err("the armed rename fails the swap");
    assert!(
        error.to_string().contains("rename failure seam"),
        "the failure that prompted the cleanup is the one reported: {error}"
    );
    assert_labels_are_bounded(&recorded);
    assert_eq!(
        outcome_count(&recorded, "failure"),
        1,
        "the replacement failed whatever the cleanup did: {recorded:?}"
    );
    assert_eq!(
        count(&recorded, CLEANUP_FAILURES, &[]),
        1,
        "a cleanup that did not complete is counted once: {recorded:?}"
    );
    assert!(
        is_described(&recorded, CLEANUP_FAILURES, &[]),
        "the counter must carry a description: {recorded:?}"
    );
    assert_eq!(
        fs::read_to_string(&file).expect("read the target"),
        original,
        "a failed replacement leaves the target unchanged"
    );
    let residue = temporary_path(Utf8Path::new("sample.md"), 0);
    assert!(
        dir.path()
            .join(residue.file_name().expect("candidate name"))
            .exists(),
        "a cleanup that failed leaves its temporary file behind: {recorded:?}"
    );
}
