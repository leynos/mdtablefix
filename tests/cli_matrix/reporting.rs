//! Reporting-mode invariants for the CLI matrix.
//!
//! `--check` and `--diff` report what the printing mode would write, so every
//! claim here is measured against that mode's own output for the same document
//! rather than against a count written into the test. The report line, the
//! diff body, and the printed text are three renderings of one assessment, and
//! the assertions below hold them to each other.

use std::fs;

use anyhow::{Context as _, Result};

use super::{ExecutionMode, PhysicalCase, RunResult, STAGED_FILE, fixture_path};

/// The line counts one reporting mode claims for one file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReportCounts {
    /// Lines the formatter adds.
    pub(crate) insertions: usize,
    /// Lines the formatter removes.
    pub(crate) deletions: usize,
}

impl ReportCounts {
    /// The counts of a report that describes no change.
    const NONE: Self = Self {
        insertions: 0,
        deletions: 0,
    };

    /// Whether these counts describe no change at all.
    pub(crate) fn is_empty(self) -> bool { self == Self::NONE }
}

/// Asserts the contract of one reporting run, returning whether it drifted.
///
/// `printed` is the standard output of the `Stdout` run for the same logical
/// case. The reporting modes assess exactly the document that mode prints, so
/// that text is the oracle: a file drifts precisely when the printed document
/// differs from the file, and the run must then exit `1` and name the edit, or
/// else exit `0` and say nothing at all.
pub(crate) fn assert_reporting_invariants(
    case: &PhysicalCase,
    printed: &str,
    result: &RunResult,
) -> Result<bool> {
    let fixture = fs::read(fixture_path(case.logical.fixture))
        .with_context(|| format!("read matrix fixture '{}'", case.logical.fixture))?;
    assert_eq!(
        result.file_content,
        fixture,
        "{}: a reporting mode must not write",
        case.snapshot_name(),
    );

    let source = std::str::from_utf8(&fixture).expect("matrix fixtures are UTF-8");
    let drifted = printed != source;
    let expected = i32::from(drifted);
    assert_eq!(
        result.output.status.code(),
        Some(expected),
        "{}: a reporting mode must exit {expected} for this file, stderr: {}",
        case.snapshot_name(),
        String::from_utf8_lossy(&result.output.stderr),
    );

    let counts = counts_for(case, result);
    assert_eq!(
        !counts.is_empty(),
        drifted,
        "{}: the report and the exit status must describe the same run",
        case.snapshot_name(),
    );
    // A report line carries counts and no content, so only the verbose mode has
    // a document to read back.
    if drifted && case.mode == ExecutionMode::Diff {
        assert_diff_describes_the_document(case, printed, source, result);
    }

    Ok(drifted)
}

/// The counts the mode of `case` reported.
fn counts_for(case: &PhysicalCase, result: &RunResult) -> ReportCounts {
    match case.mode {
        ExecutionMode::Check => check_counts(case, result),
        ExecutionMode::Diff => diff_counts(case, result),
        mode => unreachable!("{mode:?} is not a reporting mode"),
    }
}

/// The counts a `--check` run reported for `case`.
///
/// A clean file produces no output at all and a drifting one produces exactly
/// one line, `<path> +<n> -<m>`. Anything else is a payload no consumer could
/// parse, so it fails here rather than being interpreted.
pub(crate) fn check_counts(case: &PhysicalCase, result: &RunResult) -> ReportCounts {
    let stdout = String::from_utf8_lossy(&result.output.stdout);
    if stdout.is_empty() {
        return ReportCounts::NONE;
    }

    let mut lines = stdout.lines();
    let line = lines.next().expect("a non-empty report has a first line");
    assert!(
        lines.next().is_none(),
        "{}: one file is one report line: {stdout:?}",
        case.snapshot_name(),
    );
    let (path, counts) = parse_report_line(line)
        .unwrap_or_else(|| panic!("{}: {line:?} is not a report line", case.snapshot_name()));
    assert_eq!(
        path,
        STAGED_FILE,
        "{}: the report must name the file as it was given",
        case.snapshot_name(),
    );

    counts
}

