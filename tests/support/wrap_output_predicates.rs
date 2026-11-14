//! Predicates over rendered wrapping output.
//!
//! Both helpers answer a question about *rendered* Markdown rather than about
//! the wrapping algorithm: whether the output contains a padded code span, and
//! whether a line opens a task-list item. They live here rather than beside the
//! properties so `tests/wrap_properties.rs` stays inside the 400-line file cap
//! `AGENTS.md` sets; the properties were pushed over it by the doc comments
//! these helpers need to stay self-explanatory.

/// Reports whether what sits between two backticks is a code span carrying the
/// MD038 defect: a span whose content begins or ends with a space.
///
/// An empty pair of backticks is not a span, so it is never padded; it reports
/// `false` rather than being special-cased by every caller.
pub fn is_space_padded_code_span(code: &str) -> bool {
    if code.is_empty() {
        return false;
    }
    code.starts_with(' ') || code.ends_with(' ')
}

/// Reports whether `line` opens a GitHub task-list item.
///
/// Both spellings count: an unticked box and a ticked one are the same
/// construct, and the property that uses this only cares that a marker was
/// emitted.
pub fn is_checklist_marker(line: &str) -> bool {
    line.starts_with("- [ ] ") || line.starts_with("- [x] ")
}
