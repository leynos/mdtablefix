//! Mode-independent file analysis and the documented exit statuses.
//!
//! This is the application boundary: the one place in the binary where the
//! command line is resolved into an input source, where a directory capability
//! is turned into text and back, and where the process exit status is decided.
//! It sits beside `src/main.rs` rather than in the library, because the
//! library's entry points stay infallible and free of filesystem policy.
//!
//! The reporting modes are handed a [`ReadOnlyDir`], so "a reporting mode
//! cannot write" is a property of the type they receive rather than of a test
//! double that declines to write.

use std::path::PathBuf;

use anyhow::{Context, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use mdtablefix::{
    LineEndingCounts,
    io::{SourceDocument, replace_file},
    report::{DiffOptions, FileReport, LineDelta, render_report_line, write_unified_diff},
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// No mode flag: the formatted text goes to standard output.
    Print,
    /// `--in-place`: rewrite drifting files.
    InPlace,
    /// `--check`: report drift, and fail on it.
    Check,
    /// `--diff`: show what would change, and fail on drift.
    Diff,
}

impl Mode {
    /// Whether this mode reports what it found rather than printing contents.
    ///
    /// The reporting modes are exactly the ones that fail on drift, because a
    /// mode that only describes files must not also silently rewrite them.
    /// The check and diff modes differ only in how they render what they
    /// found, so a mode added here without a matching failure would be a mode
    /// that describes a drift it does not report.
    #[must_use]
    pub const fn reports(self) -> bool { matches!(self, Self::Check | Self::Diff) }

    /// The verb naming what this mode does to a file, for error contexts.
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::InPlace => "writing",
            Self::Print | Self::Check | Self::Diff => "reading",
        }
    }
}

/// The diff configuration the reporting rendering uses.
///
/// Three lines of context is the unified-diff convention. The degradation
/// threshold is a line count rather than a wall-clock budget: a timeout would
/// make the output depend on how fast the machine happened to be, which is
/// exactly what `INV-DETERMINISTIC` forbids.
const DIFF_OPTIONS: DiffOptions = DiffOptions {
    context_radius: 3,
    patience_threshold: 1000,
};

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

/// Where the text to format comes from.
///
/// An explicit answer rather than "the file list is empty", because the two
/// facts that emptiness would conflate are not the same: "no paths were named,
/// so read standard input" and "the selected source resolved to no paths". The
/// second is a legitimate outcome for a source that discovers its own inputs —
/// it must be able to name nothing and still exit [`ExitStatus::Success`],
/// rather than fall through to a standard input that may be a terminal. See
/// `AX-6`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inputs {
    /// No paths were named: the document is read from standard input.
    Stdin,
    /// The named files, in argument order.
    Files(Vec<Utf8PathBuf>),
}

impl Inputs {
    /// Resolves the command line's positional arguments.
    ///
    /// The conversion to [`Utf8PathBuf`] happens once, here, rather than per
    /// file: a path that is not valid UTF-8 cannot name a file within a
    /// [`Dir`] capability, so a run containing one fails as a whole instead of
    /// being counted as a single file's error while its siblings proceed.
    ///
    /// # Errors
    ///
    /// Returns an error naming the offending path if any argument is not valid
    /// UTF-8.
    pub fn resolve(files: Vec<PathBuf>) -> anyhow::Result<Self> {
        if files.is_empty() {
            return Ok(Self::Stdin);
        }
        let files = files
            .into_iter()
            .map(|path| {
                Utf8PathBuf::from_path_buf(path)
                    .map_err(|path| anyhow!("converting {} to a UTF-8 path", path.display()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(Self::Files(files))
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

/// Writes the formatted text back.
///
/// Only reachable from [`Mode::InPlace`], and only for a file whose bytes would
/// change: see [`Assessment::is_changed`]. The replacement is atomic and
/// capability-scoped, and it renames a temporary over the target, so an
/// unconditional call would swap the inode of a file it left byte-identical.
/// See [`mdtablefix::io::replace_file`].
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
    // The two reporting modes render the same finding in different shapes, and
    // an unchanged file renders as nothing under either: a clean file must
    // leave standard output empty rather than print an empty diff. The
    // `is_changed` guards are what carry that, which is why the unchanged arms
    // are written last.
    let payload = match mode {
        // The move is the last use of the assessment on this arm; the arms
        // below borrow it, and cannot run beside this one.
        Mode::Print => assessment.formatted,
        Mode::Check if is_changed => format!("{}\n", render_report_line(display_path, delta)),
        Mode::Diff if is_changed => render_diff(display_path, &assessment)?,
        // A clean file is left alone byte for byte. The write would be
        // invisible in the text but not in the file: `replace_file` renames a
        // temporary over the target, so it would swap the inode and the
        // modification time of a file it did not change, and `make`-style
        // staleness checks would see a rebuild where there was nothing to
        // rebuild.
        Mode::InPlace if is_changed => {
            write_back(directory, storage_key, &assessment)?;
            String::new()
        }
        Mode::InPlace | Mode::Check | Mode::Diff => String::new(),
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

/// Renders the unified diff for one changed file.
///
/// The renderer streams into a byte sink rather than a `String`, because that
/// is what lets it go straight to standard output when the caller wants it to.
/// Here the payload has to be a `String`, so the bytes are collected and then
/// validated: both sides of the diff came from `String`s, so the validation
/// cannot fail in practice, but the driver does not get to assume it.
fn render_diff(display_path: &Utf8Path, assessment: &Assessment) -> anyhow::Result<String> {
    let mut buffer = Vec::new();
    write_unified_diff(
        &mut buffer,
        display_path,
        &assessment.original,
        &assessment.formatted,
        DIFF_OPTIONS,
    )
    .with_context(|| format!("rendering the diff for {display_path}"))?;

    String::from_utf8(buffer)
        .with_context(|| format!("validating the rendered diff for {display_path} as UTF-8"))
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
#[path = "driver_contract_tests.rs"]
mod contract_tests;
#[cfg(test)]
#[path = "driver_in_place_tests.rs"]
mod in_place_tests;
#[cfg(test)]
#[path = "driver_report_tests.rs"]
mod report_tests;
#[cfg(test)]
#[path = "driver_test_support.rs"]
mod test_support;
