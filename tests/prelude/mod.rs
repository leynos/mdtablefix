//! Common imports for integration tests.

// Each integration test compiles this prelude as its own module and consumes a
// different subset of the shared utilities.
#[allow(unused_imports)]
pub use assert_cmd::{Command, prelude::*};
#[allow(unused_imports)]
pub use predicates::prelude::*;
#[allow(unused_imports)]
pub use rstest::{fixture, rstest};

#[macro_use]
#[path = "../common/mod.rs"]
mod common;
#[allow(unused_imports)]
pub use common::*;
