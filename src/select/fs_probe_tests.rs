//! Tests for the working-tree probe, against a real temporary tree.

use std::io::{self, ErrorKind};

use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;

use super::{AmbientPathProbe, confined_to, unnameable};
use crate::select::{
    extensions::ExtensionFilter,
    policy::{PathKind, PathProbe, select_files},
};

fn at(path: &str) -> Utf8PathBuf { Utf8PathBuf::from(path) }

/// A temporary tree and its path, which the guard keeps alive.
fn temp_root() -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .expect("a UTF-8 temporary directory");
    (directory, root)
}

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

#[test]
fn a_regular_file_is_identified_by_an_absolute_canonical_path() {
    let (_guard, root) = temp_root();
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
#[test]
fn a_symlink_is_a_link_even_when_its_target_is_a_regular_file() {
    let (_guard, root) = temp_root();
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
#[test]
fn a_candidate_behind_a_symlinked_directory_is_outside_the_root() {
    let (_guard, root) = temp_root();
    let (_outside_guard, outside) = temp_root();
    write(&outside, "guide.md", "|A|B|\n");
    std::os::unix::fs::symlink(&outside, root.join("docs")).expect("link the fixture directory");

    // The premise: the candidate is a regular file where the link points, so
    // this case fails for an implementation that never leaves `root`.
    assert!(
        outside.join("guide.md").is_file(),
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
#[test]
fn a_candidate_behind_an_in_tree_link_is_confined() {
    let (_guard, root) = temp_root();
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
#[test]
fn a_root_reached_through_a_link_still_confines_its_candidates() {
    let (_guard, base) = temp_root();
    write(&base, "real/docs/guide.md", "|A|B|\n");
    std::os::unix::fs::symlink("real", base.join("link")).expect("link the fixture root");

    assert!(
        matches!(
            probe(&base.join("link"), "docs/guide.md"),
            PathKind::RegularFile(_)
        ),
        "a root named through a link is the same tree as the one it names"
    );
}

#[test]
fn an_absent_path_is_missing() {
    let (_guard, root) = temp_root();
    assert_eq!(probe(&root, "docs/gone.md"), PathKind::Missing);
    assert_eq!(
        probe(&root, "docs/gone.md/deeper.md"),
        PathKind::Missing,
        "a path whose parent is absent is not a regular file"
    );
}

#[test]
fn a_directory_is_neither_a_regular_file_nor_a_symlink() {
    let (_guard, root) = temp_root();
    std::fs::create_dir_all(root.join("docs")).expect("create the fixture directory");
    assert_eq!(probe(&root, "docs"), PathKind::Other);
}

/// How a canonicalization failure is classified, as a function of the error
/// kind rather than of a tree.
///
/// A path `symlink_metadata` has already accepted can reach this decision again
/// only by losing a race with the filesystem, so no fixture stages the arm that
/// is not `NotFound`. `NotFound` is a candidate that is gone, or staged for
/// deletion, and is reported as the absence the selection has a rule for; every
/// other kind leaves the file present but unnameable, which the run is told
/// about rather than left to infer.
#[rstest]
#[case(ErrorKind::NotFound, true)]
#[case(ErrorKind::PermissionDenied, false)]
#[case(ErrorKind::NotADirectory, false)]
fn a_canonicalization_failure_is_classified_by_its_kind(
    #[case] kind: ErrorKind,
    #[case] absent: bool,
) {
    let path = at("/repo/docs/guide.md");
    match (unnameable(path.clone(), io::Error::from(kind)), absent) {
        (Ok(PathKind::Missing), true) => {}
        (Err(error), false) => {
            assert_eq!(
                error.path, path,
                "the failure names the path it could not read"
            );
            assert_eq!(error.source.kind(), kind, "the cause is reported unchanged");
        }
        (outcome, _) => panic!("{kind:?} was classified as {outcome:?}"),
    }
}

/// A root that does not exist confines nothing.
///
/// Stated against the predicate rather than staged through
/// [`AmbientPathProbe::probe`], which answers `Missing` for every candidate
/// before the root is ever asked about: a working directory removed mid-run is
/// the only way to arrive here, and that is a race no fixture should have to
/// win. Confinement that could not be established must not be reported as
/// confinement, and a selection over a tree that is not there names nothing.
#[test]
fn a_root_that_does_not_exist_confines_nothing() {
    let (_guard, root) = temp_root();
    let gone = root.join("gone");

    assert!(
        !confined_to(&gone, &at("/canonical/guide.md"))
            .expect("an absent root is not a failure to read it"),
        "a root that cannot be resolved confines nothing"
    );
}

/// A root that exists but cannot be resolved is reported, not answered.
///
/// The distinction this draws is the same one the probe draws for a candidate:
/// absence is an answer the caller has a rule for, and any other failure is a
/// question that went unasked. A root reached through a file is the staging
/// that needs no permission trick, so this case holds for a privileged test
/// runner as well as an unprivileged one.
#[test]
fn a_root_that_cannot_be_resolved_is_reported() {
    let (_guard, root) = temp_root();
    write(&root, "blocker", "not a directory\n");
    let unreachable = root.join("blocker/sub");

    let error = confined_to(&unreachable, &at("/canonical/guide.md"))
        .expect_err("a root that cannot be resolved is a failure, not confinement");
    assert_eq!(
        error.path, unreachable,
        "the failure names the root it could not read"
    );
    assert_ne!(
        error.source.kind(),
        ErrorKind::NotFound,
        "a root behind a file is present, not absent: {error:?}"
    );
}

/// A loop among a candidate's ancestors is a question the probe could not ask,
/// not an absence.
///
/// Stated as "not `NotFound`" rather than as a specific kind: the kernel reports
/// the loop, and which [`ErrorKind`] the platform maps it to is not this tool's
/// to pin — on this project's pinned toolchain the loop kind is still unstable.
#[cfg(unix)]
#[test]
fn a_symbolic_link_loop_is_an_error_rather_than_a_missing_file() {
    let (_guard, root) = temp_root();
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
        ErrorKind::NotFound,
        "a loop must not be reported as a file that is merely gone: {error:?}"
    );
}

#[test]
fn an_absolute_candidate_is_probed_where_it_points() {
    let (_guard, root) = temp_root();
    write(&root, "docs/guide.md", "|A|B|\n");
    let absolute = root.join("docs/guide.md");
    assert_eq!(
        probe(&root, absolute.as_str()),
        probe(&root, "docs/guide.md"),
        "a candidate is not re-rooted when it is already absolute"
    );
}

#[test]
fn selection_over_a_real_tree_takes_regular_files_alone() {
    let (_guard, root) = temp_root();
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
#[test]
fn two_hard_links_are_two_identities_and_neither_is_dropped() {
    use std::os::unix::fs::MetadataExt;

    let (_guard, root) = temp_root();
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
