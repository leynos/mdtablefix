//! The replacement entry points: reading a document, formatting it, and
//! putting the result back through the directory capability the caller holds.
//!
//! [`rewrite`] and [`rewrite_no_wrap`] read a file, hand its lines to
//! [`crate::process`] for transformation, and write the result back.
//! [`replace_file`] is the step the executable calls with the capability it
//! already holds, and [`replace_file_if_unchanged`] is that step for a caller
//! that has just read the target and must not overwrite a change it has not
//! seen.
//!
//! This is where a document's line-ending style is selected and reported: the
//! decision is taken at the boundary that produces output, and the policy
//! behind it lives in `super::line_endings`, which emits nothing.

use std::{io, path::Path, sync::OnceLock, time::Instant};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use metrics::{Unit, counter, describe_counter, describe_histogram, histogram};
use tracing::{debug, trace};

use super::{
    document::SourceDocument,
    line_endings::LineEndingCounts,
    swap::{create_temporary_file, remove_temporary_file, write_and_swap},
};
use crate::process::{process_stream, process_stream_no_wrap};

/// Read `path`, process the contents with `f`, and write the result back.
///
/// The document boundary is taken here: the body handed to `f` carries no
/// byte-order mark, and the result is written back with the mark and the
/// line-ending style holding the majority of the file's line endings. The
/// decision is reported at `debug` level under `operation`'s name.
///
/// This helper encapsulates the common pattern used by [`rewrite`] and
/// [`rewrite_no_wrap`], including the conditional replacement: the text this
/// function read is what the target must still hold, so a file another process
/// changed while it was being reformatted is reported rather than overwritten.
///
/// Visible to `super` rather than private so the unit tests can drive it with
/// an identity transform: [`rewrite`] and [`rewrite_no_wrap`] always transform
/// the content, so neither can separate the bytes the boundary restores from
/// the bytes a transform produces. Nothing else calls it.
///
/// # Errors
/// Returns an error if reading or writing the file fails, or if the file
/// changed between the read and the replacement.
pub(super) fn rewrite_with<F>(path: &Path, operation: &str, f: F) -> std::io::Result<()>
where
    F: Fn(&[String]) -> Vec<String>,
{
    let (directory, name) = open_parent(path)?;
    let text = directory.read_to_string(&name)?;
    let document = SourceDocument::parse(&text);
    let path_text = path.to_string_lossy();
    report_line_endings(document.counts(), operation, path_text.as_ref());
    let lines: Vec<String> = document.body().lines().map(str::to_string).collect();
    let fixed = f(&lines);
    if !replace_file_if_unchanged(&directory, &name, &text, &document.render(&fixed))? {
        return Err(io::Error::other(format!(
            "{path_text} changed while it was being rewritten; leaving the changed file alone"
        )));
    }
    Ok(())
}

/// Reports the line-ending decision at the boundary that made it.
///
/// Every boundary that selects an output style emits the same message with the
/// same `crlf_count`, `lone_lf_count`, and `selected_ending` fields, so one
/// filter finds them all: this one for the library entry points, and the
/// executable's copy for its input and output boundaries, which a separate
/// crate cannot share. `operation` names the boundary — `"rewrite"` and
/// `"rewrite_no_wrap"` here, `"file"` and `"stdin"` there — and `path` names
/// the file being rewritten.
///
/// This is deliberately not a public function, and deliberately not part of
/// `super::line_endings`: the queries that count and select emit nothing, and
/// only the boundary that acts on the answer logs it.
fn report_line_endings(counts: LineEndingCounts, operation: &str, path: &str) {
    debug!(
        operation,
        path = %path,
        crlf_count = counts.crlf_count,
        lone_lf_count = counts.lone_lf_count,
        selected_ending = counts.ending.as_str(),
        "selected the majority line ending"
    );
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
pub(super) fn register_metrics() {
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
    describe_counter!(
        "mdtablefix_io_temporary_cleanup_failures_total",
        "Temporary files a failed replacement could not remove"
    );
    describe_counter!(
        "mdtablefix_io_symlink_declined_total",
        "Symbolic-link targets declined rather than replaced"
    );
}

/// Atomically replaces `path` inside `directory` with `contents`.
///
/// The replacement is written to a temporary file beside the target and
/// renamed over it, so the swap is atomic on POSIX filesystems and a failure
/// before the rename leaves the original file untouched. A freshly created
/// temporary file does not inherit the target's permissions, so the target's
/// permissions are applied to it before the rename; `prepare_destination` in
/// the swap module holds the step that only Windows needs before that rename
/// can be attempted. Any failure after the temporary file exists triggers a
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
    replace(directory, path, None, contents).map(|_| ())
}

