//! Unit tests for the assessment and the reporting modes' payloads.
//!
//! `assess` is the read half of every mode; `analyse` is what the reporting
//! modes do with it. Both are pinned here rather than through the binary, so a
//! payload can be asserted without the process boundary between.

use camino::Utf8Path;
use mdtablefix::report::LineDelta;
// Wrapper over `tracing_test::traced_test`; see `test_macros` for why.
use test_macros::traced_test;

use super::{
    ConflictGuard,
    Mode,
    analyse,
    assess,
    test_support::{ALIGNED, RAGGED, align, fixture, identity, read, readable},
};

/// `INV-PREDICTS`: a formatter that reproduces its input reports no change, so
/// a clean file is never reported as drift.
#[test]
fn assess_reports_no_change_for_its_own_output() {
    let (_dir, directory) = fixture("clean.md", ALIGNED);

    let assessment = assess(&readable(&directory), Utf8Path::new("clean.md"), &identity)
        .expect("assess fixture");

    assert!(
        !assessment.is_changed(),
        "a fixed point must report no change"
    );
}

/// `INV-PREDICTS`: a formatter that rewrites the text reports a change, which
/// is what makes the no-change case above meaningful.
#[test]
fn assess_reports_a_change_when_the_formatter_rewrites() {
    let (_dir, directory) = fixture("ragged.md", RAGGED);

    let assessment =
        assess(&readable(&directory), Utf8Path::new("ragged.md"), &align).expect("assess fixture");

    assert!(assessment.is_changed());
}

/// `INV-BOM`: a marked file that needs no Markdown change is not reported as
/// drift, because the mark survives both parsing and rendering.
#[test]
fn assess_keeps_a_byte_order_mark() {
    let (_dir, directory) = fixture("bom.md", &format!("\u{FEFF}{ALIGNED}"));

    let assessment =
        assess(&readable(&directory), Utf8Path::new("bom.md"), &identity).expect("assess fixture");

    assert!(
        !assessment.is_changed(),
        "the byte-order mark must survive the round trip"
    );
}

/// A file that cannot be read is an error rather than an unchanged file.
#[test]
fn assess_fails_for_a_missing_file() {
    let (_dir, directory) = fixture("clean.md", ALIGNED);

    let result = assess(
        &readable(&directory),
        Utf8Path::new("missing.md"),
        &identity,
    );

    assert!(result.is_err(), "reading a missing file must fail");
}

/// `INV-NOWRITE`: a reporting mode reports drift and leaves the file alone.
#[test]
fn check_reports_drift_without_writing() {
    let (_dir, directory) = fixture("ragged.md", RAGGED);

    let (report, payload) = analyse(
        Mode::Check,
        ConflictGuard::unguarded(),
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &align,
    )
    .expect("analyse fixture");

    assert!(report.is_changed);
    assert_eq!(report.display_path, Utf8Path::new("ragged.md"));
    assert_eq!(report.delta, LineDelta::between(RAGGED, ALIGNED));
    assert_eq!(payload, "ragged.md +3 -3\n");
    assert_eq!(
        read(&directory, "ragged.md"),
        RAGGED,
        "a reporting mode must not write"
    );
}

/// A clean file produces no report line: the summary counts it as unchanged,
/// and standard output stays a list of drifting files only.
#[test]
fn check_reports_a_clean_file_with_no_payload() {
    let (_dir, directory) = fixture("clean.md", ALIGNED);

    let (report, payload) = analyse(
        Mode::Check,
        ConflictGuard::unguarded(),
        &directory,
        Utf8Path::new("clean.md"),
        Utf8Path::new("clean.md"),
        &identity,
    )
    .expect("analyse fixture");

    assert!(!report.is_changed);
    assert_eq!(report.delta, LineDelta::default());
    assert_eq!(payload, "");
}

/// The line-ending report names the path the user wrote, not the bare name the
/// capability reads by.
///
/// Two files called `a.md` in different directories would otherwise carry the
/// same `path` field, and a subscriber could not tell them apart. The two names
/// differ only for a nested file, which is what makes this discriminating: had
/// they been equal, the assertion would hold whichever one the report used.
#[test]
#[traced_test]
fn check_reports_the_path_the_user_wrote() {
    // The capability is the file's parent, as `open_file_parent` makes it: the
    // name the capability reads by is the bare `inner.md`, and the name the
    // user wrote is the one with the directory in front of it.
    let (_dir, directory) = fixture("nested/inner.md", RAGGED);
    let nested = directory
        .open_dir(Utf8Path::new("nested"))
        .expect("open the nested directory");

    let (_report, _payload) = analyse(
        Mode::Check,
        ConflictGuard::unguarded(),
        &nested,
        Utf8Path::new("nested/inner.md"),
        Utf8Path::new("inner.md"),
        &align,
    )
    .expect("analyse fixture");

    assert!(
        logs_contain("nested/inner.md"),
        "the line-ending report must name the path the user wrote"
    );
}

/// Prefixes every line of `text` with `marker`, which is how a unified diff
/// body presents its two sides.
fn marked(marker: char, text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        out.push(marker);
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `INV-EXIT` under the verbose rendering: the payload is a unified diff that
/// names the file on both sides, and the file is still not written.
#[test]
fn diff_reports_a_unified_diff_without_writing() {
    let (_dir, directory) = fixture("ragged.md", RAGGED);

    let (report, payload) = analyse(
        Mode::Diff,
        ConflictGuard::unguarded(),
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &align,
    )
    .expect("analyse fixture");

    assert!(report.is_changed);
    assert_eq!(report.delta, LineDelta::between(RAGGED, ALIGNED));
    assert_eq!(
        payload,
        format!(
            "--- ragged.md\n+++ ragged.md\n@@ -1,3 +1,3 @@\n{}{}",
            marked('-', RAGGED),
            marked('+', ALIGNED),
        )
    );
    assert_eq!(
        read(&directory, "ragged.md"),
        RAGGED,
        "a reporting mode must not write"
    );
}

/// A clean file produces no diff at all — not a header with no hunks, and not
/// an empty hunk. The unchanged arm is what this pins, and it is separate from
/// the `--check` case because the two renderings share that arm.
#[test]
fn diff_reports_a_clean_file_with_no_payload() {
    let (_dir, directory) = fixture("clean.md", ALIGNED);

    let (report, payload) = analyse(
        Mode::Diff,
        ConflictGuard::unguarded(),
        &directory,
        Utf8Path::new("clean.md"),
        Utf8Path::new("clean.md"),
        &identity,
    )
    .expect("analyse fixture");

    assert!(!report.is_changed);
    assert_eq!(payload, "");
}

/// The bare mode prints the formatted text and leaves the file alone: printing
/// is a read, however the payload is later rendered.
#[test]
fn print_reports_the_formatted_text_without_writing() {
    let (_dir, directory) = fixture("ragged.md", RAGGED);

    let (_report, payload) = analyse(
        Mode::Print,
        ConflictGuard::unguarded(),
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &align,
    )
    .expect("analyse fixture");

    assert_eq!(payload, ALIGNED);
    assert_eq!(read(&directory, "ragged.md"), RAGGED);
}

/// The capability names a file inside it, so a path outside the capability is
/// not addressable at all.
#[test]
fn assess_declines_a_path_outside_the_capability() {
    let (_dir, directory) = fixture("clean.md", ALIGNED);

    let result = assess(
        &readable(&directory),
        Utf8Path::new("../escape.md"),
        &identity,
    );

    assert!(result.is_err(), "a path outside the capability must fail");
}
