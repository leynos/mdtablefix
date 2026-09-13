//! Guards the Verus workflow, Make targets, and verification-ledger contract.
//!
//! `make verus-selftest` is the executable non-vacuity check in continuous
//! integration. It runs `verus/smoke.rs` and requires the deliberately false
//! assertion to fail, so the Makefile tests use a controlled fake runner rather
//! than a local Verus installation.

#![cfg(unix)]

use std::process::{Command, Output};

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs::{Permissions, PermissionsExt},
    fs_utf8::Dir,
};
use rstest::{fixture, rstest};
use serde_yaml::{Mapping, Value};
use tempfile::TempDir;

const VERUS_WORKFLOW: &str = include_str!("../.github/workflows/verus.yml");
const VERUS_VERSION: &str = include_str!("../tools/verus/VERSION");
const MAKEFILE: &str = include_str!("../Makefile");
const MISSING_SYMBOL_DIAGNOSTIC: &str =
    "verification ledger names missing symbol: missing_verified_symbol";
const PREFIX_ONLY_SYMBOL_DIAGNOSTIC: &str =
    "verification ledger names missing symbol: process_stream";

fn manifest_dir() -> Utf8PathBuf { Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

fn utf8(path: &std::path::Path) -> Result<&Utf8Path> {
    Utf8Path::from_path(path).context("temporary directory path should be UTF-8")
}

fn open_dir(dir: &Utf8Path) -> Result<Dir> {
    Dir::open_ambient_dir(dir, ambient_authority()).with_context(|| format!("open directory {dir}"))
}

fn fixture_text(name: &str, extension: &str) -> Result<String> {
    let path = format!("tests/data/verification_ledger/{name}.{extension}");
    let root = manifest_dir();
    open_dir(&root)?
        .read_to_string(&path)
        .with_context(|| format!("read verification-ledger fixture {path}"))
}

fn script_path() -> Utf8PathBuf { manifest_dir().join("scripts/check-verification-ledger.sh") }

#[fixture]
fn materialised_ledger(#[default("valid")] name: &str) -> Result<TempDir> {
    let directory = TempDir::new().context("create temporary ledger directory")?;
    let root = utf8(directory.path())?;
    let handle = open_dir(root)?;
    handle
        .create_dir("docs")
        .context("create temporary documentation directory")?;
    handle
        .create_dir("src")
        .context("create temporary source directory")?;
    handle
        .write("docs/verification.md", fixture_text(name, "txt")?)
        .context("write temporary verification ledger")?;
    handle
        .write("src/kernel.rs", fixture_text(name, "rs")?)
        .context("write temporary source fixture")?;
    Ok(directory)
}

fn run_ledger_check(directory: &Utf8Path) -> Output {
    Command::new(script_path())
        .arg(directory)
        .env_remove("RG")
        .output()
        .expect("failed to execute check-verification-ledger.sh")
}

struct FakeProverTools {
    _directory: TempDir,
    path: Utf8PathBuf,
    log_path: Utf8PathBuf,
    smoke_mode: &'static str,
}

fn fake_prover_tools(smoke_mode: &'static str) -> Result<FakeProverTools> {
    let directory = TempDir::new().context("create fake prover-tools directory")?;
    let root = utf8(directory.path())?;
    let handle = open_dir(root)?;
    let path = root.join("prover-tools");
    let log_path = root.join("prover-tools.log");
    let script = r#"#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "${FAKE_PROVER_TOOLS_LOG:?}"
if [[ "$*" == "verus run --repo-root . --proof-file verus/smoke.rs" ]]; then
    case "${FAKE_PROVER_TOOLS_SMOKE_MODE:?}" in
        rejected)
            echo "Verus proofs failed"
            exit 1
            ;;
        accepted) exit 0 ;;
        unrelated_failure)
            echo "runner unavailable"
            exit 1
            ;;
    esac
