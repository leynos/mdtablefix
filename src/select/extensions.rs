//! Parsing and matching of `--md-exts` values.
//!
//! Depends on `camino` and `thiserror` alone: no filesystem, no subprocess, no
//! capability. The type is named for what it does rather than for Markdown,
//! because `--md-exts` accepts any extension.

#[cfg(test)]
#[path = "extensions_tests.rs"]
mod tests;
