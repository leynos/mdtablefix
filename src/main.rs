use clap::Parser;
use mdtablefix::{process_stream, rewrite};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Reflow broken markdown tables")]
struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place", requires = "files")]
    in_place: bool,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}

/// Entry point for the command-line tool that reflows broken markdown tables.
///
/// Parses command-line arguments to determine whether to process files in place, print fixed output to standard output, or read from standard input. Handles file I/O and error propagation as needed.
///
/// # Returns
///
/// Returns `Ok(())` if all operations complete successfully; otherwise, returns an error if argument validation or file processing fails.
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
        let fixed = process_stream(&lines);
        println!("{}", fixed.join("\n"));
        return Ok(());
    }

    for path in cli.files {
        if cli.in_place {
            rewrite(&path)?;
        } else {
            let content = fs::read_to_string(&path)?;
            let lines: Vec<String> = content.lines().map(str::to_string).collect();
            let fixed = process_stream(&lines);
            println!("{}", fixed.join("\n"));
        }
    }

    Ok(())
}
