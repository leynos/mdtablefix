//! File helpers for rewriting Markdown documents.
//!
//! Rewrites replace the target through a temporary file in the same directory
//! and a rename, so a failure part-way through leaves the original file intact
//! rather than truncated. Every filesystem operation runs through a `cap_std`
//! directory capability: [`rewrite`] and [`rewrite_no_wrap`] open one for the
//! target's parent, and the CLI passes the capability it already holds.
//!
//! The module is split by concern, and each part is a child module here:
//!
//! - `line_endings` owns the policy — which terminator an output document is written with, and how
//!   the majority style is counted out of the input.
//! - `replace` owns the replacement entry points, including the boundary that selects a document's
//!   style and reports the decision it made.
//! - `swap` owns the temporary file and the rename that make a replacement atomic.
//!
//! [`detect_line_ending`], [`count_line_endings`], and [`serialize_lines`] are
//! pure queries and emit nothing. The rewrite helpers report the decision, with
//! the counts behind it, at `debug` level, so a caller can trace why a file was
//! rewritten with the endings it has without the query itself becoming
//! side-effecting.
//!
//! The rationale, the rejected alternatives and the known limitations are
//! recorded in `docs/adrs/0007-line-ending-detection.md`.

mod line_endings;
mod replace;
mod swap;

pub use line_endings::{
    LineEnding,
    LineEndingCounts,
    count_line_endings,
    detect_line_ending,
    serialize_lines,
};
/// Re-exposes the internals the unit tests drive directly, so that a test
/// module reaches them without naming the child module they live in.
#[cfg(test)]
use replace::{register_metrics, remove_failed_temporary_file};
pub use replace::{replace_file, rewrite, rewrite_no_wrap};
#[cfg(test)]
use swap::{
    TEMP_FILE_ATTEMPTS,
    cleanup_failure_seam,
    create_temporary_file,
    rename_failure_seam,
    temporary_path,
};

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