/// The counts a `--diff` run marked in its body for `case`.
///
/// A clean file produces no diff at all; a drifting one produces a unified diff
/// whose two header lines name the file and whose body marks every change.
pub(crate) fn diff_counts(case: &PhysicalCase, result: &RunResult) -> ReportCounts {
    let stdout = String::from_utf8_lossy(&result.output.stdout);
    if stdout.is_empty() {
        return ReportCounts::NONE;
    }

    let body = diff_body(case, &stdout);
    ReportCounts {
        insertions: count_marked(body, '+'),
        deletions: count_marked(body, '-'),
    }
}

/// The body of a unified diff, after the two lines that name the file.
///
/// The header is what makes a diff self-describing, so a payload without it is
/// unparseable rather than merely terse, and fails here.
fn diff_body<'a>(case: &PhysicalCase, stdout: &'a str) -> &'a str {
    let header = format!("--- {STAGED_FILE}\n+++ {STAGED_FILE}\n");
    stdout.strip_prefix(&header).unwrap_or_else(|| {
        panic!(
            "{}: a diff must open with {header:?}: {stdout:?}",
            case.snapshot_name(),
        )
    })
}

/// How many lines of a diff body carry `marker`.
fn count_marked(body: &str, marker: char) -> usize {
    body.lines().filter(|line| line.starts_with(marker)).count()
}

/// Asserts the diff describes the document the printing mode wrote.
///
/// A unified diff stands between the two documents it is made from, so it can
/// be checked against both: applying it to the file must yield the formatted
/// text the printing mode printed. That is what makes the payload a diff of
/// *this* document rather than a plausible body with the right counts.
fn assert_diff_describes_the_document(
    case: &PhysicalCase,
    printed: &str,
    source: &str,
    result: &RunResult,
) {
    let stdout = String::from_utf8_lossy(&result.output.stdout);
    let body = diff_body(case, &stdout);

    assert_eq!(
        apply(&case.snapshot_name(), body, source),
        printed,
        "{}: applying the diff to the file must print the document the printing mode wrote",
        case.snapshot_name(),
    );
}

/// Applies a diff body to the document it was made from.
///
/// A hunk carries only its changed lines and their context, so the lines it
/// skips over are absent from the payload: applying a patch copies them from
/// the original, as it does the tail beyond the last hunk. The marked lines are
/// checked against the two documents on the way through, which is the part that
/// makes a diff of the wrong text fail here rather than merely look odd.
fn apply(name: &str, body: &str, original: &str) -> String {
    let mut source = original.lines();
    let mut consumed = 0;
    let mut document = String::new();

    for line in body.lines() {
        let Some((marker, rest)) = line.split_at_checked(1) else {
            continue;
        };
        match marker {
            // Everything before a hunk is unchanged, exactly as an applied
            // patch would leave it.
            "@" => {
                while consumed + 1 < hunk_start(name, line) {
                    add(&mut document, next_line(name, &mut source));
                    consumed += 1;
                }
            }
            " " => {
                let text = next_line(name, &mut source);
                assert_eq!(text, rest, "{name}: a context line must come from the file");
                add(&mut document, text);
                consumed += 1;
            }
            "-" => {
                let text = next_line(name, &mut source);
                assert_eq!(text, rest, "{name}: a deletion must come from the file");
                consumed += 1;
            }
            "+" => add(&mut document, rest),
            _ => {}
        }
    }
    // Everything after the last hunk is unchanged as well.
    for text in source {
        add(&mut document, text);
    }

    document
}

/// The first line of a hunk's original side, from `@@ -<start>[,<n>] +… @@`.
fn hunk_start(name: &str, header: &str) -> usize {
    header
        .split(' ')
        .nth(1)
        .and_then(|range| range.strip_prefix('-'))
        .and_then(|range| range.split(',').next())
        .and_then(|start| start.parse().ok())
        .unwrap_or_else(|| panic!("{name}: {header:?} is not a hunk header"))
}

