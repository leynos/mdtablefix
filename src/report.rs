//! Pure reporting: line deltas and their rendering.
//!
//! Nothing here opens a path, reads a file of its own accord, or defines an
//! error type: a writer's failure is the caller's own `std::io::Error`,
//! returned untouched. The module is named `report` rather than `check`
//! because it serves `--check` and `--diff` equally.

pub mod delta;
pub mod render;

pub use crate::report::{
    delta::LineDelta,
    render::{DiffOptions, render_report_line, render_summary, write_unified_diff},
};

/// What one file's analysis produced, ready to render.
///
/// Returning a value rather than pre-rendered text keeps a future
/// `--format=json` a leaf addition.
///
/// # Examples
///
/// ```
/// use camino::Utf8PathBuf;
/// use mdtablefix::report::{FileReport, LineDelta};
///
/// let report = FileReport {
///     display_path: Utf8PathBuf::from("docs/a.md"),
///     is_changed: true,
///     delta: LineDelta::default(),
/// };
/// assert!(report.is_changed);
/// assert!(!report.delta.has_changes());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReport {
    /// The path as supplied on the command line.
    pub display_path: camino::Utf8PathBuf,
    /// Whether the file's bytes would change.
    pub is_changed: bool,
    /// Line counts, zero in both components when `is_changed` is false.
    pub delta: LineDelta,
}
