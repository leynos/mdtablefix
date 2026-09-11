//! Tests for the two conflict predicates.
//!
//! Both are about the false-positive rate as much as about detection: a
//! document that *discusses* conflict markers must not be taken for a
//! conflicted one, so the "expected false" cases carry as much weight as the
//! true ones.

use camino::Utf8Path;
use rstest::rstest;

use super::{has_conflict_markers, operation_in_progress};

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
// An exact seven-character run, at the start of a line.
#[case("<<<<<<<< HEAD\n=======\n>>>>>>> side\n", false)]
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

/// The case the exactness buys: a setext heading underline is seven `=`
/// characters at the start of a line, and prose can name the other two markers
/// without writing them at the start of a line.
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

#[derive(Debug, Clone, Copy)]
enum Marker {
    File(&'static str),
    Directory(&'static str),
}

#[rstest]
#[case(Marker::File("MERGE_HEAD"))]
#[case(Marker::Directory("rebase-merge"))]
#[case(Marker::Directory("rebase-apply"))]
#[case(Marker::File("CHERRY_PICK_HEAD"))]
fn an_in_progress_operation_is_detected(#[case] marker: Marker) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let git_dir = Utf8Path::from_path(directory.path()).expect("a UTF-8 temporary directory");
    assert!(
        !operation_in_progress(git_dir),
        "the fixture must start idle"
    );

    match marker {
        Marker::File(name) => std::fs::write(git_dir.join(name), ""),
        Marker::Directory(name) => std::fs::create_dir_all(git_dir.join(name)),
    }
    .expect("create the marker");

    assert!(
        operation_in_progress(git_dir),
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
        !operation_in_progress(git_dir),
        "a directory holding the entries an idle repository holds is not mid-operation"
    );
}
