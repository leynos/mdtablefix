//! The modes, what each one prints, and the exit status it earns.
//!
//! [`Mode`] names what the caller asked for, [`ExitStatus`] is the documented
//! process status, and the rendering helpers here turn the difference an
//! [`Assessment`](super::source::Assessment) measured into the line `--check`
//! prints, the unified diff `--diff` prints, or the path `--list-files`
//! prints.
//!
//! Nothing here reads or writes a file: a mode is a value, and rendering is a
//! pure function of the assessment the boundary in the parent module made.

use anyhow::Context;
use camino::Utf8Path;
use mdtablefix::{
    LineEndingCounts,
    report::{DiffOptions, write_unified_diff},
};
use tracing::debug;

use super::source::Assessment;

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
    /// `--list-files`: print the selected paths, and read none of them.
    ///
    /// A mode rather than a flag on the selection, because the group it belongs
    /// to already forbids combining it with the other three, and because the
    /// selection is asked for paths under every mode.
    ListFiles,
}

impl Mode {
    /// Whether this mode reports what it found rather than printing contents.
    ///
    /// The reporting modes are exactly the ones that fail on drift, because a
    /// mode that only describes files must not also silently rewrite them.
    /// The check and diff modes differ only in how they render what they
    /// found, so a mode added here without a matching failure would be a mode
    /// that describes a drift it does not report.
    ///
    /// `--list-files` is not among them: it reports paths, not drift, and it
    /// must exit `0` for a tree full of drift it was never asked to assess.
    #[must_use]
    pub const fn reports(self) -> bool { matches!(self, Self::Check | Self::Diff) }

    /// The verb naming what this mode does to a file, for error contexts.
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::InPlace => "writing",
            Self::ListFiles => "listing",
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

/// Renders one selected path as the single line `--list-files` prints for it.
///
/// A path is one line only while it holds no line terminator, and Git's index
/// may hold one that does: the NUL framing that lists the candidates is what
/// lets such a name through the selection in the first place. The terminator,
/// and the backslash that escapes it, are therefore written as `\n`, `\r`, and
/// `\\`, so that one printed line is one selected path and a reader can
/// recover the name from it. Every other character is printed as itself, so a
/// path that holds none of the three is unchanged.
pub(super) fn listed(display_path: &Utf8Path) -> String {
    // The backslash first: escaping it after the others would escape the
    // escapes they introduced.
    let escaped = display_path
        .as_str()
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r");

    format!("{escaped}\n")
}

/// Renders the unified diff for one changed file.
///
/// The renderer streams into a byte sink rather than a `String`, because that
/// is what lets it go straight to standard output when the caller wants it to.
/// Here the payload has to be a `String`, so the bytes are collected and then
/// validated: both sides of the diff came from `String`s, so the validation
/// cannot fail in practice, but the driver does not get to assume it.
pub(super) fn render_diff(
    display_path: &Utf8Path,
    assessment: &Assessment,
) -> anyhow::Result<String> {
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

/// Reports the line-ending decision at the command boundary that made it.
///
/// The library keeps the same report for its own entry points, but the binary
/// is a separate crate and cannot share it, so the message is repeated here with
/// the same fields: one filter finds every boundary. `operation` names the
/// boundary — `"file"` when formatting a file, `"stdin"` when reading standard
/// input — and `path` names the file being formatted, or is `None` for standard
/// input, which is reported as its own source rather than left nameless.
///
/// It takes the counts the pure `count_line_endings` query already produced, so
/// no query emits events and only the boundary that acts on the answer logs it:
/// [`analyse`](super::analyse) for a file, and `format_stdin` for standard
/// input. It is `pub` because the crate root is not a descendant of this module
/// and cannot see a private item, not because the library exposes it.
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
