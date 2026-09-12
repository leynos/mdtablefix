//! Tests for the two conflict predicates.
//!
//! Both are about the false-positive rate as much as about detection: a
//! document that *discusses* conflict markers must not be taken for a
//! conflicted one, so the "expected false" cases carry as much weight as the
//! true ones.

use std::io;

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;

use super::{
    ConflictGuard,
    has_conflict_markers,
    marker_present,
    opened_directory,
    operation_in_progress,
};

/// A temporary directory, its path, and the capability the scan reads it
/// through.
///
/// The markers are written through a capability of the same kind the scan uses,
/// so no fixture can pass by having written something the scan could not have
/// read. The [`tempfile::TempDir`] is part of the return value so the caller
/// keeps it alive for as long as the path is used.
fn git_dir_fixture() -> (tempfile::TempDir, Utf8PathBuf, Dir) {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let git_dir = Utf8PathBuf::from_path_buf(temporary.path().to_path_buf())
        .expect("a UTF-8 temporary directory");
    let directory =
        Dir::open_ambient_dir(&git_dir, ambient_authority()).expect("open the fixture directory");

    (temporary, git_dir, directory)
}

/// Git's own shape: three markers, each at the start of a line, with the
/// conflicted content between them.
const CONFLICTED: &str = "<<<<<<< HEAD\n| A | B |\n=======\n| A | C |\n>>>>>>> side\n";

#[rstest]
#[case(CONFLICTED, true)]
#[case("<<<<<<< HEAD\n=======\n>>>>>>> side\n", true)]
// One marker short: the two sides of a boundary are not a boundary.
#[case("<<<<<<< HEAD\n| A |\n=======\n| B |\n", false)]
#[case("<<<<<<< HEAD\n| A |\n>>>>>>> side\n", false)]
#[case("| A |\n=======\n| B |\n>>>>>>> side\n", false)]
// Seven, Git's default length, at the start of a line.
// A longer run is the same marker at a configured `conflict-marker-size`, so
// the length is a floor rather than a measurement: eight `<` is emphasis in
// general prose, and is not one during an operation this scan is gated on.
#[case("<<<<<<<< HEAD\n=======\n>>>>>>> side\n", true)]
#[case("<<<<<<<<<<<< HEAD\n============\n>>>>>>>>>>>> side\n", true)]
// Not at the start of a line, which is where a run is a setext heading
// underline or emphasis rather than a marker.
#[case("  <<<<<<< HEAD\n=======\n>>>>>>> side\n", false)]
#[case("<<<<<<< HEAD\n =======\n>>>>>>> side\n", false)]
// A fenced example that mentions one marker, which is what prose about
// conflicts looks like.
#[case("```\n<<<<<<< HEAD\n```\n", false)]
fn all_three_markers_are_required_at_the_start_of_a_line(
    #[case] content: &str,
    #[case] expected: bool,
) {
    assert_eq!(has_conflict_markers(content), expected, "{content:?}");
}

/// What requiring all three forms buys: prose can name a marker without
/// writing one at the start of a line, so a setext heading underline supplies
/// the separator and nothing else.
#[test]
fn a_document_about_conflict_markers_is_not_a_conflicted_one() {
    let prose = "Resolving conflicts\n=======\n\nGit writes `<<<<<<< HEAD` where the first side \
                 begins and `>>>>>>>` at the end.\n";
    assert!(!has_conflict_markers(prose), "{prose:?}");
}

/// The limitation, stated rather than discovered: the predicate is not a
/// Markdown parser, so a fenced example carrying all three markers is
/// indistinguishable from a conflict. `--allow-conflicted` is the answer for a
/// repository that documents conflict markers, and it is why this scan is
/// gated on an operation actually being in progress.
#[test]
fn a_fenced_example_carrying_all_three_markers_still_counts() {
    let fenced = "```text\n<<<<<<< HEAD\n=======\n>>>>>>> side\n```\n";
    assert!(has_conflict_markers(fenced));
}

/// The three states a guard can be in, as a case names them.
#[derive(Debug, Clone, Copy)]
enum Guarding {
    /// Nothing to consult, which is also the guard an `--allow-conflicted` run
    /// gets: the user has asked for the rewrite whatever the repository is
    /// doing.
    Nothing,
    /// A repository that is not mid-operation.
    Idle,
    /// A repository that is mid-operation.
    MidOperation,
}

