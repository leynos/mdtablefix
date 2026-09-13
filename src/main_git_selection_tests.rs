//! Tests for the command boundary's repository-selection event.

use test_macros::traced_test;

use super::{git_inputs::SelectionStatistics, report_selection};
use crate::{driver::Inputs, git_inputs::GitSelection, select::ConflictGuard};

/// The event carries bounded counts and never a configured extension value.
#[test]
#[traced_test]
fn the_selection_log_names_no_extension_value() {
    let selection = GitSelection {
        inputs: Inputs::Files(Vec::new()),
        guard: ConflictGuard::unguarded(),
        skipped_non_utf8: 0,
        selection: SelectionStatistics {
            candidates: 4,
            selected: 2,
            extension_count: 2,
        },
    };

    report_selection(&selection);

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
