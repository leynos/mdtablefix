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
            // other kind leaves the question unasked rather than answered.
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(PathKind::Missing),
            Err(source) => {
                return Err(ProbeError {
                    path: absolute,
                    source,
                });
            }
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
            Err(error) => unnameable(absolute, error),
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
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(ProbeError {
                path: root.to_owned(),
                source,
            });
        }
    };

    // A root that cannot be spelled confines nothing, for the same reason a
    // candidate that cannot be spelled is not a regular file.
    Ok(Utf8PathBuf::from_path_buf(root).is_ok_and(|root| canonical.starts_with(root)))
}

/// Classifies a failed canonicalization by the kind of failure.
///
/// Absence is reported as absence, because it is the one cause the caller can
/// act on: a candidate staged for deletion, or one removed between the metadata
/// read in [`PathProbe::probe`] and this call's own. Every other kind leaves the
/// file present but unnameable, which the run is told about rather than
/// silently skipping.
///
/// The reason this is a function of the error rather than a pair of match arms
/// in the probe above: a path `symlink_metadata` has already accepted can reach
/// the second arm only by losing a race with the filesystem, so no fixture can
/// stage it. Here, both arms are a test's to cover.
fn unnameable(path: Utf8PathBuf, error: io::Error) -> Result<PathKind, ProbeError> {
    if error.kind() == ErrorKind::NotFound {
        Ok(PathKind::Missing)
    } else {
        Err(ProbeError {
            path,
            source: error,
        })
    }
}

#[cfg(test)]
#[path = "fs_probe_tests.rs"]
mod tests;
