//! Integration tests for the atomic write contract of the public helpers.
//!
//! `rewrite` and `rewrite_no_wrap` are the library's whole-file entry points.
//! Each test drives them as a caller would and asserts the observable contract:
//! a read-only target in a writable directory is replaced with its read-only
//! attribute preserved, and a failure part-way through the replacement leaves
//! the original byte-identical with no temporary file behind.
//!
//! The read-only case is the platform-sensitive one: Windows marks the
//! destination `FILE_ATTRIBUTE_READONLY`, which `MoveFileExW` may refuse to
//! replace, so the test runs on both platforms rather than only where the mode
//! bits make it easy.

#[cfg(unix)]
use std::{fmt::Write as _, os::unix::fs::PermissionsExt, path::PathBuf};
use std::{fs, path::Path};

use mdtablefix::{rewrite, rewrite_no_wrap};
use rstest::rstest;
use tempfile::tempdir;

/// A table that needs reflowing.
const BROKEN: &str = "|A|B|\n|1|2|\n";

/// The same table after a default rewrite.
const FIXED: &str = "| A | B |\n| 1 | 2 |\n";

/// Environment variable naming the entry point the child must exercise.
#[cfg(unix)]
const CHILD_OPERATION: &str = "MDTABLEFIX_WRITE_FAILURE_OPERATION";

/// Environment variable naming the file the child must rewrite.
#[cfg(unix)]
const CHILD_TARGET: &str = "MDTABLEFIX_WRITE_FAILURE_TARGET";

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

/// Marks `path` read-only the way its platform records it.
///
/// Unix keeps mode bits, which are set exactly so the umask cannot weaken the
/// assertion; Windows keeps a read-only attribute, which is what
/// [`std::fs::Permissions`] exposes there.
fn set_read_only(path: &Path) {
    let mut permissions = fs::metadata(path).expect("read metadata").permissions();
    #[cfg(unix)]
    permissions.set_mode(0o444);
    #[cfg(not(unix))]
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).expect("make the target read-only");
}

/// Asserts that the replacement left `path` read-only.
fn assert_read_only(path: &Path) {
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

/// A table whose rewrite is far larger than the file-size cap the
/// write-failure test imposes on its child.
#[cfg(unix)]
fn large_table() -> String {
    let mut text = String::from("|Name|Value|\n|--|--|\n");
    for row in 0..200 {
        let _ = writeln!(text, "|name-{row}|value-{row}|");
    }
    text
}

/// Rewrites `path` with the public helper named by `operation`.
#[cfg(unix)]
fn rewrite_with(operation: &str, path: &Path) -> std::io::Result<()> {
    match operation {
        "rewrite" => rewrite(path),
        "rewrite_no_wrap" => rewrite_no_wrap(path),
        other => panic!("unknown rewrite operation {other}"),
    }
}

/// A read-only destination is replaced rather than refused, and stays
/// read-only.
///
/// The atomic swap needs write permission on the directory, not on the file, so
/// the replacement must not be blocked by the target's own read-only state.
/// Windows is the interesting case: the destination carries
/// `FILE_ATTRIBUTE_READONLY`, which the rename must ignore while preserving it
/// on the file that replaces the target.
#[rstest]
#[case(rewrite)]
#[case(rewrite_no_wrap)]
fn read_only_target_is_replaced(#[case] rewrite_fn: fn(&Path) -> std::io::Result<()>) {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("sample.md");
    fs::write(&target, BROKEN).expect("write fixture");
    set_read_only(&target);

    rewrite_fn(&target).expect("a read-only target must still be replaced");

    assert_eq!(fs::read_to_string(&target).expect("read target"), FIXED);
    assert_read_only(&target);
    assert_eq!(
        entry_names(dir.path()),
        vec!["sample.md"],
        "a successful rewrite must leave no temporary file behind"
    );
}

/// The child role of `write_failure_leaves_the_original_byte_identical`: it
/// rewrites the target while a one-block file-size cap is in force, so writing
/// the temporary file must be refused part-way through.
///
/// Runs only when the parent set [`CHILD_OPERATION`]; the parent asserts on the
/// file this process leaves behind.
#[cfg(unix)]
#[test]
fn write_failure_child() {
    let Ok(operation) = std::env::var(CHILD_OPERATION) else {
        return;
    };
    let target = PathBuf::from(std::env::var(CHILD_TARGET).expect("the child's target path"));

    let error =
        rewrite_with(&operation, &target).expect_err("the file-size cap must refuse the write");

    assert_eq!(
        error.raw_os_error(),
        Some(libc::EFBIG),
        "the write must be refused for outgrowing the cap: {error:?}"
    );
}

/// A failed replacement must leave a regular-file target byte-identical, with
/// no temporary file to clean up later.
///
/// The failure is forced after the write starts: the child runs under
/// `ulimit -f`, so the temporary file outgrows the cap mid-write. That limit is
/// process-wide, so the rewrite runs in a child process rather than in this
/// test.
#[cfg(unix)]
#[rstest]
#[case("rewrite")]
#[case("rewrite_no_wrap")]
fn write_failure_leaves_the_original_byte_identical(#[case] operation: &str) {
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
        .arg("trap '' XFSZ; ulimit -f 1; exec \"$1\" --exact write_failure_child --nocapture")
        .arg("sh")
        .arg(std::env::current_exe().expect("path to this test binary"))
        .env(CHILD_OPERATION, operation)
        .env(CHILD_TARGET, &target)
        .env("LLVM_PROFILE_FILE", "/dev/null")
        .output()
        .expect("run the rewrite in a child under a file-size limit");

    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child must run the write-failure test and no other: {output:?}"
    );
    assert!(
        output.status.success(),
        "the child must observe a refused write and exit cleanly: {output:?}"
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
