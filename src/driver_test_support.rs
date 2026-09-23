//! Fixtures shared by the driver's unit tests.
//!
//! A module of its own so the three test modules can build the same
//! capability-scoped directories, and assert against the same two tables,
//! without an ambient filesystem write apiece. Every item is `pub(super)`
//! because the only modules that use them are `super`'s descendants.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use mdtablefix::io::SourceDocument;
use tempfile::{TempDir, tempdir};

use super::ReadOnlyDir;

/// A ragged table, whose every line the aligning formatter below replaces.
pub(super) const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// The aligned table the same formatter produces.
pub(super) const ALIGNED: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// A formatter that reproduces its input.
///
/// A true fixed point for these fixtures: the document boundary re-renders the
/// body with its own mark and line endings, so an unchanged assessment is
/// distinguishable from a changed one.
pub(super) fn identity(document: &SourceDocument<'_>) -> String {
    document.render(
        document
            .body()
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>()
            .as_slice(),
    )
}

/// A formatter that always produces the aligned table.
///
/// Every line of a ragged fixture differs from its aligned counterpart, so the
/// counts these tests assert are the three replacements `--check` reports.
pub(super) fn align(document: &SourceDocument<'_>) -> String {
    document.render(
        ALIGNED
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>()
            .as_slice(),
    )
}

/// Writes `content` as `name` in a fresh capability-scoped directory, creating
/// the directory `name` names if it has one.
///
/// The fixture is created through the capability rather than beside it, so a
/// test's only view of the file is the one every mode under test is handed.
/// The [`TempDir`] is returned so the caller keeps it alive for the length of
/// the test; dropping it would delete the directory the capability names.
pub(super) fn fixture(name: &str, content: &str) -> Result<(TempDir, Dir)> {
    let dir = tempdir().context("create temporary directory")?;
    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("the temporary directory path is not UTF-8: {path:?}"))?;
    let directory =
        Dir::open_ambient_dir(&path, ambient_authority()).context("open directory capability")?;
    let name = Utf8Path::new(name);
    if let Some(parent) = name.parent().filter(|parent| !parent.as_str().is_empty()) {
        directory
            .create_dir_all(parent)
            .with_context(|| format!("create fixture parent directory {parent}"))?;
    }
    directory
        .write(name, content)
        .with_context(|| format!("write fixture {name}"))?;

    Ok((dir, directory))
}

/// Reads `name` through the capability, so a rewrite is observed as the
/// capability sees it rather than through an ambient path.
pub(super) fn read(directory: &Dir, name: &str) -> Result<String> {
    directory
        .read_to_string(Utf8Path::new(name))
        .with_context(|| format!("read fixture {name}"))
}

/// A read-only view of `directory`.
pub(super) fn readable(directory: &Dir) -> Result<ReadOnlyDir> {
    directory
        .try_clone()
        .map(ReadOnlyDir::new)
        .context("duplicate directory capability")
}
