//! Protects the release workflow's binary provenance and publication contract.

use anyhow::{Context, Result, ensure};
// The only use drives the CLI through `bash`, so the import is dead weight
// everywhere that has no `bash` on the same terms, and `-D warnings` in CI
// rejects it there.
#[cfg(target_family = "unix")]
use assert_cmd::Command;
use serde_yaml::{Mapping, Value};

// The paths are relative to this file's own directory, `tests/`.
#[path = "support/release_assertions.rs"]
mod release_assertions;

use release_assertions::{
    assert_build_runs_each_target_independently,
    assert_cross_cache_step,
    assert_cross_checksum_environment,
    assert_cross_install_step,
    assert_no_aggregate_release_job,
    assert_prepare_command,
    assert_prepare_upload_publish_order,
    assert_publish_step_is_idempotent,
    assert_release_build_command,
    assert_setup_rust_inputs,
    assert_timestamp_command,
};

const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release.yml");
const CHECKOUT_ACTION: &str = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1";
const CACHE_ACTION: &str = "actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9";
const UPLOAD_ACTION: &str = "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a";
const CROSS_SHA256: &str = "642375d1bcf3bd88272c32ba90e999f3d983050adf45e66bd2d3887e8e838bad";
const CROSS_VERSION_PROBE: &str = concat!(
    "probe_cross_version() {\n",
    "  (\n",
    "    cd \"${RUNNER_TEMP}\"\n",
    "    \"${cross_binary}\" --version 2>/dev/null\n",
    "  ) | sed -n '1p'\n",
    "}",
);

fn parse_workflow() -> Result<Value> {
    serde_yaml::from_str(RELEASE_WORKFLOW).context("parse release workflow YAML")
}

fn as_mapping<'a>(value: &'a Value, description: &str) -> Result<&'a Mapping> {
    value
        .as_mapping()
        .with_context(|| format!("{description} should be a mapping"))
}

fn get<'a>(mapping: &'a Mapping, key: &str) -> Result<&'a Value> {
    mapping
        .get(Value::String(key.to_owned()))
        .with_context(|| format!("mapping should define {key}"))
}

fn job<'a>(workflow: &'a Value, name: &str) -> Result<&'a Mapping> {
    let root = as_mapping(workflow, "workflow")?;
    let jobs = as_mapping(get(root, "jobs")?, "jobs")?;
    as_mapping(get(jobs, name)?, name)
}

fn steps(job: &Mapping) -> Result<&[Value]> {
    get(job, "steps")?
        .as_sequence()
        .map(Vec::as_slice)
        .context("job steps should be a sequence")
}

fn step_mapping<'a>(step: &'a Value, description: &str) -> Result<&'a Mapping> {
    as_mapping(step, description)
}

fn named_step<'a>(steps: &'a [Value], name: &str) -> Result<&'a Mapping> {
    steps
        .iter()
        .find_map(|step| {
            let mapping = step.as_mapping()?;
            (get_string(mapping, "name") == Some(name)).then_some(mapping)
        })
        .with_context(|| format!("job should define the {name} step"))
}

fn named_step_index(steps: &[Value], name: &str) -> Result<usize> {
    steps
        .iter()
        .position(|step| {
            step.as_mapping()
                .and_then(|mapping| get_string(mapping, "name"))
                == Some(name)
        })
        .with_context(|| format!("job should define the {name} step"))
}

fn get_string<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a str> {
    mapping
        .get(Value::String(key.to_owned()))
        .and_then(Value::as_str)
}

fn nested_string<'a>(mapping: &'a Mapping, group: &str, key: &str) -> Option<&'a str> {
    mapping
        .get(Value::String(group.to_owned()))
        .and_then(Value::as_mapping)
        .and_then(|nested| get_string(nested, key))
}

fn command(step: &Mapping) -> Result<&str> {
    get_string(step, "run").context("step should define a run command")
}

fn assert_fragments_in_order(command: &str, fragments: &[&str]) -> Result<()> {
    let mut search_start = 0;
    for fragment in fragments {
        let relative_index = command[search_start..]
            .find(fragment)
            .with_context(|| format!("command should contain {fragment:?} in order"))?;
        search_start += relative_index + fragment.len();
    }
    Ok(())
}

