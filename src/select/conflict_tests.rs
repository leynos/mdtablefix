//! Tests for the two conflict predicates.
//!
//! Both are about the false-positive rate as much as about detection: a
//! document that *discusses* conflict markers must not be taken for a
//! conflicted one, so the "expected false" cases carry as much weight as the
//! true ones.

use camino::Utf8Path;
use rstest::rstest;

use super::{ConflictGuard, has_conflict_markers, operation_in_progress};

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
    let directory = tempfile::tempdir().expect("a temporary directory");
    let git_dir = Utf8Path::from_path(directory.path()).expect("a UTF-8 temporary directory");
    if matches!(guarding, Guarding::MidOperation) {
        std::fs::write(git_dir.join("MERGE_HEAD"), "").expect("create the marker");
    }
    let guard = match guarding {
        Guarding::Nothing => ConflictGuard::unguarded(),
        Guarding::Idle | Guarding::MidOperation => ConflictGuard::guarded(git_dir),
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
    let directory = tempfile::tempdir().expect("a temporary directory");
    let file = directory.path().join("not-a-directory");
    std::fs::write(&file, "").expect("create a file where a Git directory was expected");
    let git_dir = Utf8Path::from_path(&file).expect("a UTF-8 path");

    let refused = ConflictGuard::guarded(git_dir)
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
    let directory = tempfile::tempdir().expect("a temporary directory");
    let git_dir = Utf8Path::from_path(directory.path()).expect("a UTF-8 temporary directory");
    assert!(
        !operation_in_progress(git_dir).expect("read the idle fixture"),
        "the fixture must start idle"
    );

    match marker {
        Marker::File(name) => std::fs::write(git_dir.join(name), ""),
        Marker::Directory(name) => std::fs::create_dir_all(git_dir.join(name)),
    }
    .expect("create the marker");

    assert!(
        operation_in_progress(git_dir).expect("read the marked fixture"),
        "{marker:?} must signal an operation in progress"
    );
}

#[test]
fn an_idle_git_directory_is_not_mid_operation() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let git_dir = Utf8Path::from_path(directory.path()).expect("a UTF-8 temporary directory");
    std::fs::create_dir_all(git_dir.join("objects")).expect("create an object store");
    std::fs::create_dir_all(git_dir.join("refs")).expect("create a ref store");
    for name in ["HEAD", "ORIG_HEAD", "SQUASH_MSG", "COMMIT_EDITMSG"] {
        std::fs::write(git_dir.join(name), "").expect("create a file an idle repository holds");
    }

    assert!(
        !operation_in_progress(git_dir).expect("read the idle repository"),
        "a directory holding the entries an idle repository holds is not mid-operation"
    );
}

/// A marker that cannot be tested for is an error, not an answer.
///
/// A `git_dir` that is a regular file is the cheapest way to stage this: a
/// marker path *through* that file fails with `ENOTDIR`, and a run that read
/// that as "no operation in progress" would rewrite a conflicted file on the
/// strength of a question it never answered.
#[test]
fn an_unreadable_repository_is_an_error_rather_than_an_answer() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let file = directory.path().join("not-a-directory");
    std::fs::write(&file, "").expect("create a file where a Git directory was expected");
    let git_dir = Utf8Path::from_path(&file).expect("a UTF-8 path");

    let error = operation_in_progress(git_dir).expect_err("a file is not a Git directory");

    assert_eq!(error.git_dir, git_dir);
    assert_ne!(
        error.source.kind(),
        std::io::ErrorKind::NotFound,
        "an unanswered question must not be reported as absence"
    );
    // `ENOTDIR` is the Unix kind a path through a regular file produces; the
    // portable claim above is the one this test is about.
    #[cfg(unix)]
    assert_eq!(error.source.kind(), std::io::ErrorKind::NotADirectory);
}
