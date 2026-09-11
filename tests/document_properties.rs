//! Document-boundary tests: line endings, byte-order marks, and the
//! serialization fixed point.
//!
//! Every case drives the real binary over a fixture in `tests/data/document/`.
//! The fixtures deliberately avoid a `.md` extension so that `make fmt` leaves
//! them alone. Byte-exact comparison is the point: these files are the
//! regression oracle for the serialization path.

use assert_cmd::Command;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;
use tempfile::{TempDir, tempdir};

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

/// The fixture's name, addressed relative to the directory capability.
const FIXTURE_NAME: &str = "fixture.dat";

/// Opens a directory capability for `tempdir` and returns it with its UTF-8
/// host path.
///
/// The host path is kept for exactly one purpose: naming the file to the
/// subprocess, which cannot inherit the capability. Every read and write this
/// test performs goes through the returned `Dir`, so the fixture is reached the
/// way the binary reaches it rather than through ambient filesystem calls.
fn capability_directory(tempdir: &TempDir) -> (Dir, Utf8PathBuf) {
    let path = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
        .expect("temporary directory path is UTF-8");
    let directory = Dir::open_ambient_dir(&path, ambient_authority())
        .expect("failed to open temporary directory");
    (directory, path)
}

/// Runs `mdtablefix --in-place` over `input` and returns the resulting bytes.
fn rewrite_in_place(input: &[u8]) -> Vec<u8> {
    let dir = tempdir().expect("temporary directory");
    let (directory, root) = capability_directory(&dir);
    let name = Utf8Path::new(FIXTURE_NAME);
    directory.write(name, input).expect("write fixture");
    Command::cargo_bin("mdtablefix")
        .expect("cargo binary")
        .arg("--in-place")
        .arg(root.join(FIXTURE_NAME).as_std_path())
        .assert()
        .success();
    directory.read(name).expect("read result")
}

#[rstest]
#[case::crlf_ragged(include_bytes!("data/document/crlf_ragged.dat"), document(&TABLE, "\r\n", false))]
#[case::crlf_clean(include_bytes!("data/document/crlf_clean.dat"), document(&TABLE, "\r\n", false))]
#[case::mixed_lf_majority(
    include_bytes!("data/document/mixed_lf_majority.dat"),
    document(&TABLE, "\n", false)
)]
#[case::mixed_crlf_majority(
    include_bytes!("data/document/mixed_crlf_majority.dat"),
    document(&TABLE, "\r\n", false)
)]
#[case::mixed_tie(include_bytes!("data/document/mixed_tie.dat"), document(&["alpha", "beta"], "\n", false))]
#[case::mixed_in_fence(
    include_bytes!("data/document/mixed_in_fence.dat"),
    document(&["| A   | B   |", "| --- | --- |", "| 1   | 2   |", "", "```sh", "echo hi", "```"], "\r\n", false)
)]
// The byte-order mark is split off before formatting, so the table is
// detected and reflowed as usual and the mark is restored on output.
#[case::bom_ragged(
    include_bytes!("data/document/bom_ragged.dat"),
    document(&TABLE, "\n", true)
)]
#[case::bom_crlf_clean(
    include_bytes!("data/document/bom_crlf_clean.dat"),
    document(&TABLE, "\r\n", true)
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
fn stdin_preserves_line_endings() {
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
        escaped(&document(&TABLE, "\r\n", false)),
        "stdin should be reflowed and keep its line-ending style"
    );
}