fn assert_checkout_ref(job: &Mapping) -> Result<()> {
    let checkout = step_mapping(
        steps(job)?
            .first()
            .context("job should start with checkout")?,
        "checkout step",
    )?;
    ensure!(get_string(checkout, "uses") == Some(CHECKOUT_ACTION));
    ensure!(nested_string(checkout, "with", "ref") == Some("${{ env.RELEASE_TAG }}"));
    Ok(())
}

#[test]
fn cross_comes_from_a_verified_official_archive() -> Result<()> {
    let workflow = parse_workflow()?;
    let root = as_mapping(&workflow, "workflow")?;
    assert_cross_checksum_environment(root)?;

    let build_steps = steps(job(&workflow, "build")?)?;
    assert_setup_rust_inputs(build_steps)?;
    assert_cross_cache_step(build_steps)?;
    assert_cross_install_step(build_steps)?;
    let add_target = command(named_step(build_steps, "Add release target")?)?;
    ensure!(add_target == "rustup target add ${{ matrix.target }}");
    Ok(())
}

#[test]
#[cfg(target_family = "unix")]
fn cross_version_probe_preserves_failure_after_banner() -> Result<()> {
    let runner_temp = tempfile::tempdir().context("create probe working directory")?;
    let script = [
        "set -uo pipefail\n",
        "fake_cross() {\n",
        "  printf 'cross 0.2.5\\nhost cargo fallback\\n'\n",
        "  return 17\n",
        "}\n",
        "cross_binary=fake_cross\n",
        CROSS_VERSION_PROBE,
        "\n",
        "if installed_version=\"$(probe_cross_version)\"; then\n",
        "  exit 1\n",
        "fi\n",
        "[[ \"${installed_version}\" == 'cross 0.2.5' ]]\n",
    ]
    .concat();
    Command::new("bash")
        .arg("-c")
        .arg(script)
        .env("RUNNER_TEMP", runner_temp.path())
        .assert()
        .success();
    Ok(())
}

#[test]
fn successful_targets_publish_without_waiting_for_the_matrix() -> Result<()> {
    let workflow = parse_workflow()?;
    let build = job(&workflow, "build")?;
    assert_build_runs_each_target_independently(build)?;
    assert_prepare_upload_publish_order(steps(build)?)?;
    assert_publish_step_is_idempotent(steps(build)?)?;
    assert_no_aggregate_release_job(&workflow)
}

#[test]
fn cargo_binstall_archives_are_reproducible() -> Result<()> {
    let workflow = parse_workflow()?;
    let build_steps = steps(job(&workflow, "build")?)?;
    let timestamp_index = named_step_index(build_steps, "Set reproducible release timestamp")?;
    let build_index = named_step_index(build_steps, "Build release binary")?;
    let prepare_index = named_step_index(build_steps, "Prepare artifact")?;
    ensure!(timestamp_index < build_index && build_index < prepare_index);
    assert_release_build_command(build_steps)?;
    assert_timestamp_command(build_steps)?;
    assert_prepare_command(build_steps)?;
    Ok(())
}

#[test]
fn every_release_step_runs_under_bash() -> Result<()> {
    // Windows runners default to PowerShell, so the job pins bash once rather
    // than per step; a step that opted out would silently change semantics.
    let workflow = parse_workflow()?;
    let build = job(&workflow, "build")?;
    ensure!(get_string(build, "runs-on") == Some("${{ matrix.runner }}"));
    let defaults = as_mapping(get(build, "defaults")?, "build defaults")?;
    let run_defaults = as_mapping(get(defaults, "run")?, "build run defaults")?;
    ensure!(get_string(run_defaults, "shell") == Some("bash"));
    for step in steps(build)? {
        let mapping = step_mapping(step, "build step")?;
        ensure!(
            get_string(mapping, "shell").is_none(),
            "steps should inherit the job's bash default"
        );
    }
    Ok(())
}

#[test]
fn manual_dispatch_builds_the_requested_release_tag() -> Result<()> {
    let workflow = parse_workflow()?;
    let root = as_mapping(&workflow, "workflow")?;
    assert_manual_trigger_requires_release_tag(root)?;
    assert_release_tag_environment_and_concurrency(root)?;
    assert_both_jobs_check_out_the_release_tag(&workflow)?;
    assert_verify_precedes_create(steps(job(&workflow, "prepare-release")?)?)
}

