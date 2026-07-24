//! End-to-end snapshots for CLI frontmatter processing.

use assert_cmd::Command;
use rstest::rstest;

fn run_cli(input: &str, args: &[&str]) -> String {
    let mut command = Command::cargo_bin("mdtablefix").expect("find binary");
    let output = command
        .args(args)
        .write_stdin(input)
        .output()
        .expect("run binary");
    assert!(
        output.status.success(),
        "binary failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout is UTF-8")
}

#[rstest]
#[case::default_with_frontmatter(
    "cli_frontmatter_default",
    concat!("---\n", "title: Example\n", "---\n", "\n", "|A|B|\n", "|1|2|\n"),
    &[],
)]
#[case::renumber_and_breaks_with_frontmatter(
    "cli_frontmatter_renumber_and_breaks",
    concat!(
        "---\n",
        "title: Example\n",
        "---\n",
        "\n",
        "3. Third item\n",
        "5. Fifth item\n",
        "\n",
        "---\n",
    ),
    &["--renumber", "--breaks"],
)]
#[case::without_frontmatter(
    "cli_without_frontmatter",
    concat!("|A|B|\n", "|1|2|\n"),
    &[],
)]
fn cli_frontmatter_snapshots(#[case] name: &str, #[case] input: &str, #[case] args: &[&str]) {
    insta::with_settings!({
        snapshot_path => "snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(name, run_cli(input, args));
    });
}
