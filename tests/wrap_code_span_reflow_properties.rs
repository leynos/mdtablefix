//! Property tests for cross-line inline-code wrapping stability.

use mdtablefix::wrap::wrap_text;
use proptest::prelude::*;
use unicode_width::UnicodeWidthStr;

proptest! {
    #[test]
    fn prefixed_cross_line_code_span_with_tail_reaches_a_fixed_point(
        before in "[a-z]{4,16}( [a-z]{4,16}){1,3}",
        code_first in "[a-z]{8,24}",
        code_second in "[a-z]{8,24}",
        tail in "[a-z]{4,12}( [a-z]{4,12}){4,10}",
        width in 48usize..=80,
    ) {
        let input = vec![
            format!("- {before} `{code_first}"),
            format!("  {code_second}` {tail}"),
            "  concluding words extend the same list item.".to_string(),
        ];

        let once = wrap_text(&input, width);
        prop_assert_eq!(wrap_text(&once, width), once);
    }

    #[test]
    fn conforming_cross_line_code_span_never_becomes_overlong(
        first in "[a-z]{20,36}",
        second in "[a-z]{20,36}",
        third in "[a-z]{20,36}",
        width in 48usize..=64,
    ) {
        let joined_span = format!("`{first} {second} {third}`");
        let input = vec![
            "Introductory prose:".to_string(),
            format!("`{first}"),
            second,
            format!("{third}`"),
        ];
        prop_assume!(input.iter().all(|line| UnicodeWidthStr::width(line.as_str()) <= width));
        prop_assume!(UnicodeWidthStr::width(joined_span.as_str()) > width);

        let output = wrap_text(&input, width);

        prop_assert_eq!(&output, &input);
        prop_assert!(output.iter().all(|line| UnicodeWidthStr::width(line.as_str()) <= width));
    }
}
