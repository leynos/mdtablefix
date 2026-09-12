//! The selection contract's invariants, as properties over generated sets.
//!
//! The example cases are beside these in `policy_tests.rs`, which also holds the
//! fake probe and the `at`/`regular` constructors both files state their cases
//! with. Each property asserts its own non-vacuity as well as its claim: a
//! property that never reaches the interesting shape passes for the wrong
//! reason, and the shape it never reached is exactly what a weakening of the
//! implementation would stop producing.

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
};

use camino::{Utf8Path, Utf8PathBuf};
use proptest::{collection::vec, prelude::*, test_runner::TestRunner};

use super::{
    PathKind,
    PathProbe,
    select_files,
    tests::{FakeProbe, at, regular, select_root},
};
use crate::select::extensions::ExtensionFilter;

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
            let selected = select_files(&paths, select_root(), &filter, &probe)
                .expect("the fake probe answers every candidate it is handed");

            // Soundness: nothing reaches the output that the rule does not name.
            for path in &selected {
                prop_assert!(
                    filter.matches(path),
                    "{} does not carry a configured extension",
                    path
                );
                prop_assert!(
                    matches!(
                        probe.probe(select_root(), path),
                        Ok(PathKind::RegularFile(_))
                    ),
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
/// order pinned in `distinct_entries_are_all_selected_sorted_byte_wise` is what
/// states that, since checking sortedness with the same `Ord` the
/// implementation sorts by would prove nothing.
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
            let in_listing_order = select_files(&paths, select_root(), &filter, &probe)
                .expect("the fake probe answers every candidate it is handed");
            let in_shuffled_order = select_files(&shuffled, select_root(), &filter, &probe)
                .expect("the fake probe answers every candidate it is handed");
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
