//! Prefix-focused wrap tests extracted to keep `tests.rs` below 400 lines.

use rstest::rstest;

use crate::wrap::{BlockquotePrefix, prefix_line, wrap_text};

#[rstest]
#[case("- item", "- ", "item", false)]
#[case("  1. item", "  1. ", "item", false)]
#[case("> quote", "> ", "quote", true)]
#[case("> - item", "> - ", "item", false)]
#[case("[^7]: note", "[^7]: ", "note", false)]
fn prefix_line_extracts_supported_prefixes(
    #[case] input: &str,
    #[case] expected_prefix: &str,
    #[case] expected_rest: &str,
    #[case] expected_repeat: bool,
) {
    let blockquote = BlockquotePrefix::parse(input);
    let inner = blockquote.map_or(input, |prefix| prefix.inner());
    let prefix = prefix_line(inner, blockquote).expect("supported prefix should parse");
    assert_eq!(prefix.prefix.as_ref(), expected_prefix);
    assert_eq!(prefix.rest, expected_rest);
    assert_eq!(prefix.repeat_prefix, expected_repeat);
}

#[rstest]
#[case("")]
#[case("plain text")]
#[case("    code")]
#[case("| table |")]
#[case("# Heading")]
fn prefix_line_rejects_non_prefixed_lines(#[case] input: &str) {
    assert!(prefix_line(input, None).is_none());
}

#[test]
fn wrap_with_prefix_single_line() {
    let input = vec![">> hello world".to_string()];
    let wrapped = wrap_text(&input, 80);
    assert_eq!(wrapped, vec![">> hello world".to_string()]);
}

#[test]
fn wrap_with_prefix_multiline_uses_continuation() {
    let input = vec!["> alpha beta gamma".to_string()];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(
        wrapped,
        vec![
            "> alpha".to_string(),
            "> beta".to_string(),
            "> gamma".to_string(),
        ]
    );
}

#[test]
fn wrap_text_repeats_nested_blockquote_prefix() {
    let prefix = "> > ";
    let input = vec![
        concat!(
            "> > This nested quote contains enough text to require wrapping so that we can verify ",
            "multi-level handling."
        )
        .to_string(),
    ];
    let wrapped = wrap_text(&input, 80);
    assert!(wrapped.len() > 1);
    assert!(wrapped.iter().all(|line| line.starts_with("> > ")));
    let wrapped_payload = wrapped
        .iter()
        .map(|line| {
            line.strip_prefix(prefix)
                .expect("nested blockquote line keeps its prefix")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let normalized_payload = wrapped_payload
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        normalized_payload,
        input[0]
            .strip_prefix(prefix)
            .expect("original test input uses the nested blockquote prefix")
    );
}

#[test]
fn wrap_with_prefix_plain_indent_both_lines() {
    let input = vec!["  alpha beta gamma".to_string()];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(
        wrapped,
        vec![
            "  alpha".to_string(),
            "  beta".to_string(),
            "  gamma".to_string(),
        ]
    );
}

#[rstest(
    input,
    width,
    expected,
    case(
        vec![
            concat!(
                "[^5]: Given When Then - Martin Fowler, accessed on 14 July 2025, ",
                "<https://martinfowler.com/bliki/GivenWhenThen.html>"
            )
            .to_string(),
        ],
        80,
        vec![
            "[^5]: Given When Then - Martin Fowler, accessed on 14 July 2025,"
                .to_string(),
            "      <https://martinfowler.com/bliki/GivenWhenThen.html>"
                .to_string(),
        ]
    ),
    case(
        vec![
            concat!(
                "- [ ] Create a `HttpTravelTimeProvider` struct that implements the ",
                "`TravelTimeProvider` trait."
            )
            .to_string(),
        ],
        70,
        vec![
            "- [ ] Create a `HttpTravelTimeProvider` struct that implements the"
                .to_string(),
            "      `TravelTimeProvider` trait.".to_string(),
        ]
    )
)]
fn wrap_text_preserves_prefixed_continuation_alignment(
    input: Vec<String>,
    width: usize,
    expected: Vec<String>,
) {
    let wrapped = wrap_text(&input, width);
    assert_eq!(wrapped, expected);
}
