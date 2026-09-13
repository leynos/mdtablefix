//! Tests for the selection contract in [`super`], stated as example cases.
//!
//! Everything here runs against a fake probe, so no test needs a repository, a
//! temporary directory, or a working directory of its own. The invariants that
//! hold of every candidate set are stated as properties in
//! `policy_property_tests.rs` beside these, and the one case that cannot be
//! stated against a fake at all — that two hard links keep distinct identities —
//! needs the real adapter and lives in [`crate::select::fs_probe`]'s tests,
//! because this module may not import `std::fs`.

use std::collections::BTreeMap;

use rstest::rstest;

use super::{CandidatePath, FileIdentity, PathKind, PathProbe, ProbeFailure, select_files};
use crate::select::extensions::ExtensionFilter;

/// `path` as a candidate, spelled the way `git ls-files` spells it.
pub(super) fn at(path: &str) -> CandidatePath { CandidatePath::new(path.to_owned()) }

/// A regular-file verdict whose identity is `key`.
///
/// Two paths passed the same key collide exactly as two names for one
/// directory entry do, which is how the deduplication cases are stated without
/// a filesystem.
pub(super) fn regular(key: &str) -> PathKind {
    PathKind::RegularFile(FileIdentity::from_canonical_name(format!(
        "/canonical/{key}"
    )))
}

/// A probe that answers from a table and touches nothing.
pub(super) struct FakeProbe(BTreeMap<CandidatePath, PathKind>);

impl FakeProbe {
    pub(super) fn new(entries: impl IntoIterator<Item = (CandidatePath, PathKind)>) -> Self {
        Self(entries.into_iter().collect())
    }
}

impl PathProbe for FakeProbe {
    /// Answers `Missing` for anything the table omits, so a case can never pass
    /// because a path was absent from the fixture.
    fn probe(&self, candidate: &CandidatePath) -> Result<PathKind, ProbeFailure> {
        Ok(self.0.get(candidate).cloned().unwrap_or(PathKind::Missing))
    }
}

/// A probe that answers the same verdict for every path.
struct FixedProbe(PathKind);

impl PathProbe for FixedProbe {
    fn probe(&self, _candidate: &CandidatePath) -> Result<PathKind, ProbeFailure> {
        Ok(self.0.clone())
    }
}

/// A probe for which every candidate but one is a regular file.
struct UnreadableProbe {
    /// The candidate it cannot read, spelled as `git ls-files` spells it.
    unreadable: &'static str,
}

impl PathProbe for UnreadableProbe {
    fn probe(&self, candidate: &CandidatePath) -> Result<PathKind, ProbeFailure> {
        if candidate.as_str() == self.unreadable {
            Err(ProbeFailure::unreadable(
                candidate.clone(),
                "permission denied".to_owned(),
            ))
        } else {
            Ok(regular(candidate.as_str()))
        }
    }
}

/// `select_files` over `candidates` against `probe`, for the cases where the
/// probe answers.
fn selecting<P: PathProbe + ?Sized>(candidates: &[CandidatePath], probe: &P) -> Vec<CandidatePath> {
    select_files(candidates, &ExtensionFilter::default(), probe)
        .expect("the fixture's probe answers for every path it is asked about")
}

/// INV-PROBE-EXCLUSIONS, with the fixture's extension held constant so that the
/// verdict alone is attributable.
#[rstest]
#[case(
    PathKind::RegularFile(FileIdentity::from_canonical_name("/canonical/guide.md".to_owned())),
    true
)]
#[case(PathKind::Missing, false)]
#[case(PathKind::Symlink, false)]
#[case(PathKind::Other, false)]
// A regular file the selection still refuses, because writing it would write
// outside the tree the selection was made in. The case matters here rather
// than only in the probe's own tests: it is the one verdict where "reads as a
// regular file" and "is selected" part company.
#[case(PathKind::OutsideRoot, false)]
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
    let candidates = paths.map(at).to_vec();
    assert_eq!(selecting(&candidates, &probe), vec![at("docs/guide.md")]);
}

/// INV-DEDUP, second case: two spellings of one directory entry, as a
/// case-insensitive filesystem allows.
#[test]
fn two_spellings_of_one_entry_collapse_to_the_first_path() {
    let paths = ["Readme.md", "README.md"];
    let probe = FakeProbe::new(paths.map(|path| (at(path), regular("readme.md"))));

    assert_eq!(
        probe.probe(&at(paths[0])).expect("the fake probe answers"),
        probe.probe(&at(paths[1])).expect("the fake probe answers"),
        "the two spellings must share one identity for this case to mean anything"
    );

    for candidates in [
        paths.map(at).to_vec(),
        paths.iter().rev().copied().map(at).collect::<Vec<_>>(),
    ] {
        assert_eq!(
            selecting(&candidates, &probe),
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
    assert_eq!(
        selecting(&candidates, &probe),
        vec![at("README.mdc"), at("docs/guide.md"), at("notes.md")],
        "byte-wise: uppercase sorts before lowercase, and a prefix before its extension"
    );
}

/// The rule for a candidate that cannot be classified: the selection stops,
/// and says which path it could not read.
#[test]
fn a_candidate_the_probe_cannot_read_fails_the_selection() {
    let candidates = [at("docs/guide.md"), at("docs/notes.md"), at("docs/api.md")];
    let error = select_files(
        &candidates,
        &ExtensionFilter::default(),
        &UnreadableProbe {
            unreadable: "docs/notes.md",
        },
    )
    .expect_err("an unreadable candidate must not be silently skipped");

    assert_eq!(error.path, at("docs/notes.md"));
    assert_eq!(error.detail, "permission denied");
}
