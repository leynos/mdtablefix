//! The candidate source: the `git` invocations `--git` is built on.
//!
//! Depends on `std::process` and `camino`. It is the only place in the tree
//! that spawns `git`, and it deliberately knows nothing about extensions or
//! identity: it reports what the index holds and where the Git directory is,
//! and [`crate::select::policy`] decides what that means. What comes back is
//! parsed and made safe in [`super::git_output`].
//!
//! `git`'s own diagnostics are relayed, never asserted on: they are localised
//! and version-dependent. Every message this module shows a user is a `Display`
//! impl this repository owns, and the relayed text is one line of it, scrubbed
//! by [`relayable`](super::git_output::relayable).
//!
//! Every invocation is also traced, because it crosses a process boundary: a
//! `debug` span names the operation ([`Operation::name`]), and one `debug` event
//! per invocation carries that name, the outcome, the elapsed time, and — for a
//! failure — the bounded category [`GitListError::category`] returns. Nothing
//! traced is a path or Git's own text: a span field is not a metric label, but
//! the same discipline is kept, so a host can chart this path without storing
//! the tree a run was given.

use std::{
    ffi::OsString,
    io,
    process::{Command, ExitStatus},
    time::Instant,
};

use camino::{Utf8Path, Utf8PathBuf};
use tracing::{Span, debug, field};

use super::git_output::{CandidateListing, relayable, split_nul_delimited};

/// The program every invocation runs unless a caller names another.
const PROGRAM: &str = "git";

/// The subcommand, and the arguments that make its framing machine-readable.
///
/// `-z` terminates each path with a NUL, so a path containing a newline or a
/// quote needs no escaping, and `--deduplicate` is passed because refusing a
/// correct upstream flag so that a domain invariant has something to test
/// would be the test wagging the design. No `--full-name`: the listing stays
/// relative to the directory the command runs in, so `--git` is
/// subtree-scoped.
const SUBCOMMAND: &str = "ls-files";
const FRAMING: [&str; 3] = ["-z", "--deduplicate", "--cached"];

/// The subcommand that reports where the Git directory is.
///
/// `--absolute-git-dir` rather than a walk looking for a `.git` entry: a
/// linked worktree and a submodule both hold a `.git` *file* naming a
/// directory elsewhere, and `GIT_DIR` may point anywhere at all, so a walk
/// would misresolve exactly the repositories a merge is most likely to be
/// paused in. It also answers for a bare repository, where the working tree
/// has no `.git` entry at all.
const REV_PARSE: &str = "rev-parse";
const ABSOLUTE_GIT_DIR: &str = "--absolute-git-dir";

/// What `--include-untracked` adds: the files Git would commit, and nothing
/// it is told to ignore.
const UNTRACKED: [&str; 2] = ["--others", "--exclude-standard"];

/// One `git` invocation this module makes.
///
/// A closed set, because its two renderings both leave this process: the
/// subcommand is argv, and the name is a tracing field. A field drawn from a
/// type with two values cannot carry a path, and cannot grow to carry one
/// without the enum growing first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    /// The candidate listing, `git ls-files`.
    LsFiles,
    /// The Git directory query, `git rev-parse`.
    RevParse,
}

impl Operation {
    /// The subcommand, as passed to the program and named in a diagnostic.
    const fn subcommand(self) -> &'static str {
        match self {
            Self::LsFiles => SUBCOMMAND,
            Self::RevParse => REV_PARSE,
        }
    }

    /// The tracing name. Distinct from the subcommand only in being our
    /// spelling rather than Git's, and never a field a host has to escape.
    const fn name(self) -> &'static str {
        match self {
            Self::LsFiles => "ls_files",
            Self::RevParse => "rev_parse",
        }
    }
}

/// Lists candidates by running `git ls-files`.
#[derive(Debug, Clone)]
pub struct GitLsFiles {
    program: OsString,
    include_untracked: bool,
}

impl GitLsFiles {
    #[must_use]
    pub fn new(include_untracked: bool) -> Self { Self::with_program(PROGRAM, include_untracked) }

    /// Uses a specific program. The seam that lets tests drive the failure
    /// paths without a real Git installation, and that makes failure-message
    /// snapshots a function of our code rather than of the machine's `git`.
    #[must_use]
    pub fn with_program(program: impl Into<OsString>, include_untracked: bool) -> Self {
        Self {
            program: program.into(),
            include_untracked,
        }
    }

    /// # Errors
    ///
    /// See [`GitListError`].
    pub fn list_candidates(&self, dir: &Utf8Path) -> Result<CandidateListing, GitListError> {
        let mut args = vec![SUBCOMMAND];
        args.extend_from_slice(&FRAMING);
        if self.include_untracked {
            args.extend_from_slice(&UNTRACKED);
        }
        let output = self.run(Operation::LsFiles, &args, dir)?;

        Ok(split_nul_delimited(&output.stdout))
    }

