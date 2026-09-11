//! The candidate source: one `git ls-files` invocation.
//!
//! Depends on `std::process` and `camino`. It is the only place in the tree
//! that spawns `git`, and it deliberately knows nothing about extensions or
//! identity: it reports what the index holds, and [`crate::select::policy`]
//! decides what that means.
//!
//! `git`'s own diagnostics are relayed, never asserted on: they are localised
//! and version-dependent. Every message this module shows a user is a `Display`
//! impl this repository owns.

use std::{
    ffi::OsString,
    io,
    process::{Command, ExitStatus},
};

use camino::{Utf8Path, Utf8PathBuf};

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

/// What `--include-untracked` adds: the files Git would commit, and nothing
/// it is told to ignore.
const UNTRACKED: [&str; 2] = ["--others", "--exclude-standard"];

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
        let mut command = Command::new(&self.program);
        command.arg(SUBCOMMAND).args(FRAMING);
        if self.include_untracked {
            command.args(UNTRACKED);
        }

        // `output()`, not `spawn()` plus a hand-rolled read: `output()` drains
        // both pipes concurrently and cannot deadlock, where a "streaming"
        // variant does once Git's output exceeds the pipe capacity.
        let output = command.current_dir(dir).output().map_err(|source| {
            let program = self.program.to_string_lossy().into_owned();
            if source.kind() == io::ErrorKind::NotFound {
                GitListError::ProgramNotFound { program }
            } else {
                GitListError::Spawn { program, source }
            }
        })?;

        if !output.status.success() {
            return Err(GitListError::Failed {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        Ok(split_nul_delimited(&output.stdout))
    }
}

/// Candidate paths, with a count of paths that were not valid UTF-8.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CandidateListing {
    pub paths: Vec<Utf8PathBuf>,
    pub skipped_non_utf8: usize,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GitListError {
    #[error("`{program}` is not installed or not on PATH")]
    ProgramNotFound { program: String },
    #[error("running `{program} ls-files`")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("`git ls-files` failed with exit status {status}")]
    Failed {
        status: ExitStatus,
        /// Git's own diagnostic, relayed verbatim.
        stderr: String,
    },
}

/// Splits a NUL-terminated byte stream, counting entries that are not UTF-8.
///
/// An empty segment is skipped rather than reported as an empty path, so empty
/// input yields an empty listing. Git terminates even the last path, so the
/// only empty segments are the one after the final NUL and those in input this
/// tool was handed rather than input Git wrote; neither names a file.
///
/// A path that is not UTF-8 is counted, not dropped silently: it cannot be
/// reported as a [`Utf8PathBuf`], and a caller that acts on the listing must be
/// able to say how many files it could not consider.
pub(crate) fn split_nul_delimited(bytes: &[u8]) -> CandidateListing {
    let mut listing = CandidateListing::default();
    for segment in bytes.split(|byte| *byte == 0) {
        if segment.is_empty() {
            continue;
        }
        match std::str::from_utf8(segment) {
            Ok(path) => listing.paths.push(Utf8PathBuf::from(path)),
            Err(_) => listing.skipped_non_utf8 += 1,
        }
    }

    listing
}

#[cfg(test)]
#[path = "git_ls_files_tests.rs"]
mod tests;
