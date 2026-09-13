//! What a failed `git` invocation means to whoever has to read it.
//!
//! Separate from the invocations themselves because its two renderings serve
//! readers the invocation never meets. [`GitListError::diagnostic`] is the line
//! a user reads: this repository's own wording with Git's text appended,
//! because a terminal shows one failure rather than a failure and an appendix.
//! [`Display`] alone is the half a test may assert on, since Git's own words
//! are localised and version-dependent. Selection queries return these errors
//! as data; the command boundary decides what, if anything, to report.
//!
//! [`Display`]: std::fmt::Display

use std::{io, process::ExitStatus};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GitListError {
    #[error("`{program}` is not installed or not on PATH")]
    ProgramNotFound { program: String },
    #[error("running `{command}`")]
    Spawn {
        /// The command as it would be typed, e.g. `git ls-files`.
        command: String,
        #[source]
        source: io::Error,
    },
    // `{status}`, not `exit status {status}`: `ExitStatus`'s own rendering is
    // already "exit status: 1" on Unix and "exit code: 1" on Windows, so
    // spelling the words here as well would say it twice.
    #[error("`{command}` failed with {status}")]
    Failed {
        /// The command as it would be typed, e.g. `git ls-files`.
        command: String,
        status: ExitStatus,
        /// Git's own diagnostic, relayed through
        /// [`relayable`](super::git_output::relayable).
        stderr: String,
    },
    #[error("`{command}` did not report a usable Git directory")]
    NoGitDir {
        /// The command as it would be typed, e.g. `git rev-parse`.
        command: String,
    },
}

impl GitListError {
    /// The one line this failure prints, with Git's own diagnostic appended.
    ///
    /// Separate from [`Display`](std::fmt::Display) so that this tool's
    /// wording stays a message of this repository's own — the part a test may
    /// assert on, per AX-GIT-NLS — while git's text is relayed beside it rather
    /// than inside it. The two are one line because a user reading a terminal
    /// reads one failure, not a failure and an appendix.
    #[must_use]
    pub fn diagnostic(&self) -> String {
        match self {
            Self::Spawn { source, .. } => format!("{self}: {source}"),
            Self::Failed { stderr, .. } if !stderr.is_empty() => format!("{self}: {stderr}"),
            _ => self.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "git_failure_tests.rs"]
mod tests;
