//! Tests for the selection contract in [`super`].
//!
//! Everything here runs against a fake probe, so no test needs a repository, a
//! temporary directory, or a working directory of its own. The one property
//! that cannot be stated that way — that two hard links keep distinct
//! identities — needs the real adapter and lives in
//! [`crate::select::fs_probe`]'s tests, because this module may not import
//! `std::fs`.

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
};

use camino::{Utf8Path, Utf8PathBuf};
use proptest::{collection::vec, prelude::*, test_runner::TestRunner};
use rstest::rstest;

use super::{FileIdentity, PathKind, PathProbe, select_files};
use crate::select::extensions::ExtensionFilter;

/// The root every fake probe is handed and ignores.
fn select_root() -> &'static Utf8Path { Utf8Path::new("/repo") }

/// `path` as a candidate, spelled the way `git ls-files` spells it.
fn at(path: &str) -> Utf8PathBuf { Utf8PathBuf::from(path) }

/// A regular-file verdict whose identity is `key`.
///
/// Two paths passed the same key collide exactly as two names for one
/// directory entry do, which is how the deduplication cases are stated without
/// a filesystem.
fn regular(key: &str) -> PathKind {
    let canonical = Utf8PathBuf::from(format!("/canonical/{key}"));
    PathKind::RegularFile(FileIdentity::from_canonical_path(canonical))
}

/// A probe that answers from a table and touches nothing.
struct FakeProbe(BTreeMap<Utf8PathBuf, PathKind>);

impl FakeProbe {
    fn new(entries: impl IntoIterator<Item = (Utf8PathBuf, PathKind)>) -> Self {
        Self(entries.into_iter().collect())
    }
}

impl PathProbe for FakeProbe {
    /// Answers `Missing` for anything the table omits, so a case can never pass
    /// because a path was absent from the fixture.
    fn probe(&self, _root: &Utf8Path, path: &Utf8Path) -> PathKind {
        self.0.get(path).cloned().unwrap_or(PathKind::Missing)
    }
}

/// A probe that answers the same verdict for every path.
struct FixedProbe(PathKind);

impl PathProbe for FixedProbe {
    fn probe(&self, _root: &Utf8Path, _path: &Utf8Path) -> PathKind { self.0.clone() }
}

/// Candidates and the verdict each one draws.
///
/// The pool mixes matching extensions, non-matching ones, an extensionless
/// name, a dotfile, and nested directories; the verdicts are drawn from all
/// four [`PathKind`] variants.
const POOL: [&str; 12] = [
    "docs/guide.md",
    "docs/deep/nested/notes.markdown",
    "README.mdc",
    "top.md",
    "docs/UPPER.MD",
    "src/lib.rs",
    "Cargo.toml",
    "assets/logo.png",
    "Makefile",
    ".hidden",
    ".config/notes.md",
    "docs/notes.mdx",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Regular,
    Missing,
    Symlink,
    Other,
}

fn verdict() -> impl Strategy<Value = Verdict> {
    prop_oneof![
        Just(Verdict::Regular),
        Just(Verdict::Missing),
        Just(Verdict::Symlink),
        Just(Verdict::Other),
    ]
}

fn candidate() -> impl Strategy<Value = Utf8PathBuf> {
    (0..POOL.len()).prop_map(|index| at(POOL[index]))
}

fn candidate_and_verdict() -> impl Strategy<Value = Vec<(Utf8PathBuf, Verdict)>> {
    vec((candidate(), verdict()), 0..=30)
}

fn kind_for(verdict: Verdict, path: &Utf8Path) -> PathKind {
    match verdict {
        Verdict::Regular => regular(path.as_str()),
        Verdict::Missing => PathKind::Missing,
        Verdict::Symlink => PathKind::Symlink,
        Verdict::Other => PathKind::Other,
    }
}

/// The verdict each candidate path draws, with the last entry winning.
///
/// One map feeds both the probe and the expectation, so a case that names the
/// same path twice cannot have the fixture answer one verdict while the
/// expectation assumes another — which is exactly what two independent folds
/// over `candidates` would allow, since a probe table keeps the last verdict
/// and a `any` keeps the first.
fn verdicts_for(candidates: &[(Utf8PathBuf, Verdict)]) -> BTreeMap<Utf8PathBuf, Verdict> {
    candidates.iter().cloned().collect()
}

fn probe_for(verdicts: &BTreeMap<Utf8PathBuf, Verdict>) -> FakeProbe {
    FakeProbe::new(
        verdicts
            .iter()
            .map(|(path, verdict)| (path.clone(), kind_for(*verdict, path))),
    )
}

