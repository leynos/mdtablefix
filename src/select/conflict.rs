//! The two predicates that make a mid-merge rewrite safe.
//!
//! Depends on `std::fs` and `camino`. Both predicates are deliberately narrow:
//! [`has_conflict_markers`] requires all three marker forms, each at the start
//! of a line with an exact seven-character run, so a document *discussing*
//! conflict markers is not mistaken for a conflicted one; and
//! [`operation_in_progress`] narrows the scan further, to a repository actually
//! mid-merge, mid-rebase, or mid-cherry-pick.
//!
//! Reflowing across the markers restructures text on both sides of the
//! boundary, so the user would resolve against corrupted content and commit it
//! into a rewritten history, where `git rebase --abort` is gone.

#[cfg(test)]
#[path = "conflict_tests.rs"]
mod tests;
