//! The refusal for a selected configuration file that is not there.
//!
//! A file the user names explicitly is not an automatic candidate, so its
//! absence cannot be treated as "nothing configured here": the run would
//! proceed on defaults the user believes they overrode. The case is split from
//! [`super::error_tests`] because it is the one error in that module raised
//! before any file is read, and moving it keeps that module under the line
//! limit.

use super::test_support::{Fixture, resolve};

/// A selected file that does not exist is an error, not a silent skip.
#[test]
fn a_missing_selected_file_is_an_error() {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();
    environment.insert("MDTABLEFIX_CONFIG_PATH", fixture.root().join("absent.toml"));

    let error = resolve(&["mdtablefix"], &environment).expect_err("the file is absent");

    assert!(
        error
            .to_string()
            .contains("required configuration file not found"),
        "{error}"
    );
}
