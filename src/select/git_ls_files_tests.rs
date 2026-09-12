//! Tests for what Git's output is read as, with no process in sight.
//!
//! The invocations themselves are pinned in `git_ls_files_git_tests.rs`, which
//! runs a real `git`; what is here is the text handling around them, where a
//! program would only make it harder to aim at one case.

use rstest::rstest;

use super::without_line_terminator;

/// The one terminator Git writes, and the characters it must not take with it.
///
/// Exactly one: a directory named `repo\n` is reported by `rev-parse` as a line
/// ending in two newlines, so stripping both would answer with a path that does
/// not exist. Everything before that terminator is the name, spaces, further
/// newlines, and carriage returns included.
#[rstest]
#[case("repo\n", "repo")]
#[case("repo\r\n", "repo")]
#[case("repo\n\n", "repo\n")]
#[case("repo ", "repo ")]
#[case("repo\r", "repo\r")]
// An empty report stays empty, so the caller's own `NoGitDir` answers for it
// rather than a path of nothing.
#[case("", "")]
#[case("\n", "")]
fn one_line_terminator_is_stripped_and_the_rest_is_the_path(
    #[case] reported: &str,
    #[case] expected: &str,
) {
    assert_eq!(without_line_terminator(reported), expected, "{reported:?}");
}
