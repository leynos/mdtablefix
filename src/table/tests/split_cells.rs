//! Property and regression tests for Markdown table-cell splitting.

use proptest::prelude::*;

use super::super::*;

#[test]
fn preserves_control_characters_escaped_pipes_and_backslashes() {
    assert_eq!(
        split_cells("| \u{1f} | middle \\| pipe | trailing\\ |"),
        vec![
            "\u{1f}".to_string(),
            "middle | pipe".to_string(),
            "trailing\\".to_string(),
        ]
    );
}

#[test]
fn preserves_an_escaped_terminal_pipe_before_a_closing_delimiter() {
    assert_eq!(split_cells("| value\\||"), vec!["value|".to_string()]);
}

proptest! {
    #[test]
    fn round_trips_arbitrary_unicode_payloads_ending_in_a_pipe(
        characters in prop::collection::vec(
            any::<char>().prop_filter(
                "table cells must remain on one source line",
                |character| !matches!(character, '\r' | '\n'),
            ),
            0..=24,
        ),
    ) {
        let content = characters.into_iter().collect::<String>();
        let payload = format!("x{content}|");
        let escaped = payload.replace('|', "\\|");

        prop_assert_eq!(split_cells(&format!("| {escaped} |")), vec![payload]);
    }
}
