//! Integration tests for the atomic `--in-place` write contract.
//!
//! Each test drives the real binary and asserts on the observable contract: a
//! successful rewrite preserves the target's mode and leaves no temporary file
//! behind, a failed rewrite leaves the original byte-identical, and a symlinked
//! target is declined rather than replaced by a regular file. A file that is
//! already formatted is not rewritten at all, so a symlink to one is not
//! declined — there is nothing to decline.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use assert_cmd::Command;
#[cfg(unix)]
use predicates::str::contains;
use tempfile::tempdir;

// The failure paths live in a Unix-only module: each induces its failure
// through a Unix facility, so on other platforms there is nothing to run.
#[cfg(unix)]
#[path = "in_place_atomic/failure.rs"]
mod failure;

/// A table that needs reflowing.
const BROKEN: &str = "|A|B|\n|1|2|\n";

/// The same table after a default rewrite.
const FIXED: &str = "| A | B |\n| 1 | 2 |\n";

/// Runs `mdtablefix --in-place` on `path`.
fn in_place(path: &std::path::Path) -> assert_cmd::assert::Assert { in_place_all(&[path]) }

/// Runs `mdtablefix --in-place` on several paths, in argument order.
fn in_place_all(paths: &[&std::path::Path]) -> assert_cmd::assert::Assert {
    let mut command =
        Command::cargo_bin("mdtablefix").expect("failed to create cargo command for mdtablefix");
    command.arg("--in-place");
    for path in paths {
        command.arg(path);
    }
    command.assert()
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

/// Marks `path` read-only the way its platform records it.
///
/// Unix keeps mode bits, which are set exactly so the umask cannot weaken the
/// assertion; Windows keeps a read-only attribute, which is what
/// [`std::fs::Permissions`] exposes there.
fn set_read_only(path: &std::path::Path) {
    let mut permissions = fs::metadata(path).expect("read metadata").permissions();
    #[cfg(unix)]
    permissions.set_mode(0o444);
    #[cfg(not(unix))]
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).expect("make the target read-only");
}

/// Asserts that the run left `path` read-only.
fn assert_read_only(path: &std::path::Path) {
    let permissions = fs::metadata(path).expect("read metadata").permissions();
    assert!(
        permissions.readonly(),
        "the read-only attribute must survive the swap"
    );
    #[cfg(unix)]
    assert_eq!(
        permissions.mode() & 0o777,
        0o444,
        "the read-only mode must survive the swap"
    );
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

/// `--in-place` replaces a read-only target rather than refusing it, and the
/// target stays read-only.
///
/// The atomic swap needs write permission on the directory, not on the file.
/// Windows is the interesting case here: the destination carries
/// `FILE_ATTRIBUTE_READONLY`, which the CLI's rename must ignore while
/// preserving it on the file that replaces the target.
#[test]
fn in_place_replaces_read_only_file() {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("sample.md");
    fs::write(&target, BROKEN).expect("write fixture");
    set_read_only(&target);

    in_place(&target).success().stderr("");

    assert_eq!(fs::read_to_string(&target).expect("read target"), FIXED);
    assert_read_only(&target);
    assert_eq!(
        entry_names(dir.path()),
        vec!["sample.md"],
        "replacing a read-only target must leave no temporary file behind"
    );
}

#[cfg(unix)]
#[test]
fn in_place_retries_past_a_stale_temporary_file() {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("sample.md");
    fs::write(&target, BROKEN).expect("write fixture");
    // The shell occupies the first candidate name using the process id that
    // `exec` hands to the binary, so the run must retry rather than reuse it.
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg("touch sample.md.mdtablefix-$$-0.tmp; exec \"$1\" --in-place sample.md")
        .arg("sh")
        .arg(env!("CARGO_BIN_EXE_mdtablefix"))
        .current_dir(dir.path())
        .status()
        .expect("run mdtablefix through sh");

    assert!(
        status.success(),
        "a stale temporary name must not fail the run"
    );
    assert_eq!(fs::read_to_string(&target).expect("read target"), FIXED);
    let names = entry_names(dir.path());
    assert_eq!(
        names.len(),
        2,
        "only the target and the stale file may remain: {names:?}"
    );
    assert_eq!(names[0], "sample.md", "unexpected entries: {names:?}");
    assert!(
        names[1].starts_with("sample.md.mdtablefix-") && names[1].ends_with("-0.tmp"),
        "the stale file must survive untouched: {names:?}"
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

    // A declined rewrite is an error like any other: the exit contract reserves
    // `2` for a file that could not be rewritten, so "non-zero" is too loose.
    in_place(&link).code(2).stderr(contains("symlink"));

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

/// A file that is already formatted is not rewritten.
///
/// The bytes written would be the bytes already there, but the file would not
/// be the same file: the replacement renames a temporary over the target, so an
/// unconditional write would swap the inode and move the modification time. A
/// build system watching this file would see a change where there was none.
#[cfg(unix)]
#[test]
fn in_place_leaves_a_clean_file_untouched() {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("clean.md");
    fs::write(&target, FIXED).expect("write fixture");
    let before = fs::metadata(&target).expect("read metadata before");

    in_place(&target).success();

    let after = fs::metadata(&target).expect("read metadata after");
    assert_eq!(
        before.ino(),
        after.ino(),
        "a clean file must not be replaced through a temporary"
    );
    assert_eq!(
        before.mtime(),
        after.mtime(),
        "a clean file's modification time must not move"
    );
    assert_eq!(
        before.mtime_nsec(),
        after.mtime_nsec(),
        "a clean file's modification time must not move"
    );
    assert_eq!(fs::read_to_string(&target).expect("read fixture"), FIXED);
    assert_eq!(entry_names(dir.path()), vec!["clean.md"]);
}

/// A clean file is not written, so a symlink to one is not declined.
///
/// Declining a symlinked target is a property of the replacement, not of the
/// run: with nothing to write there is nothing to decline, and the run succeeds
/// without touching the link or its target.
/// [`in_place_declines_symlinked_target`] pins the other half, where the target
/// does need rewriting.
#[cfg(unix)]
#[test]
fn in_place_accepts_a_symlink_to_a_clean_file() {
    let dir = tempdir().expect("create temporary directory");
    let real = dir.path().join("real.md");
    let link = dir.path().join("link.md");
    fs::write(&real, FIXED).expect("write fixture");
    std::os::unix::fs::symlink("real.md", &link).expect("create symlink");

    in_place(&link).success();

    assert_eq!(fs::read_to_string(&real).expect("read real file"), FIXED);
    assert!(
        fs::symlink_metadata(&link)
            .expect("read link metadata")
            .file_type()
            .is_symlink(),
        "the symlink itself must survive"
    );
    assert_eq!(entry_names(dir.path()), vec!["link.md", "real.md"]);
}
