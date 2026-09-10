//! File helpers for rewriting Markdown documents.
//!
//! Rewrites replace the target through a temporary file in the same directory
//! and a rename, so a failure part-way through leaves the original file intact
//! rather than truncated. Every filesystem operation runs through a `cap_std`
//! directory capability: [`rewrite`] and [`rewrite_no_wrap`] open one for the
//! target's parent, and the CLI passes the capability it already holds.
//!
//! [`rewrite`] and [`rewrite_no_wrap`] read a file, hand its lines to
//! [`crate::process`] for transformation, and write the result back. The
//! transformation stays line-ending agnostic; this module owns the
//! line-ending policy instead. [`detect_line_ending`] counts the document's
//! line feed and carriage return and line feed (CRLF) endings and selects the
//! style holding the strict majority, and [`serialize_lines`] re-terminates
//! every reformatted line with that style. Detection is per document, so no
//! transform module needs to know which terminator the source file uses.
//!
//! [`detect_line_ending`] is a pure query and emits nothing. The rewrite
//! helpers below report the decision, with the counts behind it, at `debug`
//! level, so a caller can trace why a file was rewritten with the endings it
//! has without the query itself becoming side-effecting.
//!
//! The rationale, the rejected alternatives and the known limitations are
//! recorded in `docs/adrs/0007-line-ending-detection.md`.

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

/// The line-ending style of a document.
///
/// Rewriting preserves the style that dominates the input so that a file
/// authored on Windows is not silently rewritten to line feeds, which would
/// otherwise show up as a whole-file diff that changes no Markdown content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEnding {
    /// A single line feed, `\n`.
    Lf,
    /// A carriage return followed by a line feed, `\r\n`.
    Crlf,
}

impl LineEnding {
    /// Returns the characters written between lines.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mdtablefix::LineEnding;
    ///
    /// assert_eq!(LineEnding::Lf.as_str(), "\n");
    /// assert_eq!(LineEnding::Crlf.as_str(), "\r\n");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

/// Selects the line-ending style holding a strict majority of `text`'s line
/// endings.
///
/// CRLF pairs are counted first and then subtracted from the total line-feed
/// count to obtain the lone line feeds. Counting line feeds directly would
/// count every CRLF twice and leave CRLF unable to win.
/// [`LineEnding::Crlf`] is selected only when CRLF pairs strictly outnumber
/// lone line feeds.
///
/// Text with an exact tie and text with no line endings at all both select
/// [`LineEnding::Lf`], so the result is deterministic and never depends on
/// which style happens to appear first.
///
/// Only CRLF and LF are recognized. A lone carriage return is content rather
/// than a line ending, matching the `str::lines` split used to read the
/// document.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::{LineEnding, detect_line_ending};
///
/// assert_eq!(detect_line_ending("alpha\nbeta\n"), LineEnding::Lf);
/// assert_eq!(detect_line_ending("alpha\r\nbeta\r\n"), LineEnding::Crlf);
/// assert_eq!(detect_line_ending("alpha\r\nbeta\n"), LineEnding::Lf);
/// assert_eq!(detect_line_ending("alpha"), LineEnding::Lf);
/// ```
#[must_use]
pub fn detect_line_ending(text: &str) -> LineEnding { count_line_endings(text).ending }

/// The line-ending counts of a document, and the style they select.
///
/// [`count_line_endings`] returns this so a caller can report or act on how
/// one-sided the majority vote was, rather than only on its outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineEndingCounts {
    /// The style the counts select.
    pub ending: LineEnding,
    /// The number of carriage return and line feed (CRLF) pairs.
    pub crlf_count: usize,
    /// The number of lone line feeds, which no carriage return precedes.
    pub lone_lf_count: usize,
}

