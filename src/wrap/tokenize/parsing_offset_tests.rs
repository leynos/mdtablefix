//! Invalid-offset rejection tests for the parsing helpers.
//!
//! These helpers take a byte offset and must classify positions that no
//! reachable caller produces: an offset inside a multibyte scalar, an offset
//! past the end, and `usize::MAX`. Each table pairs a valid call with the
//! rejected positions so a helper that wrongly rejected ordinary input fails
//! here rather than passing vacuously.
//!
//! They live apart from `parsing_tests` so that file stays within the
//! 400-line limit; the split follows the `#[path]` submodule pattern already
//! used by `parsing.rs` for `parsing_tracing_snapshots`.

use rstest::rstest;

use super::*;

#[rstest]
#[case::valid_bang("![alt](url)", 0, '!', true)]
#[case::bang_without_bracket("! not", 0, '!', false)]
#[case::not_a_bang("x[alt](url)", 0, 'x', false)]
// Offset 1 is inside the two-byte `å`, so it is not a character boundary. The
// unchecked form advanced to byte 2, which is exactly the `[`, and returned
// true for a position that cannot occur in real tokenization.
#[case::mid_character("å[", 1, '!', false)]
#[case::past_end("å[", "å[".len() + 1, '!', false)]
// `usize::MAX` panicked on the unchecked addition in debug and wrapped to 0 in
// release, where the wrapped index was then probed as if it were valid.
#[case::max_offset("![alt](url)", usize::MAX, '!', false)]
fn looks_like_image_start_rejects_invalid_offsets(
    #[case] text: &str,
    #[case] idx: usize,
    #[case] ch: char,
    #[case] expected: bool,
) {
    assert_eq!(looks_like_image_start(text, idx, ch), expected);
}

#[rstest]
#[case::mid_character("å[", 1)]
#[case::past_end("å[", "å[".len() + 1)]
fn parse_link_text_returns_none_for_invalid_start_offsets(
    #[case] text: &str,
    #[case] start: usize,
) {
    assert_eq!(parse_link_text(text, start), None);
}

#[rstest]
#[case::mid_character("å(", 1)]
#[case::past_end("å(", "å(".len() + 1)]
fn parse_link_url_returns_none_for_invalid_start_offsets(#[case] text: &str, #[case] start: usize) {
    assert_eq!(parse_link_url(text, start), None);
}

#[rstest]
#[case::mid_character("åx", 1)]
#[case::past_end("abc", 4)]
#[case::max_offset("abc", usize::MAX)]
fn parse_link_or_image_falls_back_for_invalid_start_offsets(
    #[case] text: &str,
    #[case] start: usize,
) {
    // The helper returns a (token, next_index) pair rather than an Option, so
    // its fallback is the empty token and the end of the text: `collect_range`
    // rejects the range and `fallback_single_char` cannot find a scalar at an
    // invalid offset, so `next` is `text.len()`.
    assert_eq!(
        parse_link_or_image(text, start),
        (String::new(), text.len())
    );
}
