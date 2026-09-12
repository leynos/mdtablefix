//! Tests for the selection contract in [`super`], stated as example cases.
//!
//! Everything here runs against a fake probe, so no test needs a repository, a
//! temporary directory, or a working directory of its own. The invariants that
//! hold of every candidate set are stated as properties in
//! `policy_property_tests.rs` beside these, and the one case that cannot be
//! stated against a fake at all — that two hard links keep distinct identities —
//! needs the real adapter and lives in [`crate::select::fs_probe`]'s tests,
//! because this module may not import `std::fs`.
//!
//! The fake probes and the two constructors are shared with the property tests,
//! which is why the shared items are `pub(super)` rather than private.

use std::{collections::BTreeMap, io};

use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;

use super::{FileIdentity, PathKind, PathProbe, ProbeError, select_files};
use crate::select::extensions::ExtensionFilter;

/// The root every fake probe is handed and ignores.
pub(super) fn select_root() -> &'static Utf8Path { Utf8Path::new("/repo") }

/// `path` as a candidate, spelled the way `git ls-files` spells it.
pub(super) fn at(path: &str) -> Utf8PathBuf { Utf8PathBuf::from(path) }

/// A regular-file verdict whose identity is `key`.
///
/// Two paths passed the same key collide exactly as two names for one
/// directory entry do, which is how the deduplication cases are stated without
/// a filesystem.
pub(super) fn regular(key: &str) -> PathKind {
    let canonical = Utf8PathBuf::from(format!("/canonical/{key}"));
    PathKind::RegularFile(FileIdentity::from_canonical_path(canonical))
}

/// A probe that answers from a table and touches nothing.
pub(super) struct FakeProbe(BTreeMap<Utf8PathBuf, PathKind>);

impl FakeProbe {
    pub(super) fn new(entries: impl IntoIterator<Item = (Utf8PathBuf, PathKind)>) -> Self {
        Self(entries.into_iter().collect())
    }
}

impl PathProbe for FakeProbe {
    /// Answers `Missing` for anything the table omits, so a case can never pass
    /// because a path was absent from the fixture.
    fn probe(&self, _root: &Utf8Path, path: &Utf8Path) -> Result<PathKind, ProbeError> {
        Ok(self.0.get(path).cloned().unwrap_or(PathKind::Missing))
    }
}

/// A probe that answers the same verdict for every path.
struct FixedProbe(PathKind);

impl PathProbe for FixedProbe {
    fn probe(&self, _root: &Utf8Path, _path: &Utf8Path) -> Result<PathKind, ProbeError> {
        Ok(self.0.clone())
    }
}

/// A probe for which every candidate but one is a regular file.
struct UnreadableProbe {
    /// The candidate it cannot read, spelled as `git ls-files` spells it.
    unreadable: &'static str,
    /// Why it cannot read it.
    kind: io::ErrorKind,
}

impl PathProbe for UnreadableProbe {
    fn probe(&self, root: &Utf8Path, path: &Utf8Path) -> Result<PathKind, ProbeError> {
        if path == Utf8Path::new(self.unreadable) {
            Err(ProbeError {
                path: root.join(path),
                source: io::Error::from(self.kind),
            })
        } else {
            Ok(regular(path.as_str()))
        }
    }
}

/// `select_files` over `candidates` against `probe`, for the cases where the
/// probe answers.
fn selecting<P: PathProbe + ?Sized>(candidates: &[Utf8PathBuf], probe: &P) -> Vec<Utf8PathBuf> {
    select_files(
        candidates,
        select_root(),
        &ExtensionFilter::default(),
        probe,
    )
    .expect("the fixture's probe answers for every path it is asked about")
}

