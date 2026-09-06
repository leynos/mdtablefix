//! Contract coverage for the environment-access lint policy.
//!
//! The policy has three parts and no single file holds all of them, so nothing
//! in the build connects them: `clippy.toml` names the prohibited methods, each
//! package manifest raises `clippy::disallowed_methods` to `deny`, and the
//! Makefile's Clippy gate runs over every target and feature with warnings
//! denied. Drop any one part and the other two still look correct while the
//! policy stops being enforced. These tests tie the three together.
//!
//! The policy itself is recorded in
//! `docs/adrs/0006-environment-seam-taxonomy.md`.
//!
//! Repository files are pulled in with `include_str!`, so deleting one is a
//! compile error rather than a silent skip, and the tests need no filesystem
//! access of their own.
//!
//! Mutation proof (2026-09-06). Each mutation was applied alone, the suite run,
//! and the mutation reverted. Failures, in order:
//!
//! ```text
//! deleting the std::env::set_var entry from clippy.toml
//!   -> clippy_configuration_disallows_every_environment_method
//!      clippy.toml must disallow std::env::set_var, found [...]
//! changing the root manifest's lint level from "deny" to "warn"
//!   -> every_package_denies_disallowed_methods
//!      Cargo.toml must set clippy disallowed_methods to deny, found Some("warn")
//! deleting -D warnings from CLIPPY_FLAGS
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      CLIPPY_FLAGS must contain -D warnings, found [...]
//! ```

use anyhow::{Context, Result, ensure};
use toml::{Table, Value};

/// Every method the environment-access policy prohibits.
///
/// Reading the process environment is disallowed so behaviour that depends on
/// a variable takes it as an argument; mutating it is disallowed so tests stay
/// parallelizable.
const PROHIBITED_ENVIRONMENT_METHODS: [&str; 6] = [
    "std::env::var",
    "std::env::var_os",
    "std::env::vars",
    "std::env::vars_os",
    "std::env::set_var",
    "std::env::remove_var",
];

/// Each package manifest that must enforce the policy, with its source.
///
/// Both packages are listed by name because `test-macros` compiles its own
/// targets and so needs the lint level in its own right; issue #439 will make
/// them one workspace, at which point `[lints] workspace = true` becomes the
/// expected spelling and [`clippy_lint_level`] resolves it.
const PACKAGE_MANIFESTS: [(&str, &str); 2] = [
    ("Cargo.toml", include_str!("../Cargo.toml")),
    (
        "test-macros/Cargo.toml",
        include_str!("../test-macros/Cargo.toml"),
    ),
];

const CLIPPY_CONFIGURATION: &str = include_str!("../clippy.toml");
const ROOT_MANIFEST: &str = include_str!("../Cargo.toml");
const MAKEFILE: &str = include_str!("../Makefile");

/// Return the `path` field of every `disallowed-methods` entry in `clippy.toml`.
fn disallowed_method_paths(configuration: &str) -> Result<Vec<String>> {
    let configuration: Table = configuration
        .parse()
        .context("parse the Clippy configuration")?;
    let methods = configuration
        .get("disallowed-methods")
        .and_then(Value::as_array)
        .context("clippy.toml should declare disallowed-methods")?;
    Ok(methods
        .iter()
        .filter_map(|method| method.get("path").and_then(Value::as_str))
        .map(str::to_owned)
        .collect())
}

/// Return the level `manifest` gives a Clippy lint, following workspace
/// inheritance into `workspace_manifest` when the package opts in with
/// `[lints] workspace = true`.
///
/// A level may be spelled as a bare string or as a table with a `level` key,
/// so both are read.
fn clippy_lint_level(
    manifest: &str,
    workspace_manifest: &str,
    lint: &str,
) -> Result<Option<String>> {
    let manifest: Table = manifest.parse().context("parse the package manifest")?;
    let inherits = manifest
        .get("lints")
        .and_then(|lints| lints.get("workspace"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let table = if inherits {
        let workspace: Table = workspace_manifest
            .parse()
            .context("parse the workspace manifest")?;
        workspace
            .get("workspace")
            .and_then(|workspace| workspace.get("lints"))
            .and_then(|lints| lints.get("clippy"))
            .cloned()
    } else {
        manifest
            .get("lints")
            .and_then(|lints| lints.get("clippy"))
            .cloned()
    };
    Ok(table
        .as_ref()
        .and_then(|table| table.get(lint))
        .and_then(|entry| {
            entry
                .as_str()
                .or_else(|| entry.get("level").and_then(Value::as_str))
        })
        .map(str::to_owned))
}

/// Scenario: the Clippy configuration is read for its prohibited methods.
/// Invariant: every method the policy names is still listed, so removing one
/// cannot silently reopen ambient access to the process environment.
#[test]
fn clippy_configuration_disallows_every_environment_method() -> Result<()> {
    let paths = disallowed_method_paths(CLIPPY_CONFIGURATION)?;
    for required in PROHIBITED_ENVIRONMENT_METHODS {
        ensure!(
            paths.iter().any(|path| path == required),
            "clippy.toml must disallow {required}, found {paths:?}"
        );
    }
    Ok(())
}

/// Scenario: each package manifest is read for its `disallowed_methods` level.
/// Invariant: both packages deny the lint, whether declared in the package or
/// inherited from the workspace, so a configured-but-warned method cannot pass
/// the gate.
#[test]
fn every_package_denies_disallowed_methods() -> Result<()> {
    for (name, manifest) in PACKAGE_MANIFESTS {
        let level = clippy_lint_level(manifest, ROOT_MANIFEST, "disallowed_methods")
            .with_context(|| format!("read the Clippy lint level from {name}"))?;
        ensure!(
            level.as_deref() == Some("deny"),
            "{name} must set clippy disallowed_methods to deny, found {level:?}"
        );
    }
    Ok(())
}

/// Scenario: the Makefile's Clippy gate is read.
/// Invariant: it compiles every target and feature with warnings denied, and
/// `lint` actually invokes it, so the deny reaches test and benchmark targets
/// rather than the library alone.
#[test]
fn clippy_gate_denies_warnings_across_targets_and_features() -> Result<()> {
    let flags = MAKEFILE
        .lines()
        .find_map(|line| line.strip_prefix("CLIPPY_FLAGS ?="))
        .context("Makefile should define CLIPPY_FLAGS")?;
    for required in ["--all-targets", "--all-features", "-D warnings"] {
        ensure!(
            flags.contains(required),
            "CLIPPY_FLAGS must contain {required}, found `{flags}`"
        );
    }
    ensure!(
        MAKEFILE.contains("$(CARGO) clippy $(CLIPPY_FLAGS)"),
        "the lint target must invoke Cargo Clippy with CLIPPY_FLAGS"
    );
    Ok(())
}
