//! Selection of the files `--git` acts on.
//!
//! Binary-private, like [`crate::cli`]: `src/lib.rs` does not declare it, so the
//! tree adds no public API. `src/main.rs` declares it alone.
//!
//! Dependencies point inwards. [`policy`] states the selection contract and
//! names the [`PathProbe`](policy::PathProbe) port; it imports neither
//! `std::fs`, `std::process`, nor `cap_std`, so the rule it states can be read
//! and tested without a filesystem. [`fs_probe`] and [`git_ls_files`] are the
//! adapters that answer it against the working tree and against
//! `git ls-files`. [`conflict`] holds the two predicates that keep a rewrite
//! from corrupting an in-progress merge resolution. [`extensions`] parses and
//! matches `--md-exts` values and knows nothing of either.

mod conflict;
mod extensions;
mod fs_probe;
mod git_ls_files;
mod policy;
