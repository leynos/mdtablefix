//! Binary entry point for `mdtablefix`.
//!
//! Parses command-line arguments and coordinates Markdown formatting. When
//! file paths are supplied, processing occurs in parallel and files may be
//! rewritten in place. Without paths the tool reads from standard input and
//! prints results to stdout while preserving the input order.

/// Detects and splits leading YAML frontmatter for CLI processing so command
/// handlers can preserve the prefix while applying transforms to the Markdown
/// body.
#[path = "frontmatter.rs"]
mod frontmatter;

use std::{
    borrow::Cow,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::Parser;
use mdtablefix::{Options, format_breaks, process::process_stream_inner, renumber_lists};
use rayon::prelude::*;

use crate::frontmatter::split_leading_yaml_frontmatter;

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
    // Split off leading YAML frontmatter to preserve it from all transforms
    let (frontmatter_prefix, body) = split_leading_yaml_frontmatter(lines);

    // Use process_stream_inner directly since we've already split frontmatter
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

    // Prepend the preserved frontmatter prefix
    let mut result = frontmatter_prefix.to_vec();
    result.extend(out);
    result
}

/// Reads and formats a file without modifying it.
fn format_to_string(path: &Path, opts: FormatOpts) -> anyhow::Result<String> {
    let content =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    let fixed = process_lines(&lines, opts);
    // Keep file output newline-terminated, matching the CLI stdout contract.
    Ok(if fixed.is_empty() {
        String::new()
    } else {
        fixed.join("\n") + "\n"
    })
}

/// Reads, formats, and rewrites a file in place.
fn rewrite_in_place(path: &Path, opts: FormatOpts) -> anyhow::Result<()> {
    let output = format_to_string(path, opts)?;
    fs::write(path, output).with_context(|| format!("writing {}", path.display()))
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
            .map(|p| rewrite_in_place(p, cli.opts))
            .collect();
        report_results(results, |()| {})?;
    } else {
        let results: Vec<anyhow::Result<String>> = cli
            .files
            .par_iter()
            .map(|p| format_to_string(p, cli.opts))
            .collect();
        report_results(results, |out| print!("{out}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit and property tests for the binary's file-output contracts.

    use std::fs;

    use proptest::prelude::*;

    use super::{FormatOpts, format_to_string, rewrite_in_place};

    fn prose_word_strategy() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop_oneof![
                Just("alpha".to_string()),
                Just("beta".to_string()),
                Just("gamma".to_string()),
                Just("delta".to_string()),
                Just("evidence".to_string()),
                Just("formatting".to_string()),
            ],
            1..20,
        )
        .prop_map(|words| words.join(" "))
    }

    proptest! {
        #[test]
        fn formatting_matches_in_place_output(
            prose in prose_word_strategy(),
            table_cell in prose_word_strategy(),
        ) {
            let input = format!(
                "{prose}\n\n| Name | Notes |\n|---|---|\n| {table_cell} | value |\n"
            );
            let directory = tempfile::tempdir()
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let formatted_path = directory.path().join("formatted.md");
            let rewritten_path = directory.path().join("rewritten.md");
            fs::write(&formatted_path, &input)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            fs::write(&rewritten_path, input)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;

            let formatted = format_to_string(&formatted_path, FormatOpts {
                wrap: false,
                renumber: false,
                breaks: false,
                ellipsis: false,
                fences: false,
                footnotes: false,
                code_emphasis: false,
                headings: false,
            })
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
            rewrite_in_place(&rewritten_path, FormatOpts {
                wrap: false,
                renumber: false,
                breaks: false,
                ellipsis: false,
                fences: false,
                footnotes: false,
                code_emphasis: false,
                headings: false,
            })
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let rewritten = fs::read_to_string(&rewritten_path)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;

            prop_assert_eq!(formatted, rewritten);
        }
    }
}
