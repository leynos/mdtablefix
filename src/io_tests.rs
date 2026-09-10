//! Unit tests for file rewriting.

use std::{fs, path::Path};
#[cfg(unix)]
use std::{fs::Permissions, os::unix::fs::PermissionsExt};

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
#[case("sample.md", 0)]
#[case("sub/dir/sample.md", 1)]
#[case("/tmp/dir/sample.md", 15)]
fn temporary_path_is_a_sibling(#[case] path: &str, #[case] attempt: u32) {
    let target = Utf8Path::new(path);
    let temp = temporary_path(target, attempt);
    assert_eq!(temp.parent(), target.parent());
    let name = temp.file_name().unwrap();
    assert!(
        name.starts_with("sample.md"),
        "temporary name should extend the target name"
    );
    assert!(
        name.ends_with(&format!("-{attempt}.tmp")),
        "the attempt should be visible in the candidate name: {name}"
    );
}

#[test]
fn create_temporary_file_retries_past_an_occupied_candidate() {
    let dir = tempdir().expect("create temporary directory");
    let root = Utf8Path::from_path(dir.path()).expect("convert the temporary directory to UTF-8");
    let directory =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");
    let name = Utf8Path::new("sample.md");
    // Candidate names are a pure function of the target, the process id and
    // the attempt, so the test can occupy the first candidate exactly.
    let occupied = temporary_path(name, 0);
    let occupied_name = occupied.file_name().expect("occupied candidate file name");
    fs::write(dir.path().join(occupied_name), "").expect("occupy the first candidate name");

    let (temp, _file) =
        create_temporary_file(&directory, name).expect("retry past the occupied name");

    assert_eq!(
        temp,
        temporary_path(name, 1),
        "the next candidate must be tried"
    );
    assert_eq!(
        entry_names(dir.path()),
        vec![
            occupied_name.to_string(),
            temp.file_name()
                .expect("chosen candidate file name")
                .to_string(),
        ],
        "the occupied candidate must survive untouched"
    );
}

#[test]
fn create_temporary_file_reports_an_exhausted_name_space() {
    let dir = tempdir().expect("create temporary directory");
    let root = Utf8Path::from_path(dir.path()).expect("convert the temporary directory to UTF-8");
    let directory =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");
    let name = Utf8Path::new("sample.md");
    for attempt in 0..TEMP_FILE_ATTEMPTS {
        let candidate = temporary_path(name, attempt);
        let candidate_name = candidate.file_name().expect("candidate file name");
        fs::write(dir.path().join(candidate_name), "").expect("occupy the candidate name");
    }

    let error = create_temporary_file(&directory, name).expect_err("every candidate is occupied");

    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(
        error.to_string().contains("sample.md"),
        "the error must name the target: {error}"
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

/// Marks `path` read-only the way its platform records it.
#[cfg(unix)]
fn set_read_only(path: &Path) {
    // Set exactly, so the umask cannot weaken the assertion.
    set_mode(path, 0o444);
}

#[cfg(not(unix))]
fn set_read_only(path: &Path) {
    let mut permissions = fs::metadata(path).expect("read metadata").permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).expect("make the target read-only");
}

/// Asserts that `path` is read-only.
#[cfg(unix)]
fn assert_read_only(path: &Path) {
    let mode = fs::metadata(path)
        .expect("read metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o444, "the read-only mode must survive the failure");
}

#[cfg(not(unix))]
fn assert_read_only(path: &Path) {
    let permissions = fs::metadata(path).expect("read metadata").permissions();
    assert!(
        permissions.readonly(),
        "a failed swap must leave the read-only attribute restored"
    );
}

/// A swap that fails after the destination was prepared puts back what the
/// preparation changed.
///
/// The seam is the only way to reach the rollback: a rename that fails for a
/// reason a test can construct fails before the destination is prepared. Both
/// platforms assert the same outcome, but only Windows clears the destination's
/// read-only attribute before the rename, so only there does the rollback have
/// something to restore.
#[rstest]
#[case(rewrite)]
#[case(rewrite_no_wrap)]
fn failed_swap_restores_the_prepared_destination(
    #[case] rewrite_fn: fn(&Path) -> std::io::Result<()>,
) {
    let dir = tempdir().unwrap();
    let file = dir.path().join("sample.md");
    let original = "|A|B|\n|1|2|";
    fs::write(&file, original).unwrap();
    set_read_only(&file);

    let _armed = rename_failure_seam::arm();
    let error = rewrite_fn(&file).expect_err("the armed seam must fail the swap");

    assert!(
        error.to_string().contains("seam"),
        "the failure must be the armed seam rather than an unrelated one: {error}"
    );
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        original,
        "a failed swap must leave the original byte-identical"
    );
    assert_read_only(&file);
    assert_eq!(
        entry_names(dir.path()),
        vec!["sample.md"],
        "a failed swap must leave no temporary file behind"
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
