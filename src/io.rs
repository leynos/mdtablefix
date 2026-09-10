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
    sync::OnceLock,
    time::Instant,
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, File, OpenOptions, Permissions},
};
use metrics::{Unit, counter, describe_counter, describe_histogram, histogram};
use tracing::{debug, trace};

use crate::process::{process_stream, process_stream_no_wrap};

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

/// Declares the metrics emitted by the replacement path.
///
/// The descriptions are registered once per process rather than per
/// replacement: they never change, and the work is not worth repeating on the
/// path that exists to write a file.
fn describe_metrics() {
    static DESCRIPTION: OnceLock<()> = OnceLock::new();
    DESCRIPTION.get_or_init(register_metrics);
}

/// Registers the descriptions of every metric the replacement path emits.
///
/// Every metric name and label value is fixed, so a recorder's cardinality
/// stays bounded: no path, file name, or error text is used as a label. The
/// library emits these metrics but never installs a recorder; a host
/// application installs one, as documented in the developers' guide. The tests
/// call this directly, so their assertions on the declared unit and description
/// do not depend on which test warmed [`describe_metrics`]'s `OnceLock`.
fn register_metrics() {
    describe_counter!(
        "mdtablefix_io_replace_total",
        "Atomic file replacements attempted, by outcome"
    );
    describe_histogram!(
        "mdtablefix_io_replace_duration_seconds",
        Unit::Seconds,
        "Duration of atomic file replacements, by outcome"
    );
    describe_counter!(
        "mdtablefix_io_temporary_name_collisions_total",
        "Temporary names rejected because they were already taken"
    );
    describe_counter!(
        "mdtablefix_io_temporary_name_exhausted_total",
        "Replacements abandoned because every candidate temporary name was taken"
    );
}

/// Atomically replaces `path` inside `directory` with `contents`.
///
/// The replacement is written to a temporary file beside the target and
/// renamed over it, so the swap is atomic on POSIX filesystems and a failure
/// before the rename leaves the original file untouched. A freshly created
/// temporary file does not inherit the target's permissions, so the target's
/// permissions are preserved across the swap; [`swap_into_place`] holds the
/// per-platform step. Any failure after the temporary file exists triggers a
/// best-effort attempt to remove it.
///
/// Symbolic links are declined rather than replaced: the rename would swap the
/// link entry itself for a regular file and leave the real file untouched.
///
/// # Errors
/// Returns an error if the target cannot be inspected, if the temporary file
/// cannot be written, or if the rename fails.
#[tracing::instrument(level = "debug", skip(directory, contents), fields(path = %path))]
pub fn replace_file(directory: &Dir, path: &Utf8Path, contents: &str) -> io::Result<()> {
    describe_metrics();
    let started = Instant::now();
    let outcome = replace_file_inner(directory, path, contents);
    let result = if outcome.is_ok() {
        "success"
    } else {
        "failure"
    };
    counter!("mdtablefix_io_replace_total", "outcome" => result).increment(1);
    // The duration is recorded for failures too, so a replacement that stalls
    // before it fails is visible rather than missing from the distribution.
    histogram!(
        "mdtablefix_io_replace_duration_seconds",
        "outcome" => result
    )
    .record(started.elapsed().as_secs_f64());
    outcome
}

/// Performs the replacement that [`replace_file`] reports the outcome of.
fn replace_file_inner(directory: &Dir, path: &Utf8Path, contents: &str) -> io::Result<()> {
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
        match directory.remove_file(&temp_path) {
            Ok(()) => trace!("temporary file removed after failure"),
            Err(cleanup_error) => {
                debug!(
                    error_category = ?cleanup_error.kind(),
                    "temporary file cleanup failed"
                );
            }
        }
    }
    outcome
}

/// Writes `contents` to `temp_path`, applies `permissions`, and renames the
/// result over `path`.
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
    swap_into_place(directory, temp_path, path, permissions)
}

