//! Snapshot tests for link reference definition preservation during wrapping.

use mdtablefix::wrap::wrap_text;

fn wrap_lines(input: &str) -> String {
    let lines: Vec<String> = input.lines().map(str::to_owned).collect();
    wrap_text(&lines, 80).join("\n")
}

fn assert_wrap_snapshot(name: &str, input: &str) {
    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!(name, wrap_lines(input));
    });
}

#[test]
fn single_link_ref_no_title() {
    assert_wrap_snapshot("single_link_ref_no_title", "[foo]: https://example.com");
}

#[test]
fn single_link_ref_inline_title() {
    assert_wrap_snapshot(
        "single_link_ref_inline_title",
        "[foo]: https://example.com \"My Title\"",
    );
}

#[test]
fn link_ref_next_line_title() {
    assert_wrap_snapshot(
        "link_ref_next_line_title",
        "[foo]: https://example.com\n\"My Title\"",
    );
}

#[test]
fn multiple_link_refs() {
    assert_wrap_snapshot(
        "multiple_link_refs",
        concat!(
            "[foo]: https://example.com\n",
            "[bar]: https://example.org\n",
            "[baz]: https://example.net"
        ),
    );
}

#[test]
fn link_ref_mixed_paragraph() {
    assert_wrap_snapshot(
        "link_ref_mixed_paragraph",
        concat!(
            "First paragraph text here.\n",
            "\n",
            "[foo]: https://example.com\n",
            "\n",
            "Second paragraph text here."
        ),
    );
}
