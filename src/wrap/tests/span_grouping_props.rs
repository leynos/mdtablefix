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
    ]
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
}