/// Atomically replaces `path` in `directory` with `contents`, but only while
/// `path` still holds `expected`.
///
/// This is the replacement a caller that has just read the target wants. A
/// separate read before [`replace_file`] would decide on text that a concurrent
/// writer can change during the temporary file's creation, write, and flush,
/// and the rename would then discard that writer's work. Here the comparison is
/// made inside the swap, after everything expensive is done and as close to the
/// rename as the platform allows, so that window is the rename itself. It is
/// not a true compare-and-swap: no rename on any supported platform compares
/// contents, so a writer that lands between the comparison and the rename still
/// wins. Every earlier window — the whole formatting run — is closed.
///
/// Returns `Ok(false)` when the target no longer holds `expected` and its
/// temporary file was removed: the target is left exactly as it was, and the
/// caller decides what a target that moved on means for its run. A target that
/// cannot be read back, or a temporary file that cannot be removed after a
/// declined replacement, is an error: a caller that asked for a conditional
/// replacement must not be told it succeeded, or that the condition failed,
/// when the question could not be put or cleaned up.
///
/// # Errors
/// Returns an error if the target cannot be inspected, if it cannot be read
/// back for the comparison, if the temporary file cannot be written or removed,
/// or if the rename fails.
#[tracing::instrument(level = "debug", skip(directory, expected, contents), fields(path = %path))]
pub fn replace_file_if_unchanged(
    directory: &Dir,
    path: &Utf8Path,
    expected: &str,
    contents: &str,
) -> io::Result<bool> {
    replace(directory, path, Some(expected), contents)
}

/// Performs the replacement [`replace_file`] and [`replace_file_if_unchanged`]
/// report the outcome of.
///
/// One implementation for both, so the symlink refusal, the temporary-file
/// cleanup, and the metrics cannot disagree about what a replacement is: the
/// conditional entry point only adds the text to compare against.
fn replace(
    directory: &Dir,
    path: &Utf8Path,
    expected: Option<&str>,
    contents: &str,
) -> io::Result<bool> {
    describe_metrics();
    let started = Instant::now();
    let outcome = replace_inner(directory, path, expected, contents);
    let result = match &outcome {
        Ok(true) => "success",
        Ok(false) => "unchanged",
        Err(_) => "failure",
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

/// Performs the replacement that [`replace`] reports the outcome of.
fn replace_inner(
    directory: &Dir,
    path: &Utf8Path,
    expected: Option<&str>,
    contents: &str,
) -> io::Result<bool> {
    let metadata = directory.symlink_metadata(path).inspect_err(|error| {
        debug!(error_category = ?error.kind(), "replacement failed");
    })?;
    if metadata.file_type().is_symlink() {
        counter!("mdtablefix_io_symlink_declined_total").increment(1);
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
    let outcome = write_and_swap(
        directory,
        &temp_path,
        path,
        expected,
        contents,
        &permissions,
        file,
    );
    match outcome {
        Ok(true) => Ok(true),
        Ok(false) => {
            // Nothing was replaced, so the temporary file is this run's to
            // remove, exactly as it is after a failure.
            debug!("replacement declined: the target changed since it was read");
            remove_temporary_file(directory, &temp_path)?;
            Ok(false)
        }
        Err(error) => {
            debug!(error_category = ?error.kind(), "replacement failed");
            remove_failed_temporary_file(directory, &temp_path);
            Err(error)
        }
    }
}

/// Removes the temporary file a replacement that did not complete left behind,
/// counting a cleanup that does not complete.
///
/// Best effort by design: the failure that prompted the cleanup is the one the
/// caller must see, so a cleanup that fails is counted and traced rather than
/// returned, and the next run retries past any stale name it finds. The count
/// carries no labels — `mdtablefix_io_replace_total` already reports the
/// replacement as a `failure`, or as `unchanged` when the target was left as it
/// was — so a recorder's cardinality stays bounded.
pub(super) fn remove_failed_temporary_file(directory: &Dir, temp_path: &Utf8Path) {
    match remove_temporary_file(directory, temp_path) {
        Ok(()) => trace!("temporary file removed after failure"),
        Err(cleanup_error) => {
            counter!("mdtablefix_io_temporary_cleanup_failures_total").increment(1);
            debug!(
                error_category = ?cleanup_error.kind(),
                "temporary file cleanup failed"
            );
        }
    }
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
/// The file must still hold the text this call read: a file another process
/// changed meanwhile is left as that process wrote it, and the error says so.
///
/// # Errors
/// Returns an error if reading or writing the file fails, or if the file
/// changed between the read and the replacement.
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
/// The file must still hold the text this call read: a file another process
/// changed meanwhile is left as that process wrote it, and the error says so.
///
/// # Errors
/// Returns an error if reading or writing the file fails, or if the file
/// changed between the read and the replacement.
pub fn rewrite_no_wrap(path: &Path) -> std::io::Result<()> {
    rewrite_with(path, "rewrite_no_wrap", process_stream_no_wrap)
}
