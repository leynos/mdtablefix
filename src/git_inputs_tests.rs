//! Unit tests for what `--git` reports, and for what it refuses to report.
//!
//! Two boundaries with one theme: the composition root answers questions and
//! hands the answers on as data. A selection that lost candidates returns a
//! warning for the command boundary to print rather than printing it itself,
//! and the log line it emits carries a count rather than the extension values
//! the user typed, which are arbitrary text of arbitrary length.

// Wrapper over `tracing_test::traced_test`; see `test_macros` for why.
use test_macros::traced_test;

use super::{GitSelection, report_selection};
use crate::{
    driver::Inputs,
    select::{ConflictGuard, extensions::ExtensionFilter},
};

/// A selection that lost `skipped` candidates to the UTF-8 boundary.
fn selection(skipped: usize) -> GitSelection {
    GitSelection {
        inputs: Inputs::Files(Vec::new()),
        guard: ConflictGuard::unguarded(),
        skipped_non_utf8: skipped,
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

/// The selection's log line carries a count rather than the extension values.
///
/// `--md-exts` accepts any string, any number of times, so a field holding the
/// rendered filter would put caller-controlled text into every event a run
/// emits. The count answers the operator's question without that, and this test
/// is what keeps it that way: an extension value that appears in the log is a
/// failure, not a cosmetic difference.
#[test]
#[traced_test]
fn the_selection_log_names_no_extension_value() {
    let extensions: ExtensionFilter = ["md".to_owned(), "secret-internal-extension".to_owned()]
        .into_iter()
        .collect();

    report_selection(4, 2, &extensions);

    assert!(logs_contain("selected files from the repository"));
    assert!(logs_contain("candidates=4"));
    assert!(logs_contain("selected=2"));
    assert!(logs_contain("extension_count=2"));
    assert!(!logs_contain("secret-internal-extension"));
    assert!(
        !logs_contain("extensions="),
        "the event must carry no field holding the rendered filter"
    );
}