/// The next line of the document a diff was made from.
fn next_line<'a>(name: &str, source: &mut impl Iterator<Item = &'a str>) -> &'a str {
    source
        .next()
        .unwrap_or_else(|| panic!("{name}: the diff marks more lines than the file has"))
}

/// Appends one line to `document`, newline included.
fn add(document: &mut String, line: &str) {
    document.push_str(line);
    document.push('\n');
}

/// Parses `<path> +<n> -<m>`, the one shape a report line takes.
///
/// The path is everything before the final two space-separated fields, so a
/// path containing a space still parses.
fn parse_report_line(line: &str) -> Option<(&str, ReportCounts)> {
    let mut fields = line.rsplitn(3, ' ');
    let deletions: usize = fields.next()?.strip_prefix('-')?.parse().ok()?;
    let insertions: usize = fields.next()?.strip_prefix('+')?.parse().ok()?;
    let path = fields.next()?;

    (!path.is_empty()).then_some((
        path,
        ReportCounts {
            insertions,
            deletions,
        },
    ))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the reporting-mode helpers.

    use rstest::rstest;

    use super::{ReportCounts, apply, parse_report_line};

    #[rstest]
    #[case("docs/a.md +12 -8", "docs/a.md", 12, 8)]
    #[case("input.dat +1 -0", "input.dat", 1, 0)]
    #[case("a b.md +3 -4", "a b.md", 3, 4)]
    fn parse_report_line_reads_path_and_counts(
        #[case] line: &str,
        #[case] path: &str,
        #[case] insertions: usize,
        #[case] deletions: usize,
    ) {
        let expected = ReportCounts {
            insertions,
            deletions,
        };

        assert_eq!(parse_report_line(line), Some((path, expected)));
    }

    #[rstest]
    #[case("")]
    #[case("docs/a.md")]
    #[case("docs/a.md +12")]
    #[case("docs/a.md 12 -8")]
    #[case("docs/a.md +x -8")]
    #[case(" +12 -8")]
    fn parse_report_line_rejects_anything_else(#[case] line: &str) {
        assert_eq!(parse_report_line(line), None);
    }

    #[test]
    fn apply_rebuilds_the_document_from_a_hunk() {
        let body = "@@ -1,3 +1,3 @@\n-|A|B|\n+| A | B |\n context\n";

        assert_eq!(
            apply("unit", body, "|A|B|\ncontext\n"),
            "| A | B |\ncontext\n",
        );
    }

    /// A hunk shows its own change and three lines of context, so a change
    /// further down the file leaves the lines between and after the hunks out
    /// of the payload: only applying the patch brings them back.
    #[test]
    fn apply_copies_the_lines_between_and_after_hunks() {
        let original =
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n";
        let body = "@@ -1 +1 @@\n-one\n+ONE\n@@ -9,2 +9,2 @@\n-nine\n-ten\n+NINE\n+TEN\n";

        assert_eq!(
            apply("unit", body, original),
            "ONE\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nNINE\nTEN\neleven\ntwelve\n",
        );
    }

    #[test]
    #[should_panic(expected = "a deletion must come from the file")]
    fn apply_rejects_a_deletion_the_file_does_not_carry() {
        apply("unit", "@@ -1 +1 @@\n-one\n+ONE\n", "other\n");
    }

    /// Anything else in a body is ignored, which is what lets the
    /// `\ No newline at end of file` marker through untouched.
    #[test]
    fn apply_ignores_lines_that_carry_no_marker() {
        assert_eq!(apply("unit", "not a hunk\n", "other\n"), "other\n");
    }

    #[test]
    #[should_panic(expected = "is not a hunk header")]
    fn apply_rejects_a_malformed_hunk_header() { apply("unit", "@@ nonsense\n", "other\n"); }

    #[test]
    fn apply_ignores_the_no_newline_marker() {
        let body = "@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n";

        assert_eq!(apply("unit", body, "old\n"), "new\n");
    }
}
