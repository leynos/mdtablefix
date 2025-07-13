use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use clap::Parser;
use mdtablefix::{format_breaks, process_stream, process_stream_no_wrap, renumber_lists};

#[derive(Parser)]
#[command(about = "Reflow broken markdown tables")]
struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place", requires = "files")]
    in_place: bool,
    #[command(flatten)]
    opts: FormatOpts,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}

#[derive(clap::Args)]
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
}

fn process_lines(lines: &[String], wrap: bool, renumber: bool, breaks: bool) -> Vec<String> {
    let mut out = if wrap {
        process_stream(lines)
    } else {
        process_stream_no_wrap(lines)
    };
    if renumber {
        out = renumber_lists(&out);
    }
    if breaks {
        out = format_breaks(&out);
    }
    out
}

fn rewrite_path(path: &Path, wrap: bool, renumber: bool, breaks: bool) -> std::io::Result<()> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    let fixed = process_lines(&lines, wrap, renumber, breaks);
    fs::write(path, fixed.join("\n") + "\n")
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
        let fixed = process_lines(&lines, cli.opts.wrap, cli.opts.renumber, cli.opts.breaks);
        println!("{}", fixed.join("\n"));
        return Ok(());
    }

    for path in cli.files {
        if cli.in_place {
            rewrite_path(&path, cli.opts.wrap, cli.opts.renumber, cli.opts.breaks)?;
        } else {
            let content = fs::read_to_string(&path)?;
            let lines: Vec<String> = content.lines().map(str::to_string).collect();
            let fixed = process_lines(&lines, cli.opts.wrap, cli.opts.renumber, cli.opts.breaks);
            println!("{}", fixed.join("\n"));
        }
    }

    Ok(())
}
