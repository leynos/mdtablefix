//! Snapshot coverage for leading-indent handling at each consumer boundary.

use mdtablefix::{attach_orphan_specifiers, convert_html_tables, reflow_table, wrap_text};

fn lines(input: &[&str]) -> Vec<String> { input.iter().map(ToString::to_string).collect() }

fn assert_snapshot(name: &str, output: &[String]) {
    insta::with_settings!({
        snapshot_path => "snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(name, output.join("\n"));
    });
}

#[test]
fn leading_indent_html_conversion() {
    assert_snapshot(
        "leading_indent_html_conversion",
        &convert_html_tables(&lines(&[
            "  <table>",
            "\t<tr><th>Header</th></tr>",
            " \t<tr><td>Cell</td></tr>",
            "  </table>",
        ])),
    );
}

#[test]
fn leading_indent_fence_attachment() {
    assert_snapshot(
        "leading_indent_fence_attachment",
        &attach_orphan_specifiers(&lines(&[" \trust", "\t```", "\tlet value = 1;", "\t```"])),
    );
}

#[test]
fn leading_indent_table_reflow() {
    assert_snapshot(
        "leading_indent_table_reflow",
        &reflow_table(&lines(&[
            "\t | Header | Value |",
            "\t | --- | --- |",
            "\t | A | B |",
        ])),
    );
}

#[test]
fn leading_indent_paragraph_wrap() {
    assert_snapshot(
        "leading_indent_paragraph_wrap",
        &wrap_text(
            &lines(&["  An indented paragraph keeps its indentation when it wraps across lines."]),
            30,
        ),
    );
}
