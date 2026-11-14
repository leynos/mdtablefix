//! Property coverage for the capture contract shared by wrapping fences.

use proptest::prelude::*;

use super::is_fence;

proptest! {
    #[test]
    fn fence_captures_round_trip_generated_delimiters(
        indent in "[ \\t]{0,4}",
        blockquote_depth in 0_usize..=4,
        marker in prop_oneof![Just('`'), Just('~')],
        marker_length in 3_usize..=12,
        info in "[^\\r\\n]{0,40}",
    ) {
        let blockquote = "> ".repeat(blockquote_depth);
        let prefix = format!("{indent}{blockquote}");
        let delimiter = marker.to_string().repeat(marker_length);
        let line = format!("{prefix}{delimiter}{info}");
        let absorbed_marker_count = info.chars().take_while(|character| *character == marker).count();
        let expected_delimiter = marker.to_string().repeat(marker_length + absorbed_marker_count);
        let expected_info = &info[absorbed_marker_count..];

        let captures = is_fence(&line);

        prop_assert_eq!(
            captures,
            Some((prefix.as_str(), expected_delimiter.as_str(), expected_info)),
        );
        let (captured_prefix, captured_delimiter, captured_info) =
            captures.expect("generated fence should match");
        prop_assert_eq!(
            format!("{captured_prefix}{captured_delimiter}{captured_info}"),
            line,
        );
    }
}