/// The refusal is the conjunction of three facts, and each of them is load
/// bearing: a rewrite that ignores the operation corrupts a resolution, one
/// that ignores the markers rewrites an unresolved file, and one that ignores
/// `--allow-conflicted` cannot be overridden by the user.
#[rstest]
#[case(Guarding::Nothing, true, false)]
#[case(Guarding::Nothing, false, false)]
#[case(Guarding::Idle, true, false)]
#[case(Guarding::Idle, false, false)]
#[case(Guarding::MidOperation, true, true)]
#[case(Guarding::MidOperation, false, false)]
fn a_conflicted_file_is_refused_only_mid_operation_and_without_the_override(
    #[case] guarding: Guarding,
    #[case] conflicted: bool,
    #[case] expected: bool,
) {
    let (_temporary, git_dir, directory) = git_dir_fixture();
    if matches!(guarding, Guarding::MidOperation) {
        directory
            .write("MERGE_HEAD", "")
            .expect("create the marker");
    }
    let guard = match guarding {
        Guarding::Nothing => ConflictGuard::unguarded(),
        Guarding::Idle | Guarding::MidOperation => ConflictGuard::guarded(&git_dir),
    };
    let content = if conflicted {
        CONFLICTED
    } else {
        "| A | B |\n"
    };

    assert_eq!(
        guard.refuses(content).expect("read the repository"),
        expected,
        "guarding={guarding:?} conflicted={conflicted}"
    );
}

/// A run with nothing to consult never refuses: there is no directory to ask,
/// which is what keeps an ordinary run — and a run whose user passed
/// `--allow-conflicted` — from spawning `git` for the guard.
#[test]
fn the_unguarded_run_refuses_nothing() {
    let refused = ConflictGuard::unguarded()
        .refuses(CONFLICTED)
        .expect("the unguarded guard asks nothing");

    assert!(!refused);
}

/// The marker scan decides before the repository is consulted, so a document
/// carrying none of the three markers never has its repository read.
///
/// The Git directory here is a regular file, which every marker test fails on:
/// a clean document comes back unrefused anyway, which is only possible if the
/// scan ran first. It is the ordering, rather than the answer, that this pins.
#[test]
fn a_document_without_markers_never_asks_the_repository() {
    let (_temporary, root, directory) = git_dir_fixture();
    directory
        .write("not-a-directory", "")
        .expect("create a file where a Git directory was expected");
    let git_dir = root.join("not-a-directory");

    let refused = ConflictGuard::guarded(&git_dir)
        .refuses("| A | B |\n")
        .expect("a document without markers must not ask the repository");

    assert!(!refused);
}

