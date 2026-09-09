//! File helpers for rewriting Markdown documents.
//!
//! Rewrites replace the target through a temporary file in the same directory
//! and a rename, so a failure part-way through leaves the original file intact
//! rather than truncated.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::process::{process_stream, process_stream_no_wrap};

/// Counter that keeps temporary file names unique within a process.
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Candidate temporary names to try before conceding that a stale temporary
/// file from an earlier killed run is in the way.
const TEMP_FILE_ATTEMPTS: u32 = 16;

/// Read `path`, process the contents with `f`, and write the result back.
///
/// This helper encapsulates the common pattern used by [`rewrite`] and
/// [`rewrite_no_wrap`].
///
/// # Errors
/// Returns an error if reading or writing the file fails.
fn rewrite_with<F>(path: &Path, f: F) -> std::io::Result<()>
where
    F: Fn(&[String]) -> Vec<String>,
{
    let text = fs::read_to_string(path)?;
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let fixed = f(&lines);
    let output = if fixed.is_empty() {
        String::new()
    } else {
        fixed.join("\n") + "\n"
    };
    write_atomically(path, &output)
}

/// Writes `contents` to `path` through a same-directory temporary file.
///
/// The temporary file is given the target's permissions and renamed over it,
/// so a failure before the rename leaves the original file untouched. Any
/// failure after the temporary file exists removes it again.
///
/// # Errors
/// Returns an error if the target's metadata cannot be read, if the temporary
/// file cannot be written, or if the rename fails.
fn write_atomically(path: &Path, contents: &str) -> std::io::Result<()> {
    let permissions = fs::metadata(path)?.permissions();
    let (temp_path, file) = create_temporary_file(path)?;
    let outcome = write_and_swap(&temp_path, path, contents, permissions, file);
    if outcome.is_err() {
        // Best effort: failing to clean up must not mask the original error,
        // and the next run retries past any stale name it finds.
        let _ = fs::remove_file(&temp_path);
    }
    outcome
}

/// Writes `contents` to `temp_path`, copies `permissions` across, and renames
/// the result over `path`.
fn write_and_swap(
    temp_path: &Path,
    path: &Path,
    contents: &str,
    permissions: fs::Permissions,
    mut file: fs::File,
) -> io::Result<()> {
    file.write_all(contents.as_bytes())?;
    file.flush()?;
    file.sync_all()?;
    // Close the handle before renaming: Windows refuses to replace a
    // destination that another handle holds open without delete sharing.
    drop(file);
    fs::set_permissions(temp_path, permissions)?;
    fs::rename(temp_path, path)
}

/// Creates a new temporary file beside `path`.
///
/// The name carries the process id and a per-process counter, so concurrent
/// writers in the same directory never collide. A temporary file left behind by
/// a killed run can still occupy a candidate name, so the counter advances and
/// the attempt is retried.
fn create_temporary_file(path: &Path) -> io::Result<(PathBuf, fs::File)> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    for _ in 0..TEMP_FILE_ATTEMPTS {
        let temp_path = temporary_path(path);
        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("no free temporary file name beside {}", path.display()),
    ))
}

/// Builds a candidate temporary path beside `path`.
fn temporary_path(path: &Path) -> PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".mdtablefix-{}-{counter}.tmp", std::process::id()));
    path.with_file_name(name)
}

/// Rewrite a file in place with wrapped tables.
///
/// The replacement is written to a temporary file beside `path` and renamed
/// over it, so a failure before the rename leaves the original file intact. The
/// original file mode is preserved.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite(path: &Path) -> std::io::Result<()> { rewrite_with(path, process_stream) }

/// Rewrite a file in place without wrapping text.
///
/// The replacement is written to a temporary file beside `path` and renamed
/// over it, so a failure before the rename leaves the original file intact. The
/// original file mode is preserved.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite_no_wrap(path: &Path) -> std::io::Result<()> {
    rewrite_with(path, process_stream_no_wrap)
}

#[cfg(test)]
mod tests {
    //! Unit tests for file rewriting.

    use std::path::Path;
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
    #[case("/tmp/dir/sample.md")]
    fn temporary_path_is_a_sibling(#[case] path: &str) {
        let target = Path::new(path);
        let temp = temporary_path(target);
        assert_eq!(temp.parent(), target.parent());
        assert!(
            temp.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("sample.md"),
            "temporary name should extend the target name"
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
}
