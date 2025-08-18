//! Common imports for integration tests.
#![allow(unfulfilled_lint_expectations)]

#[expect(unused_imports, reason = "re-exporting common test utilities")]
pub use assert_cmd::{Command, prelude::*};
#[expect(unused_imports, reason = "re-exporting common test utilities")]
pub use predicates::prelude::*;
#[expect(unused_imports, reason = "re-exporting common test utilities")]
pub use rstest::{fixture, rstest};

#[macro_use]
#[path = "../common/mod.rs"]
mod common;
#[expect(unused_imports, reason = "re-exporting common test utilities")]
pub use common::*;
