//! Unit tests for text wrapping functionality.
//!
//! Tests are split across focused modules so each file stays within the
//! project line-count guideline.

mod blockquote;
mod classify_block;
mod fence_tracker;
mod inline_wrapping;
mod link_ref_regex;
mod link_reference_definitions;
mod link_reference_state_props;
mod link_reference_state_unit;
mod prefix;
mod span_grouping_props;
mod span_state;
mod token_grouping;

const TRAILING_PUNCTUATION_CHARS: &[char] = &[
    '.', ',', ';', ':', '!', '?', ')', ']', '"', '\'', '…', '—', '–', '»', '›', '）', '］', '】',
    '》', '」', '』', '、', '。', '，', '：', '；', '！', '？', '”', '’',
];
