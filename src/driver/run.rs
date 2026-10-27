//! Running a command that has already been resolved.
//!
//! One entry point, [`execute`], dispatches on the resolved [`Inputs`]: standard
//! input is formatted and printed, and a file selection is analysed under the
//! chosen [`Mode`]. A closed pipe is an ordinary early exit here rather than an
//! error, because the reader stopping says nothing about this run.
//!
//! Declared from `src/driver.rs` rather than from `src/main.rs`, so the
//! composition root stays a description of what a run *is* — parse, resolve the
//! settings, select the inputs — and this module describes what a run *does*.
//! That split is what keeps `src/main.rs` under its 400-line cap once the
//! configuration wiring arrives.

use std::io::{self, BufWriter, Read, Write};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use mdtablefix::{io::SourceDocument, report::render_summary};
use rayon::prelude::*;

use super::{
    ExitStatus,
    Formatter,
    Inputs,
    Mode,
    analyse,
    exit_status,
    in_argument_order,
    report_line_endings,
};
use crate::{
    command::{FormatOpts, format_lines, formatting_closure},
    metrics::record_analysis,
    select::ConflictGuard,
};

/// Opens a file's parent directory and returns its relative UTF-8 path.
///
/// This is the only ambient filesystem boundary for CLI file processing. The
/// returned directory capability restricts subsequent handler I/O to the
/// selected file's parent directory.
///
/// The path is already UTF-8, because [`Inputs::resolve`] converts every
/// positional argument once, before any file is analysed.
pub(crate) fn open_file_parent(path: &Utf8Path) -> anyhow::Result<(Dir, Utf8PathBuf)> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
        .unwrap_or(Utf8Path::new("."));
    let file_name = path
        .file_name()
        .with_context(|| format!("selecting file name from {path}"))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .with_context(|| format!("opening {parent}"))?;

    Ok((directory, Utf8PathBuf::from(file_name)))
}

/// Renders standard-input output.
///
/// Standard input keeps its historical contract of printing one terminator
/// even when it produces no lines, whereas an empty file produces empty
/// output. `tests/parallel.rs` pins that difference.
///
/// An input that produces no lines prints the bare terminator and nothing
/// else, so a byte-order mark on a mark-only input is not echoed. Nothing is
/// written back to a file on this path, so the mark cannot be lost.
pub(crate) fn render_stdin_output(document: &SourceDocument<'_>, fixed: &[String]) -> String {
    if fixed.is_empty() {
        document.ending().as_str().to_string()
    } else {
        document.render(fixed)
    }
}

/// Formats standard input and renders it for standard output.
///
/// Standard input is the boundary with no path to name, so its report says so
/// rather than omitting the field. The document boundary is otherwise taken
/// exactly as it is for a file: the body carries no byte-order mark, and the
/// majority style of the input decides the terminators.
pub(crate) fn format_stdin(input: &str, opts: FormatOpts) -> String {
    let document = SourceDocument::parse(input);
    report_line_endings(document.counts(), "stdin", None);
    render_stdin_output(&document, &format_lines(document.body(), opts))
}

/// Analyses one command-line path, naming it in any error.
///
/// The context encloses opening the parent directory as well as the analysis,
/// so an error from either names the file as the user wrote it rather than
/// only its parent. The mode's verb says which operation failed: `--in-place`
/// writes, and every other mode reads.
fn analyse_one(
    mode: Mode,
    guard: &ConflictGuard,
    path: &Utf8Path,
    format: &Formatter,
) -> anyhow::Result<(mdtablefix::report::FileReport, String)> {
    open_file_parent(path)
        .and_then(|(directory, storage_key)| {
            analyse(mode, guard, &directory, path, &storage_key, format)
        })
        .with_context(|| format!("{} {}", mode.verb(), path))
}

/// Writes `text` to standard output.
///
/// Standard output is written through an explicit handle rather than `print!`,
/// which panics on a write failure. A closed pipe is a normal early exit, and a
/// panic here would exit `101`, a status this tool does not document.
fn write_stdout(text: &str) -> anyhow::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(text.as_bytes())?;
    stdout.flush()?;

    Ok(())
}

/// Whether any cause in the chain is a write to a closed pipe.
fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
    })
}

/// Runs the mode the resolved settings select, and returns its status.
///
/// A resolution failure never reaches here; it is reported and recorded by the
/// composition root. A file that cannot be read or rewritten is reported as it
/// is encountered, counted in the summary, and folded into
/// [`ExitStatus::Error`], so one unreadable file does not abandon the rest.
///
/// A closed pipe is an ordinary early exit, as in `mdtablefix --check *.md |
/// head`: the reader stopped early, which says nothing about this run, so the
/// run is treated as the success it is. Rust ignores `SIGPIPE`, so the write
/// reports `EPIPE` instead, and `print!` would turn that into the undocumented
/// status `101`.
///
/// # Errors
///
/// Returns an error only for an infrastructure failure of standard input or
/// output. The caller records it before returning it.
pub(crate) fn execute(
    mode: Mode,
    guard: &ConflictGuard,
    inputs: Inputs,
    opts: FormatOpts,
) -> anyhow::Result<ExitStatus> {
    let result = match inputs {
        Inputs::Stdin => run_stdin(opts),
        Inputs::Files(files) => run_files(mode, guard, &files, opts),
    };

    match result {
        Err(error) if is_broken_pipe(&error) => Ok(ExitStatus::Success),
        settled => settled,
    }
}