/// INV-EXT-SOUND and INV-EXT-COMPLETE, the two halves of the rule.
///
/// Stated together on purpose: soundness alone admits an implementation that
/// selects nothing, and completeness alone admits one that selects everything.
#[test]
fn a_path_is_selected_exactly_when_it_matches_and_probes_as_a_regular_file() {
    let filter = ExtensionFilter::default();
    let mut runner = TestRunner::default();
    let saw_empty = Cell::new(false);
    let saw_non_empty = Cell::new(false);
    let saw_probe_only_exclusion = Cell::new(false);

    runner
        .run(&candidate_and_verdict(), |candidates| {
            let paths: Vec<Utf8PathBuf> = candidates.iter().map(|(path, _)| path.clone()).collect();
            let verdicts = verdicts_for(&candidates);
            let probe = probe_for(&verdicts);
            let selected = select_files(&paths, select_root(), &filter, &probe);

            // Soundness: nothing reaches the output that the rule does not name.
            for path in &selected {
                prop_assert!(
                    filter.matches(path),
                    "{} does not carry a configured extension",
                    path
                );
                prop_assert!(
                    matches!(probe.probe(select_root(), path), PathKind::RegularFile(_)),
                    "{} was selected without a regular-file verdict",
                    path
                );
            }

            // Completeness: nothing the rule names is left out.
            let expected: BTreeSet<&Utf8Path> = verdicts
                .iter()
                .filter(|(path, verdict)| **verdict == Verdict::Regular && filter.matches(path))
                .map(|(path, _)| path.as_path())
                .collect();
            let actual: BTreeSet<&Utf8Path> = selected.iter().map(Utf8PathBuf::as_path).collect();
            prop_assert_eq!(&actual, &expected);
            prop_assert_eq!(selected.len(), actual.len(), "a path was selected twice");

            if selected.is_empty() {
                saw_empty.set(true);
            } else {
                saw_non_empty.set(true);
            }
            if verdicts
                .iter()
                .any(|(path, verdict)| *verdict != Verdict::Regular && filter.matches(path))
            {
                saw_probe_only_exclusion.set(true);
            }
            Ok(())
        })
        .expect("matching extension plus a regular-file verdict decide selection");

    assert!(
        saw_empty.get() && saw_non_empty.get() && saw_probe_only_exclusion.get(),
        "the run must observe an empty selection ({}), a non-empty one ({}), and a \
         matching-extension candidate excluded by its verdict alone ({})",
        saw_empty.get(),
        saw_non_empty.get(),
        saw_probe_only_exclusion.get()
    );
}

/// A permutation of `items` driven by `keys`.
///
/// Stated here rather than drawn from a random-number generator so that the
/// test needs no shuffling dependency and the permutation is reproducible from
/// the failing case alone.
fn permute(items: &[Utf8PathBuf], keys: &[u8]) -> Vec<Utf8PathBuf> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|index| (keys[*index], *index));
    order
        .into_iter()
        .map(|index| items[index].clone())
        .collect()
}

fn candidates_and_keys() -> impl Strategy<Value = (Vec<Utf8PathBuf>, Vec<u8>)> {
    vec(candidate(), 0..=12).prop_flat_map(|paths| {
        let shared = paths.clone();
        vec(any::<u8>(), paths.len()).prop_map(move |keys| (shared.clone(), keys))
    })
}

/// INV-ORDER-DET: the selection is a function of the candidate multiset.
///
/// `git ls-files` output is not globally sorted, and the order is
/// user-visible, because `--list-files` and the printed concatenation both
/// follow it. The output is sorted byte-wise on the UTF-8 path; the explicit
/// order pinned in `distinct_entries_are_all_selected` is what states that,
/// since checking sortedness with the same `Ord` the implementation sorts by
/// would prove nothing.
#[test]
fn selection_does_not_depend_on_the_order_the_listing_arrives_in() {
    let filter = ExtensionFilter::default();
    let mut runner = TestRunner::default();
    let saw_a_different_order = Cell::new(false);
    let saw_several = Cell::new(false);

    runner
        .run(&candidates_and_keys(), |(paths, keys)| {
            let shuffled = permute(&paths, &keys);
            if shuffled != paths {
                saw_a_different_order.set(true);
            }
            let probe = FakeProbe::new(
                paths
                    .iter()
                    .map(|path| (path.clone(), regular(path.as_str()))),
            );
            let in_listing_order = select_files(&paths, select_root(), &filter, &probe);
            let in_shuffled_order = select_files(&shuffled, select_root(), &filter, &probe);
            prop_assert_eq!(
                &in_listing_order,
                &in_shuffled_order,
                "the selection must not depend on listing order"
            );
            if in_listing_order.len() >= 2 {
                saw_several.set(true);
            }
            Ok(())
        })
        .expect("selection is a function of the candidate multiset");

    assert!(
        saw_a_different_order.get(),
        "the generated permutation must differ from the listing, or the property is vacuous"
    );
    assert!(
        saw_several.get(),
        "at least one case must select two or more paths, or ordering carries no information"
    );
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
    let selected = select_files(
        &candidates,
        select_root(),
        &ExtensionFilter::default(),
        &FixedProbe(verdict.clone()),
    );
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
    let selected = select_files(
        &candidates,
        select_root(),
        &ExtensionFilter::default(),
        &probe,
    );
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
        probe.probe(select_root(), Utf8Path::new(paths[0])),
        probe.probe(select_root(), Utf8Path::new(paths[1])),
        "the two spellings must share one identity for this case to mean anything"
    );

    let filter = ExtensionFilter::default();
    for candidates in [
        paths.map(at).to_vec(),
        paths.iter().rev().copied().map(at).collect::<Vec<_>>(),
    ] {
        let selected = select_files(&candidates, select_root(), &filter, &probe);
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
    let selected = select_files(
        &candidates,
        select_root(),
        &ExtensionFilter::default(),
        &probe,
    );
    assert_eq!(
        selected,
        vec![at("README.mdc"), at("docs/guide.md"), at("notes.md")],
        "byte-wise: uppercase sorts before lowercase, and a prefix before its extension"
    );
}
