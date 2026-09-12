//! The working-tree adapter for [`PathProbe`](crate::select::policy::PathProbe).
//!
//! Depends on `std::fs` and `camino`. It is the only place in the selection
//! tree that inspects the filesystem, and it reads metadata alone: selection
//! never opens a file, so a path this module reports as a regular file is not
//! thereby readable, and the writer still decides what to do about that.
//!
//! Absence is a classification, because the selection has a rule for a
//! candidate that is gone. Every other failure to read one is returned as a
//! [`ProbeError`]: a permission failure or a path through a file is not a
//! verdict about the file, and a run that could not classify a candidate must
//! say so rather than format the ones it could. See
//! [`select_files`](crate::select::policy::select_files).
//!
//! Which of the two a failure is cannot be read off the failure alone: Windows
//! reports a path through a file with the same `NOT_FOUND` as a path that is
//! gone, so the question is put to the path's ancestors rather than to the leaf.
//! [`unreachable`] is that walk, and [`absent`] is where the answer is given.
//!
//! The reads are the ambient filesystem's rather than a capability's, and
//! deliberately: confinement is what this module decides, and a capability
//! rooted at the working directory reports a candidate that escapes and one
//! that cannot be read as the same `PermissionDenied`, which would leave
//! [`PathKind::OutsideRoot`] unstatable. ADR 0010 states the rule; the Git
//! directory the conflict guard opens is its one exception.
//!
//! Classification uses `symlink_metadata`, not `metadata`, because a link's
//! extension says nothing about its target and rewriting through one escapes
//! the selection. For the same reason the canonical path is confined to the
//! tree: `symlink_metadata` does not follow a candidate's *final* component,
//! but it does see through a symlinked ancestor, so `docs/guide.md` is reported
//! as a regular file even when `docs` is a link to a directory outside the
//! working tree. See [`PathKind::OutsideRoot`].

use std::io::{self, ErrorKind};

use camino::{Utf8Path, Utf8PathBuf};

use super::policy::{FileIdentity, PathKind, PathProbe, ProbeError};

/// Probes the real working tree using `std::fs::symlink_metadata`.
///
/// `symlink_metadata`, not `metadata`: see [`PathKind::Symlink`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AmbientPathProbe;

impl PathProbe for AmbientPathProbe {
    fn probe(&self, root: &Utf8Path, path: &Utf8Path) -> Result<PathKind, ProbeError> {
        // `join` replaces rather than appends when `path` is already absolute,
        // so a candidate a caller resolved itself is asked about where it
        // actually points.
        let absolute = root.join(path);
        let metadata = match std::fs::symlink_metadata(&absolute) {
            Ok(metadata) => metadata,
            // Absence is the one failure the selection has a rule for — a
            // staged deletion, or a file removed since Git listed it. Every
            // other kind leaves the question unasked rather than answered, and
            // [`unreadable`] is what tells the two apart.
            Err(source) => return unreadable(absolute, source),
        };
        if metadata.is_symlink() {
            return Ok(PathKind::Symlink);
        }
        if !metadata.is_file() {
            return Ok(PathKind::Other);
        }

        // An identity is only useful if it is the one true name, so a canonical
        // path that cannot be spelled — or that fails to resolve — is not
        // reported as a regular file.
        match std::fs::canonicalize(&absolute) {
            Ok(canonical) => match Utf8PathBuf::from_path_buf(canonical) {
                Ok(canonical) => identify(root, canonical),
                Err(_) => Ok(PathKind::Other),
            },
            Err(source) => unreadable(absolute, source),
        }
    }
}

/// Names a canonical path, unless it lies outside the tree `root` names.
///
/// The last decision the probe makes, and the one that keeps a selection from
/// reaching through a symlinked ancestor: a candidate is what its real path
/// says it is, and a real path that leaves `root` is not a file this selection
/// may name.
fn identify(root: &Utf8Path, canonical: Utf8PathBuf) -> Result<PathKind, ProbeError> {
    if confined_to(root, &canonical)? {
        Ok(PathKind::RegularFile(FileIdentity::from_canonical_path(
            canonical,
        )))
    } else {
        Ok(PathKind::OutsideRoot)
    }
}

/// Whether `canonical` lies within the tree `root` names.
///
/// Both sides are canonicalized. A root reached through a link — `/tmp` on a
/// system where it is one — would otherwise disagree with every path beneath
/// it, and refuse the whole tree. A root that does not exist confines nothing:
/// containment cannot be established, and assuming it held is exactly the escape
/// this rule exists to stop.
///
/// # Errors
///
/// Returns a [`ProbeError`] if the root exists but cannot be read. A root that
/// cannot be resolved leaves every candidate's classification unwarranted, and
/// the selection does not report a confinement it could not establish.
fn confined_to(root: &Utf8Path, canonical: &Utf8Path) -> Result<bool, ProbeError> {
    let root = match std::fs::canonicalize(root) {
        Ok(root) => root,
        // A root that is not there confines nothing, and a root that cannot be
        // reached is a question the selection reports rather than answers.
        // Which of the two this is the probe's own classifier decides, so that
        // a root and a candidate are held to the same rule.
        Err(source) => {
            absent(root, source).map_err(|source| ProbeError {
                path: root.to_owned(),
                source,
            })?;
            return Ok(false);
        }
    };

    // A root that cannot be spelled confines nothing, for the same reason a
    // candidate that cannot be spelled is not a regular file.
    Ok(Utf8PathBuf::from_path_buf(root).is_ok_and(|root| canonical.starts_with(root)))
}

