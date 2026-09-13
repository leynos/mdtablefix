//! The failure type's renderings, for the cases that need no process to reach.
//!
//! The cases that do need one — the status of a real failure, and the wording a
//! user reads when Git wrote nothing at all — are stated where the invocations
//! are, in `git_ls_files_git_tests.rs`.

use std::io;

use rstest::rstest;

use super::GitListError;

/// The category of each failure that needs no process to reach.
///
/// A closed set of four, so a host aggregating failures cannot be handed a
/// value that grows with the trees a run was given. The nonzero exit is driven
/// for real in `git_ls_files_git_tests.rs`, because an `ExitStatus` cannot be
/// built portably without a process to produce one.
#[rstest]
#[case(
    GitListError::ProgramNotFound { program: "git".to_string() },
    "program_not_found"
)]
#[case(
    GitListError::Spawn {
        command: "git ls-files".to_string(),
        source: io::Error::new(io::ErrorKind::PermissionDenied, "fixture"),
    },
    "spawn"
)]
#[case(
    GitListError::NoGitDir { command: "git rev-parse".to_string() },
    "no_git_dir"
)]
fn a_category_names_the_class_a_host_may_aggregate(
    #[case] error: GitListError,
    #[case] expected: &str,
) {
    assert_eq!(error.category(), expected);
}
