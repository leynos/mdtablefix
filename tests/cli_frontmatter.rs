//! CLI tests for YAML frontmatter handling.

use assert_cmd::Command;
use rstest::rstest;

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

/// Stdin→stdout equality cases for YAML frontmatter handling.
#[rstest]
#[case::preserved(&[], concat!(
    "---\n",
    "title: Example\n",
    "author: Test\n",
    "---\n",
    "\n",
    "|A|B|\n",
    "|1|2|\n",
), concat!(
    "---\n",
    "title: Example\n",
    "author: Test\n",
    "---\n",
    "\n",
    "| A | B |\n",
    "| 1 | 2 |\n",
))]
#[case::dot_closer(&[], concat!(
    "---\n",
    "title: Example\n",
    "...\n",
    "# Heading\n",
), concat!(
    "---\n",
    "title: Example\n",
    "...\n",
    "# Heading\n",
))]
#[case::later_dash_block_not_frontmatter(&[], concat!(
    "# Heading\n",
    "\n",
    "---\n",
    "\n",
    "Text after break\n",
), concat!(
    "# Heading\n",
    "\n",
    "---\n",
    "\n",
    "Text after break\n",
))]
#[case::with_renumber(&["--renumber"], concat!(
    "---\n",
    "title: Example\n",
    "---\n",
    "\n",
    "3. Third item\n",
    "5. Fifth item\n",
), concat!(
    "---\n",
    "title: Example\n",
    "---\n",
    "\n",
    "1. Third item\n",
    "2. Fifth item\n",
))]
#[case::malformed_treated_as_body(&[], concat!(
    "---\n",
    "This is not valid YAML frontmatter\n",
    "and there is no closing delimiter.\n",
), concat!(
    "---\n",
    "This is not valid YAML frontmatter\n",
    "and there is no closing delimiter.\n",
))]
fn test_cli_yaml_frontmatter_stdin(
    #[case] args: &[&str],
    #[case] input: &str,
    #[case] expected: &str,
) {
    let mut cmd = Command::cargo_bin("mdtablefix").expect("find binary");
    cmd.args(args)
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected.to_string());
}

/// In-place file modification cases for YAML frontmatter handling.
#[rstest]
#[case::basic(&[], concat!(
    "---\n",
    "title: Example\n",
    "---\n",
    "\n",
    "|A|B|\n",
    "|1|2|\n",
), concat!(
    "---\n",
    "title: Example\n",
    "---\n",
    "\n",
    "| A | B |\n",
    "| 1 | 2 |\n",
))]
fn test_cli_yaml_frontmatter_in_place_variants(
    #[case] args: &[&str],
    #[case] input: &str,
    #[case] expected: &str,
) {
    run_in_place(args, input, expected);
}

// Cannot be parameterised: uses partial/line-level assertions rather than stdout equality.
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
    let cmd_result = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command")
        .arg("--wrap")
        .write_stdin(input)
        .assert()
        .success();
    let output = String::from_utf8_lossy(&cmd_result.get_output().stdout);
    assert!(output.starts_with("---\ntitle: Example\n---\n"));
}

// Cannot be parameterised: uses partial/line-level assertions rather than stdout equality.
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
    let cmd_result = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command")
        .args(["--breaks", "--wrap"])
        .write_stdin(input)
        .assert()
        .success();
    let output = String::from_utf8_lossy(&cmd_result.get_output().stdout);
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
