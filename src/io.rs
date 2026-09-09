//! File helpers for rewriting Markdown documents.
//!
//! Rewrites replace the target through a temporary file in the same directory
//! and a rename, so a failure part-way through leaves the original file intact
//! rather than truncated. Every filesystem operation runs through a `cap_std`
//! directory capability: [`rewrite`] and [`rewrite_no_wrap`] open one for the
//! target's parent, and the CLI passes the capability it already holds.

use std::{
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, File, OpenOptions, Permissions},
};
use tracing::{debug, trace};

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
    let (directory, name) = open_parent(path)?;
    let text = directory.read_to_string(&name)?;
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let fixed = f(&lines);
    let output = if fixed.is_empty() {
        String::new()
    } else {
        fixed.join("\n") + "\n"
    };
    replace_file(&directory, &name, &output)
}

/// Opens a directory capability for the parent of `path`.
///
/// This is the library's only ambient filesystem entry point; every later
/// operation runs through the returned capability. The file name is returned
/// separately so callers address the target relative to that capability.
fn open_parent(path: &Path) -> io::Result<(Dir, Utf8PathBuf)> {
    let path = Utf8Path::from_path(path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path is not valid UTF-8: {}", path.display()),
        )
    })?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
        .unwrap_or(Utf8Path::new("."));
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("no file name in {path}"),
        )
    })?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    Ok((directory, Utf8PathBuf::from(name)))
}

/// Atomically replaces `path` inside `directory` with `contents`.
///
/// The replacement is written to a temporary file beside the target and
/// renamed over it, so the swap is atomic on POSIX filesystems and a failure
/// before the rename leaves the original file untouched. A freshly created
/// temporary file does not inherit the target's permissions, so they are
/// copied across before the swap. Any failure after the temporary file exists
/// triggers a best-effort attempt to remove it.
///
/// Symbolic links are declined rather than replaced: the rename would swap the
/// link entry itself for a regular file and leave the real file untouched.
///
/// # Errors
/// Returns an error if the target cannot be inspected, if the temporary file
/// cannot be written, or if the rename fails.
#[tracing::instrument(level = "debug", skip(directory, contents), fields(path = %path))]
pub fn replace_file(directory: &Dir, path: &Utf8Path, contents: &str) -> io::Result<()> {
    let metadata = directory.symlink_metadata(path).inspect_err(|error| {
        debug!(error_category = ?error.kind(), "replacement failed");
    })?;
    if metadata.file_type().is_symlink() {
        debug!(error_category = "symlink_target", "rewrite declined");
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("refusing to replace the symlink {path}; rewrite its target instead"),
        ));
    }
    trace!("target metadata read");
    let permissions = metadata.permissions();
    let (temp_path, file) = create_temporary_file(directory, path).inspect_err(|error| {
        debug!(error_category = ?error.kind(), "replacement failed");
    })?;
    let outcome = write_and_swap(directory, &temp_path, path, contents, permissions, file);
    if let Err(error) = &outcome {
        debug!(error_category = ?error.kind(), "replacement failed");
        // Best effort: failing to clean up must not mask the original error,
        // and the next run retries past any stale name it finds.
        if directory.remove_file(&temp_path).is_ok() {
            trace!("temporary file removed after failure");
        }
    }
    outcome
}

/// Writes `contents` to `temp_path`, copies `permissions` across, and renames
/// the result over `path`.
fn write_and_swap(
    directory: &Dir,
    temp_path: &Utf8Path,
    path: &Utf8Path,
    contents: &str,
    permissions: Permissions,
    mut file: File,
) -> io::Result<()> {
    file.write_all(contents.as_bytes())?;
    file.flush()?;
    debug!(bytes = contents.len(), "temporary file written");
    file.sync_all()?;
    debug!("temporary file synced");
    // Close the handle before renaming: Windows refuses to replace a
    // destination that another handle holds open without delete sharing.
    drop(file);
    directory.set_permissions(temp_path, permissions)?;
    debug!("target mode applied");
    directory.rename(temp_path, directory, path)?;
    debug!("target replaced");
    Ok(())
}

/// Creates a new temporary file beside `path` inside `directory`.
///
/// The name carries the process id and a per-process counter, so concurrent
/// writers in the same directory never collide. A temporary file left behind by
/// a killed run can still occupy a candidate name, so the counter advances and
/// the attempt is retried.
fn create_temporary_file(directory: &Dir, path: &Utf8Path) -> io::Result<(Utf8PathBuf, File)> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    for attempt in 0..TEMP_FILE_ATTEMPTS {
        let temp_path = temporary_path(path);
        match directory.open_with(&temp_path, &options) {
            Ok(file) => {
                debug!(attempt, "temporary file created");
                return Ok((temp_path, file));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                trace!(
                    attempt,
                    reason = "already_exists",
                    "temporary name rejected"
                );
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("no free temporary file name beside {path} after {TEMP_FILE_ATTEMPTS} attempts"),
    ))
}

/// Builds a candidate temporary path beside `path`.
///
/// Any parent components are preserved, so the temporary file always lands in
/// the target's own directory and the rename never crosses a directory.
fn temporary_path(path: &Utf8Path) -> Utf8PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!(
        "{}.mdtablefix-{}-{counter}.tmp",
        path.file_name().unwrap_or_default(),
        std::process::id()
    );
    path.with_file_name(name)
}

/// Rewrite a file in place with wrapped tables.
///
/// The replacement is written to a temporary file beside `path` and renamed
/// over it, so a failure before the rename leaves the original file intact. The
/// original file mode is preserved. Symbolic links are declined.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite(path: &Path) -> std::io::Result<()> { rewrite_with(path, process_stream) }

/// Rewrite a file in place without wrapping text.
///
/// The replacement is written to a temporary file beside `path` and renamed
/// over it, so a failure before the rename leaves the original file intact. The
/// original file mode is preserved. Symbolic links are declined.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite_no_wrap(path: &Path) -> std::io::Result<()> {
    rewrite_with(path, process_stream_no_wrap)
}

#[cfg(test)]
#[path = "io_tracing_tests.rs"]
mod tracing_tests;

#[cfg(test)]
#[path = "io_tests.rs"]
mod tests;
