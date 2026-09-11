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
    borrow::Cow,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use clap::Parser;
use mdtablefix::{
    Options,
    format_breaks,
    io::SourceDocument,
    process::{process_stream_inner, process_with_frontmatter},
    renumber_lists,
    report::{FileReport, render_summary},
};
use rayon::prelude::*;

mod driver;

use driver::{ExitStatus, Formatter, Mode, analyse, exit_status, in_argument_order};

#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
#[command(group(clap::ArgGroup::new("mode").multiple(false).requires("files")))]
struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place", group = "mode")]
    in_place: bool,
    /// Report which files would be reformatted, and by how many lines
    #[arg(long = "check", group = "mode")]
    check: bool,
    /// Print a unified diff for each file that would be reformatted
    #[arg(long = "diff", group = "mode")]
    diff: bool,
    #[command(flatten)]
    opts: FormatOpts,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}

impl Cli {
    /// The mode the flags select.
    ///
    /// The argument group already guarantees that at most one flag is set, and
    /// that any flag at all requires files, so these branches cannot disagree
    /// with the parser: they only name the decision it made.
    fn mode(&self) -> Mode {
        if self.in_place {
            Mode::InPlace
        } else if self.check {
            Mode::Check
        } else if self.diff {
            Mode::Diff
        } else {
            Mode::Print
        }
    }
}

#[derive(clap::Args, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI exposes independent flags via separate switches"
)]
struct FormatOpts {
    /// Wrap paragraphs and list items to 80 columns
    #[arg(long = "wrap")]
    wrap: bool,
    /// Renumber ordered list items
    #[arg(long = "renumber")]
    renumber: bool,
    /// Reformat thematic breaks as underscores
    #[arg(long = "breaks")]
    breaks: bool,
    /// Replace "..." with the ellipsis character
    #[arg(long = "ellipsis")]
    ellipsis: bool,
    /// Normalise fence delimiters to three backticks
    #[arg(long = "fences")]
    fences: bool,
    /// Convert bare numeric references and the final numbered list to
    /// Markdown footnote links
    #[arg(long = "footnotes")]
    footnotes: bool,
    /// Fix emphasis markers adjacent to inline code
    #[arg(long = "code-emphasis")]
    code_emphasis: bool,
    /// Convert Setext-style headings to hash-prefixed headings
    #[arg(long = "headings")]
    headings: bool,
}

impl From<FormatOpts> for Options {
    fn from(opts: FormatOpts) -> Self {
        Self {
            wrap: opts.wrap,
            ellipsis: opts.ellipsis,
            fences: opts.fences,
            footnotes: opts.footnotes,
            code_emphasis: opts.code_emphasis,
            headings: opts.headings,
        }
    }
}

fn process_lines(lines: &[String], opts: FormatOpts) -> Vec<String> {
    process_with_frontmatter(lines, |body| {
        let mut out = process_stream_inner(body, opts.into());
        if opts.renumber {
            out = renumber_lists(&out);
        }
        if opts.breaks {
            out = format_breaks(&out)
                .into_iter()
                .map(Cow::into_owned)
                .collect();
        }
        out
    })
}

/// Opens a file's parent directory and returns its relative UTF-8 path.
///
/// This is the only ambient filesystem boundary for CLI file processing. The
/// returned directory capability restricts subsequent handler I/O to the
/// selected file's parent directory.
fn open_file_parent(path: &Path) -> anyhow::Result<(Dir, Utf8PathBuf)> {
    let path = Utf8Path::from_path(path)
        .with_context(|| format!("converting {} to a UTF-8 path", path.display()))?;
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

/// Formats `content` into output lines, leaving the terminator to the caller.
///
/// This is the pure half of both command boundaries: it neither reads an input
/// nor selects a line ending, so each boundary parses its document, reports the
/// decision, and only then renders these lines with the style it chose.
fn format_lines(content: &str, opts: FormatOpts) -> Vec<String> {
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    process_lines(&lines, opts)
}

/// The formatting closure every mode shares.
///
/// Built once and passed by reference, so the modes cannot diverge: this is
/// the same function a reporting mode assesses and `--in-place` writes.
fn formatting_closure(opts: FormatOpts) -> impl Fn(&SourceDocument<'_>) -> String + Sync {
    move |document| document.render(&format_lines(document.body(), opts))
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
    path: &Path,
    format: &Formatter,
) -> anyhow::Result<(FileReport, String)> {
    open_file_parent(path)
        .and_then(|(directory, storage_key)| {
            // `open_file_parent` has already rejected a non-UTF-8 path, so
            // this only keeps the function total instead of panicking on a
            // path the user supplied.
            let display_path = Utf8Path::from_path(path)
                .with_context(|| format!("converting {} to a UTF-8 path", path.display()))?;
            analyse(mode, &directory, display_path, &storage_key, format)
        })
        .with_context(|| format!("{} {}", mode.verb(), path.display()))
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

/// Runs the requested mode and returns the documented exit status.
///
/// The only errors returned here are infrastructure failures — standard input
/// or standard output. A file that cannot be read or rewritten is reported as
/// it is encountered, counted in the summary, and folded into
/// [`ExitStatus::Error`], so one unreadable file does not abandon the rest.
fn run() -> anyhow::Result<ExitStatus> {
    let cli = Cli::parse();

    if cli.files.is_empty() {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        write_stdout(&format_stdin(&input, cli.opts))?;
        return Ok(ExitStatus::Success);
    }

    let mode = cli.mode();
    let format = formatting_closure(cli.opts);
    let results = in_argument_order(
        cli.files
            .par_iter()
            .enumerate()
            .map(|(index, path)| (index, analyse_one(mode, path, &format)))
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
        // A closed pipe is an ordinary early exit, as in
        // `mdtablefix --check *.md | head`: the reader stopped early, which
        // says nothing about this run. Rust ignores `SIGPIPE`, so the write
        // reports `EPIPE` instead, and `print!` would turn that into the
        // undocumented status `101`.
        Err(error) if is_broken_pipe(&error) => ExitStatus::Success.code(),
        Err(error) => {
            eprintln!("{error:?}");
            ExitStatus::Error.code()
        }
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
