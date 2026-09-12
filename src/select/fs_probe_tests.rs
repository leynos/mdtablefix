//! Tests for the working-tree probe, against a real temporary tree.

use std::io::{self, ErrorKind};

use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;

use super::{AmbientPathProbe, Reading, confined_to, nearest_existing, unreadable};
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

/// How a failed read is classified, for the kinds that decide alone.
///
/// `NotFound` is not among these cases: what it means depends on the ancestors
/// of the path it was reported for, and
/// [`a_read_that_fails_under_a_file_is_not_an_absence`] stages that. Each kind
/// here leaves the file present but unreadable — a permission failure, and the
/// `ENOTDIR` Unix produces for a path through a file, which reaches the caller
/// unchanged because it is already the answer the ancestor walk arrives at.
///
/// Stated as a function of the failure rather than through
/// [`AmbientPathProbe::probe`], which no fixture can make report either of
/// these: a path `symlink_metadata` has already accepted can reach this decision
/// only by losing a race with the filesystem.
#[rstest]
#[case(ErrorKind::PermissionDenied)]
#[case(ErrorKind::NotADirectory)]
fn a_failure_that_is_not_absence_is_reported_unchanged(#[case] kind: ErrorKind) {
    let path = at("/repo/docs/guide.md");

    let error = unreadable(path.clone(), io::Error::from(kind))
        .expect_err("a file that is present but unreadable is not absent");

    assert_eq!(
        error.path, path,
        "the failure names the path it could not read"
    );
    assert_eq!(error.source.kind(), kind, "the cause is reported unchanged");
}

/// Absence is answered for the whole path, not for the leaf that failed.
///
/// Two shapes look identical to a leaf's own failure, and only the ancestors
/// tell them apart. A file that is gone from a directory that is there is the
/// staged deletion the selection skips; a path that runs through a regular file
/// cannot be there at all, and reading it as a deletion would skip a candidate
/// the run was asked to consider. Windows reports both as `NOT_FOUND`, where
/// Unix says `ENOTDIR`, so the second shape is staged here rather than left to
/// a platform that never asks the question.
#[rstest]
#[case::gone("gone.md", true)]
#[case::through_a_file("blocker/guide.md", false)]
fn a_read_that_fails_under_a_file_is_not_an_absence(#[case] relative: &str, #[case] missing: bool) {
    let (_guard, root) = temp_root();
    write(&root, "blocker", "not a directory\n");
    let path = root.join(relative);

    let outcome = unreadable(path.clone(), io::Error::from(ErrorKind::NotFound));
    if missing {
        assert_eq!(
            outcome.ok(),
            Some(PathKind::Missing),
            "{relative} is gone, and that is the answer the selection has a rule for"
        );
        return;
    }

    let error = outcome.expect_err("a path through a file is not an absence");
    assert_eq!(
        error.path, path,
        "the failure names the candidate it could not read"
    );
    assert_eq!(
        error.source.kind(),
        ErrorKind::NotADirectory,
        "the kind Unix reports for it, reported on every platform"
    );
}

/// A path no part of which is there is absent, not unreachable.
///
/// The walk stops at the first ancestor that exists, and here that is the root
/// of the fixture: what is missing is a whole subtree, which is what a staged
/// deletion of one looks like.
#[test]
fn a_read_that_fails_where_the_whole_path_is_gone_is_an_absence() {
    let (_guard, root) = temp_root();
    let path = root.join("gone/sub/guide.md");

    assert_eq!(
        unreadable(path, io::Error::from(ErrorKind::NotFound)).ok(),
        Some(PathKind::Missing),
        "a subtree that is gone is absent, not unreachable"
    );
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
/// runner as well as an unprivileged one — and, because the classification asks
/// the root's ancestors rather than its own failure alone, the kind asserted
/// below is the same on every platform, including the one that reports it as
/// absence.
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
    assert_eq!(
        error.source.kind(),
        ErrorKind::NotADirectory,
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

/// A failure reading an ancestor stops the walk, and is reported as it arrived.
///
/// The arm this states is the walk's own, and no fixture stages it: a candidate
/// reaches the walk only through a `NotFound` on its leaf, and the filesystem
/// has answered for every ancestor above it by then. The walk takes its reader
/// as a parameter for exactly this case, so the arm is a test's to cover —
/// including the replacement that walks past it and calls the candidate absent.
#[test]
fn a_failure_reading_an_ancestor_is_reported_rather_than_walked_past() {
    let docs = at("/repo/docs");
    let mut read_paths = Vec::new();

    let error = nearest_existing(Some(docs.as_path()), |ancestor| {
        read_paths.push(ancestor.to_owned());
        if ancestor == docs.as_path() {
            Reading::Failed(io::Error::from(ErrorKind::PermissionDenied))
        } else {
            Reading::Directory
        }
    })
    .expect("an ancestor that cannot be read is a failure, not an absence");

    assert_eq!(
        error.kind(),
        ErrorKind::PermissionDenied,
        "the cause is reported unchanged"
    );
    assert_eq!(
        read_paths,
        vec![docs],
        "the walk stops at the ancestor it could not read rather than climbing to /repo, which is \
         a directory and would answer that nothing is missing"
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