fi
"#;
    handle
        .write("prover-tools", script)
        .context("write fake prover-tools runner")?;
    handle
        .set_permissions("prover-tools", Permissions::from_mode(0o755))
        .context("make fake prover-tools runner executable")?;
    Ok(FakeProverTools {
        _directory: directory,
        path,
        log_path,
        smoke_mode,
    })
}

fn make_command(target: &str, runner: &FakeProverTools) -> Command {
    let mut command = Command::new("make");
    command
        .arg("--no-print-directory")
        .arg(target)
        .current_dir(manifest_dir())
        .env("PROVER_TOOLS", &runner.path)
        .env(
            "VERUS_RUN",
            format!("{} verus run --repo-root .", runner.path),
        )
        .env("FAKE_PROVER_TOOLS_LOG", &runner.log_path)
        .env("FAKE_PROVER_TOOLS_SMOKE_MODE", runner.smoke_mode);
    command
}

fn runner_log(runner: &FakeProverTools) -> Result<String> {
    let root = runner
        .path
        .parent()
        .context("fake prover-tools path has no parent")?;
    open_dir(root)?
        .read_to_string("prover-tools.log")
        .context("read fake prover-tools log")
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

fn checkout_does_not_persist_credentials(workflow: &Value) -> Result<bool> {
    let root = mapping(workflow, "workflow")?;
    let jobs = mapping(get(root, "jobs")?, "jobs")?;
    let verify = mapping(get(jobs, "verify")?, "verify job")?;
    let steps = get(verify, "steps")?
        .as_sequence()
        .context("verify job steps should be a sequence")?;
    Ok(steps.iter().filter_map(Value::as_mapping).any(|step| {
        step.get(Value::String("uses".to_owned()))
            .and_then(Value::as_str)
            .is_some_and(|uses| uses.starts_with("actions/checkout@"))
            && step
                .get(Value::String("with".to_owned()))
                .and_then(Value::as_mapping)
                .and_then(|config| config.get(Value::String("persist-credentials".to_owned())))
                .and_then(Value::as_bool)
                .is_some_and(|persist| !persist)
    }))
}

fn workflow_has_required_pull_request_triggers(workflow: &Value) -> Result<bool> {
    let root = mapping(workflow, "workflow")?;
    let events = mapping(get(root, "on")?, "workflow events")?;
    let pull_request = mapping(get(events, "pull_request")?, "pull-request event")?;
    let types = get(pull_request, "types")?
        .as_sequence()
        .context("pull-request event types should be a sequence")?;
    let has_required_types = ["opened", "synchronize", "reopened"]
        .iter()
        .all(|required| {
            types
                .iter()
                .filter_map(Value::as_str)
                .any(|configured| configured == *required)
        });

    Ok(has_required_types && events.contains_key(Value::String("workflow_dispatch".to_owned())))
}

fn workflow_uses_pinned_verus_cache(workflow: &Value) -> Result<bool> {
    let root = mapping(workflow, "workflow")?;
    let environment = mapping(get(root, "env")?, "workflow environment")?;
    let version = get(environment, "VERUS_VERSION")?
        .as_str()
        .context("Verus version should be a string")?;
    let expected_version = VERUS_VERSION.trim();
    if version != expected_version {
        return Ok(false);
    }

    let jobs = mapping(get(root, "jobs")?, "jobs")?;
    let verify = mapping(get(jobs, "verify")?, "verify job")?;
    let steps = get(verify, "steps")?
        .as_sequence()
        .context("verify job steps should be a sequence")?;
    Ok(steps.iter().filter_map(Value::as_mapping).any(|step| {
        step.get(Value::String("uses".to_owned()))
            .and_then(Value::as_str)
            .is_some_and(|uses| uses.starts_with("actions/cache@"))
            && step
                .get(Value::String("with".to_owned()))
                .and_then(Value::as_mapping)
                .is_some_and(|config| {
                    config
                        .get(Value::String("path".to_owned()))
                        .and_then(Value::as_str)
                        == Some(".verus/${{ env.VERUS_VERSION }}")
                        && config
                            .get(Value::String("key".to_owned()))
                            .and_then(Value::as_str)
                            == Some(
                                "verus-${{ runner.os }}-${{ runner.arch }}-${{ env.VERUS_VERSION \
                                 }}",
                            )
                })
    }))
}

#[test]
fn workflow_runs_the_proof_and_non_vacuity_targets() -> Result<()> {
    let workflow = parse_workflow()?;
    let commands = run_commands(&workflow)?;

    ensure!(commands.contains(&"make verus"));
    ensure!(commands.contains(&"make verus-selftest"));
    ensure!(checkout_does_not_persist_credentials(&workflow)?);
    ensure!(workflow_has_required_pull_request_triggers(&workflow)?);
    ensure!(workflow_uses_pinned_verus_cache(&workflow)?);
    ensure!(MAKEFILE.contains(
        "git+https://github.com/leynos/rust-prover-tools@$(shell cat tools/rust-prover-tools/REF)"
    ));
    Ok(())
}

#[rstest]
#[case::valid("valid", 0, None)]
#[case::missing_symbol("missing_symbol", 1, Some(MISSING_SYMBOL_DIAGNOSTIC))]
#[case::prefix_only("prefix_only", 1, Some(PREFIX_ONLY_SYMBOL_DIAGNOSTIC))]
fn ledger_check_requires_an_exact_declaration(
    #[case] fixture_name: &str,
    #[case] expected_status: i32,
    #[case] expected_diagnostic: Option<&str>,
    #[from(materialised_ledger)]
    #[with(fixture_name)]
    materialised_ledger: Result<TempDir>,
) -> Result<()> {
    let directory = materialised_ledger?;
    let output = run_ledger_check(utf8(directory.path())?);

    assert_eq!(
        output.status.code(),
        Some(expected_status),
        "ledger fixture {fixture_name}"
    );
    if let Some(expected_diagnostic) = expected_diagnostic {
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim_end(),
            expected_diagnostic
        );
    }
    Ok(())
}

