//! The command line, and the formatting pipeline its options select.
//!
//! Everything here that touches a document is pure: arguments are parsed, the
//! options are resolved into [`Options`], and a document's lines are formatted.
//! Nothing here reads a document input or writes an output, which is what lets
//! the binary's two boundaries — a file and standard input — share one pipeline
//! without sharing an I/O policy.
//!
//! Declared only from `src/main.rs`, so this module belongs to the binary and
//! `src/lib.rs` does not name it: nothing here adds public API. It depends on
//! `crate::driver` for [`Mode`], on `crate::select` for the extension parser
//! `--md-exts` is validated by, and on the library's [`Options`], and nothing
//! depends on it but the binary's composition root. It lives in its own file
//! because `src/main.rs` is capped at 400 lines, not because the command line
//! is a layer: it is the outermost adapter, and the mode it names is passed
//! inward unchanged.

use std::{borrow::Cow, path::PathBuf};

use clap::{CommandFactory, FromArgMatches, Parser, error::ErrorKind, parser::ValueSource};
use mdtablefix::{
    Options,
    format_breaks,
    io::SourceDocument,
    process::{process_stream_inner, process_with_frontmatter},
    renumber_lists,
};

use crate::{
    driver::Mode,
    select::extensions::{ExtensionFilter, parse_extension},
};

#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
#[command(group(
    clap::ArgGroup::new("inputs").args(["files", "git"]).multiple(false)
))]
#[command(group(clap::ArgGroup::new("mode").multiple(false).requires("inputs")))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI exposes independent flags via separate switches"
)]
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
    /// Print the selected paths and exit, without reading or writing them
    #[arg(long = "list-files", group = "mode")]
    list_files: bool,
    /// Select Markdown files tracked by Git beneath the current directory
    #[arg(long = "git")]
    git: bool,
    /// Also select untracked files that Git does not ignore
    #[arg(long = "include-untracked")]
    include_untracked: bool,
    /// File extensions to select under `--git`
    // One `default_value` rather than `default_values`, so `--help` renders the
    // default in the same comma-separated form the flag accepts:
    // `default_values` is shown space-joined, which reads as one extension with
    // spaces in it. `value_delimiter` splits this apart again, so the filter
    // built from it holds the same three extensions either way, and the value
    // source is `DefaultValue` in both spellings.
    #[arg(
        long = "md-exts",
        value_name = "EXT",
        value_delimiter = ',',
        default_value = "md,mdc,markdown",
        value_parser = parse_extension,
    )]
    md_exts: Vec<String>,
    /// Rewrite files containing conflict markers during a merge or rebase
    #[arg(long = "allow-conflicted")]
    allow_conflicted: bool,
    #[command(flatten)]
    pub opts: FormatOpts,
    /// Markdown files to fix
    pub files: Vec<PathBuf>,
}

/// The flags that only make sense with `--git`, each paired with the form the
/// user wrote it in.
///
/// `--md-exts` is checked by its [`ValueSource`] rather than by its value,
/// because its default means it always carries one: a user who typed
/// `--md-exts md` explicitly is asking for `--git`, and must be told so, while
/// a user who typed nothing is not.
fn git_only_flags(cli: &Cli, matches: &clap::ArgMatches) -> [(&'static str, bool); 4] {
    let explicit_exts = matches.value_source("md_exts") != Some(ValueSource::DefaultValue);

    [
        ("--include-untracked", cli.include_untracked),
        ("--allow-conflicted", cli.allow_conflicted),
        ("--list-files", cli.list_files),
        ("--md-exts", explicit_exts),
    ]
}

impl Cli {
    /// Parses and enforces the dependencies `clap` cannot express here.
    ///
    /// `requires = "git"` is not dependable for a flag that takes no value: on
    /// clap 4.6.6, with `git` in the `inputs` group and `files` a positional
    /// `Vec`, `--list-files a.md` is silently accepted with no `--git` in
    /// sight. So each dependency is checked after parsing, and reported as a
    /// clap error, which keeps the exit status at 2 and the usage footer.
    ///
    /// # Panics
    ///
    /// Terminates the process on a parse error, exactly as [`Cli::parse`] does:
    /// that is clap's contract for a command-line tool.
    pub fn parse_validated() -> Self {
        let matches = Self::command().get_matches();
        let cli = Self::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
        for (name, present) in git_only_flags(&cli, &matches) {
            if present && !cli.git {
                Self::command()
                    .error(
                        ErrorKind::MissingRequiredArgument,
                        format!("{name} requires --git"),
                    )
                    .exit();
            }
        }

        cli
    }

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
        } else if self.list_files {
            Mode::ListFiles
        } else {
            Mode::Print
        }
    }

    /// Whether the selection is the files Git reports.
    pub fn selects_from_git(&self) -> bool { self.git }

    /// Whether the selection also holds untracked files.
    pub fn includes_untracked(&self) -> bool { self.include_untracked }

    /// Whether conflicted files may be rewritten anyway.
    pub fn allows_conflicted(&self) -> bool { self.allow_conflicted }

    /// The extensions the selection keeps.
    ///
    /// Each value has already been through the parser `--md-exts` declares, so
    /// this only has to fold them into the set that holds them.
    pub fn extensions(&self) -> ExtensionFilter { self.md_exts.iter().cloned().collect() }
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
            code_emphasis: opts.code_emphasis,
            headings: opts.headings,
        }
    }
}

/// Runs every selected transformation over `lines`.
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
