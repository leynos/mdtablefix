//! Tests for the working-tree probe, against a real temporary tree.

use std::io::ErrorKind;

use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;

use super::{AmbientPathProbe, unnameable};
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

fn probe(root: &Utf8Path, path: &str) -> PathKind {
    AmbientPathProbe.probe(root, Utf8Path::new(path))
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
/// only by losing a race with the filesystem, so the second arm has no fixture
/// that stages it. `NotFound` is a candidate that is gone, or staged for
/// deletion; every other kind is a file present but unnameable.
#[rstest]
#[case(ErrorKind::NotFound, PathKind::Missing)]
#[case(ErrorKind::PermissionDenied, PathKind::Other)]
fn a_canonicalization_failure_is_classified_by_its_kind(
    #[case] kind: ErrorKind,
    #[case] expected: PathKind,
) {
    assert_eq!(unnameable(kind), expected);
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

    let selected = select_files(
        &[
            at("docs/guide.md"),
            at("docs/subdir.md"),
            at("docs/gone.md"),
            at("draft.markdown"),
        ],
        &root,
        &ExtensionFilter::default(),
        &AmbientPathProbe,
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

    let selected = select_files(
        &[at("a.md"), at("b.md")],
        &root,
        &ExtensionFilter::default(),
        &AmbientPathProbe,
    );
    assert_eq!(
        selected,
        vec![at("a.md"), at("b.md")],
        "neither link may be dropped, or formatting it would leave the other stale"
    );
}
