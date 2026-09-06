//! Contract coverage for the environment-access lint policy.
//!
//! The policy has three parts and no single file holds all of them, so nothing
//! in the build connects them: `clippy.toml` names the prohibited methods, each
//! package manifest raises `clippy::disallowed_methods` to `deny`, and the
//! Makefile's `lint` recipe runs Clippy over both packages, every target, and
//! every feature with warnings denied. Drop any one part and the other two
//! still look correct while the policy stops being enforced. These tests tie
//! the three together.
//!
//! The Makefile check parses the `lint` recipe and expands its Make variables
//! rather than searching the file for a command string. A matching string in a
//! comment, in another target, or covering only the root package would satisfy
//! a file-wide search while the gate no longer enforces the policy.
//!
//! The policy itself is recorded in
//! `docs/adrs/0006-environment-seam-taxonomy.md`.
//!
//! Repository files are pulled in with `include_str!`, so deleting one is a
//! compile error rather than a silent skip, and the tests need no filesystem
//! access of their own.
//!
//! Mutation proof (2026-09-06). Each mutation was applied alone, the suite run,
//! and the mutation reverted. Every one failed, with the message shown:
//!
//! ```text
//! delete the std::env::set_var entry from clippy.toml
//!   -> clippy_configuration_disallows_every_environment_method
//!      clippy.toml must disallow std::env::set_var, found [...]
//! change the root manifest's lint level from "deny" to "warn"
//!   -> every_package_denies_disallowed_methods
//!      Cargo.toml must set clippy disallowed_methods to deny, found Some("warn")
//! delete -D warnings from CLIPPY_FLAGS
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      the Clippy command [...] must contain -D warnings
//! delete --all-targets from CLIPPY_FLAGS
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      the Clippy command [...] must contain --all-targets
//! delete the --manifest-path test-macros/Cargo.toml command from the recipe
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      the lint target must run Clippy over test-macros/Cargo.toml, found [...]
//! comment out both Clippy commands in the recipe
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      the lint target should have a recipe
//! ```
//!
//! Enforcement itself was proven separately, in each package: a temporary
//! `std::env::var` call in `src/lib.rs`, and another in
//! `test-macros/src/lib.rs`, each failed `make lint` with "use of a disallowed
//! method" and the configured reason string. Both were reverted.
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

/// Return `name`'s value from a simple Make assignment (`=`, `?=`, or `:=`).
///
/// Recursive and conditional assignments differ in when Make expands them, not
/// in the text they hold, so one reader covers all three.
fn make_assignment<'a>(makefile: &'a str, name: &str) -> Option<&'a str> {
    makefile.lines().find_map(|line| {
        let rest = line.strip_prefix(name)?.trim_start();
        for operator in ["?=", ":=", "="] {
            if let Some(value) = rest.strip_prefix(operator) {
                return Some(value.trim());
            }
        }
        None
    })
}

/// Expand `$(NAME)` references against the Makefile's own assignments.
///
/// Two passes cover a variable defined in terms of another, which is as deep as
/// this Makefile goes. An unresolved reference is left intact so it shows up in
/// a failure message rather than vanishing.
fn expand_make_variables(makefile: &str, command: &str) -> String {
    let mut expanded = command.to_owned();
    for _ in 0..2 {
        let mut next = String::with_capacity(expanded.len());
        let mut rest = expanded.as_str();
        while let Some(start) = rest.find("$(") {
            let Some(end) = rest[start..].find(')').map(|offset| start + offset) else {
                break;
            };
            next.push_str(&rest[..start]);
            let name = &rest[start + 2..end];
            match make_assignment(makefile, name) {
                Some(value) => next.push_str(value),
                None => next.push_str(&rest[start..=end]),
            }
            rest = &rest[end + 1..];
        }
        next.push_str(rest);
        expanded = next;
    }
    expanded
}

/// Return the recipe lines of `target`, with Make variables expanded.
///
/// A recipe line is tab-indented; the recipe ends at the first line that is
/// neither tab-indented nor blank. Comment lines are dropped, so a commented-out
/// command cannot satisfy a coverage requirement.
fn recipe_commands(makefile: &str, target: &str) -> Result<Vec<String>> {
    let prefix = format!("{target}:");
    let body = makefile
        .lines()
        .skip_while(|line| !line.starts_with(&prefix))
        .skip(1);
    let mut commands = Vec::new();
    for line in body {
        let Some(command) = line.strip_prefix('\t') else {
            if line.trim().is_empty() {
                continue;
            }
            break;
        };
        let command = command.trim_start_matches(['@', '-', '+']).trim();
        if command.is_empty() || command.starts_with('#') {
            continue;
        }
        commands.push(expand_make_variables(makefile, command));
    }
    ensure!(
        !commands.is_empty(),
        "the {target} target should have a recipe"
    );
    Ok(commands)
}

/// Return whether `command` lints `manifest`'s package.
///
/// A `--workspace` run covers every member, so it covers both packages once
/// issue #439 lands. Until then the root package is the one a command with no
/// `--manifest-path` selects, and `test-macros` needs its manifest named.
fn covers_package(command: &str, manifest: &str) -> bool {
    if command.contains("--workspace") {
        return true;
    }
    match manifest {
        "Cargo.toml" => !command.contains("--manifest-path"),
        _ => command.contains(&format!("--manifest-path {manifest}")),
    }
}

/// Scenario: the `lint` target's recipe is read and its Make variables expanded.
/// Invariant: every Clippy command it runs denies warnings across all targets
/// and features, and between them they cover both packages, so neither a
/// weakened flag set nor a dropped package can pass the gate. Text elsewhere in
/// the Makefile, including a commented-out command, does not count.
#[test]
fn clippy_gate_denies_warnings_across_targets_and_features() -> Result<()> {
    let commands: Vec<String> = recipe_commands(MAKEFILE, "lint")?
        .into_iter()
        .filter(|command| command.contains(" clippy "))
        .collect();
    ensure!(
        !commands.is_empty(),
        "the lint target must invoke Cargo Clippy"
    );
    for command in &commands {
        for required in ["--all-targets", "--all-features", "-D warnings"] {
            ensure!(
                command.contains(required),
                "the Clippy command `{command}` must contain {required}"
            );
        }
    }
    for (manifest, _) in PACKAGE_MANIFESTS {
        ensure!(
            commands
                .iter()
                .any(|command| covers_package(command, manifest)),
            "the lint target must run Clippy over {manifest}, found {commands:?}"
        );
    }
    Ok(())
}
