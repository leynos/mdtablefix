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
//! crate owns. [`conflict`] holds the two predicates that keep a rewrite
//! from corrupting an in-progress merge resolution. [`extensions`] parses and
//! matches `--md-exts` values and knows nothing of either.

// `pub(crate)`, not `pub`: the tree is binary-private, and the composition root
// is a sibling module rather than a descendant, so it needs the path to reach
// `git_inputs`' wiring. Nothing here is reachable from `src/lib.rs`.
pub(crate) mod conflict;
pub(crate) mod extensions;
pub(crate) mod fs_probe;
pub(crate) mod git_ls_files;
pub(crate) mod git_output;
pub(crate) mod policy;
