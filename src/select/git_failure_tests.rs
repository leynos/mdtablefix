//! The failure type's renderings, for the cases that need no process to reach.
//!
//! The cases that do need one — the status of a real failure, and the wording a
//! user reads when Git wrote nothing at all — are stated where the invocations
//! are, in `git_ls_files_git_tests.rs`.

use std::io;

use super::GitListError;

/// A spawn failure includes its operating-system cause beside the command.
#[test]
fn a_spawn_diagnostic_includes_its_source() {
    let error = GitListError::Spawn {
        command: "git ls-files".to_string(),
        source: io::Error::new(io::ErrorKind::PermissionDenied, "fixture"),
    };

    assert_eq!(error.diagnostic(), "running `git ls-files`: fixture");
}
