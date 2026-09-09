//! Integration tests for the atomic `--in-place` write contract.
//!
//! Each test drives the real binary and asserts on the observable contract: a
//! successful rewrite preserves the target's mode and leaves no temporary file
//! behind, a failed rewrite leaves the original byte-identical, and a symlinked
//! target is declined rather than replaced by a regular file.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

/// A table that needs reflowing.
const BROKEN: &str = "|A|B|\n|1|2|\n";

/// The same table after a default rewrite.
const FIXED: &str = "| A | B |\n| 1 | 2 |\n";

/// Runs `mdtablefix --in-place` on `path`.
fn in_place(path: &std::path::Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("mdtablefix")
        .expect("failed to create cargo command for mdtablefix")
        .arg("--in-place")
        .arg(path)
        .assert()
}

/// Lists the sorted names of the entries in `path`.
fn entry_names(path: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(path)
        .expect("read directory")
        .map(|entry| {
            entry
                .expect("read directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn in_place_rewrites_and_leaves_no_temporary_file() {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("sample.md");
    fs::write(&target, BROKEN).expect("write fixture");

    in_place(&target).success().stdout("").stderr("");

    assert_eq!(fs::read_to_string(&target).expect("read target"), FIXED);
    assert_eq!(
        entry_names(dir.path()),
        vec!["sample.md"],
        "a successful rewrite must leave no temporary file behind"
    );
}

#[cfg(unix)]
#[test]
fn in_place_preserves_file_mode() {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("sample.md");
    fs::write(&target, BROKEN).expect("write fixture");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).expect("set mode");

    in_place(&target).success();

    let mode = fs::metadata(&target)
        .expect("read metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o640, "the swap must preserve the original mode");
}

#[cfg(unix)]
#[test]
fn in_place_replaces_read_only_file() {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("sample.md");
    fs::write(&target, BROKEN).expect("write fixture");
    // The atomic swap needs write permission on the directory, not the file.
    fs::set_permissions(&target, fs::Permissions::from_mode(0o444)).expect("set mode");

    in_place(&target).success().stderr("");

    assert_eq!(fs::read_to_string(&target).expect("read target"), FIXED);
    let mode = fs::metadata(&target)
        .expect("read metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o444, "the read-only mode must survive the swap");
}

#[cfg(unix)]
#[test]
fn in_place_failure_leaves_original_byte_identical() {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("sample.md");
    fs::write(&target, BROKEN).expect("write fixture");
    let root = dir.path().to_path_buf();
    // A read-only directory denies the temporary file while leaving the target
    // itself readable.
    fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).expect("make read-only");

    let assert = in_place(&target);

    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).expect("restore mode");
    // SAFETY: `geteuid()` has no side effects and is safe to call in tests.
    if unsafe { libc::geteuid() } == 0 {
        // Root ignores directory permission bits, so the failure path cannot
        // be induced and the assertions below would be vacuous.
        return;
    }
    assert.failure().stderr(contains("sample.md"));
    assert_eq!(
        fs::read_to_string(&target).expect("read target"),
        BROKEN,
        "a failed rewrite must leave the original byte-identical"
    );
    assert_eq!(
        entry_names(&root),
        vec!["sample.md"],
        "a failed rewrite must leave no temporary file behind"
    );
}

#[cfg(unix)]
#[test]
fn in_place_declines_symlinked_target() {
    let dir = tempdir().expect("create temporary directory");
    let real = dir.path().join("real.md");
    let link = dir.path().join("link.md");
    fs::write(&real, BROKEN).expect("write fixture");
    // A relative target keeps the link resolvable inside the directory
    // capability that the CLI opens for the link's parent.
    std::os::unix::fs::symlink("real.md", &link).expect("create symlink");

    in_place(&link).failure().stderr(contains("symlink"));

    assert_eq!(
        fs::read_to_string(&real).expect("read real file"),
        BROKEN,
        "declining a symlink must leave its target untouched"
    );
    assert!(
        fs::symlink_metadata(&link)
            .expect("read link metadata")
            .file_type()
            .is_symlink(),
        "the symlink itself must survive"
    );
    assert_eq!(entry_names(dir.path()), vec!["link.md", "real.md"]);
}
