//! Regression tests for code-emphasis processing around table reflow.

use rstest::{fixture, rstest};

use super::{Options, process_stream_inner};

/// Enables the code-emphasis transform without unrelated processing options.
#[fixture]
fn code_emphasis_options() -> Options {
    Options {
        code_emphasis: true,
        ..Default::default()
    }
}

#[rstest]
fn process_stream_inner_applies_table_code_emphasis_before_reflow(code_emphasis_options: Options) {
    let input = vec![
        "| Name  | Notes                      |".to_string(),
        "| ----- | -------------------------- |".to_string(),
        "| alpha | Use *`cargo test`* to run. |".to_string(),
    ];

    let with_code_emphasis = process_stream_inner(&input, code_emphasis_options);
    let without_code_emphasis = process_stream_inner(&input, Options::default());

    assert_eq!(
        with_code_emphasis,
        vec![
            "| Name  | Notes                    |",
            "| ----- | ------------------------ |",
            "| alpha | Use `cargo test` to run. |",
        ],
    );
    assert_eq!(without_code_emphasis, input);
}

#[rstest]
fn table_cells_receive_code_emphasis_repair_once(code_emphasis_options: Options) {
    let input = vec![
        "| H | X |".to_string(),
        "| - | - |".to_string(),
        "| a | *`*`*a* |".to_string(),
    ];

    let output = process_stream_inner(&input, code_emphasis_options);
    let expected = crate::table::reflow_table(&crate::code_emphasis::fix_code_emphasis(&input));

    assert_eq!(output, expected);
}

#[rstest]
fn pipe_prefixed_non_table_uses_the_existing_code_emphasis_path(code_emphasis_options: Options) {
    let input = vec!["| Use *`cargo test`* to run.".to_string()];

    let output = process_stream_inner(&input, code_emphasis_options);

    assert_eq!(output, crate::code_emphasis::fix_code_emphasis(&input));
}
