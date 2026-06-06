//! Property tests for date-like sequence atomicity during wrapping.

use mdtablefix::wrap::wrap_text;
use proptest::prelude::*;

const MONTH_NAMES: [&str; 23] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
];

fn month_name_strategy() -> BoxedStrategy<String> {
    prop::sample::select(&MONTH_NAMES)
        .prop_map(str::to_string)
        .boxed()
}

fn ordinal_day_strategy() -> BoxedStrategy<String> {
    (
        1u8..=31,
        prop_oneof![Just("st"), Just("nd"), Just("rd"), Just("th")],
    )
        .prop_map(|(day, suffix)| format!("{day}{suffix}"))
        .boxed()
}

fn numeric_day_strategy() -> BoxedStrategy<String> {
    (1u8..=31, any::<bool>())
        .prop_map(|(day, append_comma)| {
            if append_comma {
                format!("{day},")
            } else {
                day.to_string()
            }
        })
        .boxed()
}

fn year_strategy() -> BoxedStrategy<String> {
    (1000u16..=2999).prop_map(|year| year.to_string()).boxed()
}

fn date_sequence_tokens_strategy() -> BoxedStrategy<Vec<String>> {
    prop_oneof![
        (
            ordinal_day_strategy(),
            month_name_strategy(),
            year_strategy()
        )
            .prop_map(|(day, month, year)| vec![day, " ".into(), month, " ".into(), year]),
        (
            numeric_day_strategy(),
            month_name_strategy(),
            year_strategy()
        )
            .prop_map(|(day, month, year)| vec![day, " ".into(), month, " ".into(), year]),
        (
            month_name_strategy(),
            numeric_day_strategy(),
            year_strategy()
        )
            .prop_map(|(month, day, year)| vec![month, " ".into(), day, " ".into(), year]),
    ]
    .boxed()
}

fn prose_prefix_strategy() -> BoxedStrategy<String> {
    prop::collection::vec(
        prop::string::string_regex("[a-z]{3,8}")
            .expect("failed to build prose word regex strategy"),
        1..=6,
    )
    .prop_map(|words| words.join(" "))
    .boxed()
}

fn has_adjacent_line_date_split(output: &[String], date: &str) -> bool {
    output.windows(2).any(|lines| {
        (1..date.len()).any(|split| {
            let prefix_part = &date[..split];
            let suffix_part = &date[split..];
            lines[0].ends_with(prefix_part) && lines[1].trim_start().starts_with(suffix_part)
        })
    })
}

proptest! {
    #[test]
    fn prop_wrap_text_never_splits_fitting_date(
        (tokens, prefix) in (date_sequence_tokens_strategy(), prose_prefix_strategy()),
    ) {
        let date = tokens.join("");
        let input = format!("{prefix} {date} afterwards.");
        let output = wrap_text(&[input], date.chars().count() + 2);
        prop_assert!(output.iter().any(|line| line.contains(&date)));
    }

    #[test]
    fn prop_wrap_text_date_not_split_across_adjacent_lines(
        (tokens, prefix) in (date_sequence_tokens_strategy(), prose_prefix_strategy()),
    ) {
        let date = tokens.join("");
        let input = format!("{prefix} {date} afterwards.");
        let output = wrap_text(&[input], date.chars().count() + 2);
        prop_assert!(!has_adjacent_line_date_split(&output, &date));
    }

    #[test]
    fn prop_wrap_text_content_preserved(
        (tokens, prefix) in (date_sequence_tokens_strategy(), prose_prefix_strategy()),
    ) {
        let date = tokens.join("");
        let input = format!("{prefix} {date}.");
        let width = date.chars().count() + prefix.chars().count() + 4;
        let output = wrap_text(std::slice::from_ref(&input), width);
        prop_assert_eq!(output.join(" "), input);
    }

    #[test]
    fn prop_wrap_text_over_width_date_produces_nonempty_output(
        tokens in date_sequence_tokens_strategy(),
    ) {
        let date = tokens.join("");
        let output = wrap_text(std::slice::from_ref(&date), 1);
        prop_assert!(!output.is_empty());
        prop_assert!(output.iter().any(|line| line.contains(&date)));
    }

    #[test]
    fn prop_wrap_text_more_width_never_increases_line_count(
        (tokens, prefix, extra_width) in (
            date_sequence_tokens_strategy(),
            prose_prefix_strategy(),
            0usize..=20usize,
        ),
    ) {
        let date = tokens.join("");
        let input = format!("{prefix} {date} afterwards.");
        let width = date.chars().count() + extra_width;
        let narrower = wrap_text(std::slice::from_ref(&input), width).len();
        let wider = wrap_text(std::slice::from_ref(&input), width + 1).len();
        prop_assert!(wider <= narrower);
    }
}
