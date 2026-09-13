//! The candidate source: the `git` invocations `--git` is built on.
//!
//! Depends on `std::process` and `camino`. It is the only place in the tree
//! that spawns `git`, and it deliberately knows nothing about extensions or
//! identity: it reports what the index holds and where the Git directory is,
//! and [`crate::select::policy`] decides what that means. What comes back is
//! parsed and made safe in [`super::git_output`].
//!
//! `git`'s own diagnostics are relayed, never asserted on: they are localised
//! and version-dependent. Every failure here is a [`GitListError`], whose
//! wording is a `Display` impl this repository owns and whose relayed text is
//! one line of it, scrubbed by [`relayable`](super::git_output::relayable).
//! The type itself lives in [`super::git_failure`], beside the reader it is
//! written for.
//!
//! This adapter returns data only. The command boundary decides whether and how
//! to report a selection, so using this query from another delivery mechanism
//! never emits an event merely because it examined a repository.

use std::{ffi::OsString, io, process::Command};

use camino::{Utf8Path, Utf8PathBuf};

use super::{
    git_failure::GitListError,
    git_output::{CandidateListing, relayable, split_nul_delimited},
};

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

/// The environment variables that would point an invocation at another
/// repository.
///
/// `--git` is scoped to the directory the process runs in: the listing is
/// relative to it, and the index it is read from is the one governing it. Each
/// of these redirects one of those answers — the Git directory, the working
/// tree the index is compared against, the index file itself, and the common
/// directory the repository's shared state lives in. An ambient value is not
/// hypothetical: a hook, or a script that wraps another `git` command, leaves
/// one behind, and the result would be a listing of a different repository's
/// files with the paths still interpreted relative to this directory — a
/// silent wrong answer, and under `--in-place` a destructive one. Every
/// invocation therefore removes them.
///
/// Deliberately not the whole environment. Discovery-only variables such as
/// [`GIT_CEILING_DIRECTORIES`] and `GIT_DISCOVERY_ACROSS_FILESYSTEM` can only
/// stop the search for a repository, which fails honestly with Git's own
/// diagnostic rather than answering about a tree this run was not given; and
/// stripping the user's configuration would be a different decision from
/// stripping their redirection.
///
/// [`GIT_CEILING_DIRECTORIES`]: https://git-scm.com/docs/git#Documentation/git.txt-codeGITCEILINGDIRECTORIEScode
const REDIRECTING: [&str; 4] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
];

/// One `git` invocation this module makes.
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

        let reported = std::str::from_utf8(&output.stdout).map_err(|_| GitListError::NoGitDir {
            command: self.label(Operation::RevParse),
        })?;
        let reported = without_line_terminator(reported);
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

    /// Runs `git subcommand` with `args` in `dir`, mapping every failure.
    fn run(
        &self,
        operation: Operation,
        args: &[&str],
        dir: &Utf8Path,
    ) -> Result<std::process::Output, GitListError> {
        self.invoke(operation, args, dir)
    }

    /// The command one invocation runs: `args`, in `dir`, and without the
    /// variables that would redirect it at another repository.
    ///
    /// Separate from [`invoke`](Self::invoke) so that what the subprocess is
    /// given can be asserted without a process to run: the removals are
    /// visible through [`Command::get_envs`], which is what the test reads.
    fn command(&self, args: &[&str], dir: &Utf8Path) -> Command {
        let mut command = Command::new(&self.program);
        command.args(args).current_dir(dir);
        for variable in REDIRECTING {
            command.env_remove(variable);
        }

        command
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
        let output = self.command(args, dir).output().map_err(|source| {
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

/// What `rev-parse` reported, without the one line terminator it wrote after
/// the path.
///
/// Exactly one, and never path characters: a directory name may legitimately
/// end in a space or a newline, and the terminator is the only thing Git added
/// to whatever that name is, so anything further back belongs to the name. A
/// terminator stripped twice would send the conflict guard to `repo` for a
/// directory called `repo\n`, where it would find no marker and answer that no
/// operation is in progress.
///
/// Every trailing character is deliberately not the rule here, and neither is
/// a bare `\r`: `trim_end_matches` removes every occurrence, which is the same
/// mistake at a different scale. The text is a parameter rather than an
/// expression in the caller so that a test can hand it both terminator forms,
/// because only one of them is reachable through a Git built for this
/// platform.
fn without_line_terminator(reported: &str) -> &str {
    match reported.strip_suffix('\n') {
        Some(line) => line.strip_suffix('\r').unwrap_or(line),
        None => reported,
    }
}

#[cfg(test)]
#[path = "git_ls_files_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "git_ls_files_git_tests.rs"]
mod git_tests;
