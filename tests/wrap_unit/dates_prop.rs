//! Property tests for date-like sequence atomicity during wrapping.

use mdtablefix::{
    date_strategies::{date_sequence_tokens_strategy, month_name_strategy, year_strategy},
    wrap::wrap_text,
};
use proptest::prelude::*;

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

    #[test]
    fn prop_partial_month_year_not_grouped(
        (month, year) in (month_name_strategy(), year_strategy()),
    ) {
        let input = format!("Plan around {month} {year} carefully.");
        let target = format!("{month} {year}");
        let Some(year_start) = input.find(&year) else {
            prop_assert!(false, "generated year must appear in the input");
            unreachable!("prop_assert! returns before this point");
        };
        let width = input[..year_start].chars().count();
        let output = wrap_text(std::slice::from_ref(&input), width);
        prop_assert!(output.iter().all(|line| !line.contains(&target)));
        prop_assert_eq!(output.join(" "), input);
    }
}
