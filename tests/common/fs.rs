//! Capability-scoped filesystem support for integration tests.

use anyhow::{Context as _, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

/// Owns a temporary directory and its capability-scoped filesystem handle.
pub(crate) struct TestDir {
    directory: Dir,
    path: Utf8PathBuf,
    _guard: TempDir,
}

impl TestDir {
    /// Creates a temporary directory with a UTF-8 absolute path.
    pub(crate) fn new() -> Result<Self> {
        let guard = TempDir::new().context("create temporary test directory")?;
        let path = Utf8PathBuf::from_path_buf(guard.path().to_path_buf())
            .map_err(|path| anyhow!("temporary directory path is not UTF-8: {}", path.display()))?;
        let directory = Dir::open_ambient_dir(&path, ambient_authority())
            .context("open temporary directory capability")?;
        Ok(Self {
            directory,
            path,
            _guard: guard,
        })
    }

    /// Returns the capability used for relative filesystem operations.
    pub(crate) fn directory(&self) -> &Dir { &self.directory }

    /// Returns the UTF-8 absolute path used at process boundaries.
    pub(crate) fn path(&self) -> &Utf8Path { &self.path }
}
