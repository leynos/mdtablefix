//! The composition roots where a direct environment read is sanctioned.
//!
//! ADR-0012 permits a direct read only at a genuine composition root, under an
//! item-scoped `#[expect(..., reason = "...")]` that warns once a seam becomes
//! available. Exempting every item-scoped `expect`, as this scan first did,
//! honoured the attribute but not the rule: a contributor could put the same
//! attribute on any function, add a `std::env` read beneath it, and pass both
//! Clippy and the scan. The roots are therefore named here, and an `expect` of
//! a protected lint anywhere else is an offence.
//!
//! Adding an entry is a policy decision, not a mechanical one: it belongs in
//! the same change as the ADR paragraph and the guide row that explain why the
//! site has no seam.

use camino::Utf8Path;

/// The file and item of each sanctioned composition root.
///
/// Both are executable roots that cannot be given a seam. `write_failure_child`
/// is re-executed by its parent, so the child's environment is the only channel
/// in; `ambient_variable` reads `PROPTEST_CASES`, proptest's own knob, set by
/// whoever runs the suite rather than by any caller in this repository.
pub const SANCTIONED_ROOTS: [(&str, &str); 2] = [
    ("tests/rewrite_atomic.rs", "write_failure_child"),
    ("tests/support/idempotence_harness.rs", "ambient_variable"),
];

/// Return whether `path` is exactly `expected`, written with `/`.
///
/// Components are compared rather than the rendered string, because the walk
/// joins paths with the platform separator: on Windows every path reads
/// `tests\rewrite_atomic.rs`, and a string comparison would match nothing
/// there while matching on Linux, which is a contract that guards one platform
/// and not the other.
fn path_is(path: &Utf8Path, expected: &str) -> bool {
    let mut components = path.components().map(|component| component.as_str());
    let mut wanted = expected.split('/');
    loop {
        match (components.next(), wanted.next()) {
            (None, None) => return true,
            (Some(actual), Some(expected)) if actual == expected => {}
            _ => return false,
        }
    }
}

/// Return whether an item-scoped `expect` at `path` sits on a sanctioned root.
///
/// An attribute with no enclosing named item is never sanctioned: a crate-root
/// attribute covers everything in the file, which is the evasion the scan
/// exists to report, and there is no item whose absence of a seam could have
/// been argued.
pub fn is_sanctioned(path: &Utf8Path, item: Option<&str>) -> bool {
    let Some(item) = item else {
        return false;
    };
    SANCTIONED_ROOTS
        .iter()
        .any(|(file, name)| *name == item && path_is(path, file))
}
