//! Snapshot tests for link reference definition preservation during wrapping.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

fn wrap_lines(input: &str) -> String {
    let lines: Vec<String> = input.lines().map(str::to_owned).collect();
    wrap_text(&lines, 80).join("\n")
}

fn assert_wrap_snapshot(name: &str, input: &str) {
    insta::with_settings!({
        snapshot_path => "../snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(name, wrap_lines(input));
    });
}

#[rstest]
#[case("single_link_ref_no_title", "[foo]: https://example.com")]
#[case(
    "single_link_ref_inline_title",
    "[foo]: https://example.com \"My Title\""
)]
#[case("link_ref_next_line_title", "[foo]: https://example.com\n\"My Title\"")]
#[case("collapsed_link_ref_indented_url", "[foo]:\n  https://example.com")]
#[case(
    "collapsed_link_ref_indented_url_next_line_title",
    "[foo]:\n  https://example.com\n  \"My Title\""
)]
#[case(
    "collapsed_link_ref_indented_url_inline_title",
    "[foo]:\n  https://example.com \"My Title\""
)]
#[case(
    "multiple_link_refs",
    concat!(
        "[foo]: https://example.com\n",
        "[bar]: https://example.org\n",
        "[baz]: https://example.net"
    )
)]
#[case(
    "link_ref_mixed_paragraph",
    concat!(
        "First paragraph text here.\n",
        "\n",
        "[foo]: https://example.com\n",
        "\n",
        "Second paragraph text here."
    )
)]
fn link_reference_wrap_snapshots(#[case] name: &str, #[case] input: &str) {
    assert_wrap_snapshot(name, input);
}
