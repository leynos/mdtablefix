//! The composition root for `--git`: it wires the selection tree together.
//!
//! Declared only from `src/main.rs`, so it adds no public API. It is the one
//! module that knows all of the selection tree at once — the candidate source,
//! the policy, the working-tree probe, and the conflict guard — which is why
//! it is a sibling of `select` rather than a part of it: `select` states what
//! is selected, and this module is what turns the command line into that
//! question and the answer back into paths.
//!
//! Nothing here holds a directory capability: the paths it returns are relative
//! to the working directory, and `main` opens each file's parent as it does for
//! a path the user typed, so `--git` reaches the same capability-scoped writer
//! as every other selection.

use camino::{Utf8Path, Utf8PathBuf};

use crate::{
    command::Cli,
    driver::{Inputs, Mode},
    select::{
        ConflictGuard,
        fs_probe::AmbientPathProbe,
        git_failure::GitListError,
        git_ls_files::GitLsFiles,
        policy::{CandidatePath, ProbeFailure, select_files},
    },
};

/// Why a `--git` run could not resolve its selection.
///
/// Two failures with one report: the listing the candidates come from, and the
/// reading of the candidates themselves. Both stop the run before any file is
/// analysed and both are operational failures rather than drift, so a caller
/// reports them the same way and exits through
/// [`exit_status`](crate::driver::exit_status) with the same status.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GitInputsError {
    /// The candidate listing could not be obtained.
    #[error(transparent)]
    Git(#[from] GitListError),
    /// A listed candidate could not be classified.
    #[error(transparent)]
    Probe(#[from] ProbeFailure),
}

impl GitInputsError {
    /// The one line this failure prints, with the underlying reason appended.
    ///
    /// The same division as [`GitListError::diagnostic`]: this tool's wording
    /// first, so a test may assert on text this repository owns, then the
    /// relayed reason — Git's own diagnostic for a failed command, the operating
    /// system's for a path that could not be read.
    #[must_use]
    pub fn diagnostic(&self) -> String {
        match self {
            Self::Git(error) => error.diagnostic(),
            Self::Probe(error) => error.to_string(),
        }
    }
}

/// What `--git` resolved to: the paths to act on, and the guard governing them.
pub struct GitSelection {
    /// The selected paths, relative to the working directory, sorted byte-wise.
    ///
    /// Not Git's order: [`select_files`] sorts, because `git ls-files` output is
    /// not globally sorted and the order is user-visible through both
    /// `--list-files` and the concatenated output of a print-mode run.
    pub inputs: Inputs,
    /// The guard a rewrite must consult before it writes anything.
    pub guard: ConflictGuard,
    /// How many listed candidates could not be represented as paths.
    ///
    /// Carried rather than printed here: resolving a selection is a query, and
    /// writing a warning is command output. [`GitSelection::skipped_warning`]
    /// renders it for the boundary that owns standard error.
    pub skipped_non_utf8: usize,
    /// Bounded counts describing how the candidate set was narrowed.
    ///
    /// The command boundary may log them without emitting path names or raw
    /// extension values, while this query remains reusable as data only.
    pub selection: SelectionStatistics,
}

/// Bounded facts about a completed repository selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionStatistics {
    /// How many UTF-8 candidates Git reported.
    pub candidates: usize,
    /// How many candidates satisfied the selection policy.
    pub selected: usize,
    /// How many configured extensions the policy considered.
    pub extension_count: usize,
}

impl GitSelection {
    /// The one warning a run with unrepresentable candidates owes the user, or
    /// `None` when every candidate was a valid UTF-8 path.
    ///
    /// Reported rather than dropped in silence: a file the user can see in the
    /// repository and cannot see in the selection is otherwise a mystery. The
    /// caller writes it to standard error rather than standard output, because
    /// standard output is the selection itself — under `--list-files` a reader
    /// is parsing it, and a warning there would be an entry that is not a path.
    ///
    /// The paths themselves are not named: they are not valid UTF-8, so writing
    /// one to a terminal is exactly the operation this tool declined to
    /// perform. Only the count is, and it is a structured field on the
    /// selection for as long as possible, so the wording stays testable without
    /// a process to print it.
    #[must_use]
    pub fn skipped_warning(&self) -> Option<String> {
        let skipped = self.skipped_non_utf8;
        (skipped > 0).then(|| {
            format!("mdtablefix: {skipped} file(s) not selected: their names are not valid UTF-8")
        })
    }
}

