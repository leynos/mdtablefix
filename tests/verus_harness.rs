//! Guards the Verus workflow and verification-ledger lint contract.
//!
//! `make verus-selftest` is the executable non-vacuity check in continuous
//! integration. It runs `verus/smoke.rs` and requires the deliberately false
//! assertion to fail, so this Rust test does not need a local Verus install.

#![cfg(unix)]

use std::process::{Command, Output};

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use serde_yaml::{Mapping, Value};
use tempfile::TempDir;

const VERUS_WORKFLOW: &str = include_str!("../.github/workflows/verus.yml");
const MISSING_SYMBOL_DIAGNOSTIC: &str =
    "verification ledger names missing symbol: missing_verified_symbol";

fn manifest_dir() -> Utf8PathBuf { Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

fn utf8(path: &std::path::Path) -> &Utf8Path {
    Utf8Path::from_path(path).expect("temporary directory path should be UTF-8")
}

fn open_dir(dir: &Utf8Path) -> Dir {
    Dir::open_ambient_dir(dir, ambient_authority())
        .unwrap_or_else(|error| panic!("failed to open directory {dir}: {error}"))
}

fn fixture(name: &str) -> String {
    let path = format!("tests/data/verification_ledger/{name}.txt");
    open_dir(&manifest_dir())
        .read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read fixture {path}: {error}"))
}

fn script_path() -> Utf8PathBuf { manifest_dir().join("scripts/check-verification-ledger.sh") }

fn materialise_ledger(name: &str) -> TempDir {
    let directory = TempDir::new().expect("failed to create temporary directory");
    let root = utf8(directory.path());
    let handle = open_dir(root);
    handle
        .create_dir("docs")
        .expect("failed to create temporary documentation directory");
    handle
        .create_dir("src")
        .expect("failed to create temporary source directory");
    handle
        .write("docs/verification.md", fixture(name))
        .expect("failed to write temporary verification ledger");
    handle
        .write("src/kernel.rs", "pub fn process_stream_inner() {}\n")
        .expect("failed to write temporary source symbol");
    directory
}

fn run_ledger_check(directory: &Utf8Path) -> Output {
    Command::new(script_path())
        .arg(directory)
        .env_remove("RG")
        .output()
        .expect("failed to execute check-verification-ledger.sh")
}

fn parse_workflow() -> Result<Value> {
    serde_yaml::from_str(VERUS_WORKFLOW).context("parse Verus workflow YAML")
}

fn mapping<'a>(value: &'a Value, description: &str) -> Result<&'a Mapping> {
    value
        .as_mapping()
        .with_context(|| format!("{description} should be a mapping"))
}

fn get<'a>(mapping: &'a Mapping, key: &str) -> Result<&'a Value> {
    mapping
        .get(Value::String(key.to_owned()))
        .with_context(|| format!("mapping should define {key}"))
}

fn run_commands(workflow: &Value) -> Result<Vec<&str>> {
    let root = mapping(workflow, "workflow")?;
    let jobs = mapping(get(root, "jobs")?, "jobs")?;
    let verify = mapping(get(jobs, "verify")?, "verify job")?;
    let steps = get(verify, "steps")?
        .as_sequence()
        .context("verify job steps should be a sequence")?;
    Ok(steps
        .iter()
        .filter_map(Value::as_mapping)
        .filter_map(|step| step.get(Value::String("run".to_owned())))
        .filter_map(Value::as_str)
        .collect())
}

#[test]
fn workflow_runs_the_proof_and_non_vacuity_targets() -> Result<()> {
    let workflow = parse_workflow()?;
    let commands = run_commands(&workflow)?;

    ensure!(commands.contains(&"make verus"));
    ensure!(commands.contains(&"make verus-selftest"));
    Ok(())
}

#[test]
fn ledger_check_accepts_an_existing_symbol() {
    let directory = materialise_ledger("valid");

    let output = run_ledger_check(utf8(directory.path()));

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn ledger_check_rejects_a_missing_symbol() {
    let directory = materialise_ledger("missing_symbol");

    let output = run_ledger_check(utf8(directory.path()));

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        MISSING_SYMBOL_DIAGNOSTIC
    );
}
