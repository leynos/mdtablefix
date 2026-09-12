//! Tests for the working-tree probe, against a real temporary tree.
//!
//! The cases that state how a *failure* to read is classified are beside these,
//! in `fs_probe_failure_tests.rs`: they call the predicates directly rather than
//! through the probe, and each file keeps the fixture it needs so that neither
//! has to reach into the other.

use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::AmbientPathProbe;
use crate::select::{
    extensions::ExtensionFilter,
    policy::{PathKind, PathProbe, select_files},
};

fn at(path: &str) -> Utf8PathBuf { Utf8PathBuf::from(path) }

/// `directory` as the UTF-8 path a test works in.
fn as_path(directory: &TempDir) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).expect("a UTF-8 temporary directory")
}

/// A temporary tree, handed over as the guard that removes it.
///
/// The guard is the fixture's value and a test takes it as an argument, so the
/// binding the fixture machinery generates in the test body owns it for as long
/// as the test runs; the path is derived from it there. Nothing destructures a
/// tuple and nothing can drop the tree early — and a *derived* fixture would:
/// a fixture's dependencies are injected by value, so one taking this guard
/// would delete the tree as it returned.
#[test_macros::allow_fixture_expansion_lints]
#[fixture]
fn temp_root() -> TempDir { tempfile::tempdir().expect("a temporary directory") }

/// A second temporary tree, for the case that needs somewhere outside the first.
///
/// Confinement is a relation between two trees, so the case that asserts a
/// candidate escapes needs both: the tree the probe is confined to, and the one
/// the link points at.
#[cfg(unix)]
#[test_macros::allow_fixture_expansion_lints]
#[fixture]
fn elsewhere_root() -> TempDir { tempfile::tempdir().expect("a second temporary directory") }

fn write(root: &Utf8Path, name: &str, content: &str) {
    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the fixture directory");
    }
    std::fs::write(&path, content).expect("write a fixture");
}

/// The probe's verdict for the fixtures that have one.
///
/// The cases that exercise a *failure* call [`AmbientPathProbe::probe`]
/// directly, since a helper that panics cannot report the error they assert on.
fn probe(root: &Utf8Path, path: &str) -> PathKind {
    AmbientPathProbe
        .probe(root, Utf8Path::new(path))
        .expect("every fixture path is one the probe can read")
}

/// The selection over `candidates` in `root`, with the ambient probe.
fn selected_in(root: &Utf8Path, candidates: &[Utf8PathBuf]) -> Vec<Utf8PathBuf> {
    select_files(
        candidates,
        root,
        &ExtensionFilter::default(),
        &AmbientPathProbe,
    )
    .expect("every fixture candidate is one the probe can read")
}

#[rstest]
fn a_regular_file_is_identified_by_an_absolute_canonical_path(temp_root: TempDir) {
    let root = as_path(&temp_root);
    write(&root, "docs/guide.md", "|A|B|\n");

    let PathKind::RegularFile(identity) = probe(&root, "docs/guide.md") else {
        panic!("docs/guide.md is a regular file");
    };
    assert!(identity.as_path().is_absolute(), "{identity:?}");
    // Compared by path components rather than as text, so the assertion holds
    // where the platform spells the separator the other way round. A canonical
    // path is also the platform's own spelling of the absolute path, which on
    // Windows prefixes it with the verbatim marker.
    assert!(
        identity
            .as_path()
            .ends_with(Utf8Path::new("docs").join("guide.md")),
        "{identity:?}"
    );
    assert_eq!(identity.as_path().file_name(), Some("guide.md"));
}

