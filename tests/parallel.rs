//! Tests for parallel CLI processing of multiple files.

use assert_cmd::Command;
use rstest::rstest;

#[macro_use]
#[path = "common/mod.rs"]
mod common;
#[path = "common/fs.rs"]
mod test_fs;
use test_fs::TestDir;

#[path = "support/cli_args.rs"]
mod cli_args;
#[path = "support/fixtures.rs"]
mod fixtures;
use cli_args::run_cli_with_args;
use fixtures::broken_table;

#[rstest]
fn test_cli_parallel_empty_file_list() -> Result<(), Box<dyn std::error::Error>> {
    run_cli_with_args(&[])?.success().stdout("\n");
    Ok(())
}

#[rstest]
fn test_cli_parallel_multiple_files() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TestDir::new().expect("failed to create temporary directory");
    let mut files = Vec::new();
    let mut expected = String::new();
    for i in 0..4 {
        let file_name = format!("file{i}.md");
        let path = dir.path().join(&file_name);
        let table = vec![
            format!("| A{i} | B{i} |    |"),
            format!("| {i} | {i} |  | {i} | {i} |"),
        ];
        dir.directory()
            .write(&file_name, format!("{}\n", table.join("\n")))
            .expect("failed to write temporary file");
        expected.push_str(&mdtablefix::reflow_table(&table).join("\n"));
        expected.push('\n');
        files.push(path);
    }

    let args: Vec<&str> = files.iter().map(|path| path.as_str()).collect();
    run_cli_with_args(&args)?.success().stdout(expected);
    Ok(())
}

#[rstest]
fn test_cli_parallel_missing_file_error() {
    let dir = TestDir::new().expect("failed to create temporary directory");
    let good = dir.path().join("good.md");
    let table = vec![
        "| Q | R |    |".to_string(),
        "| 1 | 2 |  | 3 | 4 |".to_string(),
    ];
    dir.directory()
        .write("good.md", format!("{}\n", table.join("\n")))
        .expect("failed to write file");
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
fn test_cli_parallel_missing_file_in_place(
    broken_table: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = TestDir::new().expect("failed to create temporary directory");
    dir.directory()
        .write("good.md", format!("{}\n", broken_table.join("\n")))
        .expect("failed to write file");
    let good = dir.path().join("good.md");
    let missing = dir.path().join("missing.md");

    let good_str = good.as_str();
    let missing_str = missing.as_str();
    run_cli_with_args(&["--in-place", good_str, missing_str])?
        .failure()
        .stderr(predicates::str::contains("missing.md"));
    Ok(())
}
