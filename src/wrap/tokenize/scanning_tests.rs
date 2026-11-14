//! Unit tests for byte-level tokenizer scanning helpers.

use proptest::prelude::*;
use rstest::rstest;

use super::*;

#[rstest]
#[case::alpha_prefix("abc123", 0, char::is_alphabetic as fn(char) -> bool, 3)]
#[case::numeric_suffix("abc123", 3, char::is_numeric as fn(char) -> bool, "abc123".len())]
#[case::multibyte_scan(
    "åßç123",
    0,
    char::is_alphabetic as fn(char) -> bool,
    "åßç123".find('1').expect("digit should be present in test case")
)]
#[case::invalid_utf8_boundary("åx", 1, char::is_alphabetic as fn(char) -> bool, 1)]
#[case::past_end("abc", 4, char::is_alphabetic as fn(char) -> bool, 4)]
fn scan_while_cases(
    #[case] text: &str,
    #[case] start: usize,
    #[case] predicate: fn(char) -> bool,
    #[case] expected_idx: usize,
) {
    assert_eq!(scan_while(text, start, predicate), expected_idx);
}

#[rstest]
#[case::first_two("αβγδε", 0, "αβ".len(), "αβ")]
#[case::middle("αβγδε", "αβ".len(), "αβ".len() + "γδ".len(), "γδ")]
#[case::invalid_utf8_boundary("åx", 1, 3, "")]
#[case::end_past_end("abc", 0, 4, "")]
#[case::start_past_end("abc", 4, 5, "")]
#[case::reversed_range("abc", 2, 1, "")]
fn collect_range_cases(
    #[case] text: &str,
    #[case] start: usize,
    #[case] end: usize,
    #[case] expected: &str,
) {
    assert_eq!(collect_range(text, start, end), expected);
}

// `idx` is the position of the byte being classified; the run of backslashes is
// counted backwards from `idx`. So each case below points at the byte after the
// run, which is where the caller asks the question.
#[rstest]
#[case::escaped_by_one(br"\[", 1, true)]
#[case::escaped_by_three(br"\\\[", 3, true)]
#[case::escaped_by_four(br"\\\\[", 4, false)]
#[case::no_preceding_backslash(b"x[", 1, false)]
#[case::at_start(b"[", 0, false)]
// `idx == len` is a valid position, not a reject: the run is counted back from
// the end of the slice, so these two pin that the final byte is classified
// rather than skipped. The answers differ between them, so neither is vacuous.
#[case::at_end_counts_trailing_run(b"ab\\", 3, true)]
#[case::at_end_even_trailing_run(b"ab\\\\", 4, false)]
// Past the end the loop never runs, the count stays zero, and the answer is
// `false` rather than a panic or a wrapped read.
#[case::past_end(b"abc", 99, false)]
#[case::max_offset(b"abc", usize::MAX, false)]
fn has_odd_backslash_escape_bytes_cases(
    #[case] bytes: &[u8],
    #[case] idx: usize,
    #[case] expected: bool,
) {
    assert_eq!(has_odd_backslash_escape_bytes(bytes, idx), expected);
}

// `idx` is the index of the `[`, so the previous byte is the `!` being tested.
#[rstest]
#[case::escaped_bang(br"\![", 2, true)]
#[case::unescaped_bang(b"![", 1, false)]
#[case::not_a_bang(b"?[", 1, false)]
#[case::no_preceding_byte(b"[", 0, false)]
// `idx == len` is the valid end boundary; the byte before it is the `!` being
// tested, so an escape still reaches it. One case true, one false.
#[case::at_end_escaped_bang(b"\\!", 2, true)]
#[case::at_end_unescaped_bang(b"x!", 2, false)]
#[case::past_end(b"abc", 99, false)]
#[case::max_offset(b"abc", usize::MAX, false)]
fn bracket_follows_escaped_bang_cases(
    #[case] bytes: &[u8],
    #[case] idx: usize,
    #[case] expected: bool,
) {
    assert_eq!(bracket_follows_escaped_bang(bytes, idx), expected);
}

