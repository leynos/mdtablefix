//! List item wrapping tests.

use super::*;

#[test]
fn test_wrap_list_item() {
    let input = lines_vec![
        r"- This bullet item is exceptionally long and must be wrapped to keep prefix formatting intact.",
    ];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 2);
}

#[rstest]
#[case("- ", 3)]
#[case("1. ", 3)]
#[case("10. ", 3)]
#[case("100. ", 3)]
fn test_wrap_list_items_with_inline_code(#[case] prefix: &str, #[case] expected: usize) {
    let input = lines_vec![format!(
        "{prefix}`script`: A multi-line script declared with the YAML `|` block style. The entire \
         block is passed to an interpreter. If the first line begins with `#!`, Netsuke executes \
         the script verbatim, respecting the shebang."
    )];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, prefix, expected);
}

#[test]
fn test_wrap_preserves_inline_code_spans() {
    let input = lines_vec![
        "- `script`: A multi-line script declared with the YAML `|` block style. The entire block \
         is passed to an interpreter. If the first line begins with `#!`, Netsuke executes the \
         script verbatim, respecting the shebang.",
    ];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 3);
}

#[test]
fn test_wrap_multi_backtick_code() {
    let input = lines_vec![
        "- ``cmd`` executes ```echo``` output with ``json`` format and prints results to the \
         console",
    ];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 2);
}

#[test]
fn test_wrap_multiple_inline_code_spans() {
    let input = lines_vec![
        "- Use `foo` and `bar` inside ``baz`` for testing with additional commentary to exceed \
         wrapping width",
    ];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 2);
}

#[test]
fn test_wrap_long_inline_code_item() {
    let input = lines_vec![concat!(
        "- `async def on_unhandled(self, ws: WebSocketLike, message: Union[str, bytes])`:",
        " A fallback handler for messages that are not dispatched by the more specific",
        " message handlers. This can be used for raw text/binary data or messages that",
        " don't conform to the expected structured format."
    )];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 4);
    assert!(
        output
            .first()
            .expect("wrapped output should contain at least one line")
            .ends_with("`:")
    );
}

#[test]
fn test_wrap_future_attribute_punctuation() {
    let input = lines_vec![concat!(
        "- Test function (`#[awt]`) or a specific `#[future]` argument ",
        "(`#[future(awt)]`), tells `rstest` to automatically insert `.await` ",
        "calls for those futures."
    )];
    let output = process_stream(&input);
    assert_eq!(
        output,
        vec![
            "- Test function (`#[awt]`) or a specific `#[future]` argument".to_string(),
            "  (`#[future(awt)]`), tells `rstest` to automatically insert `.await` calls for".to_string(),
            "  those futures.".to_string(),
        ]
    );
}

#[test]
fn test_wrap_short_list_item() {
    let input = lines_vec!["- short item"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

#[test]
fn test_wrap_list_item_period_after_code() {
    let input: Vec<String> = include_lines!("data/bullet_full_stop_input.txt");
    let expected: Vec<String> = include_lines!("data/bullet_full_stop_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_wrap_list_item_question_mark_after_code() {
    let input: Vec<String> = include_lines!("data/bullet_question_mark_input.txt");
    let expected: Vec<String> = include_lines!("data/bullet_question_mark_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_wrap_list_item_exclamation_mark_after_code() {
    let input: Vec<String> = include_lines!("data/bullet_exclamation_mark_input.txt");
    let expected: Vec<String> = include_lines!("data/bullet_exclamation_mark_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_wrap_list_item_comma_after_code() {
    let input: Vec<String> = include_lines!("data/bullet_comma_input.txt");
    let expected: Vec<String> = include_lines!("data/bullet_comma_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_wrap_list_item_colon_after_code() {
    let input: Vec<String> = include_lines!("data/bullet_colon_input.txt");
    let expected: Vec<String> = include_lines!("data/bullet_colon_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_wrap_list_item_semicolon_after_code() {
    let input: Vec<String> = include_lines!("data/bullet_semicolon_input.txt");
    let expected: Vec<String> = include_lines!("data/bullet_semicolon_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_wrap_list_items_with_checkboxes() {
    let input = lines_vec![
        "- [ ] Create a `HttpTravelTimeProvider` struct that implements the `TravelTimeProvider` trait.",
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
        "  - [ ] Create a `HttpTravelTimeProvider` struct that implements the `TravelTimeProvider` trait.",
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

#[test]
fn test_wrap_checked_list_items_with_checkboxes() {
    let input = lines_vec![
        "- [x] Create a `HttpTravelTimeProvider` struct that implements the `TravelTimeProvider` trait.",
        concat!(
            "- [X] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix` ",
            "method to make concurrent requests to an external OSRM API's `table` ",
            "service."
        ),
    ];
    let expected = lines_vec![
        "- [x] Create a `HttpTravelTimeProvider` struct that implements the",
        "      `TravelTimeProvider` trait.",
        "- [X] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix`",
        "      method to make concurrent requests to an external OSRM API's `table`",
        "      service.",
    ];
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_wrap_checkbox_items_with_inner_spaces() {
    let input = lines_vec![
        "- [ x ] Create a `HttpTravelTimeProvider` struct that implements the `TravelTimeProvider` trait.",
        concat!(
            "- [  ] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix` ",
            "method to make concurrent requests to an external OSRM API's `table` ",
            "service."
        ),
    ];
    let expected = lines_vec![
        "- [ x ] Create a `HttpTravelTimeProvider` struct that implements the",
        "        `TravelTimeProvider` trait.",
        "- [  ] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix`",
        "       method to make concurrent requests to an external OSRM API's `table`",
        "       service.",
    ];
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_wrap_checkbox_items_without_post_marker_space() {
    let input = lines_vec![
        "- [ ]Create a `HttpTravelTimeProvider` struct that implements the `TravelTimeProvider` trait.",
        concat!(
            "- [x]Using `tokio` and `reqwest`, implement the `get_travel_time_matrix` ",
            "method to make concurrent requests to an external OSRM API's `table` ",
            "service."
        ),
    ];
    let expected = lines_vec![
        "- [ ]Create a `HttpTravelTimeProvider` struct that implements the",
        "     `TravelTimeProvider` trait.",
        "- [x]Using `tokio` and `reqwest`, implement the `get_travel_time_matrix`",
        "     method to make concurrent requests to an external OSRM API's `table`",
        "     service.",
    ];
    let output = process_stream(&input);
    assert_eq!(output, expected);
}
