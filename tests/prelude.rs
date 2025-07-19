//! Common imports for integration tests.

#[allow(unused_imports)] // re-exporting for test modules
pub use assert_cmd::Command;

#[path = "common/mod.rs"]
pub mod common;
