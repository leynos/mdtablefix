//! End-to-end tests for command-line ellipsis replacement.

use assert_cmd::Command;

/// Tests that `--ellipsis` replaces triple dots with a Unicode ellipsis.
#[test]
fn replaces_prose_ellipsis() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("foo...\n")
        .assert()
        .success()
        .stdout("foo…\n");
}

/// Tests that `--ellipsis` preserves dots within inline code spans.
#[test]
fn preserves_inline_code_span() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("before `dots...` after\n")
        .assert()
        .success()
        .stdout("before `dots...` after\n");
}

/// Tests that `--ellipsis` does not alter fenced code blocks.
#[test]
fn preserves_fenced_code_block() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("```\nlet x = ...;\n```\n")
        .assert()
        .success()
        .stdout("```\nlet x = ...;\n```\n");
}

/// Tests that `--ellipsis` preserves command output in indented code blocks.
#[test]
fn preserves_indented_code_block() {
    let input = concat!(
        "Expected test output:\n",
        "\n",
        "    running 2 tests\n",
        "    test foo ... ok\n",
        "    test bar ... ok\n",
        "    ...\n",
        "    test result: ok\n",
        "\n",
        "Prose...\n",
    );
    let expected = concat!(
        "Expected test output:\n",
        "\n",
        "    running 2 tests\n",
        "    test foo ... ok\n",
        "    test bar ... ok\n",
        "    ...\n",
        "    test result: ok\n",
        "\n",
        "Prose…\n",
    );

    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
}

/// Tests that `--ellipsis` preserves semantic dot runs in literal regions.
#[test]
fn preserves_literal_regions() {
    let input = concat!(
        "Release... notes.\n",
        "[0.1.1]: https://github.com/leynos/diesel-cte-ext/compare/",
        "v0.1.0...302d156361161fd73310926dcef6513b41f7b393\n",
        "[split]:\n",
        "  https://example.com/compare/v1...v2\n",
        "  \"Versions v1...v2\"\n",
        "See [comparison...](https://example.com/v1...v2).\n",
        "See ![diagram...](images/v1...v2.png).\n",
        "See [opening...](path\\(v1...v2).\n",
        "See [closing...](path\\)v1...v2).\n",
        "Visit https://example.com/v1...v2 directly.\n",
        "Visit <https://example.com/v1...v2> as an autolink.\n",
        "Escaped \\<https://example.com/v1...v2> normalizes.\n",
        "Open ./fixtures/.../expected.txt.\n",
    );
    let expected = concat!(
        "Release… notes.\n",
        "[0.1.1]: https://github.com/leynos/diesel-cte-ext/compare/",
        "v0.1.0...302d156361161fd73310926dcef6513b41f7b393\n",
        "[split]:\n",
        "  https://example.com/compare/v1...v2\n",
        "  \"Versions v1...v2\"\n",
        "See [comparison...](https://example.com/v1...v2).\n",
        "See ![diagram...](images/v1...v2.png).\n",
        "See [opening...](path\\(v1...v2).\n",
        "See [closing...](path\\)v1...v2).\n",
        "Visit https://example.com/v1...v2 directly.\n",
        "Visit <https://example.com/v1...v2> as an autolink.\n",
        "Escaped \\<https://example.com/v1…v2> normalizes.\n",
        "Open ./fixtures/.../expected.txt.\n",
    );

    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
}

/// Tests ellipsis replacement for sequences longer than three characters.
#[test]
fn replaces_long_sequence() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("wait....\n")
        .assert()
        .success()
        .stdout("wait….\n");
}

/// Tests that `--ellipsis` handles multiple sequences in one line.
#[test]
fn replaces_multiple_sequences() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("First... then second... done.\n")
        .assert()
        .success()
        .stdout("First… then second… done.\n");
}
