//! Tests for task-checkbox list prefixes and their indentation.

use rstest::rstest;

use super::super::process_stream;

#[test]
fn test_wrap_list_items_with_checkboxes() {
    let input = lines_vec![
        "- [ ] Create a `HttpTravelTimeProvider` struct that implements the `TravelTimeProvider` \
         trait.",
        concat!(
            "- [ ] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix` ",
            "method to make concurrent requests to an external OSRM API's `table` ",
            "service."
        ),
    ];
    let expected = lines_vec![
        "- [ ] Create a `HttpTravelTimeProvider` struct that implements the",
        "      `TravelTimeProvider` trait.",
        "- [ ] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix`",
        "      method to make concurrent requests to an external OSRM API's `table`",
        "      service.",
    ];
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_wrap_indented_list_items_with_checkboxes() {
    let input = lines_vec![
        "  - [ ] Create a `HttpTravelTimeProvider` struct that implements the \
         `TravelTimeProvider` trait.",
        concat!(
            "  - [ ] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix` ",
            "method to make concurrent requests to an external OSRM API's `table` ",
            "service."
        ),
    ];
    let expected = lines_vec![
        "  - [ ] Create a `HttpTravelTimeProvider` struct that implements the",
        "        `TravelTimeProvider` trait.",
        "  - [ ] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix`",
        "        method to make concurrent requests to an external OSRM API's `table`",
        "        service.",
    ];
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[rstest]
#[case("- [ ] ")]
#[case("  - [ ] ")]
#[case("- [x] ")]
#[case("- [X] ")]
#[case("- [x]")]
#[case("- [X]")]
#[case("- [x ] ")]
#[case("- [ X] ")] // asymmetric inner space before X
#[case("- [X ] ")] // asymmetric inner space after X
#[case("- [ x ] ")]
#[case("- [  ] ")]
#[case("- [ ]  ")]
#[case("- [x]  ")]
#[case("- [ ]")]
#[case("1. [ ] ")]
#[case("1. [X] ")]
#[case("12) [x] ")]
#[case("12) [X] ")]
#[case("* [ ] ")]
#[case("+ [ ] ")]
fn test_wrap_checkbox_prefixes(#[case] prefix: &str) {
    let body =
        "Create a `HttpTravelTimeProvider` struct that implements the `TravelTimeProvider` trait.";
    let input = lines_vec![format!("{prefix}{body}")];
    let output = process_stream(&input);
    assert!(
        output.len() >= 2,
        "expected wrapping to occur for: {prefix}{body}"
    );
    assert!(
        output[0].starts_with(prefix),
        "prefix mutated in first line for: {prefix}{body}"
    );
    let indent = prefix.chars().count();
    for (i, line) in output.iter().enumerate().skip(1) {
        assert!(
            line.starts_with(&" ".repeat(indent)),
            "indent mismatch on line {i} for: {prefix}{body}"
        );
    }
}

#[test]
fn test_wrap_tab_indented_checkbox_list_items() {
    let input = lines_vec![
        "\t- [ ] Create a `HttpTravelTimeProvider` struct that implements the \
         `TravelTimeProvider` trait.",
        concat!(
            "\t- [ ] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix` ",
            "method to make concurrent requests to an external OSRM API's `table` ",
            "service."
        ),
    ];
    let output = process_stream(&input);
    // A leading tab counts as four columns of indentation, so these lines are
    // treated as indented code blocks and remain verbatim during wrapping.
    assert_eq!(output, input);
}

#[test]
fn test_wrap_tab_indented_checkbox_without_trailing_space() {
    let input = lines_vec![
        "\t- [x]Create a `HttpTravelTimeProvider` struct that implements the `TravelTimeProvider` \
         trait.",
    ];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

#[rstest]
#[case("- [y] ", 2)]
#[case("- [Y] ", 2)]
#[case("- [✓] ", 2)]
#[case("- [] ", 2)]
fn test_wrap_non_task_markers_do_not_expand_prefix(#[case] prefix: &str, #[case] indent: usize) {
    let body =
        "Create a `HttpTravelTimeProvider` struct that implements the `TravelTimeProvider` trait.";
    let input = lines_vec![format!("{prefix}{body}")];
    let output = process_stream(&input);
    assert!(
        output.len() >= 2,
        "expected wrapping to occur for: {prefix}{body}"
    );
    assert!(
        output[0].starts_with(prefix),
        "prefix should be retained for: {prefix}{body}"
    );
    for (i, line) in output.iter().enumerate().skip(1) {
        assert!(
            line.starts_with(&" ".repeat(indent)),
            "indent must match bullet only on line {i} for: {prefix}{body}"
        );
    }
}
