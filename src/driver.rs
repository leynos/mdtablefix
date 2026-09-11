//! Mode-independent file analysis and the documented exit statuses.
//!
//! This is the application boundary: the one place in the binary where a
//! directory capability is turned into text and back, and where the process
//! exit status is decided. It sits beside `src/main.rs` rather than in the
//! library, because the library's entry points stay infallible and free of
//! filesystem policy.
//!
//! The reporting modes are handed a [`ReadOnlyDir`], so "a reporting mode
//! cannot write" is a property of the type they receive rather than of a test
//! double that declines to write.

use anyhow::Context;
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
use mdtablefix::{
    LineEndingCounts,
    io::{SourceDocument, replace_file},
    report::{FileReport, LineDelta, render_report_line},
};
use tracing::debug;

/// The formatting function every mode shares.
///
/// One closure, built once and passed by reference, is what stops a reporting
/// mode disagreeing with `--in-place` about what the formatter would write.
pub type Formatter = dyn Fn(&SourceDocument<'_>) -> String + Sync;

/// A directory capability that can only read.
///
/// The reporting modes receive this instead of a [`Dir`], so a wrong `match`
/// arm cannot write: read-only access is enforced by the type, not by
/// convention.
pub struct ReadOnlyDir(Dir);

impl ReadOnlyDir {
    /// Wraps a directory capability, discarding write access.
    #[must_use]
    pub fn new(directory: Dir) -> Self { Self(directory) }

    /// Reads `name` as UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not valid UTF-8.
    pub fn read(&self, name: &Utf8Path) -> anyhow::Result<String> {
        Ok(self.0.read_to_string(name)?)
    }
}

/// A file's current text paired with the text the formatter would write.
pub struct Assessment {
    /// The file's bytes as read, byte-order mark included.
    original: String,
    /// What the shared formatter would write instead.
    formatted: String,
}

impl Assessment {
    /// Whether writing the formatted text would change the file's bytes.
    ///
    /// A direct byte comparison, and the authoritative answer. Comparing body
    /// text instead would report a document whose mark is lost as unchanged.
    #[must_use]
    pub fn is_changed(&self) -> bool { self.original != self.formatted }
}

/// What the caller asked for.
///
/// `Mode::Diff` joins these in `EP-M4`, with the `--diff` flag that selects
/// it; the exit-status cross product grows a mode at the same time, so no
/// variant is ever left unconstructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// No mode flag: the formatted text goes to standard output.
    Print,
    /// `--in-place`: rewrite drifting files.
    InPlace,
    /// `--check`: report drift, and fail on it.
    Check,
}

impl Mode {
    /// Whether this mode reports what it found rather than printing contents.
    ///
    /// The reporting modes are exactly the ones that fail on drift, because a
    /// mode that only describes files must not also silently rewrite them.
    #[must_use]
    pub const fn reports(self) -> bool { matches!(self, Self::Check) }

    /// The verb naming what this mode does to a file, for error contexts.
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::InPlace => "writing",
            Self::Print | Self::Check => "reading",
        }
    }
}

/// The documented process exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    /// Every file was analysed, and no reporting mode found drift.
    Success,
    /// A reporting mode found at least one file that would be reformatted.
    Drift,
    /// At least one file could not be read or rewritten.
    Error,
}

impl ExitStatus {
    /// The process exit code for this status.
    #[must_use]
    pub fn code(self) -> std::process::ExitCode {
        std::process::ExitCode::from(match self {
            Self::Success => 0,
            Self::Drift => 1,
            Self::Error => 2,
        })
    }
}

/// Maps mode and observations onto an exit status.
///
/// An error yields [`ExitStatus::Error`] in every mode, because an incomplete
/// analysis must not be reported as merely drifted — and because a run that
/// could not read every file cannot claim the tree is clean. Drift yields
/// [`ExitStatus::Drift`] under the read-only reporting modes; in particular a
/// successful `--in-place` over drifting files yields [`ExitStatus::Success`],
/// as does a bare invocation, whose contract is to print the formatted text.
#[must_use]
pub fn exit_status(mode: Mode, any_drift: bool, any_error: bool) -> ExitStatus {
    if any_error {
        ExitStatus::Error
    } else if any_drift && mode.reports() {
        ExitStatus::Drift
    } else {
        ExitStatus::Success
    }
}

