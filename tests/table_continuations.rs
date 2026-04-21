//! Regression tests for Markdown tables that use continuation rows.

use mdtablefix::{Options, process_stream, process_stream_opts, reflow_table};

#[macro_use]
mod common;

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
