//! Regression tests for Markdown tables that use continuation rows.

use mdtablefix::{Options, process_stream, process_stream_opts, reflow_table};
use rstest::rstest;
use unicode_width::UnicodeWidthStr;

#[macro_use]
mod common;

fn assert_uniform_display_widths(output: &[String]) {
    assert!(!output.is_empty());

    let widths: Vec<usize> = output[0]
        .trim()
        .trim_matches('|')
        .split('|')
        .map(UnicodeWidthStr::width)
        .collect();

    for row in output {
        let cells: Vec<&str> = row.trim().trim_matches('|').split('|').collect();
        assert_eq!(cells.len(), widths.len());
        for (idx, cell) in cells.iter().enumerate() {
            assert_eq!(UnicodeWidthStr::width(*cell), widths[idx]);
        }
    }
}

#[test]
fn reflow_table_preserves_leading_empty_continuation_cells() {
    let input = lines_vec![
        "| A | B | C |",
        "| - | - | - |",
        "| x | y | one, |",
        "|   |   | two |",
    ];
    let expected = lines_vec![
        "| A   | B   | C    |",
        "| --- | --- | ---- |",
        "| x   | y   | one, |",
        "|     |     | two  |",
    ];
    assert_eq!(reflow_table(&input), expected);
}

#[test]
fn process_stream_preserves_continuation_row_columns() {
    let input = lines_vec![
        "| Module     | Stability | Types and functions                     |",
        "| ---------- | --------- | --------------------------------------- |",
        "| `api`      | Stable    | `CommandOutcome`, `ExecParams`, `exec`, |",
        "|            |           | `run_agent`, `list_containers`,         |",
        "|            |           | `stop_container`, `run_token_daemon`    |",
        "| `github`   | Internal  | Subject to change; not part of the      |",
        "|            |           | stable integration contract             |",
    ];
    let expected = lines_vec![
        "| Module   | Stability | Types and functions                     |",
        "| -------- | --------- | --------------------------------------- |",
        "| `api`    | Stable    | `CommandOutcome`, `ExecParams`, `exec`, |",
        "|          |           | `run_agent`, `list_containers`,         |",
        "|          |           | `stop_container`, `run_token_daemon`    |",
        "| `github` | Internal  | Subject to change; not part of the      |",
        "|          |           | stable integration contract             |",
    ];
    assert_eq!(process_stream(&input), expected);
}

#[test]
fn process_stream_preserves_literal_ellipsis_in_table_cells_when_disabled() {
    let input = lines_vec![
        "| Module | Notes                       |",
        "| ------ | --------------------------- |",
        "| `foo`  | preserves literal ... here  |",
        "| `bar`  | and also ... in this cell   |",
    ];
    let expected = lines_vec![
        "| Module | Notes                      |",
        "| ------ | -------------------------- |",
        "| `foo`  | preserves literal ... here |",
        "| `bar`  | and also ... in this cell  |",
    ];

    assert_eq!(process_stream(&input), expected);
}

#[test]
fn process_stream_opts_reflows_tables_after_ellipsis_in_table_cells() {
    let input = lines_vec![
        "| Module     | Stability | Types and functions                     |",
        "| ---------- | --------- | --------------------------------------- |",
        "| `config`   | Stable    | `AppConfig`, `ConfigLoadOptions`, ...   |",
        "| `engine`   | Stable    | `EngineConnector`, `ExecRequest`, ...   |",
        "| `error`    | Stable    | `PodbotError`, `ConfigError`, ...       |",
    ];
    let expected = lines_vec![
        "| Module   | Stability | Types and functions                 |",
        "| -------- | --------- | ----------------------------------- |",
        "| `config` | Stable    | `AppConfig`, `ConfigLoadOptions`, … |",
        "| `engine` | Stable    | `EngineConnector`, `ExecRequest`, … |",
        "| `error`  | Stable    | `PodbotError`, `ConfigError`, …     |",
    ];
    let output = process_stream_opts(
        &input,
        Options {
            ellipsis: true,
            ..Default::default()
        },
    );
    assert_eq!(output, expected);
}

#[test]
fn reflow_table_preserves_escaped_pipes_in_continuation_rows() {
    let input = lines_vec![
        "| Key | Notes |",
        "| --- | ----- |",
        "| `api` | first item |",
        "|     | keep \\| literal in continuation |",
    ];
    let expected = lines_vec![
        "| Key   | Notes                          |",
        "| ----- | ------------------------------ |",
        "| `api` | first item                     |",
        "|       | keep | literal in continuation |",
    ];

    assert_eq!(reflow_table(&input), expected);
}

#[rstest]
#[case(
    lines_vec![
        "| Name | Notes |",
        "| ---- | ----- |",
        "| café | naïve |",
        "| long | déjà vu |",
    ]
)]
#[case(
    lines_vec![
        "| Name | Notes |",
        "| ---- | ----- |",
        "| 漢字 | wide 🙂 |",
        "| kana | かな |",
    ]
)]
#[case(
    lines_vec![
        "| Name | Notes |",
        "| ---- | ----- |",
        "| 🙂🙂 | emoji |",
        "| text | mixé |",
    ]
)]
fn reflow_table_uses_unicode_display_widths(#[case] input: Vec<String>) {
    let output = reflow_table(&input);

    assert_uniform_display_widths(&output);
}
