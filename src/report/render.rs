//! Rendering of report lines, summaries, and unified diffs.

use std::io;

use camino::Utf8Path;
use similar::{Algorithm, TextDiff};

use super::delta::LineDelta;

/// Renders one report line, for example `docs/a.md +12 -8`.
///
/// Consumers parse by taking the final two whitespace-separated fields as the
/// counts and everything before them as the path.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
/// use mdtablefix::report::{LineDelta, render_report_line};
///
/// let delta = LineDelta::between("|A|B|\n", "| A | B |\n");
/// assert_eq!(
///     render_report_line(Utf8Path::new("docs/a.md"), delta),
///     "docs/a.md +1 -1"
/// );
/// ```
#[must_use]
pub fn render_report_line(display_path: &Utf8Path, delta: LineDelta) -> String {
    format!(
        "{display_path} +{} -{}",
        delta.insertions(),
        delta.deletions()
    )
}

/// Renders the human summary, for example
/// `2 files would be reformatted, 1 file left unchanged.`
///
/// Clauses are elided at zero and use singular or plural forms as
/// appropriate. When all three counts are zero the result is
/// `No files were analysed.`. This goes to standard error so that standard
/// output stays a machine contract.
///
/// # Examples
///
/// ```
/// use mdtablefix::report::render_summary;
///
/// assert_eq!(render_summary(0, 0, 0), "No files were analysed.");
/// assert_eq!(render_summary(1, 0, 0), "1 file would be reformatted.");
/// assert_eq!(
///     render_summary(2, 1, 0),
///     "2 files would be reformatted, 1 file left unchanged."
/// );
/// ```
#[must_use]
pub fn render_summary(changed: usize, unchanged: usize, errored: usize) -> String {
    let clauses: Vec<String> = [
        (changed, "would be reformatted"),
        (unchanged, "left unchanged"),
        (errored, "could not be read"),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, verb)| format!("{} {verb}", file_count(count)))
    .collect();

    if clauses.is_empty() {
        String::from("No files were analysed.")
    } else {
        format!("{}.", clauses.join(", "))
    }
}

/// Renders a file count with its noun, so each clause reads as English.
fn file_count(count: usize) -> String {
    if count == 1 {
        String::from("1 file")
    } else {
        format!("{count} files")
    }
}

/// Configuration for unified-diff rendering.
///
/// # Examples
///
/// ```
/// use mdtablefix::report::DiffOptions;
///
/// let options = DiffOptions {
///     context_radius: 3,
///     patience_threshold: 1000,
/// };
/// assert_eq!(options.context_radius, 3);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DiffOptions {
    /// Lines of context around each hunk. Always three.
    pub context_radius: usize,
    /// Above this many lines on either side, switch from Myers to Patience so
    /// the cost stays bounded without a wall-clock cut-off.
    pub patience_threshold: usize,
}

/// Streams a unified diff for `display_path` into `out`.
///
/// Headers name `display_path` on both sides with directory separators
/// normalized to `/`, and carry no timestamps, so output is deterministic and
/// snapshot-stable. The `\ No newline at end of file` marker is retained; it
/// can only ever appear on the `-` side, because the formatter always emits a
/// trailing terminator. Output is never colourized.
///
/// # Errors
///
/// Returns an error if `out` fails.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
/// use mdtablefix::report::{DiffOptions, write_unified_diff};
///
/// let options = DiffOptions {
///     context_radius: 3,
///     patience_threshold: 1000,
/// };
/// let mut out = Vec::new();
/// write_unified_diff(
///     &mut out,
///     Utf8Path::new("ragged.md"),
///     "|A|B|\n",
///     "| A | B |\n",
///     options,
/// )
/// .expect("writing to a Vec cannot fail");
/// assert_eq!(
///     String::from_utf8(out).expect("diff is UTF-8"),
///     "--- ragged.md\n+++ ragged.md\n@@ -1 +1 @@\n-|A|B|\n+| A | B |\n"
/// );
/// ```
pub fn write_unified_diff(
    out: &mut impl io::Write,
    display_path: &Utf8Path,
    original: &str,
    formatted: &str,
    options: DiffOptions,
) -> io::Result<()> {
    let longest = line_count(original).max(line_count(formatted));
    let algorithm = if longest > options.patience_threshold {
        Algorithm::Patience
    } else {
        Algorithm::Myers
    };
    // Separators are normalised rather than taken from `display_path` as
    // written, so a path rendered on Windows still names one file to the tools
    // that consume unified diffs.
    let header = display_path.as_str().replace('\\', "/");
    let diff = TextDiff::configure()
        .algorithm(algorithm)
        .diff_lines(original, formatted);
    let mut unified = diff.unified_diff();
    unified
        .context_radius(options.context_radius)
        .header(&header, &header);

    unified.to_writer(out)
}

