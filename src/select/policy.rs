//! What `--git` selects, stated as a rule rather than as a procedure.
//!
//! Depends on the [`PathProbe`] port, [`ExtensionFilter`], and `camino` alone.
//! Nothing here imports `std::fs`, `std::process`, or `cap_std`: the rule is
//! testable against a fake probe, and no test needs to change directory. The
//! adapters that answer the port live in [`crate::select::fs_probe`].

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};

use super::extensions::ExtensionFilter;

/// Identifies a file independently of the path used to reach it.
///
/// The canonicalized path, not `(st_dev, st_ino)`. Replacement writes a new
/// inode over the target, so an identity keyed on the inode would collapse two
/// hard links, format one, and leave the other stale. See INV-DEDUP.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileIdentity(Utf8PathBuf);

impl FileIdentity {
    /// Wraps a canonicalized path, as the probe obtains it.
    #[must_use]
    pub fn from_canonical_path(path: Utf8PathBuf) -> Self { Self(path) }

    /// The canonicalized path this identity was built from.
    #[must_use]
    pub fn as_path(&self) -> &Utf8Path { &self.0 }
}

/// What a candidate path turned out to be in the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathKind {
    /// A regular file that may be read and rewritten.
    RegularFile(FileIdentity),
    /// Absent from the working tree, for example a staged deletion.
    Missing,
    /// A symbolic link. Never followed: the link's extension says nothing
    /// about the target's type, and writing through it escapes the selection.
    Symlink,
    /// Present but neither a regular file nor a symlink, for example a
    /// submodule gitlink.
    Other,
}

/// Reports what a candidate path actually is. Implemented by adapters.
pub trait PathProbe {
    fn probe(&self, root: &Utf8Path, path: &Utf8Path) -> PathKind;
}

/// Narrows candidates to the sorted, alias-free set of files that exist as
/// regular files and carry a configured extension.
#[must_use]
pub fn select_files<P>(
    candidates: &[Utf8PathBuf],
    root: &Utf8Path,
    extensions: &ExtensionFilter,
    probe: &P,
) -> Vec<Utf8PathBuf>
where
    P: PathProbe + ?Sized,
{
    // The extension comes first so that the filesystem is touched once per
    // distinct Markdown candidate and never for a `.rs` file: on the
    // repository this was measured against, that is 28 probes rather than 416.
    let mut matching: Vec<&Utf8Path> = candidates
        .iter()
        .map(Utf8PathBuf::as_path)
        .filter(|path| extensions.matches(path))
        .collect();

    // Sorted and deduplicated before probing, so a path reported once per merge
    // stage is probed once, and so the `or_insert` below retains the
    // lexicographically first spelling of an entry two names reach.
    matching.sort_unstable();
    matching.dedup();

    let mut by_identity: BTreeMap<FileIdentity, &Utf8Path> = BTreeMap::new();
    for path in matching {
        if let PathKind::RegularFile(identity) = probe.probe(root, path) {
            by_identity.entry(identity).or_insert(path);
        }
    }

    let mut selected: Vec<Utf8PathBuf> =
        by_identity.into_values().map(Utf8Path::to_owned).collect();
    selected.sort_unstable();

    selected
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
