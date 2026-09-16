//! Finding the Rust sources the suppression scan reads.
//!
//! Separated from the judgement so that a change to what is read cannot be
//! mistaken for a change to what is refused.
//!
//! Sources are read through a `cap_std` directory handle rather than
//! `std::fs`: the scan has to see files that do not exist yet, so the
//! compile-time embedding the configuration contracts use is not an option
//! here.

use std::collections::VecDeque;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, DirEntry},
};

use super::SOURCE_EXTENSION;

/// What one directory entry contributes to the walk.
pub(super) enum Found {
    /// A subdirectory, with the capability to read it and its path.
    Directory(Dir, Utf8PathBuf),
    /// A Rust source, with its path and contents.
    Source(Utf8PathBuf, String),
    /// Anything else, which the scan does not govern.
    Ignored,
}

/// Directory names the walk does not descend into.
///
/// `target` holds build output, and a dot-prefixed name holds tool state such
/// as `.git`. Neither is a place a contributor writes a source Cargo compiles.
/// Everything else is walked, so a build script, bench, example or second
/// binary is seen wherever it is added.
pub(super) fn is_walkable(name: &str) -> bool { !name.starts_with('.') && name != "target" }

/// Render a walk prefix for a message, naming the root rather than showing an
/// empty string.
fn shown(prefix: &Utf8Path) -> &str {
    if prefix.as_str().is_empty() {
        "the repository root"
    } else {
        prefix.as_str()
    }
}

/// Classify one directory entry, reading it if it is a Rust source.
///
/// Split out from [`rust_sources`] so the walk reads as a walk: the name, type,
/// open and read steps are four more fallible operations that otherwise sit
/// between the loop and the one decision it makes.
fn classify_entry(current: &Dir, prefix: &Utf8Path, entry: &DirEntry) -> Result<Found> {
    let name = entry
        .file_name()
        .with_context(|| format!("read a file name in {}", shown(prefix)))?;
    let path = prefix.join(&name);
    if entry
        .file_type()
        .with_context(|| format!("stat {path}"))?
        .is_dir()
    {
        if !is_walkable(&name) {
            return Ok(Found::Ignored);
        }
        let child = current
            .open_dir(&name)
            .with_context(|| format!("open {path}"))?;
        return Ok(Found::Directory(child, path));
    }
    if path.extension() != Some(SOURCE_EXTENSION) {
        return Ok(Found::Ignored);
    }
    let contents = current
        .read_to_string(&name)
        .with_context(|| format!("read {path}"))?;
    Ok(Found::Source(path, contents))
}

/// Read every `.rs` file under `relative`, breadth first, with its contents.
///
/// Each directory is opened through its parent's capability rather than by
/// absolute path, so the walk cannot leave the tree it was handed. Pass `"."`
/// for `relative` to cover a whole repository; the paths then come back
/// relative to its root.
pub fn rust_sources(root: &Utf8Path, relative: &str) -> Result<Vec<(Utf8PathBuf, String)>> {
    let directory = Dir::open_ambient_dir(root.join(relative), ambient_authority())
        .with_context(|| format!("open {relative}"))?;
    let base = if relative == "." {
        Utf8PathBuf::new()
    } else {
        Utf8PathBuf::from(relative)
    };
    let mut pending = VecDeque::from([(directory, base)]);
    let mut sources = Vec::new();

    while let Some((current, prefix)) = pending.pop_front() {
        let entries = current
            .entries()
            .with_context(|| format!("read {}", shown(&prefix)))?;
        for candidate in entries {
            let entry =
                candidate.with_context(|| format!("read an entry of {}", shown(&prefix)))?;
            match classify_entry(&current, &prefix, &entry)? {
                Found::Directory(child, path) => pending.push_back((child, path)),
                Found::Source(path, contents) => sources.push((path, contents)),
                Found::Ignored => {}
            }
        }
    }
    Ok(sources)
}
