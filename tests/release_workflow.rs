//! Protects the release workflow's binary provenance and publication contract.

use anyhow::{Context, Result, ensure};
use assert_cmd::Command;
use serde_yaml::{Mapping, Value};

const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release.yml");
const CHECKOUT_ACTION: &str = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1";
const CACHE_ACTION: &str = "actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9";
const UPLOAD_ACTION: &str = "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a";
const CROSS_SHA256: &str = "642375d1bcf3bd88272c32ba90e999f3d983050adf45e66bd2d3887e8e838bad";
const RELEASE_RUST_VERSION: &str = "1.89.0";
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
    let env = as_mapping(get(root, "env")?, "workflow environment")?;
    ensure!(get_string(env, "CROSS_LINUX_X64_SHA256") == Some(CROSS_SHA256));
    ensure!(get_string(env, "RELEASE_RUST_VERSION") == Some(RELEASE_RUST_VERSION));

    let build_steps = steps(job(&workflow, "build")?)?;
    let setup_rust = named_step(build_steps, "Setup Rust")?;
    let setup_inputs = as_mapping(get(setup_rust, "with")?, "Setup Rust inputs")?;
    ensure!(get_string(setup_inputs, "toolchain") == Some("${{ env.RELEASE_RUST_VERSION }}"));
    ensure!(get(setup_inputs, "install-binstall")?.as_bool() == Some(false));
    ensure!(get(setup_inputs, "use-sccache")?.as_bool() == Some(false));
    let cache = named_step(build_steps, "Cache the official cross binaries")?;
    ensure!(get_string(cache, "uses") == Some(CACHE_ACTION));
    ensure!(
        nested_string(cache, "with", "path")
            == Some("~/.local/share/mdtablefix-tools/cross-v${{ env.CROSS_VERSION }}")
    );

    let install = command(named_step(
        build_steps,
        "Install cross from its official release",
    )?)?;
    ensure!(install.contains(CROSS_VERSION_PROBE));
    assert_fragments_in_order(
        install,
        &[
            "if [[ -x \"${cross_binary}\" ]]",
            "if ! installed_version=\"$(probe_cross_version)\"; then",
            "installed_version=\"\"",
            "if [[ \"${installed_version}\" != \"${expected_version}\" ]]",
            "url=\"https://github.com/cross-rs/cross/releases/download/",
            "curl --fail --location --proto '=https' --tlsv1.2 \"${url}\" -o \"${archive}\"",
            "echo \"${CROSS_LINUX_X64_SHA256}  ${archive}\" | sha256sum --check --status",
            "tar --extract --gzip --file \"${archive}\" --directory \"${cross_dir}\"",
            "installed_version=\"$(probe_cross_version)\"",
            "[[ \"${installed_version}\" == \"${expected_version}\" ]]",
        ],
    )?;
    ensure!(!install.contains("cargo install"));
    let add_target = command(named_step(build_steps, "Add release target")?)?;
    ensure!(
        add_target
            == "rustup target add --toolchain \"${RELEASE_RUST_VERSION}\" ${{ matrix.target }}"
    );
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
    ensure!(get_string(build, "needs") == Some("prepare-release"));
    let strategy = as_mapping(get(build, "strategy")?, "build strategy")?;
    ensure!(get(strategy, "fail-fast")?.as_bool() == Some(false));

    let build_steps = steps(build)?;
    let prepare_index = named_step_index(build_steps, "Prepare artifact")?;
    let upload_index = named_step_index(build_steps, "Upload release artifact")?;
    let publish_index = named_step_index(build_steps, "Publish this target's release assets")?;
    ensure!(prepare_index < upload_index && upload_index < publish_index);
    let upload = build_steps[upload_index]
        .as_mapping()
        .context("upload step")?;
    ensure!(get_string(upload, "uses") == Some(UPLOAD_ACTION));
    let upload_inputs = as_mapping(get(upload, "with")?, "artifact upload inputs")?;
    ensure!(get(upload_inputs, "overwrite")?.as_bool() == Some(true));
    let publish = command(named_step(
        build_steps,
        "Publish this target's release assets",
    )?)?;
    assert_fragments_in_order(
        publish,
        &[
            "for file in \"${artifact_dir}\"/*",
            "releases/tags/${RELEASE_TAG}",
            "if [[ -n \"${asset_id}\" ]]",
            "releases/assets/${asset_id}",
            "cmp --silent \"${file}\" \"${existing_file}\"",
            "continue",
            "gh release upload \"${RELEASE_TAG}\" \"${file}\"",
            "--repo \"${GITHUB_REPOSITORY}\"",
        ],
    )?;
    ensure!(!publish.contains("--clobber"));

    let jobs = as_mapping(get(as_mapping(&workflow, "workflow")?, "jobs")?, "jobs")?;
    ensure!(!jobs.contains_key(Value::String("release".to_owned())));
    Ok(())
}

