//! Tests for parallel CLI processing of multiple files.

use std::{fs::File, io::Write};

use tempfile::tempdir;

#[macro_use]
mod prelude;
use prelude::*;

#[rstest]
fn test_cli_parallel_empty_file_list() { run_cli_with_args(&[]).success().stdout("\n"); }

#[rstest]
fn test_cli_parallel_multiple_files() {
    let dir = tempdir().expect("failed to create temporary directory");
    let mut files = Vec::new();
    let mut expected = String::new();
    for i in 0..4 {
        let path = dir.path().join(format!("file{i}.md"));
        let table = vec![
            format!("| A{i} | B{i} |    |"),
            format!("| {i} | {i} |  | {i} | {i} |"),
        ];
        let mut f = File::create(&path).expect("failed to create temporary file");
        for line in &table {
            writeln!(f, "{line}").expect("failed to write line");
        }
        f.flush().expect("failed to flush file");
        drop(f);
        expected.push_str(&mdtablefix::reflow_table(&table).join("\n"));
        expected.push('\n');
        files.push(path);
    }

    let args: Vec<&str> = files.iter().map(|p| p.to_str().unwrap()).collect();
    run_cli_with_args(&args).success().stdout(expected);
}

#[rstest]
fn test_cli_parallel_missing_file_error() {
    let dir = tempdir().expect("failed to create temporary directory");
    let good = dir.path().join("good.md");
    let table = vec![
        "| Q | R |    |".to_string(),
        "| 1 | 2 |  | 3 | 4 |".to_string(),
    ];
    let mut f = File::create(&good).expect("failed to create file");
    for line in &table {
        writeln!(f, "{line}").expect("failed to write line");
    }
    f.flush().expect("failed to flush file");
    drop(f);
    let expected = mdtablefix::reflow_table(&table).join("\n") + "\n";
    let missing = dir.path().join("missing.md");

    Command::cargo_bin("mdtablefix")
        .expect("failed to create command")
        .arg(&good)
        .arg(&missing)
        .assert()
        .failure()
        .stdout(expected)
        .stderr(predicates::str::contains("missing.md"));
}

#[rstest]
fn test_cli_parallel_missing_file_in_place(broken_table: Vec<String>) {
    let dir = tempdir().expect("failed to create temporary directory");
    let good = dir.path().join("good.md");
    let mut f = File::create(&good).expect("failed to create file");
    for line in &broken_table {
        writeln!(f, "{line}").expect("failed to write line");
    }
    f.flush().expect("failed to flush file");
    drop(f);
    let missing = dir.path().join("missing.md");

    let good_str = good.to_str().unwrap();
    let missing_str = missing.to_str().unwrap();
    run_cli_with_args(&["--in-place", good_str, missing_str])
        .failure()
        .stderr(predicates::str::contains("missing.md"));
}
