//! Common imports for integration tests.

#[allow(unused_imports)] // re-exporting for test modules
pub use assert_cmd::Command;
#[allow(unused_imports)]
pub use rstest::{fixture, rstest};

#[macro_use]
#[path = "../common/mod.rs"]
mod common;
#[allow(unused_imports)]
pub use common::*;
