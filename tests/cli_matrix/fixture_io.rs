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
///
/// Only a missing fixture reports `Ok(false)`; any other metadata failure
/// propagates, so an unreadable repository root cannot read as an absent
/// fixture and silently skip the case it belongs to.
pub(crate) fn fixture_exists(file_name: &str) -> Result<bool> {
    let path = fixture_path(file_name);
    metadata_presence(&path, manifest_directory()?.metadata(&path))
}

/// Maps a fixture metadata query to presence.
///
/// `Ok(false)` selects the caller's "fixture absent" path, so collapsing a
/// permission or I/O failure into it would drop the case instead of reporting
/// the problem. Only `NotFound` means absent; everything else keeps its error
/// and gains the path that produced it.
pub(super) fn metadata_presence<T>(path: &Utf8Path, result: std::io::Result<T>) -> Result<bool> {
    match result {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("inspect matrix fixture '{path}'")),
    }
}

/// Reads a matrix fixture through the repository directory capability.
pub(super) fn read_fixture(file_name: &str) -> Result<String> {
    let path = fixture_path(file_name);
    manifest_directory()?
        .read_to_string(&path)
        .with_context(|| format!("read matrix fixture '{path}'"))
}
