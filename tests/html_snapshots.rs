//! Snapshot coverage for representative HTML-to-Markdown table conversions.

use mdtablefix::convert_html_tables;
use rstest::rstest;

fn snapshot_conversion(name: &str, input: &str) {
    let lines = input.lines().map(ToString::to_string).collect::<Vec<_>>();
    let output = convert_html_tables(&lines).join("\n");

    insta::with_settings!({
        snapshot_path => "snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(name, output);
    });
}

#[rstest]
#[case::single_row(
    "html_single_row_table",
    "<table>\n<tr><th>Name</th><th>Value</th></tr>\n</table>"
)]
#[case::multi_row(
    "html_multi_row_table",
    concat!(
        "<table>\n",
        "<tr><th>Name</th><th>Value</th></tr>\n",
        "<tr><td>Alpha</td><td>1</td></tr>\n",
        "<tr><td>Beta</td><td>22</td></tr>\n",
        "</table>",
    )
)]
#[case::nested(
    "html_nested_table",
    concat!(
        "<table>\n",
        "<tr><th>Outer</th><th>Value</th></tr>\n",
        "<tr><td><table>\n",
        "<tr><th>Inner</th></tr>\n",
        "<tr><td>Nested</td></tr>\n",
        "</table></td><td>Tail</td></tr>\n",
        "</table>",
    )
)]
#[case::sectioned(
    "html_sectioned_table",
    concat!(
        "<table>\n",
        "<thead><tr><th>Name</th><th>Value</th></tr></thead>\n",
        "<tbody>\n",
        "<tr><td>Alpha</td><td>1</td></tr>\n",
        "<tr><td>Beta</td><td>2</td></tr>\n",
        "</tbody>\n",
        "</table>",
    )
)]
fn snapshots_html_table_conversion(#[case] name: &str, #[case] input: &str) {
    snapshot_conversion(name, input);
}
