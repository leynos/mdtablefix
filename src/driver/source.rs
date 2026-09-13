//! Where the text to format comes from, and what the formatter made of it.
//!
//! [`Inputs`] resolves the command line's positional arguments into either
//! standard input or a list of paths. [`ReadOnlyDir`] and [`assess`] turn one
//! file behind a directory capability into an [`Assessment`] — the text as
//! read, paired with the text the formatter would write — and [`write_back`]
//! replaces the file with the second.
//!
//! None of this reports. The line-ending counts an assessment measured travel
//! on it, and the boundary in the parent module, which acts on them, is what
//! logs them.

use std::path::PathBuf;

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use mdtablefix::{
    LineEndingCounts,
    io::{SourceDocument, replace_file_if_unchanged},
};

use super::Formatter;

/// A directory capability that can only read.
///
/// The reporting modes receive this instead of a [`Dir`], so a wrong `match`
/// arm cannot write: read-only access is enforced by the type, not by
/// convention.
pub struct ReadOnlyDir(Dir);

impl ReadOnlyDir {
    /// Wraps a directory capability, discarding write access.
    #[must_use]
    pub fn new(directory: Dir) -> Self { Self(directory) }

    /// Reads `name` as UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not valid UTF-8.
    pub fn read(&self, name: &Utf8Path) -> anyhow::Result<String> {
        Ok(self.0.read_to_string(name)?)
    }
}

/// A file's current text paired with the text the formatter would write.
pub struct Assessment {
    /// The file's bytes as read, byte-order mark included.
    pub(super) original: String,
    /// What the shared formatter would write instead.
    pub(super) formatted: String,
    /// The line-ending counts the input was measured with.
    ///
    /// Carried rather than re-read so the boundary that acts on the answer can
    /// report it: the read-only query that produced the assessment emits
    /// nothing, and the counts are not computed twice.
    pub(super) counts: LineEndingCounts,
}

impl Assessment {
    /// Whether writing the formatted text would change the file's bytes.
    ///
    /// A direct byte comparison, and the authoritative answer. Comparing body
    /// text instead would report a document whose mark is lost as unchanged.
    #[must_use]
    pub fn is_changed(&self) -> bool { self.original != self.formatted }
}

/// Where the text to format comes from.
///
/// An explicit answer rather than "the file list is empty", because the two
/// facts that emptiness would conflate are not the same: "no paths were named,
/// so read standard input" and "the selected source resolved to no paths". The
/// second is a legitimate outcome for a source that discovers its own inputs —
/// it must be able to name nothing and still exit
/// [`ExitStatus::Success`](super::reporting::ExitStatus::Success), rather than
/// fall through to a standard input that may be a terminal. See `AX-6`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inputs {
    /// No paths were named: the document is read from standard input.
    Stdin,
    /// The named files, in argument order.
    Files(Vec<Utf8PathBuf>),
}

impl Inputs {
    /// Resolves the command line's positional arguments.
    ///
    /// The conversion to [`Utf8PathBuf`] happens once, here, rather than per
    /// file: a path that is not valid UTF-8 cannot name a file within a
    /// [`Dir`] capability, so a run containing one fails as a whole instead of
    /// being counted as a single file's error while its siblings proceed.
    ///
    /// # Errors
    ///
    /// Returns an error naming the offending path if any argument is not valid
    /// UTF-8.
    pub fn resolve(files: Vec<PathBuf>) -> anyhow::Result<Self> {
        if files.is_empty() {
            return Ok(Self::Stdin);
        }
        let files = files
            .into_iter()
            .map(|path| {
                Utf8PathBuf::from_path_buf(path)
                    .map_err(|path| anyhow!("converting {} to a UTF-8 path", path.display()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(Self::Files(files))
    }
}

/// Reads a file and pairs its text with the formatted result.
///
/// Takes [`ReadOnlyDir`], so this function cannot write, and it emits nothing:
/// the line-ending counts it measures travel on the returned [`Assessment`],
/// and the boundary that acts on them reports them. `storage_key` is the bare
/// file name within the capability, which is the name this function reads by.
///
/// # Errors
///
/// Returns an error if the file cannot be read or is not valid UTF-8.
pub fn assess(
    directory: &ReadOnlyDir,
    storage_key: &Utf8Path,
    format: &Formatter,
) -> anyhow::Result<Assessment> {
    let original = directory.read(storage_key)?;
    let document = SourceDocument::parse(&original);
    let counts = document.counts();
    let formatted = format(&document);

    Ok(Assessment {
        original,
        formatted,
        counts,
    })
}

/// Writes the formatted text back, if the file still holds the text it was
/// assessed from.
///
/// Only reachable from [`Mode::InPlace`](super::reporting::Mode::InPlace), and
/// only for a file whose bytes would change: see [`Assessment::is_changed`].
/// The replacement is atomic and capability-scoped, and it renames a temporary
/// over the target, so an unconditional call would swap the inode of a file it
/// left byte-identical. See [`mdtablefix::io::replace_file`].
///
/// The conditional entry point is the one used here rather than a plain
/// replacement: the assessment is a reading of the file that a concurrent
/// writer can invalidate while the formatter runs, and writing the formatted
/// text over the writer's version would discard work this run never saw. A
/// target that moved on is left exactly as that writer left it, and `Ok(false)`
/// says so; the boundary decides what that means for the run.
///
/// # Errors
///
/// Returns an error if the file cannot be written, or if it cannot be read back
/// for the comparison.
pub fn write_back(
    directory: &Dir,
    storage_key: &Utf8Path,
    assessment: &Assessment,
) -> anyhow::Result<bool> {
    Ok(replace_file_if_unchanged(
        directory,
        storage_key,
        &assessment.original,
        &assessment.formatted,
    )?)
}