#[cfg(unix)]
#[rstest]
fn a_symlink_is_a_link_even_when_its_target_is_a_regular_file(temp_root: TempDir) {
    let root = as_path(&temp_root);
    write(&root, "src/lib.rs", "fn main() {}\n");
    std::os::unix::fs::symlink("src/lib.rs", root.join("alias.md"))
        .expect("create the fixture symlink");

    // The premise: following the link would report a regular file, so this case
    // fails for an implementation that reads `metadata` rather than
    // `symlink_metadata`.
    assert!(
        std::fs::metadata(root.join("alias.md"))
            .expect("follow the fixture symlink")
            .is_file(),
        "the link must point at a regular file, or the case proves nothing"
    );
    assert_eq!(probe(&root, "alias.md"), PathKind::Symlink);
}

/// The escape a symlinked directory makes possible, and the reason the probe
/// compares canonical paths.
///
/// `symlink_metadata` does not follow the final component, but it does follow
/// an ancestor, so a tracked `docs/guide.md` whose `docs` is now a link out of
/// the tree is reported as a regular file. Selecting it would write through the
/// link, to a file the selection never named.
#[cfg(unix)]
#[rstest]
fn a_candidate_behind_a_symlinked_directory_is_outside_the_root(
    temp_root: TempDir,
    elsewhere_root: TempDir,
) {
    let root = as_path(&temp_root);
    let elsewhere = as_path(&elsewhere_root);
    write(&elsewhere, "guide.md", "|A|B|\n");
    std::os::unix::fs::symlink(&elsewhere, root.join("docs")).expect("link the fixture directory");

    // The premise: the candidate is a regular file where the link points, so
    // this case fails for an implementation that never leaves `root`.
    assert!(
        elsewhere.join("guide.md").is_file(),
        "the link must point at a regular file, or the case proves nothing"
    );
    assert_eq!(probe(&root, "docs/guide.md"), PathKind::OutsideRoot);

    let selected = selected_in(&root, &[at("docs/guide.md")]);
    assert!(
        selected.is_empty(),
        "a candidate that leaves the tree is not selected, got {selected:?}"
    );
}

/// The other side of the same rule: a link among a candidate's ancestors is not
/// itself the problem, so a link that stays inside the tree selects as usual.
///
/// Confinement is what the rule tests, rather than the absence of links, which
/// is why this case is not a link the probe may ignore.
#[cfg(unix)]
#[rstest]
fn a_candidate_behind_an_in_tree_link_is_confined(temp_root: TempDir) {
    let root = as_path(&temp_root);
    write(&root, "real/guide.md", "|A|B|\n");
    std::os::unix::fs::symlink("real", root.join("docs")).expect("link the fixture directory");

    assert!(
        matches!(probe(&root, "docs/guide.md"), PathKind::RegularFile(_)),
        "a link to a directory inside the tree is still inside the tree"
    );
}

/// A tree may itself be reached through a link, so both sides of the comparison
/// are canonicalized rather than only the candidate.
#[cfg(unix)]
#[rstest]
fn a_root_reached_through_a_link_still_confines_its_candidates(temp_root: TempDir) {
    let root = as_path(&temp_root);
    write(&root, "real/docs/guide.md", "|A|B|\n");
    std::os::unix::fs::symlink("real", root.join("link")).expect("link the fixture root");

    assert!(
        matches!(
            probe(&root.join("link"), "docs/guide.md"),
            PathKind::RegularFile(_)
        ),
        "a root named through a link is the same tree as the one it names"
    );
}

#[rstest]
fn an_absent_path_is_missing(temp_root: TempDir) {
    let root = as_path(&temp_root);
    assert_eq!(probe(&root, "docs/gone.md"), PathKind::Missing);
    assert_eq!(
        probe(&root, "docs/gone.md/deeper.md"),
        PathKind::Missing,
        "a path whose parent is absent is not a regular file"
    );
}

#[rstest]
fn a_directory_is_neither_a_regular_file_nor_a_symlink(temp_root: TempDir) {
    let root = as_path(&temp_root);
    std::fs::create_dir_all(root.join("docs")).expect("create the fixture directory");
    assert_eq!(probe(&root, "docs"), PathKind::Other);
}

