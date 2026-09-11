//! The working-tree adapter for [`PathProbe`](crate::select::policy::PathProbe).
//!
//! Depends on `std::fs` and `camino`. It is the only place in the selection
//! tree that inspects the filesystem, and it reads metadata alone: selection
//! never opens a file, so a path this module reports as a regular file is not
//! thereby readable, and the writer still decides what to do about that.
//!
//! Classification uses `symlink_metadata`, not `metadata`, because a link's
//! extension says nothing about its target and rewriting through one escapes
//! the selection.

use std::io::ErrorKind;

use camino::{Utf8Path, Utf8PathBuf};

use super::policy::{FileIdentity, PathKind, PathProbe};

/// Probes the real working tree using `std::fs::symlink_metadata`.
///
/// `symlink_metadata`, not `metadata`: see [`PathKind::Symlink`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AmbientPathProbe;

impl PathProbe for AmbientPathProbe {
    fn probe(&self, root: &Utf8Path, path: &Utf8Path) -> PathKind {
        // `join` replaces rather than appends when `path` is already absolute,
        // so a candidate a caller resolved itself is asked about where it
        // actually points.
        let absolute = root.join(path);
        let Ok(metadata) = std::fs::symlink_metadata(&absolute) else {
            return PathKind::Missing;
        };
        if metadata.is_symlink() {
            return PathKind::Symlink;
        }
        if !metadata.is_file() {
            return PathKind::Other;
        }

        // An identity is only useful if it is the one true name, so a path that
        // cannot be canonicalized is not reported as a regular file.
        match std::fs::canonicalize(&absolute) {
            Ok(canonical) => Utf8PathBuf::from_path_buf(canonical)
                .map_or(PathKind::Other, |canonical| {
                    PathKind::RegularFile(FileIdentity::from_canonical_path(canonical))
                }),
            Err(error) => unnameable(error.kind()),
        }
    }
}

/// Classifies a failed canonicalization by the kind of failure.
///
/// Absence is reported as absence, because it is the one cause the caller can
/// act on: a candidate staged for deletion, or one removed between the metadata
/// read in [`PathProbe::probe`] and this call's own. Every other kind leaves the
/// file present but unnameable, which is [`PathKind::Other`].
///
/// The reason this is a function of the error kind rather than a pair of match
/// arms in the probe above: a path `symlink_metadata` has already accepted can
/// reach the second arm only by losing a race with the filesystem, so no
/// fixture can stage it. Here, both arms are a test's to cover.
fn unnameable(kind: ErrorKind) -> PathKind {
    if kind == ErrorKind::NotFound {
        PathKind::Missing
    } else {
        PathKind::Other
    }
}

#[cfg(test)]
#[path = "fs_probe_tests.rs"]
mod tests;
