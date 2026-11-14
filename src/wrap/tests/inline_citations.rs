//! Inline citation attachment cases for the wrapping pipeline.
//!
//! A citation is a parenthesised link such as `([1](url))`, and the wrapper
//! must never separate its opening parenthesis from the link it introduces, nor
//! leave adjacent citations' punctuation stranded. These cases cover the
//! `([n](url))` attachment introduced in issue #325, using `rstest` for
//! fixture-driven cases and `insta` for rendered snapshots.
//!
//! The helpers live here rather than in `inline_wrapping` so that module stays
//! under the file-size ceiling; every one of them is specific to citations.

use std::fmt::Write as _;

use rstest::rstest;

use super::super::wiring::wrap_preserving_code;

fn citation_link_starts(expected_citation: &str) -> Vec<String> {
    let mut markers = Vec::new();
    let mut remaining = expected_citation;
    while let Some(open_index) = remaining.find('[') {
        let candidate = &remaining[open_index..];
        let Some(close_index) = candidate.find("](") else {
            break;
        };
        markers.push(candidate[..close_index + 2].to_string());
        remaining = &candidate[close_index + 2..];
    }
    markers
}

fn inline_citation_chain(citation_count: usize) -> String {
    let mut citation = String::new();
    for index in 1..=citation_count {
        write!(citation, "([{index}](https://example.com/ref{index}))")
            .expect("writing to String cannot fail");
    }
    citation
}

/// Asserts the four attachment invariants a citation fixture must satisfy.
///
/// The checks are stated together because they only make sense as a set: a
/// citation that stays attached but starts a continuation line is still broken
/// output, and the failure messages name which invariant was lost.
fn assert_inline_citation_invariants(lines: &[String], expected_citation: &str) {
    let citation_link_starts = citation_link_starts(expected_citation);
    assert!(
        !citation_link_starts.is_empty(),
        "expected citation fixture must contain at least one inline link",
    );
    assert!(
        lines.iter().any(|line| line.contains(expected_citation)),
        "expected citation to stay attached in {lines:?}",
    );
    assert!(
        lines.iter().all(|line| !line.ends_with('(')),
        "opening citation punctuation must not be stranded at line end: {lines:?}",
    );
    assert!(
        lines.iter().all(|line| {
            let trimmed = line.trim_start();
            citation_link_starts
                .iter()
                .all(|marker| !trimmed.starts_with(marker))
        }),
        "citation link must not start a continuation line: {lines:?}",
    );
    assert!(
        lines.iter().all(|line| line.trim() != ")("),
        "adjacent citation punctuation must not be orphaned: {lines:?}",
    );
}

#[rstest]
#[case(
    "The formatter keeps pattern([1](https://example.com/ref)) attached while wrapping.",
    32,
    "pattern([1](https://example.com/ref))"
)]
#[case(
    concat!(
        "The formatter keeps runtime([6](https://example.com/command))",
        "([7](https://example.com/event)) attached while wrapping."
    ),
    34,
    "runtime([6](https://example.com/command))([7](https://example.com/event))",
)]
fn wrap_preserving_code_keeps_inline_citation_links_attached(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected_citation: &str,
) {
    let lines = wrap_preserving_code(input, width);
    assert_inline_citation_invariants(&lines, expected_citation);
}

#[test]
fn wrap_preserving_code_snapshots_single_inline_citation() {
    let lines = wrap_preserving_code(
        "The formatter keeps pattern([1](https://example.com/ref)) attached while wrapping.",
        32,
    );
    insta::assert_snapshot!(
        lines.join("\n"),
        @r###"
The formatter keeps
pattern([1](https://example.com/ref))
attached while wrapping.
"###
    );
}

#[test]
fn wrap_preserving_code_snapshots_adjacent_inline_citations() {
    let lines = wrap_preserving_code(
        concat!(
            "The formatter keeps runtime([6](https://example.com/command))",
            "([7](https://example.com/event)) attached while wrapping."
        ),
        34,
    );
    insta::assert_snapshot!(
        lines.join("\n"),
        @r###"
The formatter keeps
runtime([6](https://example.com/command))([7](https://example.com/event))
attached while wrapping.
"###
    );
}

proptest::proptest! {
    #[test]
    fn wrap_preserving_code_keeps_generated_inline_citations_attached(
        wrap_width in 24usize..96,
        prefix_len in 0usize..32,
        citation_count in 1usize..6,
    ) {
        let citation = inline_citation_chain(citation_count);
        let expected_citation = format!("pattern{citation}");
        let input = format!(
            "{}{expected_citation} trailing words force wrapping",
            "lead ".repeat(prefix_len)
        );
        let lines = wrap_preserving_code(&input, wrap_width);

        assert_inline_citation_invariants(&lines, &expected_citation);
    }
}