/// A loop among a candidate's ancestors is a question the probe could not ask,
/// not an absence.
///
/// Stated as "not `NotFound`" rather than as a specific kind: the kernel reports
/// the loop, and which [`std::io::ErrorKind`] the platform maps it to is not
/// this tool's to pin — on this project's pinned toolchain the loop kind is
/// still unstable.
#[cfg(unix)]
#[rstest]
fn a_symbolic_link_loop_is_an_error_rather_than_a_missing_file(temp_root: TempDir) {
    let root = as_path(&temp_root);
    // A loop between ancestors, not in the final component: the probe reads
    // `symlink_metadata`, so a candidate that *is* a link is classified as one
    // without the kernel ever resolving it.
    std::os::unix::fs::symlink("b", root.join("a")).expect("create the fixture symlink");
    std::os::unix::fs::symlink("a", root.join("b")).expect("create the fixture symlink");

    let error = AmbientPathProbe
        .probe(&root, Utf8Path::new("a/guide.md"))
        .expect_err("a link loop is not an absence, and not an answer");
    assert_eq!(error.path, root.join("a/guide.md"));
    assert_ne!(
        error.source.kind(),
        std::io::ErrorKind::NotFound,
        "a loop must not be reported as a file that is merely gone: {error:?}"
    );
}

#[rstest]
fn an_absolute_candidate_is_probed_where_it_points(temp_root: TempDir) {
    let root = as_path(&temp_root);
    write(&root, "docs/guide.md", "|A|B|\n");
    let absolute = root.join("docs/guide.md");
    assert_eq!(
        probe(&root, absolute.as_str()),
        probe(&root, "docs/guide.md"),
        "a candidate is not re-rooted when it is already absolute"
    );
}

#[rstest]
fn selection_over_a_real_tree_takes_regular_files_alone(temp_root: TempDir) {
    let root = as_path(&temp_root);
    write(&root, "docs/guide.md", "|A|B|\n");
    std::fs::create_dir_all(root.join("docs/subdir.md"))
        .expect("create a directory named like a document");

    let selected = selected_in(
        &root,
        &[
            at("docs/guide.md"),
            at("docs/subdir.md"),
            at("docs/gone.md"),
            at("draft.markdown"),
        ],
    );
    assert_eq!(
        selected,
        vec![at("docs/guide.md")],
        "a directory named like a document, an absent file, and a name with a configured \
         extension that does not exist are all excluded"
    );
}

/// INV-DEDUP's negative control, and the reason the identity is the canonical
/// path rather than `(st_dev, st_ino)`.
///
/// Replacement writes a temporary file and renames it over the target, so after
/// one link of a hard-link pair is formatted the other still refers to the
/// original inode: keys on the inode would collapse the pair, format one, and
/// leave the other silently stale. Two names for one inode are two directory
/// entries, and canonicalization draws exactly that line.
#[cfg(unix)]
#[rstest]
fn two_hard_links_are_two_identities_and_neither_is_dropped(temp_root: TempDir) {
    use std::os::unix::fs::MetadataExt;

    let root = as_path(&temp_root);
    write(&root, "a.md", "|A|B|\n");
    std::fs::hard_link(root.join("a.md"), root.join("b.md")).expect("create the hard link");

    let first = std::fs::metadata(root.join("a.md")).expect("read the fixture");
    let second = std::fs::metadata(root.join("b.md")).expect("read the fixture");
    assert_eq!(
        first.ino(),
        second.ino(),
        "the fixture must present one inode under two names"
    );
    assert_eq!(first.nlink(), 2, "the fixture must be a hard-link pair");

    assert_ne!(
        probe(&root, "a.md"),
        probe(&root, "b.md"),
        "two directory entries must keep two identities"
    );

    let selected = selected_in(&root, &[at("a.md"), at("b.md")]);
    assert_eq!(
        selected,
        vec![at("a.md"), at("b.md")],
        "neither link may be dropped, or formatting it would leave the other stale"
    );
}
