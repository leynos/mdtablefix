//! Capability-scoped fixture access for CLI matrix tests.

use anyhow::{Context as _, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Returns the repository-relative path to a matrix fixture.
pub(crate) fn fixture_path(file_name: &str) -> Utf8PathBuf {
    Utf8Path::new("tests")
        .join("data")
        .join("cli-matrix")
        .join(file_name)
}

pub(super) fn manifest_directory() -> Result<Dir> {
    Dir::open_ambient_dir(
        Utf8Path::new(env!("CARGO_MANIFEST_DIR")),
        ambient_authority(),
    )
    .context("open repository root capability")
}

/// Returns whether a repository fixture is accessible through its directory capability.
pub(crate) fn fixture_exists(file_name: &str) -> Result<bool> {
    Ok(manifest_directory()?
        .metadata(fixture_path(file_name))
        .is_ok())
}

/// Reads a matrix fixture through the repository directory capability.
pub(super) fn read_fixture(file_name: &str) -> Result<String> {
    let path = fixture_path(file_name);
    manifest_directory()?
        .read_to_string(&path)
        .with_context(|| format!("read matrix fixture '{path}'"))
}
