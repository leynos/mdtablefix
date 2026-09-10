//! Unit and property tests for the binary's file-output contracts.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use mdtablefix::{LineEnding, io::replace_file};
use proptest::prelude::*;
use rstest::{fixture, rstest};
use tempfile::tempdir;

use super::{
    FormatOpts,
    format_to_string,
    open_file_parent,
    render_stdin_output,
    rewrite_in_place,
};

/// Format options with every transformation disabled.
#[fixture]
fn no_opts() -> FormatOpts {
    FormatOpts {
        wrap: false,
        renumber: false,
        breaks: false,
        ellipsis: false,
        fences: false,
        footnotes: false,
        code_emphasis: false,
        headings: false,
    }
}

/// Opens a directory capability on an ambient path, mirroring the CLI's
/// only filesystem boundary.
fn open_dir(path: &std::path::Path) -> std::io::Result<Dir> {
    let utf8 = Utf8PathBuf::from_path_buf(path.to_path_buf())
        .map_err(|path| std::io::Error::other(format!("non-UTF-8 path: {}", path.display())))?;
    Dir::open_ambient_dir(&utf8, ambient_authority())
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

#[cfg(unix)]
fn can_write_as_root() -> bool {
    // SAFETY: `geteuid()` has no side effects and is safe to call in tests.
    let uid = unsafe { libc::geteuid() };
    uid == 0
}

#[cfg(unix)]
fn set_mode(path: &std::path::Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set permissions");
}

#[rstest]
fn rewrite_in_place_leaves_no_temporary_file(no_opts: FormatOpts) {
    let dir = tempdir().expect("create temporary directory");
    let path = Utf8Path::new("sample.md");
    let directory = open_dir(dir.path()).expect("open directory capability");
    directory
        .write(path, "|A|B|\n|1|2|")
        .expect("write fixture");

    rewrite_in_place(&directory, path, no_opts).expect("rewrite in place");

    assert_eq!(entry_names(dir.path()), vec!["sample.md"]);
}

#[cfg(unix)]
#[rstest]
fn rewrite_in_place_preserves_file_mode(no_opts: FormatOpts) {
    let dir = tempdir().expect("create temporary directory");
    let path = Utf8Path::new("sample.md");
    let absolute = dir.path().join(path.as_str());
    let directory = open_dir(dir.path()).expect("open directory capability");
    directory
        .write(path, "|A|B|\n|1|2|")
        .expect("write fixture");
    set_mode(&absolute, 0o640);

    rewrite_in_place(&directory, path, no_opts).expect("rewrite in place");

    let mode = fs::metadata(&absolute)
        .expect("read metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o640, "rewrite must preserve the original mode");
}

#[cfg(unix)]
#[rstest]
fn rewrite_in_place_keeps_original_when_write_fails(no_opts: FormatOpts) {
    let dir = tempdir().expect("create temporary directory");
    let path = Utf8Path::new("sample.md");
    let absolute = dir.path().join(path.as_str());
    let original = "|A|B|\n|1|2|";
    let directory = open_dir(dir.path()).expect("open directory capability");
    directory.write(path, original).expect("write fixture");
    // A read-only directory blocks the temporary file, which is the point
    // at which the replacement would otherwise begin.
    set_mode(dir.path(), 0o555);

    let result = rewrite_in_place(&directory, path, no_opts);

    set_mode(dir.path(), 0o755);
    if can_write_as_root() {
        // Root ignores directory permission bits, so the failure path
        // cannot be induced and the assertions below would be vacuous.
        return;
    }
    assert!(result.is_err(), "expected the write to fail");
    assert_eq!(
        fs::read_to_string(&absolute).expect("read original"),
        original,
        "a failed rewrite must leave the original byte-identical"
    );
    assert_eq!(entry_names(dir.path()), vec!["sample.md"]);
}

#[cfg(unix)]
#[rstest]
fn rewrite_in_place_declines_symlinked_target(no_opts: FormatOpts) {
    let dir = tempdir().expect("create temporary directory");
    let real = dir.path().join("real.md");
    let link = dir.path().join("link.md");
    let original = "|A|B|\n|1|2|";
    fs::write(&real, original).expect("write fixture");
    // A relative target keeps the link resolvable inside the capability.
    std::os::unix::fs::symlink("real.md", &link).expect("create symlink");
    let (directory, name) = open_file_parent(&link).expect("open the CLI's directory capability");

    let err = rewrite_in_place(&directory, &name, no_opts).expect_err("symlink must be declined");

    let message = format!("{err:?}");
    assert!(message.contains("symlink"), "unexpected error: {message}");
    assert_eq!(
        fs::read_to_string(&real).expect("read real file"),
        original,
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

#[test]
fn capability_scoped_failure_removes_temporary_file() {
    let dir = tempdir().expect("create temporary directory");
    let target = dir.path().join("target.md");
    fs::create_dir(&target).expect("create target directory");
    let (directory, name) = open_file_parent(&target).expect("open the CLI's directory capability");

    // The temporary file is created, written and synced, and only then does
    // the final rename fail, because a file cannot replace a directory.
    let result = replace_file(&directory, &name, "replacement");

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
fn stdin_output_keeps_its_terminator_contract() {
    assert_eq!(render_stdin_output(&[], LineEnding::Lf), "\n");
    assert_eq!(render_stdin_output(&[], LineEnding::Crlf), "\r\n");
    let lines = vec!["| A | B |".to_string()];
    assert_eq!(
        render_stdin_output(&lines, LineEnding::Crlf),
        "| A | B |\r\n"
    );
}

fn prose_word_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just("alpha".to_string()),
            Just("beta".to_string()),
            Just("gamma".to_string()),
            Just("delta".to_string()),
            Just("evidence".to_string()),
            Just("formatting".to_string()),
        ],
        1..20,
    )
    .prop_map(|words| words.join(" "))
}

proptest! {
    #[test]
    fn formatting_matches_in_place_output(
        prose in prose_word_strategy(),
        table_cell in prose_word_strategy(),
    ) {
        let input = format!(
            "{prose}\n\n| Name | Notes |\n|---|---|\n| {table_cell} | value |\n"
        );
        let directory = tempdir()
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let directory = open_dir(directory.path())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let formatted_path = Utf8Path::new("formatted.md");
        let rewritten_path = Utf8Path::new("rewritten.md");
        directory.write(formatted_path, &input)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        directory.write(rewritten_path, input)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let formatted = format_to_string(&directory, formatted_path, no_opts())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        rewrite_in_place(&directory, rewritten_path, no_opts())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let rewritten = directory.read_to_string(rewritten_path)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(formatted, rewritten);
    }

    #[test]
    fn replace_file_leaves_only_the_target(contents in prose_word_strategy()) {
        let dir = tempdir().map_err(|error| TestCaseError::fail(error.to_string()))?;
        let directory = open_dir(dir.path())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let path = Utf8Path::new("sample.md");
        directory
            .write(path, "old contents")
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        replace_file(&directory, path, &contents)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let written = directory
            .read_to_string(path)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(written, contents);
        prop_assert_eq!(entry_names(dir.path()), vec!["sample.md".to_string()]);
    }

    /// A failure after the temporary file exists must remove it, whatever
    /// the replacement contents. A directory target makes the final rename
    /// fail deterministically on every platform.
    #[test]
    fn replace_file_failure_leaves_no_residue(contents in prose_word_strategy()) {
        let dir = tempdir().map_err(|error| TestCaseError::fail(error.to_string()))?;
        let directory = open_dir(dir.path())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let path = Utf8Path::new("target.md");
        fs::create_dir(dir.path().join("target.md"))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let result = replace_file(&directory, path, &contents);

        prop_assert!(result.is_err(), "renaming over a directory must fail");
        prop_assert!(dir.path().join("target.md").is_dir());
        prop_assert_eq!(entry_names(dir.path()), vec!["target.md".to_string()]);
    }
}
