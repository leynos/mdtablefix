//! The candidate source: one `git ls-files` invocation.
//!
//! Depends on `std::process` and `camino`. It is the only place in the tree
//! that spawns `git`, and it deliberately knows nothing about extensions or
//! identity: it reports what the index holds, and [`crate::select::policy`]
//! decides what that means.
//!
//! `git`'s own diagnostics are relayed, never asserted on: they are localised
//! and version-dependent. Every message this module shows a user is a `Display`
//! impl this repository owns.

#[cfg(test)]
#[path = "git_ls_files_tests.rs"]
mod tests;
