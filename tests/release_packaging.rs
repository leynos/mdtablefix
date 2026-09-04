//! Ties the release packaging to the crate's `cargo-binstall` metadata.
//!
//! `cargo binstall` downloads the asset named by `pkg-url` and then extracts
//! the member named by `bin-dir`. Both templates live in `Cargo.toml`, while
//! the archive is built by `scripts/package_release_artifacts.py`. Nothing in
//! the build connects the two, so these tests stage real archives for every
//! published target and check them against the rendered templates.
//!
//! Paths are [`camino`] UTF-8 types and every filesystem operation goes
//! through a [`cap_std::fs_utf8::Dir`] capability scoped to the staging root,
//! so nothing here can read or write outside the temporary directory it owns.
//! [`TempDir`] still provides that directory and [`Command`] still runs the
//! packaging scripts, which need real paths.

use std::process::Command;

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use serde_yaml::{Mapping, Value};
use tempfile::TempDir;

const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release.yml");
const CI_WORKFLOW: &str = include_str!("../.github/workflows/ci.yml");
const PACKAGE_SCRIPT: &str = "scripts/package_release_artifacts.py";
const VERIFY_SCRIPT: &str = "scripts/verify_binstall_layout.py";
const ARCHIVE_SUFFIX: &str = ".tar.gz";

/// A target whose release assets `cargo binstall` is expected to resolve.
struct ReleaseTarget {
    triple: &'static str,
    os: &'static str,
    arch: &'static str,
    /// The runner image this target must be built on.
    runner: &'static str,
    /// `cross` for targets needing a foreign libc, `cargo` for native builds.
    builder: &'static str,
    /// The archive member `bin-dir` must render to for this target.
    binary_name: &'static str,
}

/// Every target the release publishes an archive for.
///
/// `cross` cannot emit Apple or MSVC binaries from Linux, so those targets
/// build on their own runner images. macOS runners are Apple silicon, which is
/// why the Intel target is a cross-compile there rather than a native build.
const RELEASE_TARGETS: &[ReleaseTarget] = &[
    ReleaseTarget {
        triple: "x86_64-unknown-linux-gnu",
        os: "linux",
        arch: "x86_64",
        runner: "ubuntu-latest",
        builder: "cross",
        binary_name: "mdtablefix",
    },
    ReleaseTarget {
        triple: "aarch64-unknown-linux-gnu",
        os: "linux",
        arch: "aarch64",
        runner: "ubuntu-latest",
        builder: "cross",
        binary_name: "mdtablefix",
    },
    ReleaseTarget {
        triple: "x86_64-apple-darwin",
        os: "macos",
        arch: "x86_64",
        runner: "macos-15",
        builder: "cargo",
        binary_name: "mdtablefix",
    },
    ReleaseTarget {
        triple: "aarch64-apple-darwin",
        os: "macos",
        arch: "aarch64",
        runner: "macos-15",
        builder: "cargo",
        binary_name: "mdtablefix",
    },
    ReleaseTarget {
        triple: "x86_64-pc-windows-msvc",
        os: "windows",
        arch: "x86_64",
        runner: "windows-latest",
        builder: "cargo",
        binary_name: "mdtablefix.exe",
    },
    ReleaseTarget {
        triple: "x86_64-unknown-freebsd",
        os: "freebsd",
        arch: "x86_64",
        runner: "ubuntu-latest",
        builder: "cross",
        binary_name: "mdtablefix",
    },
];

/// A staging root with a capability scoped to it.
///
/// The scripts under test are external processes, so they need the ambient
/// path; the test's own reads and writes go through the capability.
struct Staging {
    _root: TempDir,
    path: Utf8PathBuf,
    dir: Dir,
}

impl Staging {
    fn new() -> Result<Self> {
        let root = TempDir::new().context("create the staging root")?;
        let path = Utf8Path::from_path(root.path())
            .context("staging root path should be UTF-8")?
            .to_owned();
        let dir = Dir::open_ambient_dir(&path, ambient_authority())
            .context("open the staging root capability")?;
        Ok(Self {
            _root: root,
            path,
            dir,
        })
    }

