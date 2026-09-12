//! Binary entry point for `mdtablefix`.
//!
//! Parses command-line arguments and coordinates Markdown formatting. When
//! file paths are supplied, they are analysed in parallel and reported in
//! argument order: each file may be printed, rewritten in place, checked for
//! drift, or shown as a unified diff. Without paths the tool reads from
//! standard input and prints results to stdout while preserving the input
//! order.
//!
//! Every mode shares one formatting closure, built once, so `--check` cannot
//! disagree with `--in-place` about what the formatter would write.

use std::{
    io::{self, Read, Write},
    process::ExitCode,
};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use clap::Parser;
use mdtablefix::{
    io::SourceDocument,
    report::{FileReport, render_summary},
};
use rayon::prelude::*;

mod command;
mod driver;
mod metrics;

use command::{Cli, FormatOpts, format_lines, formatting_closure};
use driver::{ExitStatus, Formatter, Inputs, Mode, analyse, exit_status, in_argument_order};
use metrics::{record_analysis, record_run};

/// Opens a file's parent directory and returns its relative UTF-8 path.
///
/// This is the only ambient filesystem boundary for CLI file processing. The
/// returned directory capability restricts subsequent handler I/O to the
/// selected file's parent directory.
///
/// The path is already UTF-8, because [`Inputs::resolve`] converts every
/// positional argument once, before any file is analysed.
fn open_file_parent(path: &Utf8Path) -> anyhow::Result<(Dir, Utf8PathBuf)> {
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
fn render_stdin_output(document: &SourceDocument<'_>, fixed: &[String]) -> String {
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
fn format_stdin(input: &str, opts: FormatOpts) -> String {
    let document = SourceDocument::parse(input);
    driver::report_line_endings(document.counts(), "stdin", None);
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
    path: &Utf8Path,
    format: &Formatter,
) -> anyhow::Result<(FileReport, String)> {
    open_file_parent(path)
        .and_then(|(directory, storage_key)| analyse(mode, &directory, path, &storage_key, format))
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

/// Runs the mode the command line selects, records the run, and returns its
/// status.
///
/// Resolution comes first, so a command line that cannot name its inputs fails
/// as a whole — through [`exit_status`], like every other operational failure —
/// rather than being discovered file by file. A file that cannot be read or
/// rewritten is reported as it is encountered, counted in the summary, and
/// folded into [`ExitStatus::Error`], so one unreadable file does not abandon
/// the rest.
///
/// The run is recorded here, at the boundary that decides the status and under
/// the very status this process is about to exit with, so the metric a host
/// aggregates and the exit code cannot disagree. A closed pipe is an ordinary
/// early exit, as in `mdtablefix --check *.md | head`: the reader stopped
/// early, which says nothing about this run, so the run is recorded as the
/// success it is. Rust ignores `SIGPIPE`, so the write reports `EPIPE` instead,
/// and `print!` would turn that into the undocumented status `101`. Such an
/// infrastructure failure of standard input or output is the only error that
/// leaves here, and it is recorded before it is returned.
fn run() -> anyhow::Result<ExitStatus> {
    let cli = Cli::parse();
    let mode = cli.mode();
    metrics::describe_metrics();
    let result = match Inputs::resolve(cli.files) {
        Ok(Inputs::Stdin) => run_stdin(cli.opts),
        Ok(Inputs::Files(files)) => run_files(mode, &files, cli.opts),
        Err(error) => {
            eprintln!("{error:?}");
            Ok(exit_status(mode, false, true))
        }
    };
    let result = match result {
        Err(error) if is_broken_pipe(&error) => Ok(ExitStatus::Success),
        settled => settled,
    };
    record_run(mode, result.as_ref().copied().unwrap_or(ExitStatus::Error));

    result
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

/// Analyses the named files under `mode`, in argument order.
///
/// The only errors returned here are writes to standard output; a file that
/// cannot be read or rewritten is reported and counted instead, and decides the
/// status along with the drift the reporting modes found.
fn run_files(mode: Mode, files: &[Utf8PathBuf], opts: FormatOpts) -> anyhow::Result<ExitStatus> {
    let format = formatting_closure(opts);
    let results = in_argument_order(
        files
            .par_iter()
            .enumerate()
            .map(|(index, path)| {
                (
                    index,
                    record_analysis(mode, || analyse_one(mode, path, &format)),
                )
            })
            .collect(),
    );

    let mut changed = 0;
    let mut unchanged = 0;
    let mut errored = 0;
    let mut stdout = io::stdout().lock();
    for result in results {
        match result {
            Ok((report, payload)) => {
                if report.is_changed {
                    changed += 1;
                } else {
                    unchanged += 1;
                }
                // A closed pipe surfaces here, and `main` treats it as the
                // early exit it is rather than as a failure to report.
                stdout.write_all(payload.as_bytes())?;
            }
            Err(error) => {
                // The chain matters: the outer context names the file, and the
                // cause explains the failure, such as a declined symlink.
                eprintln!("{error:?}");
                errored += 1;
            }
        }
    }
    stdout.flush()?;

    // The summary describes what was found, so it belongs to the modes that
    // report rather than print; the read-only modes keep standard output a
    // machine contract by putting it on standard error.
    if mode.reports() {
        eprintln!("{}", render_summary(changed, unchanged, errored));
    }

    Ok(exit_status(mode, changed > 0, errored > 0))
}

/// Entry point for the command-line tool that reflows broken markdown tables.
///
/// Parses command-line arguments to determine whether to process files in
/// place, check them for drift, show what would change, print fixed output to
/// standard output, or read from standard input. Handles file I/O, the
/// summary, and the exit status.
///
/// # Returns
///
/// `0` when every file was analysed and no reporting mode found drift, `1` when
/// a reporting mode did find drift, and `2` when a file could not be read or
/// rewritten.
///
/// # Examples
///
/// ```sh
/// # Fix tables in a file and print to stdout
/// mdtablefix myfile.md
///
/// # Fix tables in place
/// mdtablefix --in-place myfile.md
///
/// # Report which files would be reformatted, without writing
/// mdtablefix --check myfile.md
///
/// # Show what would change, without writing
/// mdtablefix --diff myfile.md
///
/// # Fix tables from standard input
/// cat myfile.md | mdtablefix
/// ```
fn main() -> ExitCode {
    match run() {
        Ok(status) => status.code(),
        Err(error) => {
            eprintln!("{error:?}");
            ExitStatus::Error.code()
        }
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
