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
use tracing::debug;

use crate::{
    command::Cli,
    driver::{Inputs, Mode},
    select::{
        conflict::{ConflictGuard, operation_in_progress},
        fs_probe::AmbientPathProbe,
        git_ls_files::{GitListError, GitLsFiles},
        policy::select_files,
    },
};

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
}

/// Resolves `--git` into the paths to act on, and the guard that governs them.
///
/// The result is always [`Inputs::Files`], and it may name nothing: an empty
/// selection is a success, and must not fall through to a standard input the
/// user never offered. See `AX-6`.
///
/// # Errors
///
/// Returns a [`GitListError`] if `git` cannot be run, if it fails, or if the
/// Git directory cannot be resolved for a run that needs the guard.
pub fn resolve(
    cli: &Cli,
    mode: Mode,
    working_directory: &Utf8Path,
) -> Result<GitSelection, GitListError> {
    let extensions = cli.extensions();
    let listing = GitLsFiles::new(cli.includes_untracked()).list_candidates(working_directory)?;
    report_skipped(listing.skipped_non_utf8);

    let selected = select_files(
        &listing.paths,
        working_directory,
        &extensions,
        &AmbientPathProbe,
    );
    debug!(
        candidates = listing.paths.len(),
        selected = selected.len(),
        extensions = %extensions,
        "selected files from the repository"
    );

    Ok(GitSelection {
        inputs: Inputs::Files(selected),
        guard: guard(cli, mode, working_directory)?,
    })
}

/// The guard a `--git` run must consult before it rewrites anything.
///
/// The inert guard unless this run can write and the user has not overridden
/// the refusal, so the Git directory is asked for only when the answer can
/// change what happens: `--check`, `--diff`, and `--list-files` never pay for
/// the second `git` process.
///
/// # Errors
///
/// Fails rather than returning the inert guard when the Git directory cannot be
/// resolved: a rewrite whose safety cannot be checked is not a rewrite this
/// tool performs unguarded. The failure is reported like any other operational
/// failure, per [`crate::driver::exit_status`].
fn guard(
    cli: &Cli,
    mode: Mode,
    working_directory: &Utf8Path,
) -> Result<ConflictGuard, GitListError> {
    let overridden = cli.allows_conflicted();
    if mode != Mode::InPlace || overridden {
        return Ok(ConflictGuard::unguarded());
    }

    let git_dir = GitLsFiles::new(cli.includes_untracked()).resolve_git_dir(working_directory)?;
    let in_progress = operation_in_progress(&git_dir);

    Ok(ConflictGuard::new(in_progress, overridden))
}

/// Reports, once, that some candidates could not be represented as paths.
///
/// Reported rather than dropped in silence: a file the user can see in the
/// repository and cannot see in the selection is otherwise a mystery. It goes
/// to standard error because standard output is the selection itself — under
/// `--list-files` a reader is parsing it, and a warning there would be an entry
/// that is not a path.
///
/// The paths themselves are not printed: they are not valid UTF-8, so writing
/// one to a terminal is exactly the operation this tool declined to perform.
fn report_skipped(skipped: usize) {
    if skipped > 0 {
        eprintln!("mdtablefix: {skipped} file(s) not selected: their names are not valid UTF-8");
    }
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
