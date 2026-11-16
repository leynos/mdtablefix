//! End-to-end proof that the environment-access lint actually fires.
//!
//! `tests/env_access_policy.rs` reads the configuration and asserts its shape.
//! That catches a weakened `clippy.toml` or a downgraded lint level, but a
//! configuration can parse correctly and still lint nothing: a mistyped path, a
//! `disallowed-methods` table Clippy no longer honours, or a configuration file
//! Clippy never loads would all satisfy it.
//!
//! This test closes that gap by running Clippy over a fixture package that
//! calls all six prohibited methods, with `CLIPPY_CONF_DIR` pointed at this
//! repository so the real `clippy.toml` is the one under test. It asserts one
//! diagnostic per method, each carrying the reason string the configuration
//! gives it. Delete an entry from `clippy.toml` and the count drops and the
//! test fails; the configuration-shape test would not notice a path that
//! Clippy silently ignores.
//!
//! The fixture is its own workspace root with no dependencies, so the
//! repository's own gates never build it and the Clippy run has no dependency
//! graph to resolve. It is materialized under `target/` rather than committed as
//! a package, so no repository-wide scan or `cargo package` run sees a second
//! manifest in the tree.
//!
//! The child environment is built explicitly with `Command::env`, per the
//! policy this test defends: the parent test process's environment is never
//! mutated. `CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` are redirected into
//! the fixture's own directory so the fixture build cannot collide with, or
//! leave anything in, the repository's own build trees. The directory is fixed
//! rather than temporary, so a later run reuses the previous run's build and
//! the nextest setup script (`.config/nextest.toml`) fills it before any
//! deadline starts.
//!
//! Mutation proof (2026-09-07). Each mutation was applied alone and reverted:
//!
//! ```text
//! delete the std::env::vars_os entry from clippy.toml
//!   -> std::env::vars_os should be reported once, found 0 diagnostics
//! replace the std::env::set_var entry with a bare path, dropping its reason
//!   -> the std::env::set_var diagnostic should carry the reason
//!      `use a stub environment in tests`
//! ```
//!
//! The second mutation matters because a bare path still bans the method. Only
//! the per-diagnostic reason assertion notices that the contributor has lost
//! the sentence telling them which seam to use.

use std::process::Command;

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// The fixture crate's source, stored as text so nothing compiles it in place.
const FIXTURE_SOURCE: &str = include_str!("data/env_access/prohibited.rs.txt");

/// The toolchain this repository pins, applied to the fixture so the Clippy
/// under test is the same one the repository's gates run.
const TOOLCHAIN: &str = include_str!("../rust-toolchain.toml");

/// Each prohibited method and the reason `clippy.toml` gives for banning it.
///
/// The reasons are asserted, not just the paths: a reason string is what tells
/// a contributor which seam to reach for, so losing it degrades the diagnostic
/// even while the method stays banned.
const EXPECTED_DIAGNOSTICS: [(&str, &str); 7] = [
    ("std::env::var", "inject an environment reader"),
    ("std::env::var_os", "inject an environment reader"),
    ("std::env::vars", "inject an environment reader"),
    ("std::env::vars_os", "inject an environment reader"),
    ("std::env::set_var", "use a stub environment in tests"),
    ("std::env::remove_var", "use a stub environment in tests"),
    (
        "std::env::set_current_dir",
        "process-wide working directory; pass the directory in or use Command::current_dir",
    ),
];

/// The manifest for the fixture package.
///
/// The empty `[workspace]` table makes it its own workspace root, so it is
/// never drawn into this repository's build, and it declares no dependencies,
/// so Clippy has nothing to resolve or compile beyond the one crate.
const FIXTURE_MANIFEST: &str = concat!(
    "[package]\n",
    "name = \"env-access-fixture\"\n",
    "version = \"0.0.0\"\n",
    "edition = \"2024\"\n",
    "publish = false\n",
    "\n",
    "[workspace]\n",
    "\n",
    "[lib]\n",
    "path = \"lib.rs\"\n",
);