/// Counts the lines the diff tokenizer will see in `text`.
///
/// Every `\n`, every `\r\n`, and every lone `\r` ends a line, and a non-empty
/// unterminated tail is one more. This differs from [`str::lines`] on a lone
/// carriage return, and the threshold has to be measured against the lines the
/// renderer actually produces.
fn line_count(text: &str) -> usize {
    let mut lines = 0;
    let mut unterminated = false;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\n' => {
                lines += 1;
                unterminated = false;
            }
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                lines += 1;
                unterminated = false;
            }
            _ => unterminated = true,
        }
    }
    lines + usize::from(unterminated)
}

#[cfg(test)]
mod tests {
    //! Unit tests for report rendering.

    use camino::Utf8Path;
    use rstest::rstest;

    use super::{DiffOptions, render_summary, write_unified_diff};

    #[rstest]
    #[case(0, 0, 0, "No files were analysed.")]
    #[case(0, 0, 1, "1 file could not be read.")]
    #[case(0, 0, 2, "2 files could not be read.")]
    #[case(0, 1, 0, "1 file left unchanged.")]
    #[case(0, 1, 1, "1 file left unchanged, 1 file could not be read.")]
    #[case(0, 1, 2, "1 file left unchanged, 2 files could not be read.")]
    #[case(0, 2, 0, "2 files left unchanged.")]
    #[case(0, 2, 1, "2 files left unchanged, 1 file could not be read.")]
    #[case(0, 2, 2, "2 files left unchanged, 2 files could not be read.")]
    #[case(1, 0, 0, "1 file would be reformatted.")]
    #[case(1, 0, 1, "1 file would be reformatted, 1 file could not be read.")]
    #[case(1, 0, 2, "1 file would be reformatted, 2 files could not be read.")]
    #[case(1, 1, 0, "1 file would be reformatted, 1 file left unchanged.")]
    #[case(
        1,
        1,
        1,
        "1 file would be reformatted, 1 file left unchanged, 1 file could not be read."
    )]
    #[case(
        1,
        1,
        2,
        "1 file would be reformatted, 1 file left unchanged, 2 files could not be read."
    )]
    #[case(1, 2, 0, "1 file would be reformatted, 2 files left unchanged.")]
    #[case(
        1,
        2,
        1,
        "1 file would be reformatted, 2 files left unchanged, 1 file could not be read."
    )]
    #[case(
        1,
        2,
        2,
        "1 file would be reformatted, 2 files left unchanged, 2 files could not be read."
    )]
    #[case(2, 0, 0, "2 files would be reformatted.")]
    #[case(2, 0, 1, "2 files would be reformatted, 1 file could not be read.")]
    #[case(2, 0, 2, "2 files would be reformatted, 2 files could not be read.")]
    #[case(2, 1, 0, "2 files would be reformatted, 1 file left unchanged.")]
    #[case(
        2,
        1,
        1,
        "2 files would be reformatted, 1 file left unchanged, 1 file could not be read."
    )]
    #[case(
        2,
        1,
        2,
        "2 files would be reformatted, 1 file left unchanged, 2 files could not be read."
    )]
    #[case(2, 2, 0, "2 files would be reformatted, 2 files left unchanged.")]
    #[case(
        2,
        2,
        1,
        "2 files would be reformatted, 2 files left unchanged, 1 file could not be read."
    )]
    #[case(
        2,
        2,
        2,
        "2 files would be reformatted, 2 files left unchanged, 2 files could not be read."
    )]
    fn summary_renders_every_combination(
        #[case] changed: usize,
        #[case] unchanged: usize,
        #[case] errored: usize,
        #[case] expected: &str,
    ) {
        assert_eq!(render_summary(changed, unchanged, errored), expected);
    }

    /// Byte-equal texts render as nothing at all, headers included: `similar`
    /// writes the header alongside the first hunk, and equal texts have no
    /// hunk. A document that is already formatted therefore cannot render as
    /// an empty diff carrying a file name.
    ///
    /// The driver still guards its diff arm on a change being present, so this
    /// test is what keeps that guard's absence a cost rather than a
    /// difference in output. See `EP-M6` in the `ExecPlan`.
    #[test]
    fn equal_texts_render_nothing() {
        let mut out = Vec::new();
        write_unified_diff(
            &mut out,
            Utf8Path::new("clean.md"),
            "| A | B |\n",
            "| A | B |\n",
            DiffOptions {
                context_radius: 3,
                patience_threshold: 1000,
            },
        )
        .expect("writing to a Vec cannot fail");
        assert!(out.is_empty(), "a clean file has no diff: {out:?}");
    }

    #[test]
    fn summary_grammar_is_snapshot_stable() {
        let mut rendered = String::new();
        for changed in 0..3 {
            for unchanged in 0..3 {
                for errored in 0..3 {
                    rendered.push_str(&render_summary(changed, unchanged, errored));
                    rendered.push('\n');
                }
            }
        }
        insta::assert_snapshot!("summary_grammar", rendered);
    }
}