#[derive(Debug, Clone, Copy)]
enum Marker {
    File(&'static str),
    Directory(&'static str),
}

#[rstest]
#[case(Marker::File("MERGE_HEAD"))]
// The pseudoref `git revert` writes when it stops on a conflict: without it, a
// paused revert is an operation this tool would rewrite inside.
#[case(Marker::File("REVERT_HEAD"))]
#[case(Marker::File("CHERRY_PICK_HEAD"))]
#[case(Marker::Directory("rebase-merge"))]
#[case(Marker::Directory("rebase-apply"))]
fn an_in_progress_operation_is_detected(#[case] marker: Marker) {
    let (_temporary, git_dir, directory) = git_dir_fixture();
    assert!(
        !operation_in_progress(&git_dir).expect("read the idle fixture"),
        "the fixture must start idle"
    );

    match marker {
        Marker::File(name) => directory.write(name, ""),
        Marker::Directory(name) => directory.create_dir_all(name),
    }
    .expect("create the marker");

    assert!(
        operation_in_progress(&git_dir).expect("read the marked fixture"),
        "{marker:?} must signal an operation in progress"
    );
}

#[test]
fn an_idle_git_directory_is_not_mid_operation() {
    let (_temporary, git_dir, directory) = git_dir_fixture();
    directory
        .create_dir_all("objects")
        .expect("create an object store");
    directory
        .create_dir_all("refs")
        .expect("create a ref store");
    for name in ["HEAD", "ORIG_HEAD", "SQUASH_MSG", "COMMIT_EDITMSG"] {
        directory
            .write(name, "")
            .expect("create a file an idle repository holds");
    }

    assert!(
        !operation_in_progress(&git_dir).expect("read the idle repository"),
        "a directory holding the entries an idle repository holds is not mid-operation"
    );
}

/// A marker that cannot be tested for is an error, not an answer.
///
/// A `git_dir` that is a regular file is the cheapest way to stage this, and the
/// two platforms arrive at it differently: the capability refuses the file
/// outright on Linux, and opens it on Windows, where the check on what was
/// opened is what reports it. Both are
/// [`std::io::ErrorKind::NotADirectory`] — the kind Unix reports for a path
/// through a file — so the kind is asserted rather than merely distinguished
/// from absence. A run that read the failure as "no operation in progress" would
/// rewrite a conflicted file on the strength of a question it never answered.
#[test]
fn an_unreadable_repository_is_an_error_rather_than_an_answer() {
    let (_temporary, root, directory) = git_dir_fixture();
    directory
        .write("not-a-directory", "")
        .expect("create a file where a Git directory was expected");
    let git_dir = root.join("not-a-directory");

    let error = operation_in_progress(&git_dir).expect_err("a file is not a Git directory");

    assert_eq!(error.git_dir, git_dir);
    assert_eq!(
        error.source.kind(),
        std::io::ErrorKind::NotADirectory,
        "an unanswered question must not be reported as absence"
    );
}

/// What was opened is asked what it is, before any marker is read beneath it.
///
/// The one arm Windows alone reaches: there `open_ambient_dir` accepts a regular
/// file, and every marker read under it fails with the same `NOT_FOUND` as a
/// marker that is not there — an unreadable repository read as an idle one.
/// Stated as a function of the metadata rather than through
/// [`operation_in_progress`], whose open refuses the same file on Linux before
/// this check is reached, so that the decision is covered on both platforms.
#[test]
fn a_git_directory_that_is_not_a_directory_is_reported() {
    let (_temporary, root, directory) = git_dir_fixture();
    directory
        .write("not-a-directory", "")
        .expect("create a file where a Git directory was expected");
    let git_dir = root.join("not-a-directory");
    let metadata = directory
        .metadata("not-a-directory")
        .expect("read the fixture's metadata");

    let error = opened_directory(&git_dir, &metadata).expect_err("a file is not a directory");

    assert_eq!(
        error.git_dir, git_dir,
        "the failure names the directory it was asked about"
    );
    assert_eq!(
        error.source.kind(),
        std::io::ErrorKind::NotADirectory,
        "the kind Unix reports for it, reported on every platform"
    );
}

/// The three outcomes of testing for a marker, and which of them is an answer.
///
/// The third is the one no fixture reaches through the filesystem: the
/// capability refuses a Git directory it cannot open before the loop begins,
/// and a marker name is entered in a directory that is already open. A function
/// of the result is what keeps that arm covered anyway, and for the same reason
/// the probe's `unreadable` is one.
#[rstest]
#[case(Ok(()), Some(true))]
#[case(Err(io::Error::from(io::ErrorKind::NotFound)), Some(false))]
#[case(Err(io::Error::from(io::ErrorKind::PermissionDenied)), None)]
fn an_unreadable_marker_is_an_unanswered_question_rather_than_an_absent_one(
    #[case] test: io::Result<()>,
    #[case] expected: Option<bool>,
) {
    match (marker_present(test), expected) {
        (Ok(present), Some(expected)) => assert_eq!(present, expected),
        (Err(error), None) => assert_eq!(error.kind(), io::ErrorKind::PermissionDenied),
        (outcome, _) => panic!("the marker test was read as {outcome:?}"),
    }
}

/// The one behaviour the capability changes: a Git directory that has gone
/// since the run resolved it is reported, not answered.
///
/// The guard is consulted per written file, so the directory it was given can
/// be removed while a long run is still analysing files. Reading a marker under
/// a directory that is not there used to come back as absence, which is the
/// answer "no operation in progress"; the question now fails to be asked, and
/// an unanswered question is not a licence to write.
#[test]
fn a_git_directory_that_has_gone_is_reported_rather_than_answered() {
    let (_temporary, root, _directory) = git_dir_fixture();
    let gone = root.join("gone");

    let error = operation_in_progress(&gone).expect_err("a Git directory that is not there");

    assert_eq!(
        error.git_dir, gone,
        "the failure names the directory it could not open"
    );
    assert_eq!(
        error.source.kind(),
        std::io::ErrorKind::NotFound,
        "the cause is the absence of the directory itself"
    );
}