/// The classification of a candidate whose read failed.
///
/// A `symlink_metadata` that failed outright and a `canonicalize` that failed
/// after the metadata was read are the same question — is the file gone, or
/// could it not be read? — and they must answer it the same way, because the
/// difference is a candidate silently skipped or a run told its selection was
/// incomplete. [`absent`] is where the answer is given; this is only the shape
/// the probe wants it in.
///
/// The reason this is a function of the failure rather than a pair of match arms
/// in the probe above: a path `symlink_metadata` has already accepted can reach
/// the second arm only by losing a race with the filesystem, so no fixture
/// stages it there. Here, every arm is a test's to cover.
fn unreadable(path: Utf8PathBuf, error: io::Error) -> Result<PathKind, ProbeError> {
    match absent(&path, error) {
        Ok(()) => Ok(PathKind::Missing),
        Err(source) => Err(ProbeError { path, source }),
    }
}

/// Whether a failed read of `path` is the absence the selection has a rule for.
///
/// `Ok(())` is the absence a candidate staged for deletion is skipped under, and
/// it is the one cause the caller can act on: a file removed since Git listed
/// it, or one removed between the metadata read in [`PathProbe::probe`] and the
/// canonicalization that follows. `Err` is the failure to report instead.
///
/// [`ErrorKind::NotFound`] is not by itself evidence of absence, and the
/// decision is therefore about the whole path rather than the leaf that failed;
/// see [`unreachable`]. Every other kind is a path that is present but
/// unreadable, and is reported as it arrived.
///
/// # Errors
///
/// Returns `error` unchanged when its kind is not [`ErrorKind::NotFound`], and
/// what [`unreachable`] found when it is.
fn absent(path: &Utf8Path, error: io::Error) -> Result<(), io::Error> {
    if error.kind() != ErrorKind::NotFound {
        return Err(error);
    }
    match unreachable(path) {
        Some(source) => Err(source),
        None => Ok(()),
    }
}

/// The nearest ancestor of `path` that exists, when it is not a directory.
///
/// A `NotFound` does not say which part of a path is not there. On Windows a
/// path beneath a regular file fails with `NOT_FOUND` — `ERROR_PATH_NOT_FOUND`
/// — exactly as one that is not there at all does, where Unix reports
/// `ENOTDIR`; a leaf's own failure cannot tell the two apart, and reading the
/// first as the second silently skips a candidate the run was asked to
/// consider. The nearest ancestor that exists decides, and a directory reached
/// through a link is a directory, which is why this walk follows links where
/// the probe itself does not.
///
/// `None` is the absence the selection has a rule for: no part of the path is
/// there. `Some(error)` is the failure to report instead — a path that stops at
/// something other than a directory is [`ErrorKind::NotADirectory`], the kind
/// Unix produces for it, and a walk that fails for any other reason reports
/// whatever stopped it.
fn unreachable(path: &Utf8Path) -> Option<io::Error> {
    nearest_existing(path.parent(), read_ancestor)
}

/// What the walk needs to know about one ancestor.
///
/// The walk is stated over this rather than over the metadata itself because a
/// `Metadata` cannot be fabricated, and the reading it exists for — an ancestor
/// whose own read failed with something other than absence — is one no fixture
/// stages: a candidate reaches the walk only through a `NotFound` on its own
/// leaf, by which time the filesystem has answered for every ancestor above it.
#[derive(Debug)]
enum Reading {
    /// The ancestor is a directory, so the path is reachable through it.
    Directory,
    /// The ancestor is there and is not a directory.
    NotADirectory,
    /// The ancestor could not be read.
    Failed(io::Error),
}

/// Reads one ancestor, the way the walk sees it.
fn read_ancestor(ancestor: &Utf8Path) -> Reading {
    match std::fs::metadata(ancestor) {
        Ok(metadata) if metadata.is_dir() => Reading::Directory,
        Ok(_) => Reading::NotADirectory,
        Err(error) => Reading::Failed(error),
    }
}

/// The walk, over whatever reads an ancestor.
///
/// The reader is a parameter so that the arm no fixture reaches — an ancestor
/// whose read failed with something other than absence — is a test's to cover,
/// which is also what stops the guard from being replaced by one that walks
/// past it and reports a candidate as absent.
fn nearest_existing(
    mut ancestor: Option<&Utf8Path>,
    mut read: impl FnMut(&Utf8Path) -> Reading,
) -> Option<io::Error> {
    while let Some(current) = ancestor {
        match read(current) {
            Reading::Directory => return None,
            Reading::NotADirectory => {
                return Some(io::Error::new(
                    ErrorKind::NotADirectory,
                    format!("{current} is not a directory"),
                ));
            }
            // An absence is the one reading the walk climbs past: a whole
            // subtree may be missing, and what is missing is what the walk is
            // looking for. Anything else is a question that went unasked, and
            // is reported as it arrived.
            Reading::Failed(error) if error.kind() == ErrorKind::NotFound => {
                ancestor = current.parent();
            }
            Reading::Failed(error) => return Some(error),
        }
    }
    None
}

#[cfg(test)]
#[path = "fs_probe_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "fs_probe_failure_tests.rs"]
mod failure_tests;
