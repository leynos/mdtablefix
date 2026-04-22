//! CLI regression tests for wrap behaviour around fenced code blocks.

use std::fs;

use assert_cmd::Command;
use tempfile::NamedTempFile;

/// Guards issue #261 by asserting `--wrap --in-place` leaves fenced shell
/// blocks byte-identical after a heading.
#[test]
fn cli_wrap_in_place_preserves_fenced_shell_block_verbatim() {
    let input = concat!(
        "## Verification\n",
        "\n",
        "```bash\n",
        "set -o pipefail\n",
        "make check-fmt 2>&1 | tee /tmp/fmt.log\n",
        "make lint 2>&1 | tee /tmp/lint.log\n",
        "make test 2>&1 | tee /tmp/test.log\n",
        "```\n",
    );
    let temp = NamedTempFile::new().expect("create temp file");
    fs::write(temp.path(), input).expect("write temp file");

    Command::cargo_bin("mdtablefix")
        .expect("find binary")
        .args(["--wrap", "--in-place"])
        .arg(temp.path())
        .assert()
        .success()
        .stdout("")
        .stderr("");

    let actual = fs::read_to_string(temp.path()).expect("read temp file");
    assert_eq!(
        actual, input,
        "fenced code blocks must remain byte-identical"
    );
}
