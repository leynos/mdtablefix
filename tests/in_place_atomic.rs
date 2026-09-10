//! Integration tests for the atomic `--in-place` write contract.
//!
//! Each test drives the real binary and asserts on the observable contract: a
//! successful rewrite preserves the target's mode and leaves no temporary file
//! behind, a failed rewrite leaves the original byte-identical, and a symlinked
//! target is declined rather than replaced by a regular file.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{fmt::Write as _, fs};

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

/// A table whose rewrite is far larger than the file-size cap the write-failure
/// test imposes on the child.
fn large_table() -> String {
    let mut text = String::from("|Name|Value|\n|--|--|\n");
    for row in 0..200 {
        let _ = writeln!(text, "|name-{row}|value-{row}|");
    }
    text
}

#[cfg(unix)]
#[test]
fn in_place_write_failure_leaves_original_byte_identical() {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("sample.md");
    let original = large_table();
    fs::write(&target, &original).expect("write fixture");
    // `ulimit -f` caps regular-file writes for the shell and everything it
    // execs; the ignored `SIGXFSZ` turns the overrun into `EFBIG` rather than
    // killing the process. Both are set before `exec`, so nothing runs between
    // fork and exec but `exec` itself. `LLVM_PROFILE_FILE` sends the child's
    // coverage profile to the null device: the profile outgrows the cap, and a
    // truncated profile fails `cargo llvm-cov`'s merge instead of this test.
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("trap '' XFSZ; ulimit -f 1; exec \"$1\" --in-place \"$2\"")
        .arg("sh")
        .arg(env!("CARGO_BIN_EXE_mdtablefix"))
        .arg(&target)
        .env("LLVM_PROFILE_FILE", "/dev/null")
        .output()
        .expect("run mdtablefix through sh");

    assert!(
        !output.status.success(),
        "a failed write must exit non-zero: {output:?}"
    );
    assert_eq!(
        fs::read_to_string(&target).expect("read target"),
        original,
        "a failed write must leave the original byte-identical"
    );
    assert_eq!(
        entry_names(dir.path()),
        vec!["sample.md"],
        "a failed write must leave no temporary file behind"
    );
}

#[cfg(unix)]
#[test]
fn in_place_fails_when_every_candidate_name_is_occupied() {
    let dir = tempdir().expect("create temporary directory");
    let target_dir = dir.path().join("sub");
    fs::create_dir(&target_dir).expect("create target directory");
    let target = target_dir.join("sample.md");
    fs::write(&target, BROKEN).expect("write fixture");
    // Every candidate name in the target's own directory is occupied, while
    // the working directory holds none. A run that allocated its temporary
    // file anywhere else — the working directory, or the system temporary
    // directory — would succeed, so the failure proves the candidates are
    // probed in the target's directory. `$$` is the pid that `exec` hands to
    // the binary, so the shell can predict the names it must occupy.
    let script = concat!(
        "n=0; while [ $n -le 15 ]; do ",
        ": > \"sub/sample.md.mdtablefix-$$-$n.tmp\"; ",
        "n=$((n + 1)); done; ",
        "exec \"$1\" --in-place sub/sample.md",
    );
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .arg("sh")
        .arg(env!("CARGO_BIN_EXE_mdtablefix"))
        .current_dir(dir.path())
        .output()
        .expect("run mdtablefix through sh");

    assert!(
        !output.status.success(),
        "an occupied candidate name must not divert the temporary file: {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("writing sub/sample.md"),
        "the file context must name the path as the user wrote it: {output:?}"
    );
    assert!(
        stderr.contains("Caused by:"),
        "the whole error chain must be reported, not only the file context: {output:?}"
    );
    assert!(
        stderr.contains("no free temporary file name beside sample.md"),
        "the underlying cause must reach the user: {output:?}"
    );
    assert_eq!(
        fs::read_to_string(&target).expect("read target"),
        BROKEN,
        "a failed rewrite must leave the original byte-identical"
    );
    assert_eq!(
        entry_names(&target_dir).len(),
        17,
        "the target and the occupied names must survive untouched"
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
