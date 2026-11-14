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
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use serde_yaml::{Mapping, Value};
use tempfile::TempDir;

#[path = "support/fake_prover_tools.rs"]
mod fake_prover_tools;
use fake_prover_tools::{fake_prover_tools, make_command, runner_log};

#[path = "support/verus_mutation_checks.rs"]
mod verus_mutation_checks;

const VERUS_WORKFLOW: &str = include_str!("../.github/workflows/verus.yml");
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

fn workflow_has_read_only_contents_permission(workflow: &Value) -> Result<bool> {
    let root = mapping(workflow, "workflow")?;
    let permissions = mapping(get(root, "permissions")?, "workflow permissions")?;
    Ok(permissions.len() == 1
        && permissions
            .get(Value::String("contents".to_owned()))
            .and_then(Value::as_str)
            == Some("read"))
}

/// The shared action that installs the provers, which the workflow must use.
const INSTALL_ACTION: &str = "leynos/shared-actions/.github/actions/install-prover-tools@";

/// The `with.tools` input of every step that uses `install-prover-tools`, and
/// whether each such step is pinned by a full commit SHA.
fn installer_steps(workflow: &Value) -> Result<Vec<(bool, Option<String>)>> {
    let root = mapping(workflow, "workflow")?;
    let jobs = mapping(get(root, "jobs")?, "jobs")?;
    let verify = mapping(get(jobs, "verify")?, "verify job")?;
    let steps = get(verify, "steps")?
        .as_sequence()
        .context("verify job steps should be a sequence")?;
    Ok(steps
        .iter()
        .filter_map(Value::as_mapping)
        .filter_map(|step| {
            let uses = step.get(Value::String("uses".to_owned()))?.as_str()?;
            let revision = uses.strip_prefix(INSTALL_ACTION)?;
            let is_pinned =
                revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit());
            let tools = step
                .get(Value::String("with".to_owned()))
                .and_then(Value::as_mapping)
                .and_then(|with| with.get(Value::String("tools".to_owned())))
                .and_then(Value::as_str)
                .map(str::to_owned);
            Some((is_pinned, tools))
        })
        .collect())
}

fn workflow_installs_verus_only_through_the_shared_action(workflow: &Value) -> Result<bool> {
    let installers = installer_steps(workflow)?;
    let has_verus_installer = installers
        .iter()
        .any(|(is_pinned, tools)| *is_pinned && tools.as_deref() == Some("verus"));
    let root = mapping(workflow, "workflow")?;
    let has_cache = format!("{root:?}").contains("actions/cache@");
    Ok(has_verus_installer && !has_cache)
}

#[test]
fn tools_verus_outside_the_installer_inputs_does_not_count() -> Result<()> {
    let sha = "a".repeat(40);
    let yaml = format!(
        "jobs:\n  verify:\n    steps:\n      - uses: {INSTALL_ACTION}{sha}\n        env:\n          \
         tools: verus\n        with:\n          tools: kani\n      - uses: actions/x@y\n        \
         with:\n          tools: verus\n"
    );
    let workflow: Value = serde_yaml::from_str(&yaml).context("parse negative workflow")?;
    ensure!(!workflow_installs_verus_only_through_the_shared_action(
        &workflow
    )?);
    Ok(())
}

#[test]
fn workflow_runs_the_proof_and_non_vacuity_targets() -> Result<()> {
    let workflow = parse_workflow()?;
    let commands = run_commands(&workflow)?;

    ensure!(commands.contains(&"make verus"));
    ensure!(commands.contains(&"make verus-selftest"));
    // The mutation gates are negative controls: each mutates one production
    // module and requires the run to fail with that module's own contract
    // named. A gate that is only in the Makefile is run by nothing that could
    // notice it stopping, so continuous integration has to name every one of
    // them.
    ensure!(commands.contains(&"make verus-mutation"));
    ensure!(commands.contains(&"make verus-fence-mutation"));
    ensure!(commands.contains(&"make verus-compression-mutation"));
    ensure!(checkout_does_not_persist_credentials(&workflow)?);
    ensure!(workflow_has_required_pull_request_triggers(&workflow)?);
    ensure!(workflow_has_read_only_contents_permission(&workflow)?);
    ensure!(workflow_installs_verus_only_through_the_shared_action(
        &workflow
    )?);
    ensure!(MAKEFILE.contains(
        "git+https://github.com/leynos/rust-prover-tools@$(shell cat tools/rust-prover-tools/REF)"
    ));
    Ok(())
}

#[rstest]
#[case::valid("valid", 0, None)]
#[case::missing_symbol("missing_symbol", 1, Some(MISSING_SYMBOL_DIAGNOSTIC))]
#[case::prefix_only("prefix_only", 1, Some(PREFIX_ONLY_SYMBOL_DIAGNOSTIC))]
#[case::multiline_literal("multiline_literal", 1, Some(PREFIX_ONLY_SYMBOL_DIAGNOSTIC))]
#[case::lifetime_declaration("lifetime_declaration", 0, None)]
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

/// The proof targets run on an installed verifier: no install, and the solver
/// path is the one the shared action and `make verus-install` both use.
fn ensure_ran_without_installing(log: &str) -> Result<()> {
    ensure!(
        !log.lines().any(|line| line.contains(" install")),
        "a proof target must not install a prover; log was {log:?}"
    );
    ensure!(
        log.lines()
            .any(|line| line.starts_with("VERUS_Z3_PATH=") && line.ends_with("/.verus/z3/z3")),
        "a proof target must point Verus at the installed Z3; log was {log:?}"
    );
    Ok(())
}

#[test]
fn make_verus_runs_the_library_proof_without_installing() -> Result<()> {
    let runner = fake_prover_tools("rejected")?;
    let output = make_command("verus", &runner)
        .output()
        .context("run make verus with fake prover-tools")?;

    ensure!(output.status.success());
    let log = runner_log(&runner)?;
    ensure_ran_without_installing(&log)?;
    ensure!(log.lines().any(
        |line| line == "verus run --repo-root . --no-install-missing --proof-file verus/lib.rs"
    ));
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
    ensure_ran_without_installing(&log)?;
    ensure!(
        log.lines().any(|line| line
            == "verus run --repo-root . --no-install-missing --proof-file verus/smoke.rs")
    );
    Ok(())
}

/// Regression case for issue #586: the fake runner still runs while a write
/// descriptor on it is open, as one inherited by a concurrently forked child
/// would be.
///
/// Executing a file with an open write descriptor fails with `ETXTBSY`. The
/// harness must not depend on no other thread forking at the wrong moment, so
/// it runs the stub through `bash`, which only reads it.
#[test]
fn verus_harness_issue_586_runner_survives_an_open_write_descriptor() -> Result<()> {
    let runner = fake_prover_tools("rejected")?;
    let root = runner
        .path
        .parent()
        .context("fake prover-tools path has no parent")?;
    let _held_open = open_dir(root)?
        .open_with("prover-tools", cap_std::fs::OpenOptions::new().append(true))
        .context("hold the runner open for writing")?;

    let output = make_command("verus", &runner)
        .output()
        .context("run make verus while the runner is open for writing")?;

    ensure!(
        output.status.success(),
        "make verus failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
