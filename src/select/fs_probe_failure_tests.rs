//! Tests for how the probe classifies a failure to read.
//!
//! Each is stated as a function of the failure rather than staged through
//! [`AmbientPathProbe`], whose arms no fixture can reach: a path that
//! `symlink_metadata` has already accepted can arrive at them only by losing a
//! race with the filesystem. The fixture is stated here rather than shared with
//! `fs_probe_tests.rs`, so that each file can be read on its own.

use std::io::{self, ErrorKind};

use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::{Reading, confined_to, nearest_existing, unreadable};
use crate::select::policy::PathKind;

fn at(path: &str) -> Utf8PathBuf { Utf8PathBuf::from(path) }

/// `directory` as the UTF-8 path a test works in.
fn as_path(directory: &TempDir) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).expect("a UTF-8 temporary directory")
}

/// A temporary tree, handed over as the guard that removes it.
///
/// The guard is the fixture's value and a test takes it as an argument, so the
/// binding the fixture machinery generates in the test body owns it for as long
/// as the test runs; the path is derived from it there. Nothing destructures a
/// tuple and nothing can drop the tree early — and a *derived* fixture would:
/// a fixture's dependencies are injected by value, so one taking this guard
/// would delete the tree as it returned.
#[test_macros::allow_fixture_expansion_lints]
#[fixture]
fn temp_root() -> TempDir { tempfile::tempdir().expect("a temporary directory") }

fn write(root: &Utf8Path, name: &str, content: &str) {
    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the fixture directory");
    }
    std::fs::write(&path, content).expect("write a fixture");
}

/// How a failed read is classified, for the kinds that decide alone.
///
/// `NotFound` is not among these cases: what it means depends on the ancestors
/// of the path it was reported for, and the case below stages that. Each kind
/// here leaves the file present but unreadable — a permission failure, and the
/// `ENOTDIR` Unix produces for a path through a file, which reaches the caller
/// unchanged because it is already the answer the ancestor walk arrives at.
///
/// Stated as a function of the failure rather than through
/// [`AmbientPathProbe`], which no fixture can make report either of these: a
/// path `symlink_metadata` has already accepted can reach this decision only by
/// losing a race with the filesystem.
#[rstest]
#[case(ErrorKind::PermissionDenied)]
#[case(ErrorKind::NotADirectory)]
fn a_failure_that_is_not_absence_is_reported_unchanged(#[case] kind: ErrorKind) {
    let path = at("/repo/docs/guide.md");

    let error = unreadable(path.clone(), io::Error::from(kind))
        .expect_err("a file that is present but unreadable is not absent");

    assert_eq!(
        error.path, path,
        "the failure names the path it could not read"
    );
    assert_eq!(error.source.kind(), kind, "the cause is reported unchanged");
}

/// Absence is answered for the whole path, not for the leaf that failed.
///
/// Two shapes look identical to a leaf's own failure, and only the ancestors
/// tell them apart. A file that is gone from a directory that is there is the
/// staged deletion the selection skips; a path that runs through a regular file
/// cannot be there at all, and reading it as a deletion would skip a candidate
/// the run was asked to consider. Windows reports both as `NOT_FOUND`, where
/// Unix says `ENOTDIR`, so the second shape is staged here rather than left to
/// a platform that never asks the question.
#[rstest]
fn a_read_that_fails_under_a_file_is_not_an_absence(temp_root: TempDir) {
    let root = as_path(&temp_root);
    let gone = root.join("gone.md");
    assert_eq!(
        unreadable(gone.clone(), io::Error::from(ErrorKind::NotFound)).ok(),
        Some(PathKind::Missing),
        "gone.md is gone, and that is the answer the selection has a rule for"
    );

    write(&root, "blocker", "not a directory\n");
    let through_a_file = root.join("blocker/guide.md");
    let error = unreadable(through_a_file.clone(), io::Error::from(ErrorKind::NotFound))
        .expect_err("a path through a file is not an absence");

    assert_eq!(
        error.path, through_a_file,
        "the failure names the candidate it could not read"
    );
    assert_eq!(
        error.source.kind(),
        ErrorKind::NotADirectory,
        "the kind Unix reports for it, reported on every platform"
    );
}