    /// Resolves the Git directory that governs `dir`.
    ///
    /// # Errors
    ///
    /// See [`GitListError`]. This is not a best-effort query: the conflict
    /// guard has nowhere to look without it, so a caller that needs the guard
    /// fails the run rather than proceeding unguarded.
    pub fn resolve_git_dir(&self, dir: &Utf8Path) -> Result<Utf8PathBuf, GitListError> {
        let output = self.run(Operation::RevParse, &[REV_PARSE, ABSOLUTE_GIT_DIR], dir)?;

        // Only the line terminator is trimmed, never path characters: a
        // directory name may legitimately end in a space, and trimming that
        // would send the guard looking somewhere else entirely.
        let reported = std::str::from_utf8(&output.stdout)
            .map_err(|_| GitListError::NoGitDir {
                command: self.label(Operation::RevParse),
            })?
            .trim_end_matches('\n')
            .trim_end_matches('\r');
        if reported.is_empty() {
            return Err(GitListError::NoGitDir {
                command: self.label(Operation::RevParse),
            });
        }

        Ok(Utf8PathBuf::from(reported))
    }

    /// How `operation` is named in a diagnostic.
    ///
    /// The program is spelled as the caller supplied it, so a test that drives
    /// the failure paths with another program reads as that program's failure
    /// rather than as `git`'s.
    fn label(&self, operation: Operation) -> String {
        format!(
            "{} {}",
            self.program.to_string_lossy(),
            operation.subcommand()
        )
    }

    /// Runs `git subcommand` with `args` in `dir`, tracing the invocation and
    /// mapping every failure.
    ///
    /// The span is named for the operation and carries the outcome and the
    /// elapsed time once the process has been reaped. The event beside it says
    /// the same, because a span is not a line: tracing-test does not rebuild a
    /// span's fields into an event, so a test asserting what happened reads the
    /// event, and a host drawing a timeline reads the span.
    #[tracing::instrument(
        level = "debug",
        name = "git",
        skip(self, args, dir),
        fields(
            operation = operation.name(),
            outcome = field::Empty,
            elapsed_seconds = field::Empty
        )
    )]
    fn run(
        &self,
        operation: Operation,
        args: &[&str],
        dir: &Utf8Path,
    ) -> Result<std::process::Output, GitListError> {
        let started = Instant::now();
        let result = self.invoke(operation, args, dir);
        let elapsed_seconds = started.elapsed().as_secs_f64();

        let span = Span::current();
        span.record("outcome", outcome_label(&result));
        span.record("elapsed_seconds", elapsed_seconds);
        match &result {
            Ok(_) => debug!(
                operation = operation.name(),
                outcome = outcome_label(&result),
                elapsed_seconds,
                "git invocation completed"
            ),
            Err(error) => debug!(
                operation = operation.name(),
                outcome = outcome_label(&result),
                elapsed_seconds,
                failure = error.category(),
                "git invocation failed"
            ),
        }

        result
    }

    /// Runs `git subcommand` with `args` in `dir`, mapping every failure.
    ///
    /// `output()`, not `spawn()` plus a hand-rolled read: `output()` drains
    /// both pipes concurrently and cannot deadlock, where a "streaming"
    /// variant does once Git's output exceeds the pipe capacity.
    fn invoke(
        &self,
        operation: Operation,
        args: &[&str],
        dir: &Utf8Path,
    ) -> Result<std::process::Output, GitListError> {
        let output = Command::new(&self.program)
            .args(args)
            .current_dir(dir)
            .output()
            .map_err(|source| {
                let program = self.program.to_string_lossy().into_owned();
                if source.kind() == io::ErrorKind::NotFound {
                    GitListError::ProgramNotFound { program }
                } else {
                    GitListError::Spawn {
                        command: self.label(operation),
                        source,
                    }
                }
            })?;

        if !output.status.success() {
            return Err(GitListError::Failed {
                command: self.label(operation),
                status: output.status,
                // Scrubbed here, at the one boundary where another program's
                // bytes enter this tool's output.
                stderr: relayable(&output.stderr),
            });
        }

        Ok(output)
    }
}

/// How an invocation's result is labelled, success or failure.
///
/// Two values, and no third: a run that could not classify its outcome would
/// make the field grow with the errors it might carry.
fn outcome_label<T, E>(result: &Result<T, E>) -> &'static str {
    if result.is_ok() { "success" } else { "error" }
}

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
#[path = "git_ls_files_git_tests.rs"]
mod git_tests;
