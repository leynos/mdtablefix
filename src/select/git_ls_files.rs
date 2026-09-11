//! The candidate source: the `git` invocations `--git` is built on.
//!
//! Depends on `std::process` and `camino`. It is the only place in the tree
//! that spawns `git`, and it deliberately knows nothing about extensions or
//! identity: it reports what the index holds and where the Git directory is,
//! and [`crate::select::policy`] decides what that means.
//!
//! `git`'s own diagnostics are relayed, never asserted on: they are localised
//! and version-dependent. Every message this module shows a user is a `Display`
//! impl this repository owns, and the relayed text is one line of it, scrubbed
//! by [`relayable`].

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
        let output = self.run(SUBCOMMAND, &args, dir)?;

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
        let output = self.run(REV_PARSE, &[REV_PARSE, ABSOLUTE_GIT_DIR], dir)?;

        // Only the line terminator is trimmed, never path characters: a
        // directory name may legitimately end in a space, and trimming that
        // would send the guard looking somewhere else entirely.
        let reported = std::str::from_utf8(&output.stdout)
            .map_err(|_| GitListError::NoGitDir {
                command: self.label(REV_PARSE),
            })?
            .trim_end_matches('\n')
            .trim_end_matches('\r');
        if reported.is_empty() {
            return Err(GitListError::NoGitDir {
                command: self.label(REV_PARSE),
            });
        }

        Ok(Utf8PathBuf::from(reported))
    }

    /// How `subcommand` is named in a diagnostic.
    ///
    /// The program is spelled as the caller supplied it, so a test that drives
    /// the failure paths with another program reads as that program's failure
    /// rather than as `git`'s.
    fn label(&self, subcommand: &str) -> String {
        format!("{} {subcommand}", self.program.to_string_lossy())
    }

    /// Runs `git subcommand` with `args` in `dir`, mapping every failure.
    ///
    /// `output()`, not `spawn()` plus a hand-rolled read: `output()` drains
    /// both pipes concurrently and cannot deadlock, where a "streaming"
    /// variant does once Git's output exceeds the pipe capacity.
    fn run(
        &self,
        subcommand: &str,
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
                        command: self.label(subcommand),
                        source,
                    }
                }
            })?;

        if !output.status.success() {
            return Err(GitListError::Failed {
                command: self.label(subcommand),
                status: output.status,
                // Scrubbed here, at the one boundary where another program's
                // bytes enter this tool's output.
                stderr: relayable(&output.stderr),
            });
        }

        Ok(output)
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
        /// Git's own diagnostic, relayed through [`relayable`].
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
}

/// The longest run of Git's own diagnostic that is relayed before it is cut.
///
/// A cap rather than a promise: a repository name is not this tool's to trust,
/// and a diagnostic that arrives in kilobytes would bury the message it is
/// supposed to support.
const RELAYED_LIMIT: usize = 1024;

/// Renders `bytes` as one line that is safe to write to a terminal.
///
/// Git's text is the one part of this tool's output that this repository does
/// not author, so it is scrubbed on the way through: a newline would forge a
/// second line of stderr, and a control character — an escape, say — would let
/// a path in the repository drive the terminal reading the diagnostic. Each run
/// of them becomes a single space, so a message written as several lines stays
/// readable as one, and a run at either end disappears rather than becoming a
/// gap. Bytes that are not UTF-8 become the replacement character rather than
/// being dropped, because the text is a diagnostic, not a path: nothing acts
/// on it.
fn relayable(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut scrubbed = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.chars() {
        if character.is_control() {
            // Not `scrubbed.is_empty()`: a control character before any text
            // marks no space, or the message would begin with one.
            pending_space = !scrubbed.is_empty();
        } else {
            if pending_space {
                scrubbed.push(' ');
                pending_space = false;
            }
            scrubbed.push(character);
        }
    }

    if scrubbed.chars().count() > RELAYED_LIMIT {
        let cut = scrubbed
            .char_indices()
            .nth(RELAYED_LIMIT)
            .map_or(scrubbed.len(), |(index, _)| index);
        scrubbed.truncate(cut);
        scrubbed.push('…');
    }

    scrubbed
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