    /// Stage a stand-in binary for `target` under `subdirectory`.
    ///
    /// The payload only has to be a file: the templates under test describe
    /// names and archive layout, not machine code.
    fn stage(
        &self,
        target: &ReleaseTarget,
        subdirectory: &str,
        source_date_epoch: &str,
    ) -> Result<Utf8PathBuf> {
        let stub = format!("{}.stub", target.triple);
        self.dir
            .write(&stub, b"stand-in release binary\n")
            .context("write the stand-in binary")?;
        run(
            python()
                .arg(PACKAGE_SCRIPT)
                .arg("--binary")
                .arg(self.path.join(&stub))
                .arg("--artifact-dir")
                .arg(self.path.join(subdirectory))
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
        Ok(self.path.join(subdirectory))
    }

    /// Return the sole archive name staged under `subdirectory`.
    fn archive_name(&self, subdirectory: &str) -> Result<String> {
        // Locate the archive by extension rather than by name: the name
        // carries the manifest version, which changes on every release.
        let mut names = Vec::new();
        for entry in self
            .dir
            .read_dir(subdirectory)
            .with_context(|| format!("list {subdirectory}"))?
        {
            let name = entry
                .context("read a staged directory entry")?
                .file_name()
                .context("staged file name should be UTF-8")?;
            if name.ends_with(ARCHIVE_SUFFIX) {
                names.push(name);
            }
        }
        ensure!(
            names.len() == 1,
            "{subdirectory} should hold exactly one archive, found {names:?}"
        );
        Ok(names.remove(0))
    }

    fn read(&self, relative: &str) -> Result<Vec<u8>> {
        self.dir
            .read(relative)
            .with_context(|| format!("read {relative}"))
    }

    fn read_to_string(&self, relative: &str) -> Result<String> {
        self.dir
            .read_to_string(relative)
            .with_context(|| format!("read {relative}"))
    }

    fn is_file(&self, relative: &str) -> bool { self.dir.is_file(relative) }
}

fn python() -> Command {
    let mut command = Command::new("python3");
    command.current_dir(env!("CARGO_MANIFEST_DIR"));
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
fn the_release_matrix_and_the_published_targets_agree() -> Result<()> {
    let workflow = parse(RELEASE_WORKFLOW)?;
    let rows = matrix_rows(&workflow, "build")?;

    // Every row publishes an archive, so a target present in one list and
    // absent from the other is a packaging gap either way round.
    let mut built: Vec<&str> = rows
        .iter()
        .filter_map(|row| get_string(row, "target"))
        .collect();
    built.sort_unstable();
    let mut published: Vec<&str> = RELEASE_TARGETS.iter().map(|entry| entry.triple).collect();
    published.sort_unstable();
    ensure!(
        built == published,
        "the release matrix builds {built:?} but the published set is {published:?}"
    );

    for target in RELEASE_TARGETS {
        let row = rows
            .iter()
            .find(|row| get_string(row, "target") == Some(target.triple))
            .with_context(|| format!("build matrix should cover {}", target.triple))?;
        ensure!(
            get_string(row, "os") == Some(target.os)
                && get_string(row, "arch") == Some(target.arch),
            "{} should keep its published asset naming",
            target.triple
        );
        ensure!(
            get_string(row, "runner") == Some(target.runner),
            "{} should build on {}",
            target.triple,
            target.runner
        );
        ensure!(
            get_string(row, "builder") == Some(target.builder),
            "{} should build with {}",
            target.triple,
            target.builder
        );
    }
    Ok(())
}

#[test]
fn staged_assets_match_the_binstall_templates() -> Result<()> {
    let staging = Staging::new()?;
    for target in RELEASE_TARGETS {
        let artifact_dir = staging.stage(target, target.triple, "1700000000")?;
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
    let staging = Staging::new()?;
    for target in RELEASE_TARGETS {
        staging.stage(target, target.triple, "1700000000")?;
        let extension = if target.os == "windows" { ".exe" } else { "" };
        let asset = format!("mdtablefix-{}-{}{extension}", target.os, target.arch);
        let relative = format!("{}/{asset}", target.triple);
        ensure!(
            staging.is_file(&relative),
            "{} should stage {asset}",
            target.triple
        );
        let recorded = staging.read_to_string(&format!("{relative}.sha256"))?;
        let named = recorded
            .split_whitespace()
            .nth(1)
            .context("sidecar should name its asset")?;
        ensure!(
            named == asset,
            "the sidecar should name the asset alone, got {named:?}"
        );
    }
    Ok(())
}

#[test]
fn archives_are_reproducible_for_a_fixed_timestamp() -> Result<()> {
    let staging = Staging::new()?;
    let target = &RELEASE_TARGETS[0];
    staging.stage(target, "first", "1700000000")?;
    staging.stage(target, "second", "1700000000")?;
    staging.stage(target, "later", "1800000000")?;

    let read_archive = |subdirectory: &str| -> Result<Vec<u8>> {
        let name = staging.archive_name(subdirectory)?;
        staging.read(&format!("{subdirectory}/{name}"))
    };
    ensure!(
        read_archive("first")? == read_archive("second")?,
        "two runs at the same timestamp should produce identical archives"
    );
    ensure!(
        read_archive("first")? != read_archive("later")?,
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
    // The Intel macOS target is a cross-compile on an Apple silicon runner, so
    // nothing else would notice if it stopped linking.
    ensure!(
        rows.iter()
            .any(|row| get_string(row, "target") == Some("x86_64-apple-darwin")),
        "the packaging dry run should build the Intel macOS target"
    );
    Ok(())
}
