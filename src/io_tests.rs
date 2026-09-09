//! Unit tests for file rewriting.

use std::{fs, path::Path};
#[cfg(unix)]
use std::{fs::Permissions, os::unix::fs::PermissionsExt};

#[cfg(unix)]
use libc;
use rstest::rstest;
use tempfile::tempdir;

use super::*;

#[test]
fn rewrite_roundtrip() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("sample.md");
    fs::write(&file, "|A|B|\n|1|2|").unwrap();
    rewrite(&file).unwrap();
    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("| A | B |"));
}

#[test]
fn rewrite_no_wrap_roundtrip() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("sample.md");
    fs::write(&file, "|A|B|\n|1|2|").unwrap();
    rewrite_no_wrap(&file).unwrap();
    let out = fs::read_to_string(&file).unwrap();
    assert_eq!(out, "| A | B |\n| 1 | 2 |\n");
}

/// Lists the sorted names of the entries in `path`.
fn entry_names(path: &Path) -> Vec<String> {
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

#[cfg(unix)]
fn can_write_as_root() -> bool {
    // SAFETY: `geteuid()` has no side effects and is safe to call in tests.
    let uid = unsafe { libc::geteuid() };
    uid == 0
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, Permissions::from_mode(mode)).expect("set permissions");
}

#[cfg(unix)]
fn assert_permission_error_or_root_success(result: std::io::Result<()>) {
    if can_write_as_root() {
        assert!(result.is_ok());
    } else {
        let err = result.expect_err("expected permission denied error");
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }
}

#[rstest]
#[case(rewrite)]
#[case(rewrite_no_wrap)]
fn missing_file_error(#[case] rewrite_fn: fn(&Path) -> std::io::Result<()>) {
    let dir = tempdir().unwrap();
    let file = dir.path().join("missing.md");
    let err = rewrite_fn(&file).expect_err("expected error for missing file");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[cfg(unix)]
#[rstest]
#[case(rewrite)]
#[case(rewrite_no_wrap)]
fn permission_denied_error(#[case] rewrite_fn: fn(&Path) -> std::io::Result<()>) {
    let dir = tempdir().unwrap();
    let file = dir.path().join("deny.md");
    fs::write(&file, "data").unwrap();
    // An unreadable file denies the read that precedes any write.
    set_mode(&file, 0o000);
    let result = rewrite_fn(&file);
    assert_permission_error_or_root_success(result);
}

#[rstest]
#[case(rewrite)]
#[case(rewrite_no_wrap)]
fn rewrite_leaves_no_temporary_file(#[case] rewrite_fn: fn(&Path) -> std::io::Result<()>) {
    let dir = tempdir().unwrap();
    let file = dir.path().join("sample.md");
    fs::write(&file, "|A|B|\n|1|2|").unwrap();

    rewrite_fn(&file).unwrap();

    assert_eq!(entry_names(dir.path()), vec!["sample.md"]);
}

#[cfg(unix)]
#[rstest]
#[case(rewrite)]
#[case(rewrite_no_wrap)]
fn rewrite_preserves_file_mode(#[case] rewrite_fn: fn(&Path) -> std::io::Result<()>) {
    let dir = tempdir().unwrap();
    let file = dir.path().join("mode.md");
    fs::write(&file, "|A|B|\n|1|2|").unwrap();
    set_mode(&file, 0o640);

    rewrite_fn(&file).unwrap();

    let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640, "rewrite must preserve the original mode");
}

#[cfg(unix)]
#[rstest]
#[case(rewrite)]
#[case(rewrite_no_wrap)]
fn write_failure_leaves_original_intact(#[case] rewrite_fn: fn(&Path) -> std::io::Result<()>) {
    let dir = tempdir().unwrap();
    let file = dir.path().join("sample.md");
    let original = "|A|B|\n|1|2|";
    fs::write(&file, original).unwrap();
    // A read-only directory denies the temporary file that the atomic swap
    // needs, while leaving the target itself readable.
    set_mode(dir.path(), 0o555);

    let result = rewrite_fn(&file);

    set_mode(dir.path(), 0o755);
    if can_write_as_root() {
        // Root ignores directory permission bits, so the failure path
        // cannot be induced and the assertions below would be vacuous.
        return;
    }
    let err = result.expect_err("expected permission denied error");
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        original,
        "a failed rewrite must leave the original byte-identical"
    );
    assert_eq!(entry_names(dir.path()), vec!["sample.md"]);
}

#[rstest]
#[case("sample.md")]
#[case("sub/dir/sample.md")]
#[case("/tmp/dir/sample.md")]
fn temporary_path_is_a_sibling(#[case] path: &str) {
    let target = Utf8Path::new(path);
    let temp = temporary_path(target);
    assert_eq!(temp.parent(), target.parent());
    assert!(
        temp.file_name().unwrap().starts_with("sample.md"),
        "temporary name should extend the target name"
    );
}

#[cfg(unix)]
#[rstest]
#[case(rewrite)]
#[case(rewrite_no_wrap)]
fn symlink_target_is_declined(#[case] rewrite_fn: fn(&Path) -> std::io::Result<()>) {
    let dir = tempdir().unwrap();
    let real = dir.path().join("real.md");
    let link = dir.path().join("link.md");
    let original = "|A|B|\n|1|2|";
    fs::write(&real, original).unwrap();
    // A relative target keeps the link resolvable inside the capability.
    std::os::unix::fs::symlink("real.md", &link).unwrap();

    let err = rewrite_fn(&link).expect_err("symlink must be declined");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        fs::read_to_string(&real).unwrap(),
        original,
        "declining a symlink must leave its target untouched"
    );
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the symlink itself must survive"
    );
    assert_eq!(entry_names(dir.path()), vec!["link.md", "real.md"]);
}

#[test]
fn failure_after_temporary_file_creation_removes_it() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.md");
    fs::create_dir(&target).unwrap();
    let root = Utf8Path::from_path(dir.path()).expect("UTF-8 temporary directory");
    let capability = Dir::open_ambient_dir(root, ambient_authority()).expect("open capability");

    // The temporary file is created, written and synced, and only then does
    // the final rename fail, because a file cannot replace a directory.
    let result = replace_file(&capability, Utf8Path::new("target.md"), "replacement");

    assert!(result.is_err(), "renaming over a directory must fail");
    assert!(
        target.is_dir(),
        "the failed replacement must leave the target alone"
    );
    assert_eq!(
        entry_names(dir.path()),
        vec!["target.md"],
        "a failure after the temporary file exists must remove it"
    );
}

#[test]
fn rewrite_empty_file_no_extra_newline() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("empty.md");
    fs::write(&file, "").unwrap();
    rewrite(&file).unwrap();
    let contents = fs::read_to_string(&file).unwrap();
    assert!(contents.is_empty());
}
