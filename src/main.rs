//! Binary entry point for `mdtablefix`.
//!
//! Parses command-line arguments and coordinates Markdown formatting. When
//! file paths are supplied, processing occurs in parallel and files may be
//! rewritten in place. Without paths the tool reads from standard input and
//! prints results to stdout while preserving the input order.

use std::{
    borrow::Cow,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::Parser;
use mdtablefix::{Options, format_breaks, process_stream_opts, renumber_lists};
use rayon::prelude::*;

#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place", requires = "files")]
    in_place: bool,
    #[command(flatten)]
    opts: FormatOpts,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}

#[derive(clap::Args, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI exposes five independent flags"
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
}

impl From<FormatOpts> for Options {
    fn from(opts: FormatOpts) -> Self {
        Self {
            wrap: opts.wrap,
            ellipsis: opts.ellipsis,
            fences: opts.fences,
            footnotes: opts.footnotes,
        }
    }
}

fn process_lines(lines: &[String], opts: FormatOpts) -> Vec<String> {
    let mut out = process_stream_opts(lines, opts.into());
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
}

fn handle_file(path: &Path, in_place: bool, opts: FormatOpts) -> anyhow::Result<Option<String>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    let fixed = process_lines(&lines, opts);
    if in_place {
        // Preserve compatibility with the `rewrite` helper by always ending files with a
        // trailing newline when content exists. This mirrors typical Unix tool behaviour
        // and avoids spurious diffs when rewriting in place.
        let output = if fixed.is_empty() {
            String::new()
        } else {
            fixed.join("\n") + "\n"
        };
        fs::write(path, output).with_context(|| format!("writing {}", path.display()))?;
        Ok(None)
    } else {
        Ok(Some(fixed.join("\n")))
    }
}

fn report_results<T, F>(results: Vec<anyhow::Result<T>>, mut on_ok: F) -> anyhow::Result<()>
where
    F: FnMut(T),
{
    let mut first_err: Option<anyhow::Error> = None;
    for res in results {
        match res {
            Ok(val) => on_ok(val),
            Err(e) => {
                eprintln!("{e}");
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    if let Some(err) = first_err {
        Err(err)
    } else {
        Ok(())
    }
}

/// Entry point for the command-line tool that reflows broken markdown tables.
///
/// Parses command-line arguments to determine whether to process files in place, print fixed output
/// to standard output, or read from standard input. Handles file I/O and error propagation as
/// needed.
///
/// # Returns
///
/// Returns `Ok(())` if all operations complete successfully; otherwise, returns an error if
/// argument validation or file processing fails.
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
/// # Fix tables from standard input
/// cat myfile.md | mdtablefix
/// ```
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.files.is_empty() {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let lines: Vec<String> = input.lines().map(str::to_string).collect();
        let fixed = process_lines(&lines, cli.opts);
        println!("{}", fixed.join("\n"));
        return Ok(());
    }

    if cli.in_place {
        let results: Vec<anyhow::Result<()>> = cli
            .files
            .par_iter()
            .map(|p| handle_file(p, true, cli.opts).map(|_| ()))
            .collect();
        report_results(results, |()| {})?;
    } else {
        let results: Vec<anyhow::Result<Option<String>>> = cli
            .files
            .par_iter()
            .map(|p| handle_file(p, false, cli.opts))
            .collect();
        report_results(results, |maybe_out| {
            if let Some(out) = maybe_out {
                println!("{out}");
            }
        })?;
    }

    Ok(())
}