#[test]
fn cargo_binstall_archives_are_reproducible() -> Result<()> {
    let workflow = parse_workflow()?;
    let build_steps = steps(job(&workflow, "build")?)?;
    let timestamp_index = named_step_index(build_steps, "Set reproducible release timestamp")?;
    let build_index = named_step_index(build_steps, "Build release binary")?;
    let prepare_index = named_step_index(build_steps, "Prepare artifact")?;
    ensure!(timestamp_index < build_index && build_index < prepare_index);

    // `cross` cannot emit Apple or MSVC binaries, so the native runners drive
    // Cargo directly. Both arms pin the release toolchain.
    let build = command(named_step(build_steps, "Build release binary")?)?;
    assert_fragments_in_order(
        build,
        &[
            "if [[ \"${{ matrix.builder }}\" == \"cross\" ]]",
            "cross +\"${RELEASE_RUST_VERSION}\" build --release --target ${{ matrix.target }}",
            "else",
            "cargo +\"${RELEASE_RUST_VERSION}\" build --release --target ${{ matrix.target }}",
        ],
    )?;

    let timestamp = command(named_step(
        build_steps,
        "Set reproducible release timestamp",
    )?)?;
    ensure!(
        timestamp
            == "echo \"SOURCE_DATE_EPOCH=$(git show -s --format=%ct HEAD)\" >> \"${GITHUB_ENV}\""
    );

    // The archive itself is written by the packaging script. Every runner
    // image can execute that script, and its determinism is covered by
    // tests/release_packaging.rs; the workflow's job is only to invoke it.
    let prepare = command(named_step(build_steps, "Prepare artifact")?)?;
    assert_fragments_in_order(
        prepare,
        &[
            "scripts/package_release_artifacts.py",
            "--binary \"target/${{ matrix.target }}/release/${REPO_NAME}${binary_ext}\"",
            "--artifact-dir \"artifacts/${{ matrix.os }}-${{ matrix.arch }}\"",
            "--version \"${RELEASE_TAG#v}\"",
            "--target \"${{ matrix.target }}\"",
            "--arch \"${{ matrix.arch }}\"",
        ],
    )?;
    ensure!(!prepare.contains("sha256sum"));
    ensure!(!prepare.contains("tar -C"));
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
    let triggers = as_mapping(get(root, "on")?, "workflow triggers")?;
    let dispatch = as_mapping(get(triggers, "workflow_dispatch")?, "manual trigger")?;
    let inputs = as_mapping(get(dispatch, "inputs")?, "manual inputs")?;
    let release_tag = as_mapping(get(inputs, "release_tag")?, "release_tag input")?;
    ensure!(get(release_tag, "required")?.as_bool() == Some(true));

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

    let prepare = job(&workflow, "prepare-release")?;
    let build = job(&workflow, "build")?;
    assert_checkout_ref(prepare)?;
    assert_checkout_ref(build)?;
    let prepare_steps = steps(prepare)?;
    let verify_index = named_step_index(prepare_steps, "Verify release tag matches Cargo.toml")?;
    let create_index = named_step_index(prepare_steps, "Create the GitHub release when absent")?;
    ensure!(verify_index < create_index);
    assert_fragments_in_order(
        command(
            prepare_steps[verify_index]
                .as_mapping()
                .context("verify step")?,
        )?,
        &[
            "tag=\"${RELEASE_TAG#v}\"",
            "Cargo.toml",
            "if [[ \"${tag}\" != \"${cargo_version}\" ]]",
        ],
    )?;
    let create = command(
        prepare_steps[create_index]
            .as_mapping()
            .context("create step")?,
    )?;
    ensure!(create.contains("gh release view \"${RELEASE_TAG}\""));
    ensure!(create.contains("gh release create \"${RELEASE_TAG}\""));
    ensure!(create.contains("--generate-notes --verify-tag"));
    Ok(())
}

#[test]
fn release_actions_use_immutable_commit_pins() -> Result<()> {
    let workflow = parse_workflow()?;
    for job_name in ["prepare-release", "build"] {
        for step in steps(job(&workflow, job_name)?)? {
            let Some(reference) = step
                .as_mapping()
                .and_then(|mapping| get_string(mapping, "uses"))
            else {
                continue;
            };
            let (_, pin) = reference
                .rsplit_once('@')
                .with_context(|| format!("action reference {reference:?} should contain @"))?;
            ensure!(
                pin.len() == 40 && pin.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "action reference {reference:?} should use a full commit SHA"
            );
        }
    }
    Ok(())
}
