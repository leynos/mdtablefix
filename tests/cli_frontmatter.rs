//! CLI tests for YAML frontmatter handling.

use assert_cmd::Command;

/// Helper function for in-place file modification tests.
fn run_in_place(args: &[&str], input: &str, expected: &str) {
    let temp = tempfile::NamedTempFile::new().expect("create temp file");
    std::fs::write(temp.path(), input).expect("write temp file");

    let mut cmd = Command::cargo_bin("mdtablefix").expect("find binary");
    cmd.arg("--in-place").args(args).arg(temp.path());
    cmd.assert().success();

    let actual = std::fs::read_to_string(temp.path()).expect("read temp file");
    assert_eq!(actual, expected, "in-place content mismatch");
}

/// Tests that YAML frontmatter is preserved unchanged while the body is formatted.
#[test]
fn test_cli_yaml_frontmatter_preserved() {
    let input = concat!(
        "---\n",
        "title: Example\n",
        "author: Test\n",
        "---\n",
        "\n",
        "|A|B|\n",
        "|1|2|\n",
    );
    let expected = concat!(
        "---\n",
        "title: Example\n",
        "author: Test\n",
        "---\n",
        "\n",
        "| A | B |\n",
        "| 1 | 2 |\n",
    );
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
}

/// Tests that YAML frontmatter is preserved with `--wrap` option.
#[test]
fn test_cli_yaml_frontmatter_with_wrap() {
    let input = concat!(
        "---\n",
        "title: Example\n",
        "---\n",
        "\n",
        "This is a very long paragraph that should be wrapped to 80 columns when the wrap option \
         is enabled.\n",
    );
    let binding = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command")
        .arg("--wrap")
        .write_stdin(input)
        .assert()
        .success();
    let output = String::from_utf8_lossy(&binding.get_output().stdout);
    assert!(output.starts_with("---\ntitle: Example\n---\n"));
}

/// Tests that YAML frontmatter delimiters are not rewritten by `--breaks`.
#[test]
fn test_cli_yaml_frontmatter_with_breaks() {
    let input = concat!(
        "---\n",
        "title: Example\n",
        "---\n",
        "\n",
        "Text\n",
        "\n",
        "---\n",
        "\n",
        "More text\n",
    );
    let binding = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command")
        .args(["--breaks", "--wrap"])
        .write_stdin(input)
        .assert()
        .success();
    let output = String::from_utf8_lossy(&binding.get_output().stdout);
    // Frontmatter delimiters should be preserved
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "---");
    assert_eq!(lines[1], "title: Example");
    assert_eq!(lines[2], "---");
    // The later --- should be converted to underscores (thematic break)
    let later_dashes = lines.iter().position(|l| l.starts_with("___"));
    assert!(
        later_dashes.is_some(),
        "thematic break should be underscores"
    );
}

/// Tests that YAML frontmatter with `...` closer is preserved.
#[test]
fn test_cli_yaml_frontmatter_dot_closer() {
    let input = concat!("---\n", "title: Example\n", "...\n", "# Heading\n",);
    let expected = concat!("---\n", "title: Example\n", "...\n", "# Heading\n",);
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
}

/// Tests that a `---` line later in the document (not frontmatter) is still processed.
#[test]
fn test_cli_later_dash_block_not_frontmatter() {
    let input = concat!("# Heading\n", "\n", "---\n", "\n", "Text after break\n",);
    // Without --breaks, the --- stays as is
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(input);
}

/// Tests YAML frontmatter preservation with `--in-place`.
#[test]
fn test_cli_yaml_frontmatter_in_place() {
    let input = concat!(
        "---\n",
        "title: Example\n",
        "---\n",
        "\n",
        "|A|B|\n",
        "|1|2|\n",
    );
    let expected = concat!(
        "---\n",
        "title: Example\n",
        "---\n",
        "\n",
        "| A | B |\n",
        "| 1 | 2 |\n",
    );
    run_in_place(&[], input, expected);
}

/// Tests YAML frontmatter preservation together with `--renumber`.
#[test]
fn test_cli_yaml_frontmatter_with_renumber() {
    let input = concat!(
        "---\n",
        "title: Example\n",
        "---\n",
        "\n",
        "3. Third item\n",
        "5. Fifth item\n",
    );
    let expected = concat!(
        "---\n",
        "title: Example\n",
        "---\n",
        "\n",
        "1. Third item\n",
        "2. Fifth item\n",
    );

    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--renumber")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
}

/// Tests that malformed YAML frontmatter (missing closer) is treated as body content.
#[test]
fn test_cli_malformed_yaml_frontmatter_treated_as_body() {
    // Leading '---' without a closing delimiter should be treated as normal body content,
    // not as YAML frontmatter.
    let input = concat!(
        "---\n",
        "This is not valid YAML frontmatter\n",
        "and there is no closing delimiter.\n",
    );
    let expected = input;

    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
}
