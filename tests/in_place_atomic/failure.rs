//! The failure paths of the atomic rewrite.
//!
//! Each case induces the failure through a Unix facility — a read-only
//! directory, a file-size rlimit, an occupied candidate name — so the module is
//! declared `#[cfg(unix)]` in its parent and the tests need no guard of their
//! own. What they assert is the same in every case: the original bytes survive
//! and no temporary file is left behind.

use std::{fmt::Write as _, fs, os::unix::fs::PermissionsExt};

use predicates::str::contains;
use tempfile::tempdir;

use super::{BROKEN, entry_names, in_place};

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
#[cfg(unix)]
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
