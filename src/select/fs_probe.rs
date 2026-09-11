//! The working-tree adapter for [`PathProbe`](crate::select::policy::PathProbe).
//!
//! Depends on `std::fs` and `camino`. It is the only place in the selection
//! tree that inspects the filesystem, and it reads metadata alone: selection
//! never opens a file, so a path this module reports as a regular file is not
//! thereby readable, and the writer still decides what to do about that.
//!
//! Classification uses `symlink_metadata`, not `metadata`, because a link's
//! extension says nothing about its target and rewriting through one escapes
//! the selection.

#[cfg(test)]
#[path = "fs_probe_tests.rs"]
mod tests;
