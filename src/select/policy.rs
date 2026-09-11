//! What `--git` selects, stated as a rule rather than as a procedure.
//!
//! Depends on the [`PathProbe`] port, [`ExtensionFilter`], and `camino` alone.
//! Nothing here imports `std::fs`, `std::process`, or `cap_std`: the rule is
//! testable against a fake probe, and no test needs to change directory. The
//! adapters that answer the port live in [`crate::select::fs_probe`].

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
