//! Property tests for inline span grouping invariants.
//!
//! Validates that `determine_token_span` partitions token streams without gaps
//! or overlaps and reports Unicode display widths that match grouped text.

use proptest::prelude::*;
use unicode_width::UnicodeWidthStr;

use super::super::{inline::determine_token_span, tokenize::segment_inline};

fn inline_text_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::string::string_regex(r"[\w\s.,;:!?`()\[\]^-]{0,80}")
            .expect("invalid regex for span grouping property strategy"),
        Just("`code`.[^1]".to_string()),
        Just("[text](url).[^2]".to_string()),
        Just("(`code`).[^3]".to_string()),
        Just("See (`code`).[^4] for details.".to_string()),
        Just("pattern([1](https://github.com/leynos/mdtablefix/pull/url))".to_string()),
        Just(
            concat!(
                "pattern([1](https://github.com/leynos/mdtablefix/pull/url))",
                "([2](https://github.com/leynos/mdtablefix/issues/325))"
            )
            .to_string()
        ),
    ]
}

fn inline_citation_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("pattern([1](https://github.com/leynos/mdtablefix/pull/url))".to_string()),
        Just(
            concat!(
                "pattern([1](https://github.com/leynos/mdtablefix/pull/url))",
                "([2](https://github.com/leynos/mdtablefix/issues/325))"
            )
            .to_string()
        ),
        Just("reference([42](https://github.com/leynos/mdtablefix/tree/main))".to_string()),
    ]
}

fn grouped_spans(tokens: &[String]) -> Vec<String> {
    let mut spans = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let (end, _) = determine_token_span(tokens, index);
        spans.push(tokens[index..end].join(""));
        index = end;
    }
    spans
}

proptest! {
    #[test]
    fn determine_token_span_partitions_segmented_tokens(input in inline_text_strategy()) {
        let tokens = segment_inline(&input);
        if tokens.is_empty() {
            return Ok(());
        }

        let mut index = 0;
        while index < tokens.len() {
            let (end, width) = determine_token_span(&tokens, index);
            prop_assert!(end > index, "span must advance at least one token");
            let grouped = tokens[index..end].join("");
            prop_assert_eq!(width, UnicodeWidthStr::width(grouped.as_str()));
            index = end;
        }
        prop_assert_eq!(index, tokens.len());
    }

    #[test]
    fn determine_token_span_keeps_inline_citation_chain_atomic(
        prefix in prop::string::string_regex(r"[\w ]{0,32}")
            .expect("invalid regex for citation prefix strategy"),
        citation in inline_citation_strategy(),
        suffix in prop::string::string_regex(r"[\w .,;:!?]{0,32}")
            .expect("invalid regex for citation suffix strategy"),
    ) {
        let input = format!("{prefix}{citation}{suffix}");
        let tokens = segment_inline(&input);
        let spans = grouped_spans(&tokens);

        prop_assert!(
            spans.iter().any(|span| span.contains(&citation)),
            "expected citation chain to stay within one span; tokens={tokens:?}, spans={spans:?}",
        );
    }
}