/// Reads a file and pairs its text with the formatted result.
///
/// Takes [`ReadOnlyDir`], so this function cannot write. `storage_key` is the
/// bare file name within the capability, and is also what the line-ending
/// report names: the path the user typed is attached by the caller, which is
/// the only place that knows it.
///
/// # Errors
///
/// Returns an error if the file cannot be read or is not valid UTF-8.
pub fn assess(
    directory: &ReadOnlyDir,
    storage_key: &Utf8Path,
    format: &Formatter,
) -> anyhow::Result<Assessment> {
    let original = directory.read(storage_key)?;
    let document = SourceDocument::parse(&original);
    report_line_endings(document.counts(), "file", Some(storage_key.as_str()));
    let formatted = format(&document);

    Ok(Assessment {
        original,
        formatted,
    })
}

/// Writes the formatted text back. Only reachable from [`Mode::InPlace`].
///
/// The replacement is atomic and capability-scoped: see
/// [`mdtablefix::io::replace_file`].
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn write_back(
    directory: &Dir,
    storage_key: &Utf8Path,
    assessment: &Assessment,
) -> anyhow::Result<()> {
    Ok(replace_file(directory, storage_key, &assessment.formatted)?)
}

/// Analyses one file under `mode`, returning its report and stdout payload.
///
/// The assessment is dropped before this returns, so retained memory is
/// proportional to the rendered payload rather than to twice the whole input.
///
/// # Errors
///
/// Returns an error if the file cannot be read, or — under [`Mode::InPlace`] —
/// cannot be rewritten.
pub fn analyse(
    mode: Mode,
    directory: &Dir,
    display_path: &Utf8Path,
    storage_key: &Utf8Path,
    format: &Formatter,
) -> anyhow::Result<(FileReport, String)> {
    // The read capability is derived from the caller's, so every mode reads
    // through one type and only `--in-place` holds a capability that can write.
    let readable = ReadOnlyDir::new(
        directory
            .try_clone()
            .with_context(|| format!("duplicating the capability on {storage_key}"))?,
    );
    let assessment = assess(&readable, storage_key, format)?;
    let is_changed = assessment.is_changed();
    // The counting diff is not asked to work on byte-equal texts: the byte
    // comparison above is what decides, so a clean tree costs no diff work.
    let delta = if is_changed {
        LineDelta::between(&assessment.original, &assessment.formatted)
    } else {
        LineDelta::default()
    };
    let payload = match mode {
        Mode::Print => assessment.formatted.clone(),
        Mode::Check if is_changed => format!("{}\n", render_report_line(display_path, delta)),
        Mode::Check => String::new(),
        Mode::InPlace => {
            write_back(directory, storage_key, &assessment)?;
            String::new()
        }
    };

    Ok((
        FileReport {
            display_path: display_path.to_owned(),
            is_changed,
            delta,
        },
        payload,
    ))
}

/// Orders indexed results by argument index.
///
/// Ordering is explicit rather than inherited from `rayon`'s collection order,
/// which is not a documented guarantee. See `AX-4`.
#[must_use]
pub fn in_argument_order<T>(results: Vec<(usize, T)>) -> Vec<T> {
    let mut indexed = results;
    indexed.sort_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, value)| value).collect()
}

/// Reports the line-ending decision at the command boundary that made it.
///
/// The library keeps the same report for its own entry points, but the binary
/// is a separate crate and cannot share it, so the message is repeated here with
/// the same fields: one filter finds every boundary. `operation` names the
/// boundary — `"file"` when formatting a file, `"stdin"` when reading standard
/// input — and `path` names the file being formatted, or is `None` for standard
/// input, which is reported as its own source rather than left nameless.
///
/// This stays private, and takes the counts the pure `count_line_endings` query
/// already produced, so no query emits events and only the boundary that acts on
/// the answer logs it.
pub fn report_line_endings(counts: LineEndingCounts, operation: &str, path: Option<&str>) {
    debug!(
        operation,
        path = %path.unwrap_or("<stdin>"),
        crlf_count = counts.crlf_count,
        lone_lf_count = counts.lone_lf_count,
        selected_ending = counts.ending.as_str(),
        "selected the majority line ending"
    );
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
