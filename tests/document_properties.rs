//! Document-boundary tests: line endings, byte-order marks, and the
//! serialization fixed point.
//!
//! Every case drives the real binary over a fixture in `tests/data/document/`.
//! The fixtures deliberately avoid a `.md` extension so that `make fmt` leaves
//! them alone. Byte-exact comparison is the point: these files are the
//! regression oracle for the serialization path.

use std::fs;

use assert_cmd::Command;
use rstest::rstest;
use tempfile::tempdir;

/// Canonical rendering of the three-line probe table shared by most fixtures.
///
/// `mdtablefix` pads every column to at least three characters, so a
/// single-character table renders with three-character fields.
const TABLE: [&str; 3] = ["| A   | B   |", "| --- | --- |", "| 1   | 2   |"];

/// Renders `lines` with `ending`, optionally prefixed by a byte-order mark.
fn document(lines: &[&str], ending: &str, bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if bom {
        bytes.extend_from_slice("\u{FEFF}".as_bytes());
    }
    for line in lines {
        bytes.extend_from_slice(line.as_bytes());
        bytes.extend_from_slice(ending.as_bytes());
    }
    bytes
}

/// Renders `bytes` for a readable assertion message.
fn escaped(bytes: &[u8]) -> String { String::from_utf8_lossy(bytes).escape_debug().to_string() }

/// Runs `mdtablefix --in-place` over `input` and returns the resulting bytes.
fn rewrite_in_place(input: &[u8]) -> Vec<u8> {
    let dir = tempdir().expect("temporary directory");
    let path = dir.path().join("fixture.dat");
    fs::write(&path, input).expect("write fixture");
    Command::cargo_bin("mdtablefix")
        .expect("cargo binary")
        .arg("--in-place")
        .arg(&path)
        .assert()
        .success();
    fs::read(&path).expect("read result")
}

#[rstest]
#[case::crlf_ragged(include_bytes!("data/document/crlf_ragged.dat"), document(&TABLE, "\n", false))]
#[case::crlf_clean(include_bytes!("data/document/crlf_clean.dat"), document(&TABLE, "\n", false))]
#[case::mixed_lf_majority(
    include_bytes!("data/document/mixed_lf_majority.dat"),
    document(&TABLE, "\n", false)
)]
#[case::mixed_crlf_majority(
    include_bytes!("data/document/mixed_crlf_majority.dat"),
    document(&TABLE, "\n", false)
)]
#[case::mixed_tie(include_bytes!("data/document/mixed_tie.dat"), document(&["alpha", "beta"], "\n", false))]
#[case::mixed_in_fence(
    include_bytes!("data/document/mixed_in_fence.dat"),
    document(&["| A   | B   |", "| --- | --- |", "| 1   | 2   |", "", "```sh", "echo hi", "```"], "\n", false)
)]
// The byte-order mark defeats table detection, so the header line survives
// verbatim and the separator/data pair is reflowed with the data row first.
#[case::bom_ragged(
    include_bytes!("data/document/bom_ragged.dat"),
    document(&["\u{FEFF}|A|B|", "| 1   | 2   |", "| --- | --- |"], "\n", false)
)]
#[case::bom_crlf_clean(
    include_bytes!("data/document/bom_crlf_clean.dat"),
    document(&["\u{FEFF}| A | B |", "| 1   | 2   |", "| --- | --- |"], "\n", false)
)]
#[case::lone_cr(include_bytes!("data/document/lone_cr.dat"), b"alpha\rbeta\n".to_vec())]
#[case::empty(include_bytes!("data/document/empty.dat"), Vec::new())]
#[case::no_trailing_newline(
    include_bytes!("data/document/no_trailing_newline.dat"),
    document(&TABLE, "\n", false)
)]
fn in_place_document_boundary(#[case] input: &[u8], #[case] expected: Vec<u8>) {
    let actual = rewrite_in_place(input);
    assert_eq!(escaped(&actual), escaped(&expected));
}

#[test]
fn stdin_is_normalized_to_line_feed() {
    let stdout = Command::cargo_bin("mdtablefix")
        .expect("cargo binary")
        .write_stdin(b"|A|B|\r\n|---|---|\r\n|1|2|\r\n".to_vec())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        escaped(&stdout),
        escaped(&document(&TABLE, "\n", false)),
        "stdin should be reflowed and newline-normalized"
    );
}