/// Applies `permissions` and renames `temp_path` over `path`.
///
/// The mode is set on the temporary file before the rename, so the file never
/// exists under its final name with permissions the target never had.
#[cfg(not(windows))]
fn swap_into_place(
    directory: &Dir,
    temp_path: &Utf8Path,
    path: &Utf8Path,
    permissions: Permissions,
) -> io::Result<()> {
    directory.set_permissions(temp_path, permissions)?;
    debug!("target mode applied");
    directory.rename(temp_path, directory, path)?;
    debug!("target replaced");
    Ok(())
}

/// Applies `permissions` around the rename that puts `temp_path` in place.
///
/// Windows needs the mode on either side of the rename rather than on the
/// temporary file. A destination carrying `FILE_ATTRIBUTE_READONLY` cannot be
/// replaced: the rename fails with `ERROR_ACCESS_DENIED`, and `std`'s retry
/// through `FileRenameInfoEx` does not lift the attribute, because its flags
/// have no counterpart to the
/// `FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE` that the delete path
/// relies on. The attribute is cleared on the destination before the rename and
/// reapplied to the result afterwards, while the temporary file stays writable
/// until the rename has moved it. A failed rename has the attribute restored on
/// a best-effort basis, so the target is left writable only when the mode cannot
/// be put back at all, which is reported as a failure.
#[cfg(windows)]
fn swap_into_place(
    directory: &Dir,
    temp_path: &Utf8Path,
    path: &Utf8Path,
    permissions: Permissions,
) -> io::Result<()> {
    let read_only = permissions.readonly();
    if read_only {
        let mut writable = permissions.clone();
        writable.set_readonly(false);
        directory.set_permissions(path, writable)?;
        debug!("destination read-only attribute cleared");
    }
    match directory.rename(temp_path, directory, path) {
        Ok(()) => {
            debug!("target replaced");
            if read_only {
                directory.set_permissions(path, permissions)?;
                debug!("target mode applied");
            }
            Ok(())
        }
        Err(error) => {
            if read_only {
                // Best effort: a replacement that failed must not leave the
                // target writable as its only lasting effect.
                if let Err(restore) = directory.set_permissions(path, permissions) {
                    debug!(
                        error_category = ?restore.kind(),
                        "destination mode restore failed"
                    );
                }
            }
            Err(error)
        }
    }
}

/// Creates a new temporary file beside `path` inside `directory`.
///
/// The name carries the process id and the attempt number, and the file is
/// created exclusively, so a temporary file left behind by a killed run only
/// costs one retry: the attempt advances and the next candidate is tried.
fn create_temporary_file(directory: &Dir, path: &Utf8Path) -> io::Result<(Utf8PathBuf, File)> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    for attempt in 0..TEMP_FILE_ATTEMPTS {
        let temp_path = temporary_path(path, attempt);
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
                counter!("mdtablefix_io_temporary_name_collisions_total").increment(1);
            }
            Err(error) => return Err(error),
        }
    }
    counter!("mdtablefix_io_temporary_name_exhausted_total").increment(1);
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("no free temporary file name beside {path} after {TEMP_FILE_ATTEMPTS} attempts"),
    ))
}

/// Builds candidate temporary path `attempt` beside `path`.
///
/// The name is a pure function of the target, the process id, and the attempt,
/// so a caller can predict and occupy a candidate without reaching into the
/// process. Any parent components are preserved, so the temporary file always
/// lands in the target's own directory and the rename never crosses a
/// directory.
fn temporary_path(path: &Utf8Path, attempt: u32) -> Utf8PathBuf {
    let name = format!(
        "{}.mdtablefix-{}-{attempt}.tmp",
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
#[path = "io_metrics_tests.rs"]
mod metrics_tests;

#[cfg(test)]
#[path = "io_tracing_tests.rs"]
mod tracing_tests;

#[cfg(test)]
#[path = "io_tests.rs"]
mod tests;