/// The repository root, which is the directory holding `clippy.toml`.
fn repository_root() -> Utf8PathBuf { Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// Writes `contents` to `name` only when the file differs from it.
///
/// The fixture directory is fixed, so two test processes in one checkout share
/// it. Leaving an unchanged file alone means a second process never rewrites
/// what the first is reading, and the file's modification time stays put, so
/// Cargo keeps the fixture's build instead of treating it as edited.
fn write_if_changed(directory: &Dir, name: &str, contents: &str) -> Result<()> {
    if directory
        .read_to_string(name)
        .is_ok_and(|current| current == contents)
    {
        return Ok(());
    }
    directory
        .write(name, contents)
        .with_context(|| format!("write the fixture file {name}"))
}

/// Materialize the fixture package and run Clippy over it.
///
/// The package and its build products live in one fixed directory under this
/// repository's own `target/`, so a second run finds the first run's build and
/// reuses it rather than compiling again inside its deadline. The directory is
/// separate from the repository's own build trees, and `cargo clean` removes it.
fn lint_the_fixture() -> Result<String> {
    let scratch = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/env-access-fixture");
    let root = scratch.join("package");
    Dir::create_ambient_dir_all(&root, ambient_authority())
        .with_context(|| format!("create the fixture directory {root}"))?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .with_context(|| format!("open the fixture directory {root}"))?;
    write_if_changed(&handle, "Cargo.toml", FIXTURE_MANIFEST)?;
    write_if_changed(&handle, "lib.rs", FIXTURE_SOURCE)?;
    write_if_changed(&handle, "rust-toolchain.toml", TOOLCHAIN)?;

    // `env!("CARGO")` is the compile-time path to the Cargo that built this
    // test. Reading `CARGO` from the process at run time would trip the very
    // lint under test, which is a neat demonstration of the policy.
    let output = Command::new(env!("CARGO"))
        .arg("clippy")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        // The configuration under test is this repository's own.
        .env("CLIPPY_CONF_DIR", repository_root())
        // Keep the fixture's build products in the fixture's own directory.
        .env("CARGO_TARGET_DIR", scratch.join("target"))
        .env("CARGO_BUILD_BUILD_DIR", scratch.join("build"))
        // `make test` exports `RUSTFLAGS=-D warnings`, which would turn the
        // fixture's own unrelated warnings into errors and truncate the run
        // before every method is reported. The coverage job wraps the suite in
        // `cargo llvm-cov`, which sets the rest of these; inheriting them would
        // instrument a fixture nothing measures and write its profile data over
        // the run's own.
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTDOCFLAGS")
        .env_remove("CARGO_ENCODED_RUSTDOCFLAGS")
        .env_remove("LLVM_PROFILE_FILE")
        .env_remove("CLIPPY_ARGS")
        // The fixture has no dependencies, so nothing here may reach the
        // network or wait on the index.
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .context("run Clippy over the fixture package")?;

    String::from_utf8(output.stderr).context("Clippy output should be UTF-8")
}

/// The marker opening every disallowed-method diagnostic.
const DIAGNOSTIC_MARKER: &str = "use of a disallowed method";

/// Split Clippy's output into one slice per disallowed-method diagnostic.
///
/// Each slice runs from its own marker to the next one, so a reason found
/// inside a slice belongs to that diagnostic rather than to a neighbour. A
/// whole-output search would let one surviving reason string vouch for all six.
fn diagnostics(stderr: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = stderr;
    while let Some(start) = rest.find(DIAGNOSTIC_MARKER) {
        rest = &rest[start..];
        let end = rest[DIAGNOSTIC_MARKER.len()..]
            .find(DIAGNOSTIC_MARKER)
            .map_or(rest.len(), |offset| offset + DIAGNOSTIC_MARKER.len());
        found.push(&rest[..end]);
        rest = &rest[end..];
    }
    found
}

/// Scenario: Clippy lints a package that calls all six prohibited methods,
/// under this repository's own `clippy.toml`.
/// Invariant: each method is reported exactly once, each diagnostic carries the
/// reason the configuration gives it, and there are exactly six in total. An
/// entry silently ignored by Clippy, or one that has lost its reason string,
/// fails here even though the configuration still has the right shape.
///
/// The six methods are checked in one test rather than one case each because
/// the suite runs under `cargo nextest`, which gives every case its own
/// process. Six cases would mean six Clippy runs of the same fixture.
#[test]
fn the_lint_fires_for_every_prohibited_method() -> Result<()> {
    let stderr = lint_the_fixture()?;
    let reported = diagnostics(&stderr);
    for (method, reason) in EXPECTED_DIAGNOSTICS {
        // The closing backtick keeps `std::env::var` from matching
        // `std::env::var_os` or `std::env::vars`.
        let named = format!("`{method}`");
        let matched: Vec<&&str> = reported
            .iter()
            .filter(|diagnostic| diagnostic.contains(&named))
            .collect();
        ensure!(
            matched.len() == 1,
            "{method} should be reported once, found {} diagnostics in:\n{stderr}",
            matched.len()
        );
        ensure!(
            matched[0].contains(reason),
            "the {method} diagnostic should carry the reason `{reason}`, found:\n{}",
            matched[0]
        );
    }
    ensure!(
        reported.len() == EXPECTED_DIAGNOSTICS.len(),
        "expected {} disallowed-method diagnostics, found {} in:\n{stderr}",
        EXPECTED_DIAGNOSTICS.len(),
        reported.len()
    );
    Ok(())
}