/// INV-PROBE-EXCLUSIONS, with the fixture's extension held constant so that the
/// verdict alone is attributable.
#[rstest]
#[case(
    PathKind::RegularFile(FileIdentity::from_canonical_path(at("/canonical/guide.md"))),
    true
)]
#[case(PathKind::Missing, false)]
#[case(PathKind::Symlink, false)]
#[case(PathKind::Other, false)]
fn the_probe_verdict_alone_decides(#[case] verdict: PathKind, #[case] expected: bool) {
    let candidates = [at("docs/guide.md")];
    let selected = selecting(&candidates, &FixedProbe(verdict.clone()));
    assert_eq!(
        !selected.is_empty(),
        expected,
        "a candidate with a configured extension under {verdict:?}: selected {selected:?}"
    );
}

/// INV-DEDUP, first case: one entry reported once per index stage, as
/// `git ls-files` does for a conflicted path.
#[test]
fn a_path_reported_once_per_merge_stage_is_selected_once() {
    let paths = ["docs/guide.md", "docs/guide.md", "docs/guide.md"];
    let probe = FakeProbe::new(paths.map(|path| (at(path), regular(path))));
    let candidates: Vec<Utf8PathBuf> = paths.map(at).to_vec();
    let selected = selecting(&candidates, &probe);
    assert_eq!(selected, vec![at("docs/guide.md")]);
}

/// INV-DEDUP, second case: two spellings of one directory entry, as a
/// case-insensitive filesystem allows.
#[test]
fn two_spellings_of_one_entry_collapse_to_the_first_path() {
    let paths = ["Readme.md", "README.md"];
    let probe = FakeProbe::new(paths.map(|path| (at(path), regular("readme.md"))));

    // Non-vacuity: the fixture really does present a collision.
    assert_eq!(
        probe
            .probe(select_root(), Utf8Path::new(paths[0]))
            .expect("the fake probe answers every candidate it is handed"),
        probe
            .probe(select_root(), Utf8Path::new(paths[1]))
            .expect("the fake probe answers every candidate it is handed"),
        "the two spellings must share one identity for this case to mean anything"
    );

    for candidates in [
        paths.map(at).to_vec(),
        paths.iter().rev().copied().map(at).collect::<Vec<_>>(),
    ] {
        let selected = selecting(&candidates, &probe);
        assert_eq!(
            selected,
            vec![at("README.md")],
            "the lexicographically first spelling is the one retained"
        );
    }
}

/// INV-DEDUP, third case, and the statement of the output order: distinct
/// entries are all selected, sorted byte-wise on their paths.
#[test]
fn distinct_entries_are_all_selected_sorted_byte_wise() {
    let paths = ["notes.md", "docs/guide.md", "README.mdc"];
    let probe = FakeProbe::new(paths.iter().map(|path| (at(path), regular(path))));
    let candidates = paths.map(at).to_vec();
    let selected = selecting(&candidates, &probe);
    assert_eq!(
        selected,
        vec![at("README.mdc"), at("docs/guide.md"), at("notes.md")],
        "byte-wise: uppercase sorts before lowercase, and a prefix before its extension"
    );
}

/// The rule for a candidate that cannot be classified: the selection stops, and
/// says which path it could not read.
///
/// The alternative — skipping the candidate and returning the rest — is the
/// failure this test exists to prevent, because a run that cannot read one
/// candidate does not know which files it is about to format.
#[test]
fn a_candidate_the_probe_cannot_read_fails_the_selection() {
    let candidates = [at("docs/guide.md"), at("docs/notes.md"), at("docs/api.md")];
    let probe = UnreadableProbe {
        unreadable: "docs/notes.md",
        kind: io::ErrorKind::PermissionDenied,
    };

    let error = select_files(
        &candidates,
        select_root(),
        &ExtensionFilter::default(),
        &probe,
    )
    .expect_err("an unreadable candidate must not be silently skipped");

    assert_eq!(
        error.path,
        select_root().join("docs/notes.md"),
        "the failure names the candidate it could not read, addressed as the probe was"
    );
    assert_eq!(error.source.kind(), io::ErrorKind::PermissionDenied);
}
