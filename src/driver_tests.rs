//! Unit tests for the driver: the exit-status contract, argument ordering, and
//! the read-only assessment path.
//!
//! The end-to-end contract is covered by `tests/cli_check.rs` through the real
//! binary; these tests pin the decisions that file cannot isolate, such as
//! which mode may hold a writable capability.

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use mdtablefix::{io::SourceDocument, report::LineDelta};
use rstest::rstest;
use tempfile::{TempDir, tempdir};

use super::{ExitStatus, Mode, ReadOnlyDir, analyse, assess, exit_status, in_argument_order};

/// A ragged table, whose every line the aligning formatter below replaces.
const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// The aligned table the same formatter produces.
const ALIGNED: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// A formatter that reproduces its input.
///
/// A true fixed point for these fixtures: the document boundary re-renders the
/// body with its own mark and line endings, so an unchanged assessment is
/// distinguishable from a changed one.
fn identity(document: &SourceDocument<'_>) -> String {
    document.render(
        document
            .body()
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>()
            .as_slice(),
    )
}

/// A formatter that always produces the aligned table.
///
/// Every line of a ragged fixture differs from its aligned counterpart, so the
/// counts these tests assert are the three replacements `--check` reports.
fn align(document: &SourceDocument<'_>) -> String {
    document.render(
        ALIGNED
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>()
            .as_slice(),
    )
}

/// Writes `content` as `name` in a fresh capability-scoped directory.
///
/// The [`TempDir`] is returned so the caller keeps it alive for the length of
/// the test; dropping it would delete the directory the capability names.
fn fixture(name: &str, content: &str) -> (TempDir, Dir) {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join(name), content).expect("write fixture");
    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .expect("the temporary directory path is UTF-8");
    let directory =
        Dir::open_ambient_dir(&path, ambient_authority()).expect("open directory capability");

    (dir, directory)
}

/// Reads `name` through the capability, so a rewrite is observed as the
/// capability sees it rather than through an ambient path.
fn read(directory: &Dir, name: &str) -> String {
    directory
        .read_to_string(Utf8Path::new(name))
        .expect("read fixture")
}

/// A read-only view of `directory`.
fn readable(directory: &Dir) -> ReadOnlyDir {
    ReadOnlyDir::new(
        directory
            .try_clone()
            .expect("duplicate directory capability"),
    )
}

/// `INV-EXIT`: every combination of mode, drift, and error maps to exactly one
/// documented status, with an error outranking drift in every mode and drift a
/// status only under the reporting mode.
#[rstest]
#[case(Mode::Print, false, false, ExitStatus::Success)]
#[case(Mode::Print, true, false, ExitStatus::Success)]
#[case(Mode::Print, false, true, ExitStatus::Error)]
#[case(Mode::Print, true, true, ExitStatus::Error)]
#[case(Mode::InPlace, false, false, ExitStatus::Success)]
#[case(Mode::InPlace, true, false, ExitStatus::Success)]
#[case(Mode::InPlace, false, true, ExitStatus::Error)]
#[case(Mode::InPlace, true, true, ExitStatus::Error)]
#[case(Mode::Check, false, false, ExitStatus::Success)]
#[case(Mode::Check, true, false, ExitStatus::Drift)]
#[case(Mode::Check, false, true, ExitStatus::Error)]
#[case(Mode::Check, true, true, ExitStatus::Error)]
fn exit_status_covers_inv_exit(
    #[case] mode: Mode,
    #[case] any_drift: bool,
    #[case] any_error: bool,
    #[case] expected: ExitStatus,
) {
    assert_eq!(exit_status(mode, any_drift, any_error), expected);
}

/// The three statuses are the three documented codes, and nothing else.
#[test]
fn exit_status_codes_are_the_documented_three() {
    assert_eq!(ExitStatus::Success.code(), std::process::ExitCode::from(0));
    assert_eq!(ExitStatus::Drift.code(), std::process::ExitCode::from(1));
    assert_eq!(ExitStatus::Error.code(), std::process::ExitCode::from(2));
}

/// `INV-ORDER`: results are re-ordered by argument index, whatever order the
/// parallel stage happened to produce them in.
#[test]
fn in_argument_order_restores_the_argument_sequence() {
    let shuffled = vec![(2, "charlie"), (0, "alpha"), (1, "bravo")];

    assert_eq!(
        in_argument_order(shuffled),
        vec!["alpha", "bravo", "charlie"]
    );
}

/// An empty batch is a valid batch, and yields no results rather than
/// panicking on a missing first element.
#[test]
fn in_argument_order_accepts_an_empty_batch() {
    let empty: Vec<(usize, &str)> = Vec::new();

    assert_eq!(in_argument_order(empty), Vec::<&str>::new());
}

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

/// The bare mode prints the formatted text and leaves the file alone: printing
/// is a read, however the payload is later rendered.
#[test]
fn print_reports_the_formatted_text_without_writing() {
    let (_dir, directory) = fixture("ragged.md", RAGGED);

    let (_report, payload) = analyse(
        Mode::Print,
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &align,
    )
    .expect("analyse fixture");

    assert_eq!(payload, ALIGNED);
    assert_eq!(read(&directory, "ragged.md"), RAGGED);
}

/// `--in-place` is the one mode that writes, and its payload is empty: the
/// formatted text goes to the file, not to standard output.
#[test]
fn in_place_writes_the_formatted_text() {
    let (_dir, directory) = fixture("ragged.md", RAGGED);

    let (_report, payload) = analyse(
        Mode::InPlace,
        &directory,
        Utf8Path::new("ragged.md"),
        Utf8Path::new("ragged.md"),
        &align,
    )
    .expect("analyse fixture");

    assert_eq!(payload, "");
    assert_eq!(read(&directory, "ragged.md"), ALIGNED);
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
