//! What `--git` selects, stated as a rule rather than as a procedure.
//!
//! Depends on the [`PathProbe`] port and [`ExtensionFilter`]. Paths and probe
//! failures are domain values here, so the rule does not know whether an
//! adapter represents them with `camino`, an operating-system handle, or
//! another transport. The working-tree adapter lives in
//! [`crate::select::fs_probe`].

use std::{collections::BTreeMap, fmt};

use super::extensions::ExtensionFilter;

/// A candidate name selected from Git's listing.
///
/// The spelling is deliberately opaque to the policy. It is ordered only to
/// make the externally visible selection deterministic; translating it into a
/// filesystem path belongs to an adapter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CandidatePath(String);

impl CandidatePath {
    /// Creates a candidate from the spelling supplied by an adapter.
    #[must_use]
    pub fn new(path: String) -> Self { Self(path) }

    /// Returns the candidate's stable, adapter-supplied spelling.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Display for CandidatePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Identifies a file independently of the candidate name used to reach it.
///
/// The adapter forms this from its canonical name, not from `(st_dev, st_ino)`.
/// Replacement writes a new inode over the target, so an inode identity would
/// collapse two hard links, format one, and leave the other stale. See
/// INV-DEDUP.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileIdentity(String);

impl FileIdentity {
    /// Creates an identity from an adapter's canonical name.
    #[must_use]
    pub fn from_canonical_name(name: String) -> Self { Self(name) }

    /// Returns the canonical spelling retained for this identity.
    #[cfg(test)]
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// What a candidate turned out to be in the adapter's source.
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
    /// Present but neither a regular file nor a symbolic link, for example a
    /// submodule gitlink.
    Other,
    /// A regular file whose canonical name lies outside the selected root.
    /// Never selected, because writing it would write outside the tree the
    /// selection was made in. See [`crate::select::fs_probe`].
    OutsideRoot,
}

/// The domain category for a candidate an adapter could not classify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProbeFailureKind {
    /// The adapter could not read enough state to classify the candidate.
    Unreadable,
}

/// A candidate whose class could not be established.
///
/// Distinct from [`PathKind::Missing`], which is an answer: a staged deletion
/// is absent, and absent is a class the selection has a rule for. This is the
/// absence of an answer — a permission failure, a path through a file, or a
/// symbolic-link loop — and a selection that met one cannot say what it
/// selected. See [`select_files`].
#[derive(Debug, Clone, thiserror::Error)]
#[error("reading `{path}` while selecting files: {detail}")]
pub struct ProbeFailure {
    /// The candidate the adapter could not classify.
    pub path: CandidatePath,
    /// The domain category of the failure.
    pub kind: ProbeFailureKind,
    /// The adapter's human-readable detail, for a command-boundary diagnostic.
    pub detail: String,
}

impl ProbeFailure {
    /// Records an unreadable candidate without exposing an adapter error type.
    #[must_use]
    pub fn unreadable(path: CandidatePath, detail: String) -> Self {
        Self {
            path,
            kind: ProbeFailureKind::Unreadable,
            detail,
        }
    }
}

/// Reports what a candidate path actually is. Implemented by adapters.
pub trait PathProbe {
    /// Classifies a candidate.
    ///
    /// # Errors
    ///
    /// Returns a [`ProbeFailure`] if the candidate cannot be read for a reason
    /// that is not its absence. Absence itself is [`PathKind::Missing`]: the
    /// caller has a rule for a candidate that is gone, and none for one it
    /// could not inspect.
    fn probe(&self, candidate: &CandidatePath) -> Result<PathKind, ProbeFailure>;
}

/// Narrows candidates to the sorted, alias-free set of regular files carrying
/// a configured extension.
///
/// # Errors
///
/// Returns the first [`ProbeFailure`] a candidate draws. The selection stops
/// there rather than returning what it had: a run that cannot classify one
/// candidate does not know what it is about to format, so the incomplete set
/// must not be presented as a selection. The caller reports it the way it
/// reports a failed listing, before any file is analysed.
pub fn select_files<P>(
    candidates: &[CandidatePath],
    extensions: &ExtensionFilter,
    probe: &P,
) -> Result<Vec<CandidatePath>, ProbeFailure>
where
    P: PathProbe + ?Sized,
{
    // The extension comes first so that the adapter is consulted once per
    // distinct Markdown candidate and never for a `.rs` file: on the
    // repository this was measured against, that is 28 probes rather than 416.
    let mut matching: Vec<&CandidatePath> = candidates
        .iter()
        .filter(|candidate| extensions.matches(candidate.as_str()))
        .collect();

    // Sorted and deduplicated before probing, so a path reported once per merge
    // stage is probed once, and so the `or_insert` below retains the
    // lexicographically first spelling of an entry two names reach.
    matching.sort_unstable();
    matching.dedup();

    let mut by_identity: BTreeMap<FileIdentity, &CandidatePath> = BTreeMap::new();
    for candidate in matching {
        if let PathKind::RegularFile(identity) = probe.probe(candidate)? {
            by_identity.entry(identity).or_insert(candidate);
        }
    }

    let mut selected: Vec<CandidatePath> = by_identity.into_values().cloned().collect();
    selected.sort_unstable();

    Ok(selected)
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "policy_property_tests.rs"]
mod property_tests;
