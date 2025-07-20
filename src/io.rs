//! File helpers for rewriting Markdown documents.

use std::{fs, path::Path};

use crate::process::{process_stream, process_stream_no_wrap};

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
    fs::write(path, output)
}

/// Rewrite a file in place with wrapped tables.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite(path: &Path) -> std::io::Result<()> { rewrite_with(path, process_stream) }

/// Rewrite a file in place without wrapping text.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite_no_wrap(path: &Path) -> std::io::Result<()> {
    rewrite_with(path, process_stream_no_wrap)
}

#[cfg(test)]
mod tests {
    use std::fs::Permissions;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    use libc;
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

    #[test]
    fn rewrite_missing_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("missing.md");
        let err = rewrite(&file).expect_err("expected error for missing file");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn rewrite_permission_denied() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("deny.md");
        fs::write(&file, "data").unwrap();
        fs::set_permissions(&file, Permissions::from_mode(0o444)).unwrap();
        let result = rewrite(&file);
        #[cfg(unix)]
        if unsafe { libc::geteuid() } == 0 {
            assert!(result.is_ok());
        } else {
            let err = result.expect_err("expected permission denied error");
            assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        }
        #[cfg(not(unix))]
        {
            let err = result.expect_err("expected permission denied error");
            assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        }
    }

    #[test]
    fn rewrite_no_wrap_missing_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("missing.md");
        let err = rewrite_no_wrap(&file).expect_err("expected error for missing file");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn rewrite_no_wrap_permission_denied() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("deny.md");
        fs::write(&file, "data").unwrap();
        fs::set_permissions(&file, Permissions::from_mode(0o444)).unwrap();
        let result = rewrite_no_wrap(&file);
        #[cfg(unix)]
        if unsafe { libc::geteuid() } == 0 {
            assert!(result.is_ok());
        } else {
            let err = result.expect_err("expected permission denied error");
            assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        }
        #[cfg(not(unix))]
        {
            let err = result.expect_err("expected permission denied error");
            assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        }
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
