//! Ties the release packaging to the crate's `cargo-binstall` metadata.
//!
//! `cargo binstall` downloads the asset named by `pkg-url` and then extracts
//! the member named by `bin-dir`. Both templates live in `Cargo.toml`, while
//! the archive is built by `scripts/package_release_artifacts.py`. Nothing in
//! the build connects the two, so these tests stage real archives for every
//! published target and check them against the rendered templates.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, ensure};
use serde_yaml::{Mapping, Value};

const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release.yml");
const CI_WORKFLOW: &str = include_str!("../.github/workflows/ci.yml");
const PACKAGE_SCRIPT: &str = "scripts/package_release_artifacts.py";
const VERIFY_SCRIPT: &str = "scripts/verify_binstall_layout.py";

/// A target whose release assets `cargo binstall` is expected to resolve.
struct ReleaseTarget {
    triple: &'static str,
    os: &'static str,
    arch: &'static str,
    /// The archive member `bin-dir` must render to for this target.
    binary_name: &'static str,
}

const RELEASE_TARGETS: &[ReleaseTarget] = &[
    ReleaseTarget {
        triple: "x86_64-unknown-linux-gnu",
        os: "linux",
        arch: "x86_64",
        binary_name: "mdtablefix",
    },
    ReleaseTarget {
        triple: "aarch64-unknown-linux-gnu",
        os: "linux",
        arch: "aarch64",
        binary_name: "mdtablefix",
    },
    ReleaseTarget {
        triple: "x86_64-apple-darwin",
        os: "macos",
        arch: "x86_64",
        binary_name: "mdtablefix",
    },
    ReleaseTarget {
        triple: "aarch64-apple-darwin",
        os: "macos",
        arch: "aarch64",
        binary_name: "mdtablefix",
    },
    ReleaseTarget {
        triple: "x86_64-pc-windows-msvc",
        os: "windows",
        arch: "x86_64",
        binary_name: "mdtablefix.exe",
    },
];

fn repo_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

fn python() -> Command {
    let mut command = Command::new("python3");
    command.current_dir(repo_root());
    command
}

