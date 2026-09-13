//! Selection of the files `--git` acts on.
//!
//! Binary-private, like [`crate::command`]: `src/lib.rs` does not declare it,
//! so the tree adds no public API. `src/main.rs` declares it alone.
//!
//! Dependencies point inwards. [`policy`] states the selection contract and
//! names the [`PathProbe`](policy::PathProbe) port; it imports neither
//! `std::fs`, `std::process`, nor `cap_std`, so the rule it states can be read
//! and tested without a filesystem. [`fs_probe`] and [`git_ls_files`] are the
//! adapters that answer it against the working tree and against
//! `git ls-files`, and [`git_output`] turns what `git` wrote into types this
//! crate owns. [`git_failure`] is what an invocation that did not succeed
//! means to the user who reads it and to the host that charts it. [`conflict`]
//! owns pure conflict policy and [`repository_state`] adapts the Git directory
//! to it at the write boundary. [`extensions`] parses and matches `--md-exts`
//! values and knows nothing of either.

// `pub(crate)`, not `pub`: the tree is binary-private, and the composition root
// is a sibling module rather than a descendant, so it needs the path to reach
// `git_inputs`' wiring. Nothing here is reachable from `src/lib.rs`.
pub(crate) mod conflict;
pub(crate) mod extensions;
pub(crate) mod fs_probe;
pub(crate) mod git_failure;
pub(crate) mod git_ls_files;
pub(crate) mod git_output;
pub(crate) mod policy;
pub(crate) mod repository_state;

pub(crate) use repository_state::ConflictGuard;

#[cfg(test)]
#[path = "select/conflict_tests.rs"]
mod conflict_tests;
