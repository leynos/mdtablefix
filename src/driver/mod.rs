//! Mode-independent file analysis and the documented exit statuses.
//!
//! This is the application boundary: the one place in the binary where the
//! command line is resolved into an input source, where a directory capability
//! is turned into text and back, and where the process exit status is decided.
//! It sits beside `src/main.rs` rather than in the library, whose formatting
//! entry points are infallible while its filesystem entry points return
//! `std::io::Result`. Neither carries this binary's policy: turning a result
//! into an exit status is the driver's work.
//!
//! The reporting modes are handed a [`ReadOnlyDir`], so "a reporting mode
//! cannot write" is a property of the type they receive rather than of a test
//! double that declines to write.
//!
//! What this module does not hold lives beside it. [`source`] resolves the
//! inputs and turns one file into an [`Assessment`]; [`reporting`] holds the
//! modes, the rendering each one does, and the exit statuses they earn.

mod reporting;
mod source;

use anyhow::{Context, anyhow};
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
use mdtablefix::{
    io::SourceDocument,
    report::{FileReport, LineDelta, render_report_line},
};
pub use reporting::{ExitStatus, Mode, exit_status, report_line_endings};
use reporting::{listed, render_diff};
pub use source::{Inputs, ReadOnlyDir, assess, write_back};

use crate::select::{
    ConflictGuard,
    conflict::{ConflictMarkerState, marker_state, refuses},
};

/// The formatting function every mode shares.
///
/// One closure, built once and passed by reference, is what stops a reporting
/// mode disagreeing with `--in-place` about what the formatter would write.
pub type Formatter = dyn Fn(&SourceDocument<'_>) -> String + Sync;

/// Analyses one file under `mode`, returning its report and stdout payload.
///
/// This, not [`assess`], is where the line-ending decision is reported: the
/// mode dispatched here is what acts on the assessment, whether by printing it,
/// reporting it, or writing it back, so a subscriber hears only about files
/// whose run reached the point of acting. The report names `display_path`, the
/// path as the user wrote it, rather than `storage_key`, the bare name the
/// capability reads by: two files called `a.md` in different directories would
/// otherwise share a `path` field, and a subscriber could not tell them apart.
///
/// The assessment is dropped before this returns, so retained memory is
/// proportional to the rendered payload rather than to twice the whole input.
///
/// # Errors
///
/// Returns an error if the file cannot be read; under [`Mode::InPlace`], if it
/// carries conflict markers and the guard's repository state refuses it, if
/// that state cannot be read, or if the file cannot be rewritten.
pub fn analyse(
    mode: Mode,
    guard: &ConflictGuard,
    directory: &Dir,
    display_path: &Utf8Path,
    storage_key: &Utf8Path,
    format: &Formatter,
) -> anyhow::Result<(FileReport, String)> {
    // Listing comes first, and reads nothing. The path *is* the report, so
    // there is nothing an assessment could add, and opening the file would
    // make the mode fail on a document it was never asked to look inside —
    // including one whose bytes are not UTF-8. See `REQ-GIT-010`.
    if mode == Mode::ListFiles {
        return Ok((
            FileReport {
                display_path: display_path.to_owned(),
                is_changed: false,
                delta: LineDelta::default(),
            },
            listed(display_path),
        ));
    }

    // The read capability is derived from the caller's, so every mode reads
    // through one type and only `--in-place` holds a capability that can write.
    let readable = ReadOnlyDir::new(
        directory
            .try_clone()
            .with_context(|| format!("duplicating the capability on {storage_key}"))?,
    );
    let assessment = assess(&readable, storage_key, format)?;
    report_line_endings(assessment.counts, "file", Some(display_path.as_str()));
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
        // A conflicted file is refused where the write would happen, and the
        // refusal is `is_changed`-gated: a file this run would not write has
        // nothing to refuse. Reflowing across a marker restructures text on
        // both sides of the boundary, so the user would resolve against
        // corrupted content and commit it into a rewritten history, where
        // `git rebase --abort` is gone. The one arm holds both the refusal and
        // the write because the state probe is itself a `Result`: a `match`
        // guard cannot ask with `?`, and a write whose guard went unread is not
        // a write this arm may make. The decision stays pure: it combines the
        // marker classification with the explicit state the adapter returns.
        Mode::InPlace if is_changed => {
            let markers = marker_state(&assessment.original);
            if markers == ConflictMarkerState::Present && refuses(markers, guard.operation_state()?)
            {
                return Err(anyhow!(
                    "refusing to rewrite {display_path}: it contains conflict markers and a \
                     merge, rebase, revert, or cherry-pick is in progress. Resolve it first, or \
                     pass --allow-conflicted to rewrite it anyway."
                ));
            }
            // The write is conditional on the file still holding the text the
            // assessment read. A file that changed in the meantime is not this
            // run's to overwrite: another writer's version is newer, and
            // replacing it would discard work this run never saw. Failing is
            // the honest answer — the user asked for a rewrite that did not
            // happen — and the message says which file and why.
            if !write_back(directory, storage_key, &assessment)? {
                return Err(anyhow!(
                    "not rewriting {display_path}: it changed while it was being formatted, so \
                     the text this run read is no longer there. Run mdtablefix again to format \
                     the file as it is now."
                ));
            }
            String::new()
        }
        // A clean file is left alone byte for byte, and nothing consults the
        // repository for it: the write would be invisible in the text but not
        // in the file. The replacement renames a temporary over the target, so
        // it would swap the inode and the modification time of a file it did
        // not change, and `make`-style staleness checks would see a rebuild
        // where there was nothing to rebuild.
        // Every remaining combination renders as nothing. `--list-files` is
        // named rather than matched by `_`, so that a sixth mode has to decide
        // what it prints instead of inheriting silence.
        Mode::InPlace | Mode::Check | Mode::Diff | Mode::ListFiles => String::new(),
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

#[cfg(test)]
#[path = "../driver_contract_tests.rs"]
mod contract_tests;
#[cfg(test)]
#[path = "../driver_in_place_tests.rs"]
mod in_place_tests;
#[cfg(test)]
#[path = "../driver_report_tests.rs"]
mod report_tests;
#[cfg(test)]
#[path = "../driver_test_support.rs"]
mod test_support;
