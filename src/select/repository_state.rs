//! Git-directory adapter for the conflict policy.
//!
//! [`super::conflict`] owns the pure refusal rule. This module owns the only
//! filesystem access needed to obtain its [`RepositoryOperationState`], at the
//! instant a file is about to be replaced.

use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, Metadata},
};

use super::conflict::RepositoryOperationState;

/// The entries a Git directory holds only while an operation is paused.
const IN_PROGRESS: [&str; 5] = [
    "MERGE_HEAD",
    "rebase-merge",
    "rebase-apply",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
];

/// Holds the Git directory a writing run must consult.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictGuard {
    /// Nothing to consult for a non-writing run or `--allow-conflicted`.
    Unguarded,
    /// The Git directory to consult immediately before each rewrite.
    Guarded(Utf8PathBuf),
}

impl ConflictGuard {
    /// The guard for a run with no repository to consult.
    #[must_use]
    pub const fn unguarded() -> Self { Self::Unguarded }

    /// The guard for a run that must ask this Git directory before it writes.
    #[must_use]
    pub fn guarded(git_dir: impl Into<Utf8PathBuf>) -> Self { Self::Guarded(git_dir.into()) }

    /// Reads the state the pure conflict policy must consider.
    ///
    /// The caller first classifies the document. It calls this only for a
    /// document carrying all three marker forms, so ordinary rewrites never
    /// open the Git directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the guarded Git directory cannot be read.
    pub fn operation_state(&self) -> Result<RepositoryOperationState, RepositoryStateError> {
        let Self::Guarded(git_dir) = self else {
            return Ok(RepositoryOperationState::Idle);
        };

        operation_in_progress(git_dir)
    }
}

/// Reports whether the repository is mid-operation by checking Git's fixed
/// marker entries through an opened directory capability.
///
/// # Errors
///
/// Returns an error if the Git directory or a marker cannot be inspected.
pub fn operation_in_progress(
    git_dir: &Utf8Path,
) -> Result<RepositoryOperationState, RepositoryStateError> {
    let directory = Dir::open_ambient_dir(git_dir, ambient_authority()).map_err(|source| {
        RepositoryStateError {
            git_dir: git_dir.to_owned(),
            source,
        }
    })?;
    let opened = directory
        .dir_metadata()
        .map_err(|source| RepositoryStateError {
            git_dir: git_dir.to_owned(),
            source,
        })?;
    opened_directory(git_dir, &opened)?;

    for name in IN_PROGRESS {
        let present =
            marker_present(directory.symlink_metadata(name).map(|_| ())).map_err(|source| {
                RepositoryStateError {
                    git_dir: git_dir.to_owned(),
                    source,
                }
            })?;
        if present {
            return Ok(RepositoryOperationState::InProgress);
        }
    }

    Ok(RepositoryOperationState::Idle)
}

/// Rejects a capability that opened something other than a directory.
pub(super) fn opened_directory(
    git_dir: &Utf8Path,
    metadata: &Metadata,
) -> Result<(), RepositoryStateError> {
    if metadata.is_dir() {
        return Ok(());
    }

    Err(RepositoryStateError {
        git_dir: git_dir.to_owned(),
        source: io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("`{git_dir}` is not a directory"),
        ),
    })
}

/// Converts an entry probe into a presence answer or a real I/O failure.
pub(super) fn marker_present(test: io::Result<()>) -> Result<bool, io::Error> {
    match test {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// The state of a Git directory could not be read.
#[derive(Debug, thiserror::Error)]
#[error("reading the state of the Git directory `{git_dir}`")]
pub struct RepositoryStateError {
    /// The Git directory whose markers were being tested for.
    pub git_dir: Utf8PathBuf,
    /// Why the state could not be read.
    #[source]
    pub source: io::Error,
}
