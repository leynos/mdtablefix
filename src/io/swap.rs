//! The atomic swap: everything between a decided replacement and the rename
//! that puts it in place.
//!
//! [`write_and_swap`] writes the temporary file and hands it to
//! [`swap_into_place`], which applies the target's permissions, prepares the
//! destination on the platforms that need it, and renames. A failure after the
//! temporary file exists is cleaned up by [`remove_temporary_file`].
//!
//! Every function here takes the directory capability the caller already holds;
//! nothing in this module opens an ambient path.

use std::io::{self, Write};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::{Dir, File, OpenOptions, Permissions};
use metrics::counter;
use tracing::{debug, trace};

/// Candidate temporary names to try before conceding that a stale temporary
/// file from an earlier killed run is in the way.
pub(super) const TEMP_FILE_ATTEMPTS: u32 = 16;

/// Writes `contents` to `temp_path`, applies `permissions`, and renames the
/// result over `path`.
pub(super) fn write_and_swap(
    directory: &Dir,
    temp_path: &Utf8Path,
    path: &Utf8Path,
    contents: &str,
    permissions: &Permissions,
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
/// The permissions go on the temporary file first, so the file never exists
/// under its final name with permissions the target never had, and a successful
/// rename leaves the new file with the target's permissions — read-only
/// included — without a second step that could fail after the swap. Only then
/// is the destination prepared, so the window in which it is writable is as
/// short as the platform allows; [`prepare_destination`] holds what only
/// Windows needs before the rename can be attempted at all, and
/// [`restore_destination`] undoes it when the swap does not complete.
fn swap_into_place(
    directory: &Dir,
    temp_path: &Utf8Path,
    path: &Utf8Path,
    permissions: &Permissions,
) -> io::Result<()> {
    directory
        .set_permissions(temp_path, permissions.clone())
        .inspect(|()| debug!("target mode applied"))?;
    let prepared = prepare_destination(directory, path, permissions)?;
    if let Err(error) = rename_over_target(directory, temp_path, path) {
        if let Some(original) = prepared {
            restore_destination(directory, path, &original);
        }
        return Err(error);
    }
    debug!("target replaced");
    Ok(())
}

/// Prepares `path` for the rename that will replace it.
///
/// Returns the permissions to put back if the swap does not complete, or `None`
/// when the platform needs no preparation. Windows is the platform that does: a
/// destination carrying `FILE_ATTRIBUTE_READONLY` cannot be replaced. The
/// rename fails with `ERROR_ACCESS_DENIED`, and the flags that
/// `SetFileInformationByHandle` accepts for a rename have no counterpart to the
/// `FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE` that the delete path relies
/// on, so the attribute itself has to be cleared for the duration of the swap,
/// through the same directory capability as every other operation here.
#[cfg(windows)]
fn prepare_destination(
    directory: &Dir,
    path: &Utf8Path,
    permissions: &Permissions,
) -> io::Result<Option<Permissions>> {
    if !permissions.readonly() {
        return Ok(None);
    }
    let mut writable = permissions.clone();
    writable.set_readonly(false);
    directory.set_permissions(path, writable)?;
    debug!("destination read-only attribute cleared");
    Ok(Some(permissions.clone()))
}

/// The preparation a platform whose rename needs none performs.
///
/// The signature matches the Windows implementation so that the swap has one
/// body. Clearing the read-only flag here would not serve the rename, and on
/// Unix it would rewrite the target's mode for the duration of the swap:
/// `set_readonly(false)` clears all three write bits, so a `0o444` file would
/// briefly become `0o666`.
#[cfg(not(windows))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature must match the Windows preparation, which can fail"
)]
fn prepare_destination(
    _directory: &Dir,
    _path: &Utf8Path,
    _permissions: &Permissions,
) -> io::Result<Option<Permissions>> {
    Ok(None)
}

/// Puts the destination's original permissions back after a failed swap.
///
/// Best effort by design: a replacement that failed must not leave the target
/// writable as its only lasting effect, and the failure to restore must not
/// mask the reason the swap failed, so it is traced rather than returned. The
/// category is the `io::ErrorKind`, which is bounded, rather than an error
/// string or a path.
#[cfg(windows)]
fn restore_destination(directory: &Dir, path: &Utf8Path, original: &Permissions) {
    if let Err(error) = directory.set_permissions(path, original.clone()) {
        debug!(
            error_category = ?error.kind(),
            "destination mode restore failed"
        );
    }
}

/// The rollback of a preparation that no platform other than Windows makes.
#[cfg(not(windows))]
fn restore_destination(_directory: &Dir, _path: &Utf8Path, _original: &Permissions) {}

