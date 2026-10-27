//! INV-LIB: the reusable library gains no dependency on `ortho_config`.
//!
//! The manifest no longer separates the two targets — `ortho_config` is a
//! normal dependency of the package, so it is linked into the library as well
//! as the binary. What keeps the library reusable is therefore a *source*
//! boundary rather than a Cargo one: no file reachable from `src/lib.rs` may
//! name the framework. `src/config.rs` and its tests do, and `src/main.rs`
//! declares them, so they belong to the binary alone.
//!
//! The check is textual rather than a module-graph walk, so it also covers an
//! `include!` target or a path spelled inside a macro. A file that names the
//! framework anywhere at all is reported, which is stricter than "uses" and
//! cheap to keep true.
//!
//! Non-vacuity has two halves. The scan must *find* the framework in the
//! binary-only files, so a broken detector cannot pass by reporting nothing;
//! and the manifest must still declare the dependency, so the contract cannot
//! pass on a tree where the adoption never happened.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

/// The dependency whose absence from the library is the invariant.
const FRAMEWORK: &str = "ortho_config";

/// The source files allowed to name [`FRAMEWORK`], written with `/`.
///
/// Both are declared from `src/main.rs` and from nowhere else: `config_tests`
/// is a `#[path]` module of `config`, so it follows that file's ownership.
const BINARY_ONLY: [&str; 2] = ["src/config.rs", "src/config_tests.rs"];

/// The repository root, as the test binary was compiled against it.
fn repository_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// Reads a repository file as text.
fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

/// Collects every Rust source beneath `root`, excluding `target` and dotfiles.
///
/// A breadth-first walk written against `std::path`, so it behaves the same way
/// on every platform the CI matrix covers. A directory that cannot be read is
/// fatal rather than skipped: an incomplete walk could miss a library source
/// that names the framework, and so satisfy the invariant for the wrong reason.
fn rust_sources(root: &Path) -> Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .with_context(|| format!("listing {}", directory.display()))?;
        for entry in entries {
            let entry =
                entry.with_context(|| format!("reading an entry of {}", directory.display()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let path = entry.path();
            if path.is_dir() {
                if !name.starts_with('.') && name != "target" {
                    pending.push(path);
                }
            } else if name.ends_with(".rs") {
                found.push(path);
            }
        }
    }
    found.sort();

    Ok(found)
}

/// Renders `path` relative to `root`, with `/` separators.
///
/// Components are compared rather than a rendered string, because the walk joins
/// with the platform separator: on Windows every path reads `src\config.rs`, and
/// a string comparison would report every file as foreign while matching on
/// Linux.
fn relative(root: &Path, path: &Path) -> String {
    let suffix = path.strip_prefix(root).unwrap_or(path);
    suffix
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Returns the sources, of `files`, that name `needle`.
///
/// Split from the filesystem walk so the detector can be proved to fire on a
/// synthetic list, without editing a real source file.
fn offenders(files: &[(String, String)], needle: &str) -> Vec<String> {
    files
        .iter()
        .filter(|(_, contents)| contents.contains(needle))
        .map(|(path, _)| path.clone())
        .collect()
}

/// No library source names the framework.
#[test]
fn no_library_source_names_the_framework() -> Result<()> {
    let root = repository_root();
    let sources = rust_sources(&root.join("src"))?;
    ensure!(
        sources.len() > 20,
        "the walk reached only {} sources, so it is not seeing the tree",
        sources.len()
    );

    let files: Vec<(String, String)> = sources
        .iter()
        .map(|path| Ok((relative(&root, path), read(path)?)))
        .collect::<Result<_>>()?;
    let naming = offenders(&files, FRAMEWORK);

    // The witness: the framework is present in exactly the binary-only files,
    // so a detector that found nothing anywhere would fail here rather than
    // silently satisfying the invariant below.
    ensure!(
        !naming.is_empty(),
        "no source names {FRAMEWORK}, so the scan proves nothing"
    );
    for path in &naming {
        ensure!(
            BINARY_ONLY.contains(&path.as_str()),
            "{path} names {FRAMEWORK} but is not one of the binary-only sources"
        );
    }

    Ok(())
}

/// The library declares neither of the binary-only modules.
///
/// The textual scan above is only sound if `src/lib.rs` cannot reach those
/// files, so the reachability half of the invariant is asserted directly rather
/// than inferred from the allowlist.
#[test]
fn the_library_declares_no_binary_only_module() -> Result<()> {
    let root = repository_root();
    let library = read(&root.join("src/lib.rs"))?;

    for module in ["config", "config_tests"] {
        ensure!(
            !library.contains(&format!("mod {module};")),
            "src/lib.rs declares `mod {module};`, which would link {FRAMEWORK} into the library"
        );
    }

    Ok(())
}

/// The manifest still declares the framework, so the contract is not vacuous.
#[test]
fn the_manifest_declares_the_framework() -> Result<()> {
    let manifest = read(&repository_root().join("Cargo.toml"))?;

    ensure!(
        manifest.contains(&format!("{FRAMEWORK} = ")),
        "the manifest no longer declares {FRAMEWORK}, so this contract proves nothing"
    );

    Ok(())
}

/// The detector reports a library source that names the framework.
///
/// The negative control for [`no_library_source_names_the_framework`]: a
/// synthetic list states the shape a future contributor's import would take, and
/// the detector must name it. Without this, a `contains` that never matched
/// would pass every assertion above.
#[test]
fn the_detector_names_a_source_that_uses_the_framework() {
    let files = vec![
        (
            String::from("src/wrap.rs"),
            String::from("use regex::Regex;"),
        ),
        (
            String::from("src/table.rs"),
            format!("use {FRAMEWORK}::OrthoConfig;"),
        ),
    ];

    assert_eq!(
        offenders(&files, FRAMEWORK),
        vec![String::from("src/table.rs")]
    );
}