/// Formats standard input and writes the result to standard output.
///
/// The destination is standard output whatever the mode, because there is no
/// file for a mode flag to act on: the parser's `inputs` group is what
/// guarantees a mode flag arrives with a file argument instead.
fn run_stdin(opts: FormatOpts) -> anyhow::Result<ExitStatus> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    write_stdout(&format_stdin(&input, opts))?;

    Ok(ExitStatus::Success)
}

/// The number of files analysed in one parallel batch.
///
/// A selection can be an entire repository, so the results are drained in
/// chunks rather than in one pass: the retained payloads are then one chunk's
/// rather than the whole tree's, and the ordering stage below is what makes the
/// chunking unobservable — the output is argument order either way. 256 is a
/// compromise between the parallel fan-out each chunk buys and that bound.
const ANALYSIS_CHUNK: usize = 256;

/// What a run found across the files it analysed.
///
/// Carried as one value rather than three locals so the summary line and the
/// status are read from a single source, and so counting a result stays in one
/// place that both the per-chunk loop and its tests can reach.
#[derive(Default)]
struct Tally {
    /// Files whose content the mode would change.
    changed: usize,
    /// Files the mode found already correct.
    unchanged: usize,
    /// Files that could not be read, rewritten or reported.
    errored: usize,
}

impl Tally {
    /// Counts one analysed file, writing its payload when it has one.
    ///
    /// An error is reported here rather than returned, because one unreadable
    /// file must not abandon the rest of the selection. The chain matters: the
    /// outer context names the file, and the cause explains the failure, such
    /// as a declined symlink or a refused conflicted file.
    ///
    /// The only error returned is a write to standard output. A closed pipe
    /// surfaces here, and `execute` treats it as the early exit it is rather
    /// than as a failure to report.
    fn account(
        &mut self,
        result: anyhow::Result<(mdtablefix::report::FileReport, String)>,
        stdout: &mut impl Write,
    ) -> anyhow::Result<()> {
        match result {
            Ok((report, payload)) => {
                if report.is_changed {
                    self.changed += 1;
                } else {
                    self.unchanged += 1;
                }
                stdout.write_all(payload.as_bytes())?;
            }
            Err(error) => {
                eprintln!("{error:?}");
                self.errored += 1;
            }
        }

        Ok(())
    }
}

/// Analyses one chunk of files, in argument order.
///
/// `--in-place` rewrites the tree, so its files are analysed one at a time and
/// each sees what the file before it wrote. Every other mode only reads, so the
/// chunk is analysed in parallel and then sorted back into argument order: the
/// output is argument order whichever path ran, which is what makes the
/// chunking unobservable.
fn analyse_chunk(
    mode: Mode,
    guard: &ConflictGuard,
    chunk: &[Utf8PathBuf],
    format: &Formatter,
) -> Vec<anyhow::Result<(mdtablefix::report::FileReport, String)>> {
    match mode {
        Mode::InPlace => chunk
            .iter()
            .map(|path| record_analysis(mode, path, || analyse_one(mode, guard, path, format)))
            .collect(),
        Mode::Print | Mode::Check | Mode::Diff | Mode::ListFiles => in_argument_order(
            chunk
                .par_iter()
                .enumerate()
                .map(|(index, path)| {
                    (
                        index,
                        record_analysis(mode, path, || analyse_one(mode, guard, path, format)),
                    )
                })
                .collect(),
        ),
    }
}

/// Analyses the named files under `mode`, in argument order.
///
/// The only errors returned here are writes to standard output; a file that
/// cannot be read or rewritten is reported and counted instead, and decides the
/// status along with the drift the reporting modes found.
fn run_files(
    mode: Mode,
    guard: &ConflictGuard,
    files: &[Utf8PathBuf],
    opts: FormatOpts,
) -> anyhow::Result<ExitStatus> {
    let format = formatting_closure(opts);
    // One lock and one buffer for the run, not one per file: the results are
    // already assembled in memory, so the only question is how many syscalls
    // they cost.
    let mut stdout = BufWriter::new(io::stdout().lock());
    let mut tally = Tally::default();
    for chunk in files.chunks(ANALYSIS_CHUNK) {
        for result in analyse_chunk(mode, guard, chunk, &format) {
            tally.account(result, &mut stdout)?;
        }
    }
    stdout.flush()?;

    // The summary describes what was found, so it belongs to the modes that
    // report rather than print; the read-only modes keep standard output a
    // machine contract by putting it on standard error.
    if mode.reports() {
        eprintln!(
            "{}",
            render_summary(tally.changed, tally.unchanged, tally.errored)
        );
    }

    Ok(exit_status(mode, tally.changed > 0, tally.errored > 0))
}
