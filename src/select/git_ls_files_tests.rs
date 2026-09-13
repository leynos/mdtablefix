//! Tests for what Git's output is read as, and what it is run with, with no
//! process in sight.
//!
//! The invocations themselves are pinned in `git_ls_files_git_tests.rs`, which
//! runs a real `git`; what is here is the text handling around them and the
//! command they are built from, where a program would only make it harder to
//! aim at one case.

use std::ffi::OsStr;

use camino::Utf8Path;
use rstest::rstest;

use super::{GitLsFiles, REDIRECTING, without_line_terminator};

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

/// Every variable that would redirect an invocation elsewhere is removed from
/// the environment it runs with.
///
/// The names are spelled here rather than read from [`REDIRECTING`], so that
/// dropping one from the adapter fails this test rather than shrinking what it
/// checks. [`Command::get_envs`] reports a removal as a key with no value,
/// which is what makes the decision visible without a process to run: the
/// ambient environment cannot be asserted on directly, because a test that set
/// one would be mutating the environment its own process shares.
///
/// [`Command::get_envs`]: std::process::Command::get_envs
#[rstest]
#[case("GIT_DIR")]
#[case("GIT_WORK_TREE")]
#[case("GIT_INDEX_FILE")]
#[case("GIT_COMMON_DIR")]
fn a_redirecting_variable_is_removed_from_the_invocations_environment(#[case] variable: &str) {
    let command = GitLsFiles::new(false).command(&["ls-files"], Utf8Path::new("."));

    let removed: Vec<&OsStr> = command
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(key, _)| key)
        .collect();

    assert!(
        removed.contains(&OsStr::new(variable)),
        "every invocation must be told to ignore {variable}: {removed:?}"
    );
}

/// The adapter removes variables rather than clearing the environment, so the
/// user's own Git configuration still governs the selection.
///
/// Without this, a change to `env_clear()` would satisfy the test above and
/// quietly take `core.excludesFile`, an identity, and every other setting the
/// user configured out of the run.
#[test]
fn the_environment_is_pruned_rather_than_cleared() {
    let command = GitLsFiles::new(false).command(&["ls-files"], Utf8Path::new("."));
    let mut removed: Vec<&OsStr> = command
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(key, _)| key)
        .collect();
    removed.sort_unstable();

    assert_eq!(
        removed,
        [
            OsStr::new("GIT_COMMON_DIR"),
            OsStr::new("GIT_DIR"),
            OsStr::new("GIT_INDEX_FILE"),
            OsStr::new("GIT_WORK_TREE"),
        ],
        "an invocation must remove exactly the Git redirections"
    );
}

/// The declared set is exactly the one the cases above pin, so widening what
/// the adapter strips is a deliberate act rather than a silent one.
#[test]
fn the_declared_redirections_are_the_ones_the_cases_pin() {
    let mut declared = REDIRECTING.to_vec();
    declared.sort_unstable();

    assert_eq!(
        declared,
        [
            "GIT_COMMON_DIR",
            "GIT_DIR",
            "GIT_INDEX_FILE",
            "GIT_WORK_TREE"
        ],
        "a variable added to the adapter needs a case naming it"
    );
}
