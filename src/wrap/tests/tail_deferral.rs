//! Tail-deferral tests for prefixed blocks that wrap onto a continuation line.
//!
//! A prefixed source line whose rest spills past the first wrapped line is
//! buffered so that the lines below it reflow with it. Emitting a bare tail
//! instead leaves the next pass to fold those lines into the tail, so
//! `format(format(x))` would differ from `format(x)` and the formatter would
//! never reach a fixed point.
//!
//! Folding only happens when the tail re-parses as paragraph text. A nested
//! tail indented by four columns is an indented code block, a blockquote tail
//! repeats its marker, and a footnote tail is indented past the code threshold,
//! so those blocks keep the source lines below them separate.

use rstest::rstest;

use crate::wrap::wrap_text;

fn lines(text: &str) -> Vec<String> { text.lines().map(str::to_string).collect() }

/// Asserts that wrapping `input` again reproduces the first pass.
fn assert_fixed_point(input: &[String]) {
    let once = wrap_text(input, 80);
    let twice = wrap_text(&once, 80);
    assert_eq!(once, twice, "wrap_text is not a fixed point for {input:?}");
}

#[test]
fn wrap_text_reflows_a_wrapped_item_with_its_continuation() {
    let input = lines(concat!(
        "- **Ownership.** Owned by the wrap module ",
        "(`src/wrap/tracing_snapshot_support.rs`)\n",
        "  and gated behind `#[cfg(test)]`; it is `pub(crate)` test-support ",
        "code, not part"
    ));
    let expected = lines(concat!(
        "- **Ownership.** Owned by the wrap module\n",
        "  (`src/wrap/tracing_snapshot_support.rs`) and gated behind ",
        "`#[cfg(test)]`; it\n",
        "  is `pub(crate)` test-support code, not part"
    ));

    let wrapped = wrap_text(&input, 80);

    assert_eq!(wrapped, expected);
    assert_fixed_point(&input);
}

#[test]
fn wrap_text_folds_a_lazy_continuation_into_a_wrapped_item() {
    let input = lines(concat!(
        "- item text long enough to push this line far past the eighty column ",
        "limit for sure\n",
        "lazy line"
    ));
    let expected = lines(concat!(
        "- item text long enough to push this line far past the eighty column ",
        "limit for\n",
        "  sure lazy line"
    ));

    let wrapped = wrap_text(&input, 80);

    assert_eq!(wrapped, expected);
    assert_fixed_point(&input);
}

#[test]
fn wrap_text_folds_a_lazy_continuation_into_a_wrapped_ordered_item() {
    let input = lines(concat!(
        "1. item text long enough to push this line far past the eighty column ",
        "limit here indeed\n",
        "lazy line"
    ));
    let expected = lines(concat!(
        "1. item text long enough to push this line far past the eighty column ",
        "limit\n",
        "   here indeed lazy line"
    ));

    let wrapped = wrap_text(&input, 80);

    assert_eq!(wrapped, expected);
    assert_fixed_point(&input);
}

#[rstest]
#[case::item_that_fits("- short item\nseparate paragraph", "- short item\nseparate paragraph")]
#[case::nested_item_with_a_four_column_tail(
    "  - item text long enough to push this line far past the eighty column limit here \
     indeed\nlazy line",
    "  - item text long enough to push this line far past the eighty column limit\n\x20   here \
     indeed\nlazy line"
)]
#[case::footnote_definition_with_a_six_column_tail(
    "[^1]: item text long enough to push this line far past the eighty column limit yes\nlazy line",
    "[^1]: item text long enough to push this line far past the eighty column limit\n\x20     \
     yes\nlazy line"
)]
#[case::blockquote_repeating_its_marker(
    "> a quote line long enough to push this text far past the eighty column limit yes\n> next \
     quote line",
    "> a quote line long enough to push this text far past the eighty column limit\n> yes\n> next \
     quote line"
)]
#[case::blockquote_span_closed_on_a_continuation(
    "> a quote with `an open span\n> which closes here`\nlazy quote continuation",
    "> a quote with `an open span which closes here`\nlazy quote continuation"
)]
fn wrap_text_keeps_lines_separate_when_the_tail_does_not_reflow(
    #[case] text: &str,
    #[case] expected: &str,
) {
    let input = lines(text);
    let expected = lines(expected);

    assert_eq!(wrap_text(&input, 80), expected);
    assert_fixed_point(&input);
}
