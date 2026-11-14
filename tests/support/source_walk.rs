//! Collects every Rust source beneath a directory, skipping build output.
//!
//! The dependency contract needs to read the whole tree to decide which files
//! name the configuration framework, and the walk that does so is large enough
//! to be worth stating on its own: excluded directories, error policy, and
//! ordering are all decisions a caller would otherwise have to reconstruct from
//! a loop. Declared with `#[path]` from the one test file that uses it, so each
//! integration-test binary gets its own copy and this module carries no item
//! that copy does not call.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Whether `directory` is one the walk should descend into.
///
/// `target` holds build output and a dotfile directory holds editor or tooling
/// state; neither can contain a source this crate compiles, and descending into
/// `target` alone would dwarf the sources the check reads.
fn is_walkable(directory: &Path) -> bool {
    let name = directory
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());

    !name.starts_with('.') && name != "target"
}

/// The entries of `directory`, or a failure naming it.
///
/// Kept separate so the walk reads as a visit rather than as error plumbing,
/// and so the diagnostic that names an unreadable directory is written once.
fn entries_of(directory: &Path) -> Result<Vec<std::fs::DirEntry>> {
    std::fs::read_dir(directory)
        .with_context(|| format!("listing {}", directory.display()))?
        .map(|entry| entry.with_context(|| format!("reading an entry of {}", directory.display())))
        .collect()
}

/// What the walk should do with one directory entry.
///
/// Naming the three outcomes lets the visit dispatch on a value instead of
/// nesting its tests: the loop body stays one `match` however many rules a
/// classification grows.
enum Step {
    /// A directory worth descending into.
    Descend(PathBuf),
    /// A Rust source to collect.
    Source(PathBuf),
    /// Something the walk ignores, such as build output or a non-source file.
    Ignore,
}

/// Classifies one entry of the walk.
///
/// A directory the walk should not descend into is ignored rather than
/// collected, and a file is collected only for its `.rs` suffix. The suffix
/// test goes through the lossy name because a non-UTF-8 file name cannot end in
/// `.rs` as this crate's sources do.
fn classify(entry: &std::fs::DirEntry) -> Step {
    let path = entry.path();
    if path.is_dir() {
        return if is_walkable(&path) {
            Step::Descend(path)
        } else {
            Step::Ignore
        };
    }
    if entry.file_name().to_string_lossy().ends_with(".rs") {
        return Step::Source(path);
    }

    Step::Ignore
}

/// The two things one directory contributes to the walk.
///
/// Separating them keeps the walk a loop over directories rather than a loop
/// over directories containing a loop over entries containing a dispatch: each
/// directory is read once, and the result says where the walk goes next.
#[derive(Default)]
struct Visit {
    /// Subdirectories still to walk.
    descend: Vec<PathBuf>,
    /// Rust sources found directly in this directory.
    sources: Vec<PathBuf>,
}

/// Reads one directory and classifies each of its entries.
///
/// The failure names the directory, because an unreadable one aborts the walk
/// rather than shrinking it.
fn visit_directory(directory: &Path) -> Result<Visit> {
    let mut visit = Visit::default();
    for entry in entries_of(directory)? {
        match classify(&entry) {
            Step::Descend(path) => visit.descend.push(path),
            Step::Source(path) => visit.sources.push(path),
            Step::Ignore => {}
        }
    }

    Ok(visit)
}

/// Collects every Rust source beneath `root`, excluding `target` and dotfiles.
///
/// A breadth-first walk written against `std::path`, so it behaves the same way
/// on every platform the CI matrix covers. A directory that cannot be read is
/// fatal rather than skipped: an incomplete walk could miss a library source
/// that names the framework, and so satisfy the invariant for the wrong reason.
///
/// # Errors
///
/// Returns an error naming any directory that cannot be read.
pub fn rust_sources(root: &Path) -> Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(directory) = pending.pop() {
        let visit = visit_directory(&directory)?;
        pending.extend(visit.descend);
        found.extend(visit.sources);
    }
    found.sort();

    Ok(found)
}