/// A path no part of which is there is absent, not unreachable.
///
/// The walk stops at the first ancestor that exists, and here that is the root
/// of the fixture: what is missing is a whole subtree, which is what a staged
/// deletion of one looks like.
#[rstest]
fn a_read_that_fails_where_the_whole_path_is_gone_is_an_absence(temp_root: TempDir) {
    let root = as_path(&temp_root);
    let path = root.join("gone/sub/guide.md");

    assert_eq!(
        unreadable(path, io::Error::from(ErrorKind::NotFound)).ok(),
        Some(PathKind::Missing),
        "a subtree that is gone is absent, not unreachable"
    );
}

/// A failure reading an ancestor stops the walk, and is reported as it arrived.
///
/// The arm this states is the walk's own, and no fixture stages it: a candidate
/// reaches the walk only through a `NotFound` on its leaf, and the filesystem
/// has answered for every ancestor above it by then. The walk takes its reader
/// as a parameter for exactly this case, so the arm is a test's to cover —
/// including the replacement that walks past it and calls the candidate absent.
#[test]
fn a_failure_reading_an_ancestor_is_reported_rather_than_walked_past() {
    let docs = at("/repo/docs");
    let mut read_paths = Vec::new();

    let error = nearest_existing(Some(docs.as_path()), |ancestor| {
        read_paths.push(ancestor.to_owned());
        if ancestor == docs.as_path() {
            Reading::Failed(io::Error::from(ErrorKind::PermissionDenied))
        } else {
            Reading::Directory
        }
    })
    .expect("an ancestor that cannot be read is a failure, not an absence");

    assert_eq!(
        error.kind(),
        ErrorKind::PermissionDenied,
        "the cause is reported unchanged"
    );
    assert_eq!(
        read_paths,
        vec![docs],
        "the walk stops at the ancestor it could not read rather than climbing to /repo, which is \
         a directory and would answer that nothing is missing"
    );
}

/// A root that does not exist confines nothing.
///
/// Stated against the predicate rather than staged through
/// [`AmbientPathProbe`], which answers `Missing` for every candidate before the
/// root is ever asked about: a working directory removed mid-run is the only way
/// to arrive here, and that is a race no fixture should have to win.
/// Confinement that could not be established must not be reported as
/// confinement, and a selection over a tree that is not there names nothing.
#[rstest]
fn a_root_that_does_not_exist_confines_nothing(temp_root: TempDir) {
    let root = as_path(&temp_root);
    let gone = root.join("gone");

    assert!(
        !confined_to(&gone, &at("/canonical/guide.md"))
            .expect("an absent root is not a failure to read it"),
        "a root that cannot be resolved confines nothing"
    );
}

/// A root that exists but cannot be resolved is reported, not answered.
///
/// The distinction this draws is the same one the probe draws for a candidate:
/// absence is an answer the caller has a rule for, and any other failure is a
/// question that went unasked. A root reached through a file is the staging that
/// needs no permission trick, so this case holds for a privileged test runner as
/// well as an unprivileged one — and, because the classification asks the root's
/// ancestors rather than its own failure alone, the kind asserted below is the
/// same on every platform, including the one that reports it as absence.
#[rstest]
fn a_root_that_cannot_be_resolved_is_reported(temp_root: TempDir) {
    let root = as_path(&temp_root);
    write(&root, "blocker", "not a directory\n");
    let unreachable = root.join("blocker/sub");

    let error = confined_to(&unreachable, &at("/canonical/guide.md"))
        .expect_err("a root that cannot be resolved is a failure, not confinement");
    assert_eq!(
        error.path, unreachable,
        "the failure names the root it could not read"
    );
    assert_eq!(
        error.source.kind(),
        ErrorKind::NotADirectory,
        "a root behind a file is present, not absent: {error:?}"
    );
}
