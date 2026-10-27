//! INV-LIB: the reusable library gains no dependency on `ortho_config`.
//!
//! The manifest no longer separates the two targets — `ortho_config` is a
//! normal dependency of the package, so it is linked into the library as well
//! as the binary. What keeps the library reusable is therefore a *source*
//! boundary rather than a Cargo one: no file reachable from `src/lib.rs` may
//! name the framework. `src/config.rs` and its tests do, and `src/main.rs`
//! declares them, so they belong to the binary alone.
//!
//! The first check is textual, so it also covers an `include!` target or a path
//! spelled inside a macro. A file that names the framework anywhere at all is
//! reported, which is stricter than "uses" and cheap to keep true.
//!
//! The second check is structural, because text alone cannot see a module
//! reached under another name. `#[path = "config.rs"] mod settings;` declares no
//! `mod config;` and names no file, yet it links the framework into the library
//! exactly as `mod config;` would. So the reachability half walks the module
//! graph in `tests/support/module_graph.rs`, following `mod`, `#[path]` and
//! `include!` from `src/lib.rs`.
//!
//! Non-vacuity has two halves. The scan must *find* the framework in the
//! binary-only files, so a broken detector cannot pass by reporting nothing;
//! and the manifest must still declare the dependency, so the contract cannot
//! pass on a tree where the adoption never happened.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};

// The path is relative to this file's own directory, `tests/`.
#[path = "support/module_graph.rs"]
mod module_graph;

/// The dependency whose absence from the library is the invariant.
const FRAMEWORK: &str = "ortho_config";

/// The source files allowed to name [`FRAMEWORK`], written with `/`.
///
/// Every one is declared from `src/main.rs` and from nowhere else. The test
/// modules are `#[path]` modules of `config`, so they follow that file's
/// ownership and cannot be reached from the library either.
const BINARY_ONLY: [&str; 5] = [
    "src/config.rs",
    "src/config_schema.rs",
    "src/config_error_tests.rs",
    "src/config_test_support.rs",
    "src/config_tests.rs",
];

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

/// No file reachable from the library is one of the binary-only sources.
///
/// The textual scan above is only sound if `src/lib.rs` cannot reach those
/// files, so the reachability half is asserted by walking the module graph
/// rather than by matching a declaration's text. Text cannot see
/// `#[path = "config.rs"] mod settings;`, which reaches the same file under a
/// name no literal check would recognize, so the walk follows `mod`, `#[path]`
/// and `include!` alike.
#[test]
fn the_library_reaches_no_binary_only_source() -> Result<()> {
    let root = repository_root();
    let reached = module_graph::reachable(
        Utf8Path::from_path(&root.join("src/lib.rs"))
            .context("src/lib.rs is not representable as UTF-8, so the walk has nothing to read")?,
    )?;

    // The witness: the walk must reach the library's own sources, or an empty
    // result would satisfy the assertion below for the wrong reason.
    ensure!(
        reached.len() > 20,
        "the walk reached only {} sources, so it is not following declarations",
        reached.len()
    );
    for path in &reached {
        let relative = relative(&root, path.as_std_path());
        ensure!(
            !BINARY_ONLY.contains(&relative.as_str()),
            "src/lib.rs reaches {relative}, which belongs to the binary"
        );
    }

    Ok(())
}

/// The walk sees a module reached both ways, so neither rule can rot alone.
///
/// The negative control for [`the_library_reaches_no_binary_only_source`]: a
/// synthetic file states the two spellings a contributor could use, and the
/// walk must name the same target for both. Without this, a resolver that
/// silently dropped `#[path]` would pass every assertion above.
#[test]
fn the_walk_follows_both_declaration_spellings() -> Result<()> {
    let declaring = Utf8Path::new("src/example.rs");
    let source = concat!(
        "mod config;\n",
        "#[path = \"config.rs\"]\nmod settings;\n",
        "mod inline {\n",
        "    mod nested;\n",
        "}\n",
    );

    assert_eq!(
        module_graph::declared_targets(source, declaring)?,
        vec![
            vec![
                Utf8PathBuf::from("src/example/config.rs"),
                Utf8PathBuf::from("src/example/config/mod.rs")
            ],
            vec![Utf8PathBuf::from("src/config.rs")],
            vec![
                Utf8PathBuf::from("src/example/inline/nested.rs"),
                Utf8PathBuf::from("src/example/inline/nested/mod.rs"),
            ],
        ],
        "an implicit, a path-qualified and an inline-nested declaration must each resolve"
    );

    Ok(())
}

/// The walk refuses a macro that could declare a module it cannot see.
///
/// The negative control for the gap `#[path]` resolution does not close: a
/// macro invocation expands after parsing, so `sneak!()` emitting
/// `#[path = "config_schema.rs"] mod settings;` reaches a binary-only file while
/// every file the walk reads stays clean. The walk must therefore report the
/// invocation rather than skipping it, and must not report an ordinary call
/// that merely mentions modules in a string.
#[test]
fn the_walk_refuses_an_unanalysable_module_declaration() {
    let declaring = Utf8Path::new("src/example.rs");
    let refusing = concat!(
        "macro_rules! sneak {\n",
        "    () => { #[path = \"config_schema.rs\"] mod settings; };\n",
        "}\n",
        "sneak!();\n",
    );
    let error = module_graph::declared_targets(refusing, declaring)
        .expect_err("a macro that writes a module must be refused, not skipped");
    assert!(
        error.to_string().contains("sneak"),
        "the diagnostic should name the macro, got: {error}"
    );

    let ordinary = concat!(
        "fn check() {\n",
        "    assert_eq!(anyhow!(\"no mod here\").to_string(), \"no mod here\");\n",
        "}\n",
    );
    assert_eq!(
        module_graph::declared_targets(ordinary, declaring)
            .expect("an invocation mentioning no module is not an offence"),
        Vec::<module_graph::Candidates>::new()
    );
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
