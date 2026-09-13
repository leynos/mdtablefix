//! Unit tests for what `--git` reports, and for what it refuses to report.
//!
//! Two boundaries with one theme: the composition root answers questions and
//! hands the answers on as data. A selection that lost candidates returns a
//! warning for the command boundary to print rather than printing it itself.

use super::{GitSelection, SelectionStatistics};
use crate::{driver::Inputs, select::ConflictGuard};

/// A selection that lost `skipped` candidates to the UTF-8 boundary.
fn selection(skipped: usize) -> GitSelection {
    GitSelection {
        inputs: Inputs::Files(Vec::new()),
        guard: ConflictGuard::unguarded(),
        skipped_non_utf8: skipped,
        selection: SelectionStatistics {
            candidates: 0,
            selected: 0,
            extension_count: 0,
        },
    }
}

/// A selection that lost candidates owes one warning, in this tool's own words.
///
/// The wording is pinned because a reader, a script, and the ADR's transcript
/// all recognize the run by it; the count is the only thing that varies.
#[test]
fn a_selection_that_lost_candidates_owes_one_warning() {
    assert_eq!(
        selection(1).skipped_warning().as_deref(),
        Some("mdtablefix: 1 file(s) not selected: their names are not valid UTF-8")
    );
    assert_eq!(
        selection(3).skipped_warning().as_deref(),
        Some("mdtablefix: 3 file(s) not selected: their names are not valid UTF-8")
    );
}

/// A selection that lost nothing says nothing, so a run over a tree of valid
/// UTF-8 paths writes no line to standard error at all.
#[test]
fn a_selection_that_lost_nothing_owes_no_warning() {
    assert_eq!(selection(0).skipped_warning(), None);
}