fn run(command: &mut Command, description: &str) -> Result<String> {
    let output = command
        .output()
        .with_context(|| format!("run {description}"))?;
    ensure!(
        output.status.success(),
        "{description} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Stage a stand-in binary for `target` into `artifact_dir`.
///
/// The payload only has to be a file: the templates under test describe names
/// and archive layout, not machine code.
fn stage(target: &ReleaseTarget, artifact_dir: &Path, source_date_epoch: &str) -> Result<()> {
    let stub = artifact_dir
        .parent()
        .context("staging directory should have a parent")?
        .join(format!("{}.stub", target.triple));
    std::fs::write(&stub, b"stand-in release binary\n").context("write the stand-in binary")?;
    run(
        python()
            .arg(PACKAGE_SCRIPT)
            .arg("--binary")
            .arg(&stub)
            .arg("--artifact-dir")
            .arg(artifact_dir)
            .arg("--target")
            .arg(target.triple)
            .arg("--os")
            .arg(target.os)
            .arg("--arch")
            .arg(target.arch)
            .arg("--source-date-epoch")
            .arg(source_date_epoch),
        "the packaging script",
    )?;
    Ok(())
}

fn parse(workflow: &str) -> Result<Value> {
    serde_yaml::from_str(workflow).context("parse workflow YAML")
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

fn get_string<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a str> {
    mapping
        .get(Value::String(key.to_owned()))
        .and_then(Value::as_str)
}

/// Return the `include` rows of a job's build matrix.
fn matrix_rows<'a>(workflow: &'a Value, job_name: &str) -> Result<Vec<&'a Mapping>> {
    let root = as_mapping(workflow, "workflow")?;
    let jobs = as_mapping(get(root, "jobs")?, "jobs")?;
    let job = as_mapping(get(jobs, job_name)?, job_name)?;
    let strategy = as_mapping(get(job, "strategy")?, "strategy")?;
    let matrix = as_mapping(get(strategy, "matrix")?, "matrix")?;
    get(matrix, "include")?
        .as_sequence()
        .context("matrix include should be a sequence")?
        .iter()
        .map(|row| as_mapping(row, "matrix row"))
        .collect()
}

#[test]
fn every_binstall_target_is_built_by_the_release_matrix() -> Result<()> {
    let workflow = parse(RELEASE_WORKFLOW)?;
    let rows = matrix_rows(&workflow, "build")?;
    for target in RELEASE_TARGETS {
        let row = rows
            .iter()
            .find(|row| get_string(row, "target") == Some(target.triple))
            .with_context(|| format!("build matrix should cover {}", target.triple))?;
        ensure!(
            get(row, "cargo_binstall_archive")?.as_bool() == Some(true),
            "{} should publish a cargo-binstall archive",
            target.triple
        );
        ensure!(
            get_string(row, "os") == Some(target.os)
                && get_string(row, "arch") == Some(target.arch),
            "{} should keep its published asset naming",
            target.triple
        );
        let runner = get_string(row, "runner")
            .with_context(|| format!("{} should name a runner", target.triple))?;
        let expected_runner_family = match target.os {
            "linux" => "ubuntu",
            "macos" => "macos",
            _ => "windows",
        };
        ensure!(
            runner.starts_with(expected_runner_family),
            "{} should build on a {expected_runner_family} runner, not {runner}",
            target.triple
        );
        // Apple and MSVC binaries cannot come out of `cross` on Linux.
        let expected_builder = if target.os == "linux" {
            "cross"
        } else {
            "cargo"
        };
        ensure!(
            get_string(row, "builder") == Some(expected_builder),
            "{} should build with {expected_builder}",
            target.triple
        );
    }
    Ok(())
}

#[test]
fn staged_assets_match_the_binstall_templates() -> Result<()> {
    let staging = tempfile::tempdir().context("create the staging root")?;
    for target in RELEASE_TARGETS {
        let artifact_dir = staging.path().join(target.triple);
        stage(target, &artifact_dir, "1700000000")?;
        let reported = run(
            python()
                .arg(VERIFY_SCRIPT)
                .arg("--artifact-dir")
                .arg(&artifact_dir)
                .arg("--target")
                .arg(target.triple),
            "the binstall layout checker",
        )?;
        ensure!(
            reported
                .trim()
                .ends_with(&format!("-> {}", target.binary_name)),
            "{} should place {} at the archive root, got {reported:?}",
            target.triple,
            target.binary_name
        );
    }
    Ok(())
}

#[test]
fn the_bare_binary_asset_keeps_its_published_name() -> Result<()> {
    let staging = tempfile::tempdir().context("create the staging root")?;
    for target in RELEASE_TARGETS {
        let artifact_dir = staging.path().join(target.triple);
        stage(target, &artifact_dir, "1700000000")?;
        let extension = if target.os == "windows" { ".exe" } else { "" };
        let asset = artifact_dir.join(format!(
            "mdtablefix-{}-{}{extension}",
            target.os, target.arch
        ));
        ensure!(asset.is_file(), "{} should stage {asset:?}", target.triple);
        let sidecar = artifact_dir.join(format!("{}.sha256", asset_name(&asset)?));
        let recorded = std::fs::read_to_string(&sidecar).context("read the checksum sidecar")?;
        let named = recorded
            .split_whitespace()
            .nth(1)
            .context("sidecar should name its asset")?;
        ensure!(
            named == asset_name(&asset)?,
            "the sidecar should name the asset alone, got {named:?}"
        );
    }
    Ok(())
}

fn asset_name(path: &Path) -> Result<&str> {
    path.file_name()
        .and_then(|name| name.to_str())
        .context("asset should have a UTF-8 file name")
}

#[test]
fn archives_are_reproducible_for_a_fixed_timestamp() -> Result<()> {
    let staging = tempfile::tempdir().context("create the staging root")?;
    let target = &RELEASE_TARGETS[0];

    let first = staging.path().join("first");
    let second = staging.path().join("second");
    let later = staging.path().join("later");
    stage(target, &first, "1700000000")?;
    stage(target, &second, "1700000000")?;
    stage(target, &later, "1800000000")?;

    // Locate the archive by extension rather than by name: the name carries
    // the manifest version, which changes on every release.
    let read = |dir: &Path| -> Result<Vec<u8>> {
        let path = std::fs::read_dir(dir)
            .with_context(|| format!("list {dir:?}"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.to_string_lossy().ends_with(".tar.gz"))
            .with_context(|| format!("{dir:?} should hold one cargo-binstall archive"))?;
        std::fs::read(&path).with_context(|| format!("read {path:?}"))
    };
    ensure!(
        read(&first)? == read(&second)?,
        "two runs at the same timestamp should produce identical archives"
    );
    ensure!(
        read(&first)? != read(&later)?,
        "the member timestamp should come from SOURCE_DATE_EPOCH"
    );
    Ok(())
}

#[test]
fn continuous_integration_runs_the_packaging_dry_run_on_every_platform() -> Result<()> {
    let workflow = parse(CI_WORKFLOW)?;
    let rows = matrix_rows(&workflow, "binstall-packaging")?;
    for family in ["ubuntu", "macos", "windows"] {
        ensure!(
            rows.iter().any(|row| {
                get_string(row, "runner").is_some_and(|runner| runner.starts_with(family))
            }),
            "the packaging dry run should cover a {family} runner"
        );
    }
    Ok(())
}