#[test]
fn make_verus_installs_and_runs_the_library_proof() -> Result<()> {
    let runner = fake_prover_tools("rejected")?;
    let output = make_command("verus", &runner)
        .output()
        .context("run make verus with fake prover-tools")?;

    ensure!(output.status.success());
    let log = runner_log(&runner)?;
    ensure!(
        log.lines()
            .any(|line| line == "verus install --repo-root .")
    );
    ensure!(
        log.lines()
            .any(|line| line == "verus run --repo-root . --proof-file verus/lib.rs")
    );
    Ok(())
}

#[rstest]
#[case::rejected_proof("rejected", true, None)]
#[case::accepted_proof("accepted", false, Some("Verus smoke proof unexpectedly succeeded"))]
#[case::unrelated_runner_failure(
    "unrelated_failure",
    false,
    Some("Verus smoke proof did not reach the verifier")
)]
fn make_verus_selftest_accepts_only_a_rejected_smoke_proof(
    #[case] smoke_mode: &'static str,
    #[case] should_succeed: bool,
    #[case] expected_diagnostic: Option<&str>,
) -> Result<()> {
    let runner = fake_prover_tools(smoke_mode)?;
    let output = make_command("verus-selftest", &runner)
        .output()
        .context("run make verus-selftest with fake prover-tools")?;

    assert_eq!(output.status.success(), should_succeed);
    if let Some(expected_diagnostic) = expected_diagnostic {
        ensure!(
            String::from_utf8_lossy(&output.stderr).contains(expected_diagnostic),
            "expected self-test diagnostic: {expected_diagnostic}"
        );
    }
    let log = runner_log(&runner)?;
    ensure!(
        log.lines()
            .any(|line| line == "verus install --repo-root .")
    );
    ensure!(
        log.lines()
            .any(|line| line == "verus run --repo-root . --proof-file verus/smoke.rs")
    );
    Ok(())
}
