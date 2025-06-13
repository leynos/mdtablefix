use clap::Parser;
use mdtablefix::{process_stream, rewrite};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Reflow broken markdown tables")]
struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place")]
    in_place: bool,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.in_place && cli.files.is_empty() {
        anyhow::bail!("--in-place requires at least one file");
    }

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
