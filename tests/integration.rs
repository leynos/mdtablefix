use assert_cmd::Command;
use mdtablefix::{process_stream, reflow_table};
use rstest::{fixture, rstest};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[fixture]
fn broken_table() -> Vec<String> {
    vec![
        "| A | B |    |".to_string(),
        "| 1 | 2 |  | 3 | 4 |".to_string(),
    ]
}

#[fixture]
fn malformed_table() -> Vec<String> {
    vec!["| A | |".to_string(), "| 1 | 2 | 3 |".to_string()]
}

#[rstest]
fn test_reflow_basic(broken_table: Vec<String>) {
    let expected = vec!["| A | B |", "| 1 | 2 |", "| 3 | 4 |"];
    assert_eq!(reflow_table(&broken_table), expected);
}

#[rstest]
fn test_reflow_malformed_returns_original(malformed_table: Vec<String>) {
    assert_eq!(reflow_table(&malformed_table), malformed_table);
}

#[rstest]
fn test_process_stream_ignores_code_fences() {
    let lines = vec![
        "```".to_string(),
        "| not | a | table |".to_string(),
        "```".to_string(),
    ];
    assert_eq!(process_stream(&lines), lines);

    // Test with tilde-based code fences
    let tilde_lines = vec![
        "~~~".to_string(),
        "| not | a | table |".to_string(),
        "~~~".to_string(),
    ];
    assert_eq!(process_stream(&tilde_lines), tilde_lines);
}

#[rstest]
fn test_cli_in_place_requires_file() {
    Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--in-place")
        .assert()
        .failure();
}

#[rstest]
fn test_cli_process_file(broken_table: Vec<String>) {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sample.md");
    let mut f = File::create(&file_path).unwrap();
    for line in &broken_table {
        writeln!(f, "{}", line).unwrap();
    }
    Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg(&file_path)
        .assert()
        .success()
        .stdout("| A | B |\n| 1 | 2 |\n| 3 | 4 |\n");
}