/// Counts `text`'s line endings and selects the majority style.
///
/// This is [`detect_line_ending`] with the counts that decided it, so a caller
/// that reports or acts on the vote does not have to restate the counting
/// rule. CRLF pairs are counted first and subtracted from the total line-feed
/// count to obtain the lone line feeds.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::{LineEnding, count_line_endings};
///
/// let counts = count_line_endings("alpha\r\nbeta\r\ngamma\n");
///
/// assert_eq!(counts.ending, LineEnding::Crlf);
/// assert_eq!(counts.crlf_count, 2);
/// assert_eq!(counts.lone_lf_count, 1);
/// ```
#[must_use]
pub fn count_line_endings(text: &str) -> LineEndingCounts {
    let crlf_count = text.matches("\r\n").count();
    let lone_lf_count = text.matches('\n').count() - crlf_count;
    let ending = if crlf_count > lone_lf_count {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    };
    LineEndingCounts {
        ending,
        crlf_count,
        lone_lf_count,
    }
}

/// Counts `text`'s line endings, reports the vote, and returns the counts.
///
/// This is [`count_line_endings`] with the reporting boundary attached, so
/// every site that selects an output style emits the same message with the
/// same `crlf_count`, `lone_lf_count`, and `selected_ending` fields.
/// `operation` names the boundary: `"rewrite"` and `"rewrite_no_wrap"` for the
/// library entry points, and `"file"` or `"stdin"` for the executable's
/// input/output boundaries. `path` names the file where one exists, so a
/// rewritten file's event says which file it came from. Both are omitted from
/// the event when a caller does not supply them.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::{LineEnding, count_line_endings_reported};
///
/// let counts = count_line_endings_reported("alpha\r\nbeta\r\n", None, None);
///
/// assert_eq!(counts.ending, LineEnding::Crlf);
/// assert_eq!(counts.crlf_count, 2);
/// ```
#[must_use]
pub fn count_line_endings_reported(
    text: &str,
    operation: Option<&str>,
    path: Option<&str>,
) -> LineEndingCounts {
    let counts = count_line_endings(text);
    match (operation, path) {
        (Some(operation), Some(path)) => debug!(
            operation,
            path = %path,
            crlf_count = counts.crlf_count,
            lone_lf_count = counts.lone_lf_count,
            selected_ending = counts.ending.as_str(),
            "selected the majority line ending"
        ),
        (Some(operation), None) => debug!(
            operation,
            crlf_count = counts.crlf_count,
            lone_lf_count = counts.lone_lf_count,
            selected_ending = counts.ending.as_str(),
            "selected the majority line ending"
        ),
        (None, _) => debug!(
            crlf_count = counts.crlf_count,
            lone_lf_count = counts.lone_lf_count,
            selected_ending = counts.ending.as_str(),
            "selected the majority line ending"
        ),
    }
    counts
}

/// Renders `lines` as one document whose lines end with `ending`.
///
/// An empty slice yields an empty string. Otherwise every line is followed by
/// `ending`, so a non-empty result always carries one trailing terminator.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::{LineEnding, serialize_lines};
///
/// let lines = vec!["| A |".to_string(), "| 1 |".to_string()];
///
/// assert_eq!(serialize_lines(&lines, LineEnding::Lf), "| A |\n| 1 |\n");
/// assert_eq!(
///     serialize_lines(&lines, LineEnding::Crlf),
///     "| A |\r\n| 1 |\r\n"
/// );
/// assert_eq!(serialize_lines(&[], LineEnding::Crlf), "");
/// ```
#[must_use]
pub fn serialize_lines(lines: &[String], ending: LineEnding) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let terminator = ending.as_str();
    let capacity: usize = lines.iter().map(|line| line.len() + terminator.len()).sum();
    let mut output = String::with_capacity(capacity);
    for line in lines {
        output.push_str(line);
        output.push_str(terminator);
    }
    output
}

/// Candidate temporary names to try before conceding that a stale temporary
/// file from an earlier killed run is in the way.
const TEMP_FILE_ATTEMPTS: u32 = 16;