/// Resolves `--git` into the paths to act on, and the guard that governs them.
///
/// The result is always [`Inputs::Files`], and it may name nothing: an empty
/// selection is a success, and must not fall through to a standard input the
/// user never offered. See `AX-6`.
///
/// # Errors
///
/// Returns a [`GitInputsError`] if `git` cannot be run, if it fails, if the Git
/// directory cannot be resolved for a run that needs the guard, or if a listed
/// candidate cannot be classified — a permission failure or a path through a
/// file, rather than the absence the selection has a rule for. A candidate that
/// cannot be read is reported rather than skipped: the run would otherwise
/// format a set it cannot describe.
pub fn resolve(
    cli: &Cli,
    mode: Mode,
    working_directory: &Utf8Path,
) -> Result<GitSelection, GitInputsError> {
    let extensions = cli.extensions();
    let listing = GitLsFiles::new(cli.includes_untracked()).list_candidates(working_directory)?;

    let candidates: Vec<CandidatePath> = listing
        .paths
        .iter()
        .map(|path| CandidatePath::new(path.to_string()))
        .collect();
    let selected = select_files(
        &candidates,
        &extensions,
        &AmbientPathProbe::new(working_directory),
    )?;
    let selection = SelectionStatistics {
        candidates: listing.paths.len(),
        selected: selected.len(),
        extension_count: extensions.iter().count(),
    };
    Ok(GitSelection {
        inputs: Inputs::Files(
            selected
                .into_iter()
                .map(|candidate| Utf8PathBuf::from(candidate.as_str()))
                .collect(),
        ),
        guard: guard(cli, mode, working_directory)?,
        skipped_non_utf8: listing.skipped_non_utf8,
        selection,
    })
}

/// The guard a `--git` run must consult before it rewrites anything.
///
/// The unguarded one unless this run can write and the user has not overridden
/// the refusal, so the Git directory is resolved only when the answer can
/// change what happens: `--check`, `--diff`, and `--list-files` never pay for
/// the second `git` process, and neither does an `--in-place` run the user has
/// told to rewrite a conflicted file anyway.
///
/// What the guard receives is the directory, not a verdict read from it: the
/// repository is asked again immediately before each file is replaced, so a
/// merge or revert that begins mid-run is seen. See
/// [`refuses`](crate::select::conflict::refuses).
///
/// # Errors
///
/// Fails rather than returning the unguarded one when the Git directory cannot
/// be resolved: a rewrite whose safety cannot be checked is not a rewrite this
/// tool performs unguarded. The failure is reported like any other operational
/// failure, per [`crate::driver::exit_status`].
fn guard(
    cli: &Cli,
    mode: Mode,
    working_directory: &Utf8Path,
) -> Result<ConflictGuard, GitListError> {
    if mode != Mode::InPlace || cli.allows_conflicted() {
        return Ok(ConflictGuard::unguarded());
    }

    let git_dir = GitLsFiles::new(cli.includes_untracked()).resolve_git_dir(working_directory)?;

    Ok(ConflictGuard::guarded(git_dir))
}

/// A working directory as a UTF-8 path.
///
/// The conversion `Inputs::resolve` performs for positional arguments, applied
/// once here instead, so a run whose working directory cannot be represented
/// fails as a whole rather than per file.
///
/// # Errors
///
/// Returns an error if the working directory cannot be read or is not valid
/// UTF-8.
pub fn working_directory() -> anyhow::Result<Utf8PathBuf> {
    let directory = std::env::current_dir()?;
    Utf8PathBuf::from_path_buf(directory).map_err(|directory| {
        anyhow::anyhow!(
            "converting the working directory {} to a UTF-8 path",
            directory.display()
        )
    })
}

#[cfg(test)]
#[path = "git_inputs_tests.rs"]
mod tests;
