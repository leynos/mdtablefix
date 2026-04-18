//! Unit tests for text wrapping functionality.
//!
//! Tests are split across focused modules so each file stays within the
//! project line-count guideline.

mod classify_block;
mod fence_tracker;
mod inline_wrapping;
mod link_ref_regex;
mod link_reference_definitions;
mod prefix;
mod span_grouping_props;
mod span_state;
mod token_grouping;

fn wrap_text_treats_whitespace_only_lines_as_paragraph_breaks() {
    let input = vec!["foo".to_string(), "   ".to_string(), "bar".to_string()];
    let wrapped = wrap_text(&input, 80);
    assert_eq!(
        wrapped,
        vec!["foo".to_string(), String::new(), "bar".to_string()]
    );
}

const TRAILING_PUNCTUATION_CHARS: &[char] = &[
    '.', ',', ';', ':', '!', '?', ')', ']', '"', '\'', '…', '—', '–', '»', '›', '）', '］', '】',
    '》', '」', '』', '、', '。', '，', '：', '；', '！', '？', '”', '’',
];