#[rstest]
#[case("`VarGuard`s alive", "`VarGuard`".len(), "`VarGuard`s".len())]
#[case("`class`'s field", "`class`".len(), "`class`'s".len())]
#[case("`code`-style name", "`code`".len(), "`code`-style".len())]
#[case("`code`-2 next", "`code`".len(), "`code`".len())]
#[case("`code`.", "`code`".len(), "`code`".len())]
#[case("`code`**", "`code`".len(), "`code`".len())]
#[case("`code`'2 next", "`code`".len(), "`code`".len())]
#[case::invalid_utf8_boundary("åx", 1, 1)]
#[case::past_end("abc", 4, 4)]
fn scan_code_suffix_end_cases(#[case] text: &str, #[case] start: usize, #[case] expected: usize) {
    assert_eq!(scan_code_suffix_end(text, start), expected);
}

#[test]
fn parse_open_code_span_returns_active_fence() {
    assert_eq!(parse_open_code_span("`foo"), Some((1, "foo")));
    assert_eq!(parse_open_code_span("text `4.1.1"), Some((1, "4.1.1")));
    assert_eq!(parse_open_code_span("`done` `open"), Some((1, "open")));
    assert_eq!(parse_open_code_span("`done`"), None);
}

#[rstest]
#[case("Version `1.2", "beta` works.", false)]
#[case("Release `4.1.1", "rc1` candidate.", false)]
#[case("text `open", "`close rest", true)]
fn continuation_begins_with_closing_fence_matches_literal_closers_only(
    #[case] existing: &str,
    #[case] continuation: &str,
    #[case] expected: bool,
) {
    assert_eq!(
        continuation_begins_with_closing_fence(existing, continuation),
        expected
    );
}

#[rstest]
#[case("`a``b`", false)]
#[case("``ab``", false)]
#[case("``a`b`", true)]
fn has_unclosed_code_span_rejects_mid_run_closers(#[case] text: &str, #[case] expected: bool) {
    assert_eq!(has_unclosed_code_span(text), expected);
}

/// Verify `position_after_close` handles alternating escaped/literal fences correctly.
#[rstest]
// Single escaped candidate followed by a balanced literal pair: the escaped
// candidate is the real closer for the outer span.
#[case::escaped_then_balanced_literal(
    r"`a\`` b `c`",
    1,   // search_start: after opening `
    1,   // fence_len
    Some(r"`a\``` b ".len()), // byte offset of the first escaped-candidate closer
)]
// Three alternating escaped candidates before a literal that is itself paired:
// the first escaped candidate is the real closer.
#[case::three_escaped_then_paired_literal(
    r"`a\`` x `b\`` y `c\`` z `d` e `f`",
    1,
    1,
    Some(r"`a\``` x ".len()),
)]
// No escaped candidates: the first literal fence is accepted directly.
#[case::no_escaped_candidates("`abc` def", 1, 1, Some(5))]
// Escaped candidate at the end with no subsequent literal: the escaped
// candidate is the fallback.
#[case::only_escaped_candidate(r"`a\`", 1, 1, Some(r"`a\`".len()))]
// A single-length escaped closer precedes a literal fence that itself re-opens
// a span closed by a *later* escaped backtick. The forward look-ahead now
// treats that escaped run as proof the literal fence re-opens, so the outer
// span is closed by the escaped backtick before `x`, not the literal backtick
// after it.
#[case::escaped_closer_before_reopening_literal(r"`a\` x `b\`", 1, 1, Some(r"`a\`".len()))]
#[case::mid_character_search_start("å`", 1, 1, None)]
#[case::past_end_search_start("`", 2, 1, None)]
fn position_after_close_alternating_cases(
    #[case] text: &str,
    #[case] search_start: usize,
    #[case] fence_len: usize,
    #[case] expected: Option<usize>,
) {
    assert_eq!(
        position_after_close(text, search_start, fence_len),
        expected
    );
}