/// Renames `temp_path` over `path`.
///
/// A test can arm [`rename_failure_seam`] to make this fail deterministically.
/// That seam is the only way to reach the rollback in [`swap_into_place`]: a
/// rename that a test can make fail for real fails before the destination is
/// ever prepared, so the rollback would never have anything to put back.
fn rename_over_target(directory: &Dir, temp_path: &Utf8Path, path: &Utf8Path) -> io::Result<()> {
    #[cfg(test)]
    if rename_failure_seam::take() {
        return Err(io::Error::other("the rename failure seam is armed"));
    }
    directory.rename(temp_path, directory, path)
}

/// Removes the temporary file left behind by a failed replacement.
///
/// Windows refuses to delete a file carrying `FILE_ATTRIBUTE_READONLY`, and by
/// the time the swap runs the temporary file already carries the target's
/// permissions, so the attribute is cleared first. Clearing it is best effort:
/// the removal below reports its own failure, and a name that survives is
/// retried past by the next run.
pub(super) fn remove_temporary_file(directory: &Dir, temp_path: &Utf8Path) -> io::Result<()> {
    #[cfg(test)]
    if cleanup_failure_seam::take() {
        return Err(io::Error::other("the cleanup failure seam is armed"));
    }
    #[cfg(windows)]
    {
        if let Ok(metadata) = directory.metadata(temp_path) {
            let mut permissions = metadata.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                if let Err(error) = directory.set_permissions(temp_path, permissions) {
                    debug!(
                        error_category = ?error.kind(),
                        "temporary file mode could not be cleared"
                    );
                }
            }
        }
    }
    directory.remove_file(temp_path)
}

/// Creates a new temporary file beside `path` inside `directory`.
///
/// The name carries the process id and the attempt number, and the file is
/// created exclusively, so a temporary file left behind by a killed run only
/// costs one retry: the attempt advances and the next candidate is tried.
pub(super) fn create_temporary_file(
    directory: &Dir,
    path: &Utf8Path,
) -> io::Result<(Utf8PathBuf, File)> {
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
pub(super) fn temporary_path(path: &Utf8Path, attempt: u32) -> Utf8PathBuf {
    let name = format!(
        "{}.mdtablefix-{}-{attempt}.tmp",
        path.file_name().unwrap_or_default(),
        std::process::id()
    );
    path.with_file_name(name)
}

/// A test-only seam that fails the rename half of the swap.
///
/// Every rename a test can be made to fail for real fails before the
/// destination is prepared, so the rollback in [`swap_into_place`] is
/// otherwise unreachable. The arming is per-thread, because the tests that use
/// it drive the swap on the thread that armed it, and it is undone when the
/// value [`arm`] returns is dropped, so a failing assertion cannot leave the
/// failure armed for whatever runs next on that thread.
#[cfg(test)]
pub(crate) mod rename_failure_seam {
    use std::cell::Cell;

    thread_local! {
        /// Whether this thread's next rename must fail.
        static ARMED: Cell<bool> = const { Cell::new(false) };
    }

    /// Arms the seam until the returned value is dropped.
    pub(crate) fn arm() -> Armed {
        ARMED.with(|armed| armed.set(true));
        Armed
    }

    /// Disarms the seam when dropped.
    pub(crate) struct Armed;

    impl Drop for Armed {
        fn drop(&mut self) { ARMED.with(|armed| armed.set(false)); }
    }

    /// Consumes the arming, reporting whether this rename must fail.
    ///
    /// One-shot by design: arming fails exactly one swap, so a test that
    /// triggers more than one rename cannot have the seam fire twice.
    pub(crate) fn take() -> bool { ARMED.with(|armed| armed.replace(false)) }
}

/// A test-only seam that fails the removal of a temporary file.
///
/// The cleanup after a failed replacement is otherwise reachable only through a
/// real removal failure, which no test can force on every platform: a directory
/// target fails the swap before a temporary file is named, and a permission bit
/// is ignored by a run as root. The arming is per-thread, because the tests that
/// use it drive the replacement on the thread that armed it, and it is undone
/// when the value [`arm`] returns is dropped, so a failing assertion cannot
/// leave the removal failing for whatever runs next on that thread.
#[cfg(test)]
pub(crate) mod cleanup_failure_seam {
    use std::cell::Cell;

    thread_local! {
        /// Whether this thread's next removal must fail.
        static ARMED: Cell<bool> = const { Cell::new(false) };
    }

    /// Arms the seam until the returned value is dropped.
    pub(crate) fn arm() -> Armed {
        ARMED.with(|armed| armed.set(true));
        Armed
    }

    /// Disarms the seam when dropped.
    pub(crate) struct Armed;

    impl Drop for Armed {
        fn drop(&mut self) { ARMED.with(|armed| armed.set(false)); }
    }

    /// Consumes the arming, reporting whether this removal must fail.
    ///
    /// One-shot by design: arming fails exactly one removal, so a test that
    /// triggers more than one cleanup cannot have the seam fire twice.
    pub(crate) fn take() -> bool { ARMED.with(|armed| armed.replace(false)) }
}
