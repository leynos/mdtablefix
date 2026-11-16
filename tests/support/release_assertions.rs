//! Step-level assertions for the release workflow contract.
//!
//! `tests/release_workflow.rs` owns the test entry points and the YAML
//! accessors; this module holds the assertions each entry point composes, so
//! that file stays a statement of the contract. Every helper takes the part of
//! the workflow it checks and names, in its failure, what the contract wanted.

use anyhow::{Context, Result, ensure};
use serde_yaml::{Mapping, Value};

use super::{
    CACHE_ACTION,
    CROSS_SHA256,
    CROSS_VERSION_PROBE,
    UPLOAD_ACTION,
    as_mapping,
    assert_fragments_in_order,
    command,
    get,
    get_string,
    named_step,
    named_step_index,
    nested_string,
};

/// Checks the workflow environment pins the cross checksum and no toolchain.
pub fn assert_cross_checksum_environment(root: &Mapping) -> Result<()> {
    let env = as_mapping(get(root, "env")?, "workflow environment")?;
    ensure!(get_string(env, "CROSS_LINUX_X64_SHA256") == Some(CROSS_SHA256));
    ensure!(get(env, "RELEASE_RUST_VERSION").is_err());
    Ok(())
}

/// Checks the Setup Rust step leaves the toolchain to `rust-toolchain.toml`.
pub fn assert_setup_rust_inputs(build_steps: &[Value]) -> Result<()> {
    let setup_rust = named_step(build_steps, "Setup Rust")?;
    let setup_inputs = as_mapping(get(setup_rust, "with")?, "Setup Rust inputs")?;
    // No toolchain input: the tagged tree's rust-toolchain.toml selects the nightly.
    ensure!(get(setup_inputs, "toolchain").is_err());
    ensure!(get(setup_inputs, "install-binstall")?.as_bool() == Some(false));
    ensure!(get(setup_inputs, "use-sccache")?.as_bool() == Some(false));
    Ok(())
}

/// Checks the cross binaries are cached under a version-keyed path.
pub fn assert_cross_cache_step(build_steps: &[Value]) -> Result<()> {
    let cache = named_step(build_steps, "Cache the official cross binaries")?;
    ensure!(get_string(cache, "uses") == Some(CACHE_ACTION));
    ensure!(
        nested_string(cache, "with", "path")
            == Some("~/.local/share/mdtablefix-tools/cross-v${{ env.CROSS_VERSION }}")
    );
    Ok(())
}

/// Checks cross is installed from the verified official archive, never built.
pub fn assert_cross_install_step(build_steps: &[Value]) -> Result<()> {
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
    Ok(())
}

/// Checks the build job follows `prepare-release` and does not stop the
/// matrix when one target fails.
pub fn assert_build_runs_each_target_independently(build: &Mapping) -> Result<()> {
    ensure!(get_string(build, "needs") == Some("prepare-release"));
    let strategy = as_mapping(get(build, "strategy")?, "build strategy")?;
    ensure!(get(strategy, "fail-fast")?.as_bool() == Some(false));
    Ok(())
}

/// Checks no job waits for the whole matrix before publishing.
pub fn assert_no_aggregate_release_job(workflow: &Value) -> Result<()> {
    let jobs = as_mapping(get(as_mapping(workflow, "workflow")?, "jobs")?, "jobs")?;
    ensure!(!jobs.contains_key(Value::String("release".to_owned())));
    Ok(())
}

/// Checks each target prepares, uploads, then publishes its own assets, and
/// that the upload overwrites a stale artifact.
pub fn assert_prepare_upload_publish_order(build_steps: &[Value]) -> Result<()> {
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
    Ok(())
}

/// Checks the publish step skips identical assets and never clobbers.
pub fn assert_publish_step_is_idempotent(build_steps: &[Value]) -> Result<()> {
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
    Ok(())
}

/// Checks the build step picks `cross` or Cargo per matrix builder.
pub fn assert_release_build_command(build_steps: &[Value]) -> Result<()> {
    // `cross` cannot emit Apple or MSVC binaries, so the native runners drive
    // Cargo directly. Both arms build on the nightly that rust-toolchain.toml pins.
    let build = command(named_step(build_steps, "Build release binary")?)?;
    assert_fragments_in_order(
        build,
        &[
            "if [[ \"${{ matrix.builder }}\" == \"cross\" ]]",
            "cross build --release --target ${{ matrix.target }}",
            "else",
            "cargo build --release --target ${{ matrix.target }}",
        ],
    )?;
    Ok(())
}

/// Checks `SOURCE_DATE_EPOCH` is taken from the tagged commit.
pub fn assert_timestamp_command(build_steps: &[Value]) -> Result<()> {
    let timestamp = command(named_step(
        build_steps,
        "Set reproducible release timestamp",
    )?)?;
    ensure!(
        timestamp
            == "echo \"SOURCE_DATE_EPOCH=$(git show -s --format=%ct HEAD)\" >> \"${GITHUB_ENV}\""
    );
    Ok(())
}

/// Checks the artifact is packaged by the script, not by inline `tar`.
pub fn assert_prepare_command(build_steps: &[Value]) -> Result<()> {
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