proptest! {
    /// A valid byte range from `char_indices` collects its character subsequence.
    #[test]
    fn collect_range_matches_character_subsequence(
        chars in prop::collection::vec(any::<char>(), 0..64),
        start_seed in 0usize..64,
        end_seed in 0usize..64,
    ) {
        let text = chars.iter().copied().collect::<String>();
        let boundaries = text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
            .collect::<Vec<_>>();
        let first_slot = start_seed.min(chars.len());
        let second_slot = end_seed.min(chars.len());
        let start_slot = first_slot.min(second_slot);
        let end_slot = first_slot.max(second_slot);
        let expected = chars
            .get(start_slot..end_slot)
            .expect("ordered character range should stay within generated input")
            .iter()
            .copied()
            .collect::<String>();
        let start = boundaries
            .get(start_slot)
            .copied()
            .expect("character start slot should have a byte boundary");
        let end = boundaries
            .get(end_slot)
            .copied()
            .expect("character end slot should have a byte boundary");

        prop_assert_eq!(collect_range(&text, start, end), expected);
    }

    /// A reversed range is rejected, not silently reordered.
    ///
    /// This is deliberately a separate property from
    /// `collect_range_matches_character_subsequence`, which orders its two
    /// slots before use and therefore can never generate `start > end`. Keeping
    /// the ordering there and the reversal here means each property makes one
    /// claim, and the reversed case is actually exercised.
    #[test]
    fn collect_range_returns_empty_for_reversed_ranges(
        chars in prop::collection::vec(any::<char>(), 1..32),
        start_seed in 0usize..32,
        end_seed in 0usize..32,
    ) {
        let text = chars.iter().copied().collect::<String>();
        let boundaries = text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
            .collect::<Vec<_>>();
        let first_slot = start_seed.min(chars.len());
        let second_slot = end_seed.min(chars.len());
        // Order in the opposite direction: `start` is the larger slot.
        let reversed_start_slot = first_slot.max(second_slot);
        let reversed_end_slot = first_slot.min(second_slot);
        let start = boundaries
            .get(reversed_start_slot)
            .copied()
            .expect("character start slot should have a byte boundary");
        let end = boundaries
            .get(reversed_end_slot)
            .copied()
            .expect("character end slot should have a byte boundary");
        prop_assume!(start > end);

        prop_assert_eq!(collect_range(&text, start, end), "");
    }

    /// Invalid offsets inside a multibyte scalar use each helper's fallback.
    ///
    /// `scalar` is generated rather than fixed, so the interior offset is not
    /// always `1`: a three- or four-byte scalar puts it at 2 or 3, which is
    /// what catches a helper that merely happens to reject byte 1.
    #[test]
    fn invalid_multibyte_offset_uses_helper_specific_fallbacks(
        scalar in prop_oneof![
            Just('å'), Just('é'), Just('€'), Just('界'), Just('😀'), any::<char>(),
        ],
        tail in prop::collection::vec(any::<char>(), 0..32),
    ) {
        prop_assume!(scalar.len_utf8() > 1);
        let text = format!("{scalar}{}", tail.into_iter().collect::<String>());
        let inside_scalar = scalar.len_utf8() - 1;

        prop_assert_eq!(scan_while(&text, inside_scalar, char::is_alphabetic), inside_scalar);
        prop_assert_eq!(scan_code_suffix_end(&text, inside_scalar), inside_scalar);
        prop_assert_eq!(collect_range(&text, inside_scalar, text.len()), "");
    }

    /// `position_after_close` must always terminate (no infinite loop) and must
    /// never return an offset beyond `text.len()`.
    ///
    /// `search_start` is deliberately passed **unclamped**. An earlier version
    /// clamped it to `text.len()` before the call, which meant the past-end
    /// offsets this property is meant to cover were never actually reachable.
    #[test]
    fn position_after_close_always_terminates_within_bounds(
        text in prop::collection::vec(
            prop_oneof![2 => Just('`'), 1 => Just('\\'), 7 => any::<char>()],
            0..=200,
        )
            .prop_map(|chars| chars.into_iter().collect::<String>()),
        search_start in 0usize..=(1usize << 20),
        fence_len in 1usize..=4usize,
    ) {
        let result = position_after_close(&text, search_start, fence_len);
        if let Some(end) = result {
            prop_assert!(
                end <= text.len(),
                "returned offset {end} exceeds text length {}",
                text.len()
            );
            // A match can never precede the search start, but a start past the
            // end can never produce a match at all, so this still has to hold.
            prop_assert!(
                end >= search_start.min(text.len()),
                "returned offset {end} precedes search_start {search_start}"
            );
        }
    }

    /// An invalid search start must return `None`, never panic.
    ///
    /// Covers an offset inside a multibyte scalar and offsets past the end,
    /// including `usize::MAX`, all passed without clamping.
    ///
    /// The interior offset is derived from a genuine multibyte scalar placed
    /// before the fence. An ASCII byte has no interior offset — `len_utf8() - 1`
    /// is `0`, which is the valid start of the text — so deriving it from a
    /// backtick would silently assert nothing.
    #[test]
    fn position_after_close_rejects_invalid_search_offsets(
        scalar in prop_oneof![Just('å'), Just('é'), Just('€'), Just('界'), any::<char>()],
        tail in prop::collection::vec(any::<char>(), 0..24),
        overshoot in 1usize..=4096usize,
        fence_len in 1usize..=4usize,
    ) {
        prop_assume!(scalar.len_utf8() > 1);
        let text = format!("{scalar}{}", tail.into_iter().collect::<String>());
        let inside_scalar = scalar.len_utf8() - 1;

        prop_assert_eq!(position_after_close(&text, inside_scalar, fence_len), None);
        prop_assert_eq!(
            position_after_close(&text, text.len() + overshoot, fence_len),
            None
        );
        prop_assert_eq!(position_after_close(&text, usize::MAX, fence_len), None);
    }

    /// A fence-length of zero must always return `None`, for any search start.
    #[test]
    fn position_after_close_zero_fence_len_returns_none(
        text in "[ -~]{0,100}",
        search_start in 0usize..=(1usize << 20),
    ) {
        prop_assert_eq!(position_after_close(&text, search_start, 0), None);
    }

    /// Boundary integrity: when `position_after_close` returns `Some(end)`, the
    /// closing run `text[end - fence_len .. end]` must be exactly `fence_len`
    /// backticks with no backtick immediately before or after it. The returned
    /// offset therefore always sits just past a complete, isolated fence — never
    /// inside a longer backtick run.
    #[test]
    fn position_after_close_returns_isolated_fence_of_exact_length(
        text in prop::collection::vec(
            prop_oneof![2 => Just('`'), 1 => Just('\\'), 7 => any::<char>()],
            0..=200,
        )
            .prop_map(|chars| chars.into_iter().collect::<String>()),
        search_start in 0usize..=(1usize << 20),
        fence_len in 1usize..=4usize,
    ) {
        // Passed unclamped so past-end starts are exercised here too; a start
        // beyond the text simply yields no match and the body is skipped.
        if let Some(end) = position_after_close(&text, search_start, fence_len) {
            let bytes = text.as_bytes();
            prop_assert!(end <= text.len());
            prop_assert!(
                end >= fence_len,
                "returned offset {end} cannot hold a {fence_len}-byte fence"
            );
            let run_start = end - fence_len;
            let run = bytes.get(run_start..end);
            prop_assert!(
                run.is_some_and(|run| run.iter().all(|&byte| byte == b'`')),
                "closing run {:?} is not exactly {fence_len} backticks",
                text.get(run_start..end)
            );
            prop_assert!(
                run_start == 0 || bytes.get(run_start - 1) != Some(&b'`'),
                "closing run is preceded by a backtick (mid-run on the left)"
            );
            prop_assert!(
                end == text.len() || bytes.get(end) != Some(&b'`'),
                "closing run is followed by a backtick (mid-run on the right)"
            );
        }
    }
}