/// Checks the preparation and build jobs both check out the requested tag.
fn assert_both_jobs_check_out_the_release_tag(workflow: &Value) -> Result<()> {
    assert_checkout_ref(job(workflow, "prepare-release")?)?;
    assert_checkout_ref(job(workflow, "build")?)
}

/// Checks the manual trigger requires a `release_tag` input.
fn assert_manual_trigger_requires_release_tag(root: &Mapping) -> Result<()> {
    let triggers = as_mapping(get(root, "on")?, "workflow triggers")?;
    let dispatch = as_mapping(get(triggers, "workflow_dispatch")?, "manual trigger")?;
    let inputs = as_mapping(get(dispatch, "inputs")?, "manual inputs")?;
    let release_tag = as_mapping(get(inputs, "release_tag")?, "release_tag input")?;
    ensure!(get(release_tag, "required")?.as_bool() == Some(true));
    Ok(())
}

/// Checks the release tag and the per-tag concurrency group come from the
/// dispatch input, and that a running release is never cancelled.
fn assert_release_tag_environment_and_concurrency(root: &Mapping) -> Result<()> {
    let env = as_mapping(get(root, "env")?, "workflow environment")?;
    ensure!(get_string(env, "RELEASE_TAG") == Some("${{ inputs.release_tag || github.ref_name }}"));
    let concurrency = as_mapping(get(root, "concurrency")?, "release concurrency")?;
    ensure!(
        get_string(concurrency, "group")
            == Some(
                "release-${{ github.repository }}-${{ inputs.release_tag || github.ref_name }}"
            )
    );
    ensure!(get(concurrency, "cancel-in-progress")?.as_bool() == Some(false));
    Ok(())
}

/// Checks the tag is verified against `Cargo.toml` before the release exists.
fn assert_verify_precedes_create(prepare_steps: &[Value]) -> Result<()> {
    let verify_index = named_step_index(prepare_steps, "Verify release tag matches Cargo.toml")?;
    let create_index = named_step_index(prepare_steps, "Create the GitHub release when absent")?;
    ensure!(verify_index < create_index);
    assert_verify_command(&prepare_steps[verify_index])?;
    assert_create_command(&prepare_steps[create_index])
}

/// Checks the verify step compares the tag with the `Cargo.toml` version.
fn assert_verify_command(step: &Value) -> Result<()> {
    let verify = command(step.as_mapping().context("verify step")?)?;
    assert_fragments_in_order(
        verify,
        &[
            "tag=\"${RELEASE_TAG#v}\"",
            "Cargo.toml",
            "if [[ \"${tag}\" != \"${cargo_version}\" ]]",
        ],
    )
}

/// Checks the create step makes a verified-tag release with generated notes.
fn assert_create_command(step: &Value) -> Result<()> {
    let create = command(step.as_mapping().context("create step")?)?;
    ensure!(create.contains("gh release view \"${RELEASE_TAG}\""));
    ensure!(create.contains("gh release create \"${RELEASE_TAG}\""));
    ensure!(create.contains("--generate-notes --verify-tag"));
    Ok(())
}

#[test]
fn release_actions_use_immutable_commit_pins() -> Result<()> {
    let workflow = parse_workflow()?;
    for job_name in ["prepare-release", "build"] {
        for reference in job_action_references(job(&workflow, job_name)?)? {
            ensure_commit_pinned(reference)?;
        }
    }
    Ok(())
}

/// Returns the `uses:` reference of every step in `job` that names an action.
fn job_action_references(job: &Mapping) -> Result<Vec<&str>> {
    Ok(steps(job)?
        .iter()
        .filter_map(|step| {
            step.as_mapping()
                .and_then(|mapping| get_string(mapping, "uses"))
        })
        .collect())
}

/// Checks an action reference names a full 40-digit commit SHA after its `@`.
fn ensure_commit_pinned(reference: &str) -> Result<()> {
    let (_, pin) = reference
        .rsplit_once('@')
        .with_context(|| format!("action reference {reference:?} should contain @"))?;
    ensure!(
        pin.len() == 40 && pin.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "action reference {reference:?} should use a full commit SHA"
    );
    Ok(())
}
