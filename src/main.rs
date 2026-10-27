//! Binary entry point for `mdtablefix`.
//!
//! Parses command-line arguments, resolves them against any configured
//! settings, and coordinates Markdown formatting. When file paths are supplied,
//! they are analysed in parallel and reported in argument order: each file may
//! be printed, rewritten in place, checked for drift, or shown as a unified
//! diff. Without paths the tool reads from standard input and prints results to
//! stdout while preserving the input order.
//!
//! Every mode shares one formatting closure, built once, so `--check` cannot
//! disagree with `--in-place` about what the formatter would write.
//!
//! This file is the composition root and nothing more: it parses, resolves the
//! settings, selects the inputs, and reports. What a run does lives in
//! [`driver`], and how the settings are layered lives in [`config`].

use std::process::ExitCode;

use tracing::debug;

mod command;
mod config;
mod driver;
mod git_inputs;
mod metrics;
mod select;

use command::Cli;
use config::Settings;
use driver::{ExitStatus, Inputs, Mode, exit_status};
// The document-boundary helpers moved into `driver` with the rest of the run,
// but the tests that pin their behaviour live here and name them through
// `super`, so the composition root keeps a binding for them.
#[cfg(test)]
use driver::{format_stdin, open_file_parent, render_stdin_output};
use metrics::record_run;
use select::ConflictGuard;

/// Ends a run that failed before any file was processed, and records it.
///
/// A failure to name the inputs never reaches the loop that analyses them, so
/// without this it would be the one class of failure a host could not count: a
/// missing working directory, a repository that cannot be listed, a path
/// argument that cannot be resolved, or a configuration source that cannot be
/// merged would each exit non-zero having recorded nothing. The caller reports
/// the failure; this decides the status and records it, so every way a run can
/// end is counted once and the metric cannot disagree with the exit code.
fn failed_run(mode: Mode) -> ExitStatus {
    let status = exit_status(mode, false, true);
    record_run(mode, status);

    status
}

/// Resolves the `--git` selection, reporting any warning it earned.
///
/// The skipped-candidate warning is printed here rather than inside `resolve`,
/// which answers questions: a query that writes to standard error cannot be
/// reused by a caller that wants the selection without the commentary. See
/// [`git_inputs::GitSelection::skipped_warning`].
fn select_from_git(settings: &Settings, mode: Mode) -> Option<(Inputs, ConflictGuard)> {
    // The working directory is resolved before anything is selected, because it
    // is what `--git` is relative to: `git ls-files` runs in it, the selection
    // is reported relative to it, and the repository that governs it is the one
    // whose in-progress operation the guard looks for.
    let working_directory = match git_inputs::working_directory() {
        Ok(working_directory) => working_directory,
        Err(error) => {
            eprintln!("{error:?}");
            return None;
        }
    };
    match git_inputs::resolve(settings, mode, &working_directory) {
        Ok(selection) => {
            if let Some(warning) = selection.skipped_warning() {
                eprintln!("{warning}");
            }
            report_selection(&selection);
            Some((selection.inputs, selection.guard))
        }
        Err(error) => {
            // One deliberate line: this tool's own wording, with the underlying
            // reason relayed beside it. See `GitInputsError::diagnostic`.
            eprintln!("mdtablefix: {}", error.diagnostic());
            None
        }
    }
}

/// Resolves the input source the settings name, reporting any failure.
///
/// `settings.git` already folds in the rule that a typed path outranks a
/// configured one: [`Settings::resolve`] clears it whenever positional
/// arguments are present, because a positional argument is the user's own
/// statement of what to act on and a file cannot make it on their behalf.
///
/// `None` means the run has already been reported and only its status is left
/// to decide, which the caller does through [`failed_run`].
fn select_inputs(settings: &Settings, mode: Mode) -> Option<(Inputs, ConflictGuard)> {
    if settings.git {
        return select_from_git(settings, mode);
    }
    match Inputs::resolve(settings.files.clone()) {
        Ok(inputs) => Some((inputs, ConflictGuard::unguarded())),
        Err(error) => {
            eprintln!("{error:?}");
            None
        }
    }
}

/// Runs the mode the command line selects, records the run, and returns its
/// status.
///
/// Resolution comes first, so a command line that cannot name its inputs — or a
/// configuration source that cannot be merged — fails as a whole, through
/// [`exit_status`], like every other operational failure, rather than being
/// discovered file by file. A file that cannot be read or rewritten is reported
/// as it is encountered, counted in the summary, and folded into
/// [`ExitStatus::Error`], so one unreadable file does not abandon the rest.
///
/// The run is recorded here, at the boundary that decides the status and under
/// the very status this process is about to exit with, so the metric a host
/// aggregates and the exit code cannot disagree. A resolution failure returns
/// through [`failed_run`] instead, which records it the same way.
fn run() -> anyhow::Result<ExitStatus> {
    let (cli, matches) = Cli::parse_validated();
    let mode = cli.mode();
    metrics::describe_metrics();
    let settings = match Settings::resolve(&cli, &matches) {
        Ok(settings) => settings,
        Err(error) => {
            // One deliberate line, in this tool's own words, so the stream and
            // the status match every other operational failure. Configuration
            // is resolved here, before any file is touched, so a bad file
            // cannot half-format a tree.
            eprintln!("mdtablefix: {error}");
            return Ok(failed_run(mode));
        }
    };
    let Some((inputs, guard)) = select_inputs(&settings, mode) else {
        return Ok(failed_run(mode));
    };
    let result = driver::execute(mode, &guard, inputs, settings.opts);
    record_run(mode, result.as_ref().copied().unwrap_or(ExitStatus::Error));

    result
}

/// Emits the bounded facts a repository selection returned.
///
/// Extension values and paths are caller-controlled, so only counts cross this
/// command boundary. The selection query itself returns these facts without
/// emitting an event, which lets another delivery mechanism reuse it silently.
fn report_selection(selection: &git_inputs::GitSelection) {
    let statistics = selection.selection;
    debug!(
        candidates = statistics.candidates,
        selected = statistics.selected,
        extension_count = statistics.extension_count,
        "selected files from the repository"
    );
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
/// rewritten, when the inputs could not be resolved, or when a configuration
/// source could not be merged.
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
///
/// # Reformat the repository's tracked Markdown in place
/// mdtablefix --git --in-place
///
/// # List the files --git would act on, without reading any of them
/// mdtablefix --git --list-files
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

#[cfg(test)]
#[path = "main_git_selection_tests.rs"]
mod git_selection_tests;
