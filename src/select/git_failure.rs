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
//! A third reader aggregates rather than reads: [`GitListError::category`]
//! renders the same failure as one of four bounded classes, which is what a
//! host may chart where a diagnostic would put another program's words in a
//! telemetry field.
//!
//! [`Display`]: std::fmt::Display

use std::{io, process::ExitStatus};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
/// Failure returned when a Git-backed candidate query cannot complete.
///
/// Variants distinguish process startup, command failure, and repository
/// discovery so callers can report or aggregate the failure without parsing
/// diagnostic text.
pub enum GitListError {
    /// The configured Git program could not be found.
    #[error("`{program}` is not installed or not on PATH")]
    ProgramNotFound {
        /// Program name supplied to the attempted process invocation.
        program: String,
    },
    /// The Git process could not be started for an I/O reason.
    #[error("running `{command}`")]
    Spawn {
        /// The command as it would be typed, e.g. `git ls-files`.
        command: String,
        #[source]
        /// Operating-system error reported while starting Git.
        source: io::Error,
    },
    // `{status}`, not `exit status {status}`: `ExitStatus`'s own rendering is
    // already "exit status: 1" on Unix and "exit code: 1" on Windows, so
    // spelling the words here as well would say it twice.
    /// Git started but returned a non-success status.
    #[error("`{command}` failed with {status}")]
    Failed {
        /// The command as it would be typed, e.g. `git ls-files`.
        command: String,
        /// Exit status returned by Git after the command ran.
        status: ExitStatus,
        /// Git's own diagnostic, relayed through
        /// [`relayable`](super::git_output::relayable).
        stderr: String,
    },
    /// Git ran but did not return a usable repository directory.
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

    /// The bounded class this failure belongs to.
    ///
    /// What a tracing field may carry where [`diagnostic`](Self::diagnostic)
    /// carries prose: a closed set of four, none of which is a path, a status
    /// code, or anything Git wrote. `ExitStatus` is deliberately not reported
    /// as itself — the class of a failure is what a host aggregates, and the
    /// number of a failing exit code is a detail of Git's, not of ours.
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::ProgramNotFound { .. } => "program_not_found",
            Self::Spawn { .. } => "spawn",
            Self::Failed { .. } => "nonzero_exit",
            Self::NoGitDir { .. } => "no_git_dir",
        }
    }
}

#[cfg(test)]
#[path = "git_failure_tests.rs"]
mod tests;
