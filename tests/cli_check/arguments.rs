//! Arguments that fail resolution, before any file is analysed.

use std::fs;

use tempfile::tempdir;

use super::{CLEAN, run_in_os, status_of, stderr_of, stdout_of};

/// A path that is not valid UTF-8 fails the whole run, not just that one file.
///
/// Resolution happens before any file is analysed, so the exit status is the
/// documented error code and nothing is reported as clean: an argument the tool
/// cannot even name cannot be counted as one file's problem while its siblings
/// proceed. The other file is named first, so a run that had already started
/// analysing would show it.
#[cfg(unix)]
#[test]
fn a_non_utf8_path_argument_exits_error() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("clean.md"), CLEAN).expect("write fixture");
    let invalid = OsString::from_vec(b"bad\xff.md".to_vec());

    let output = run_in_os(
        dir.path(),
        &[
            OsString::from("--check"),
            OsString::from("clean.md"),
            invalid,
        ],
    );

    assert_eq!(
        status_of(&output),
        2,
        "a non-UTF-8 argument must exit 2, stderr: {}",
        stderr_of(&output)
    );
    assert!(
        stderr_of(&output).contains("UTF-8"),
        "the error must name the problem: {}",
        stderr_of(&output)
    );
    assert_eq!(
        stdout_of(&output),
        "",
        "no file may be reported from a run that could not resolve its inputs"
    );
}
