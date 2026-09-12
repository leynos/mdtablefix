//! The command line, and the formatting pipeline its options select.
//!
//! Everything here is pure: arguments are parsed, the options are resolved into
//! [`Options`], and a document's lines are formatted. Nothing here reads an
//! input or writes an output, which is what lets the binary's two boundaries —
//! a file and standard input — share one pipeline without sharing an I/O
//! policy.

use std::{borrow::Cow, path::PathBuf};

use clap::Parser;
use mdtablefix::{
    Options,
    format_breaks,
    io::SourceDocument,
    process::{process_stream_inner, process_with_frontmatter},
};

use crate::driver::Mode;

#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
#[command(group(clap::ArgGroup::new("inputs").args(["files"])))]
#[command(group(clap::ArgGroup::new("mode").multiple(false).requires("inputs")))]
pub struct Cli {
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
    pub opts: FormatOpts,
    /// Markdown files to fix
    pub files: Vec<PathBuf>,
}

impl Cli {
    /// The mode the flags select.
    ///
    /// The `mode` argument group already guarantees that at most one flag is
    /// set, and its `requires("inputs")` guarantees that any flag at all comes
    /// with a file argument, so these branches cannot disagree with the parser:
    /// they only name the decision it made.
    #[must_use]
    pub fn mode(&self) -> Mode {
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
pub struct FormatOpts {
    /// Wrap paragraphs and list items to 80 columns
    #[arg(long = "wrap")]
    pub wrap: bool,
    /// Renumber ordered list items
    #[arg(long = "renumber")]
    pub renumber: bool,
    /// Reformat thematic breaks as underscores
    #[arg(long = "breaks")]
    pub breaks: bool,
    /// Replace "..." with the ellipsis character
    #[arg(long = "ellipsis")]
    pub ellipsis: bool,
    /// Normalize fence delimiters to three backticks
    #[arg(long = "fences")]
    pub fences: bool,
    /// Convert bare numeric references and the final numbered list to
    /// Markdown footnote links
    #[arg(long = "footnotes")]
    pub footnotes: bool,
    /// Fix emphasis markers adjacent to inline code
    #[arg(long = "code-emphasis")]
    pub code_emphasis: bool,
    /// Convert Setext-style headings to hash-prefixed headings
    #[arg(long = "headings")]
    pub headings: bool,
}

impl From<FormatOpts> for Options {
    fn from(opts: FormatOpts) -> Self {
        Self {
            wrap: opts.wrap,
            ellipsis: opts.ellipsis,
            fences: opts.fences,
            footnotes: opts.footnotes,
            renumber: opts.renumber,
            code_emphasis: opts.code_emphasis,
            headings: opts.headings,
        }
    }
}

/// Runs every selected transformation over `lines`.
fn process_lines(lines: &[String], opts: FormatOpts) -> Vec<String> {
    process_with_frontmatter(lines, |body| {
        let mut out = process_stream_inner(body, opts.into());
        if opts.breaks {
            out = format_breaks(&out)
                .into_iter()
                .map(Cow::into_owned)
                .collect();
        }
        out
    })
}

/// Formats `content` into output lines, leaving the terminator to the caller.
///
/// This is the pure half of both command boundaries: it neither reads an input
/// nor selects a line ending, so each boundary parses its document, reports the
/// decision, and only then renders these lines with the style it chose.
pub fn format_lines(content: &str, opts: FormatOpts) -> Vec<String> {
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    process_lines(&lines, opts)
}

/// The formatting closure every mode shares.
///
/// Built once and passed by reference, so the modes cannot diverge: this is
/// the same function a reporting mode assesses and `--in-place` writes.
pub fn formatting_closure(opts: FormatOpts) -> impl Fn(&SourceDocument<'_>) -> String + Sync {
    move |document| document.render(&format_lines(document.body(), opts))
}