/// Read `path`, process the contents with `f`, and write the result back.
///
/// The line-ending style holding the majority of the file's line endings is
/// preserved in the rewritten file, and the decision is reported at `debug`
/// level under `operation`'s name.
///
/// This helper encapsulates the common pattern used by [`rewrite`] and
/// [`rewrite_no_wrap`].
///
/// # Errors
/// Returns an error if reading or writing the file fails.
fn rewrite_with<F>(path: &Path, operation: &str, f: F) -> std::io::Result<()>
where
    F: Fn(&[String]) -> Vec<String>,
{
    let (directory, name) = open_parent(path)?;
    let text = directory.read_to_string(&name)?;
    let path_text = path.to_string_lossy();
    let counts = count_line_endings_reported(&text, Some(operation), Some(path_text.as_ref()));
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let fixed = f(&lines);
    replace_file(&directory, &name, &serialize_lines(&fixed, counts.ending))
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
/// permissions are applied to it before the rename; [`prepare_destination`]
/// holds the step that only Windows needs before that rename can be attempted.
/// Any failure after the temporary file exists triggers a best-effort attempt
/// to remove it.
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
    let outcome = write_and_swap(directory, &temp_path, path, contents, &permissions, file);
    if let Err(error) = &outcome {
        debug!(error_category = ?error.kind(), "replacement failed");
        // Best effort: failing to clean up must not mask the original error,
        // and the next run retries past any stale name it finds.
        match remove_temporary_file(directory, &temp_path) {
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
fn remove_temporary_file(directory: &Dir, temp_path: &Utf8Path) -> io::Result<()> {
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
/// The line-ending style holding the majority of the file's line endings is
/// preserved, so a CRLF file stays CRLF.
///
/// The replacement is written to a temporary file beside `path` and renamed
/// over it, so a failure before the rename leaves the original file intact. The
/// original file mode is preserved. Symbolic links are declined.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite(path: &Path) -> std::io::Result<()> { rewrite_with(path, "rewrite", process_stream) }

/// Rewrite a file in place without wrapping text.
///
/// The line-ending style holding the majority of the file's line endings is
/// preserved, so a CRLF file stays CRLF.
///
/// The replacement is written to a temporary file beside `path` and renamed
/// over it, so a failure before the rename leaves the original file intact. The
/// original file mode is preserved. Symbolic links are declined.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub fn rewrite_no_wrap(path: &Path) -> std::io::Result<()> {
    rewrite_with(path, "rewrite_no_wrap", process_stream_no_wrap)
}

/// A test-only seam that fails the rename half of the swap.
///
/// Every rename a test can be made to fail for real fails before the
/// destination is prepared, so the rollback in [`super::swap_into_place`] is
/// otherwise unreachable. The arming is per-thread, because the tests that use
/// it drive the swap on the thread that armed it, and it is undone when the
/// value [`arm`] returns is dropped, so a failing assertion cannot leave the
/// failure armed for whatever runs next on that thread.
#[cfg(test)]
mod rename_failure_seam {
    use std::cell::Cell;

    thread_local! {
        /// Whether this thread's next rename must fail.
        static ARMED: Cell<bool> = const { Cell::new(false) };
    }

    /// Arms the seam until the returned value is dropped.
    pub(super) fn arm() -> Armed {
        ARMED.with(|armed| armed.set(true));
        Armed
    }

    /// Disarms the seam when dropped.
    pub(super) struct Armed;

    impl Drop for Armed {
        fn drop(&mut self) { ARMED.with(|armed| armed.set(false)); }
    }

    /// Consumes the arming, reporting whether this rename must fail.
    ///
    /// One-shot by design: arming fails exactly one swap, so a test that
    /// triggers more than one rename cannot have the seam fire twice.
    pub(super) fn take() -> bool { ARMED.with(|armed| armed.replace(false)) }
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

#[cfg(test)]
#[path = "io_line_ending_tests.rs"]
mod line_ending_tests;
