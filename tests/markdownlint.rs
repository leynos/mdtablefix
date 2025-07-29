//! Tests for markdownlint directive handling during wrapping.
//!
//! These tests ensure that comment directives such as
//! `<!-- markdownlint-disable-next-line -->` remain on their own line
//! after processing. Regular comments should still be wrapped normally.

use mdtablefix::process_stream;

#[macro_use]
mod prelude;
use prelude::*;

/// The disable-next-line directive must remain intact after wrapping.
#[test]
fn test_markdownlint_disable_next_line_preserved() {
    let input = lines_vec![
        "[roadmap](./roadmap.md) and expands on the design ideas described in",
        "<!--  markdownlint-disable-next-line  MD013  -->",
    ];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

/// Regular comments should still wrap when necessary.
#[test]
fn test_regular_comment_wraps_normally() {
    let input = lines_vec![
        "Intro text that preludes a lengthy comment.",
        concat!(
            "<!-- This comment contains many words and should be wrapped across ",
            "multiple lines to ensure that regular comments are formatted ",
            "correctly. -->"
        ),
    ];
    let output = process_stream(&input);
    assert_eq!(
        output,
        lines_vec![
            "Intro text that preludes a lengthy comment. <!-- This comment contains many",
            "words and should be wrapped across multiple lines to ensure that regular",
            "comments are formatted correctly. -->",
        ]
    );
}

/// Other markdownlint directives should also remain on their own lines, even
/// when indented or combined with multiple rule names.
#[rstest]
#[case("<!-- markdownlint-disable-line MD001 MD005 -->")]
#[case("<!-- markdownlint-enable MD001 -->")]
#[case("    <!-- markdownlint-disable -->")]
fn test_markdownlint_directive_variants_preserved(#[case] directive: &str) {
    let input = lines_vec!["A preceding line.", directive];
    let output = process_stream(&input);
    assert_eq!(output, input);
}
