//! End-to-end snapshots for library frontmatter processing.

use mdtablefix::process::{Options, process_stream_opts};
use rstest::rstest;

fn process_document(input: &str) -> String {
    let lines: Vec<String> = input.lines().map(str::to_owned).collect();
    process_stream_opts(&lines, Options::default()).join("\n")
}

#[rstest]
#[case::frontmatter_and_table(
    "library_frontmatter_and_table",
    concat!("---\n", "title: A | table\n", "---\n", "\n", "|A|B|\n", "|1|2|\n"),
)]
#[case::without_frontmatter(
    "library_without_frontmatter",
    concat!("|A|B|\n", "|1|2|\n"),
)]
#[case::dot_closer(
    "library_frontmatter_with_dot_closer",
    concat!("---\n", "title: Dot closer\n", "...\n", "\n", "|A|B|\n", "|1|2|\n"),
)]
fn process_stream_opts_frontmatter_snapshots(#[case] name: &str, #[case] input: &str) {
    insta::with_settings!({
        snapshot_path => "snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(name, process_document(input));
    });
}
