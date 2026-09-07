//! Contract coverage for the environment-access lint policy.
//!
//! The policy has three parts and no single file holds all of them, so nothing
//! in the build connects them: `clippy.toml` names the prohibited methods, each
//! package manifest raises `clippy::disallowed_methods` to `deny` and closes
//! the `#[allow]` route around it, and the Makefile's `lint` recipe runs Clippy
//! over both packages, every target, and every feature with warnings denied.
//! Drop any one part and the other two still look correct while the policy
//! stops being enforced. These tests tie the three together.
//!
//! They assert the policy's *shape*. `tests/env_access_enforcement.rs` asserts
//! that it *fires*, by running Clippy over a fixture package that calls all six
//! methods. Neither test subsumes the other: a configuration can have the right
//! shape and lint nothing, and a lint can fire while the gate that runs it has
//! stopped covering a package.
//!
//! The Makefile check parses the `lint` recipe rather than searching the file
//! for a command string, and judges each command as an invocation rather than
//! as text. A matching string in a comment, in another target, or covering only
//! the root package would satisfy a file-wide search while the gate no longer
//! enforces the policy; and a command such as `echo $(CARGO) clippy ...` would
//! satisfy a substring search for `clippy` while running no Clippy at all. So
//! the recipe is read as written, to judge the executable and the argument
//! order, and expanded afterwards, to judge the flags.
//!
//! The policy itself is recorded in
//! `docs/adrs/0006-environment-seam-taxonomy.md`.
//!
//! Repository files are pulled in with `include_str!`, so deleting one is a
//! compile error rather than a silent skip, and the tests need no filesystem
//! access of their own. The parsing lives in `tests/support/lint_policy.rs`;
//! the readers there are also exercised against inline fixtures at the end of
//! this file, so the spellings issues #438 and #439 introduce are covered
//! before the repository uses them.
//!
//! Mutation proof (2026-09-06). Each mutation was applied alone, the suite run,
//! and the mutation reverted. Every one failed, with the message shown:
//!
//! ```text
//! delete the std::env::set_var entry from clippy.toml
//!   -> clippy_configuration_disallows_every_environment_method
//!      clippy.toml must disallow std::env::set_var, found [...]
//! change the root manifest's disallowed_methods level from "deny" to "warn"
//!   -> every_package_denies_the_policy_lints
//!      Cargo.toml must set clippy disallowed_methods to deny, found Some("warn")
//! delete allow_attributes from the test-macros manifest
//!   -> every_package_denies_the_policy_lints
//!      test-macros/Cargo.toml must set clippy allow_attributes to deny,
//!      found None
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
//! prefix the test-macros command with `echo`, so it runs nothing
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      the lint target must run Clippy over test-macros/Cargo.toml, found [...]
//! prefix both commands with `echo`
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      the lint target must invoke Cargo Clippy
//! ```
//!
//! Enforcement itself was proven separately, in each package: a temporary
//! `std::env::var` call in `src/lib.rs`, and another in
//! `test-macros/src/lib.rs`, each failed `make lint` with "use of a disallowed
//! method" and the configured reason string. Adding a bare
//! `#[allow(clippy::disallowed_methods)]` above the first, the obvious way to
//! defeat the ban, failed `make lint` in its own right with "#[allow] attribute
//! found" and "`allow` attribute without specifying a reason". All were
//! reverted.
use anyhow::{Context, Result, ensure};
use rstest::rstest;

#[path = "support/lint_policy.rs"]
mod lint_policy;

use lint_policy::{
    clippy_lint_level,
    covers_package,
    disallowed_method_paths,
    expand_make_variables,
    is_cargo_clippy_invocation,
    recipe_commands,
};

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

/// The lints each package must deny for the policy to hold.
///
/// `disallowed_methods` is the ban itself. The other two close the obvious way
/// around it: without them a bare `#[allow(clippy::disallowed_methods)]`
/// silences the ban wherever it is written, with no reason recorded and no
/// warning when it stops applying. Denying them forces every suppression to be
/// an `#[expect]` carrying a reason, which is what the seam taxonomy requires
/// of a composition root.
const REQUIRED_DENIED_LINTS: [&str; 3] = [
    "disallowed_methods",
    "allow_attributes",
    "allow_attributes_without_reason",
];

/// Scenario: each package manifest is read for the level it gives each lint the
/// policy depends on.
/// Invariant: both packages deny all three, whether declared in the package or
/// inherited from the workspace, so neither a configured-but-warned method nor
/// a bare `#[allow]` can pass the gate.
#[test]
fn every_package_denies_the_policy_lints() -> Result<()> {
    for (name, manifest) in PACKAGE_MANIFESTS {
        for lint in REQUIRED_DENIED_LINTS {
            let level = clippy_lint_level(manifest, ROOT_MANIFEST, lint)
                .with_context(|| format!("read the {lint} level from {name}"))?;
            ensure!(
                level.as_deref() == Some("deny"),
                "{name} must set clippy {lint} to deny, found {level:?}"
            );
        }
    }
    Ok(())
}

/// Scenario: the `lint` target's recipe is read and its Make variables expanded.
/// Invariant: every Clippy command it runs denies warnings across all targets
/// and features, and between them they cover both packages, so neither a
/// weakened flag set nor a dropped package can pass the gate. Text elsewhere in
/// the Makefile, including a commented-out command, does not count.
#[test]
fn clippy_gate_denies_warnings_across_targets_and_features() -> Result<()> {
    // The recipe is read as written so the executable and the argument order can
    // be judged, then expanded so the flags can be. A command that only mentions
    // Clippy, such as one prefixed with `echo`, is filtered out here.
    let commands: Vec<String> = recipe_commands(MAKEFILE, "lint")?
        .iter()
        .filter(|command| is_cargo_clippy_invocation(MAKEFILE, command))
        .map(|command| expand_make_variables(MAKEFILE, command))
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

// ---------------------------------------------------------------------------
// Coverage for the readers themselves.
//
// The tests above run the readers against this repository, which exercises
// only the spellings it happens to use today. The readers also have to handle
// the spellings the repository will use after issues #438 and #439 land:
// `[lints] workspace = true` inheritance, a level given as a table rather than
// a bare string, and a single `--workspace` Clippy command. Those branches are
// checked here against inline fixtures, so a regression in them shows up now
// rather than during that migration.
// ---------------------------------------------------------------------------

/// A workspace manifest whose shared lint table denies the lint.
const WORKSPACE_MANIFEST: &str = concat!(
    "[workspace]\nmembers = [\".\", \"test-macros\"]\n\n",
    "[workspace.lints.clippy]\ndisallowed_methods = \"deny\"\n"
);

/// Scenario: a manifest declares the lint level in its own package table.
/// Invariant: the level is read from the package, with the workspace manifest
/// ignored, so a package that opts out of inheritance is judged on its own
/// declaration.
#[rstest]
#[case::bare_string("[lints.clippy]\ndisallowed_methods = \"deny\"\n", Some("deny"))]
#[case::level_table(
    "[lints.clippy]\ndisallowed_methods = { level = \"deny\", priority = 1 }\n",
    Some("deny")
)]
#[case::warned("[lints.clippy]\ndisallowed_methods = \"warn\"\n", Some("warn"))]
#[case::absent("[lints.clippy]\npedantic = \"warn\"\n", None)]
#[case::no_lint_table("[package]\nname = \"example\"\n", None)]
fn reads_a_package_declared_lint_level(
    #[case] manifest: &str,
    #[case] expected: Option<&str>,
) -> Result<()> {
    let level = clippy_lint_level(manifest, WORKSPACE_MANIFEST, "disallowed_methods")?;
    ensure!(
        level.as_deref() == expected,
        "expected {expected:?} from `{manifest}`, found {level:?}"
    );
    Ok(())
}

/// Scenario: a manifest opts into workspace lints with `[lints] workspace = true`,
/// which is the spelling issue #439 introduces.
/// Invariant: the level comes from the workspace manifest rather than the
/// package, so the contract keeps enforcing the deny after that migration.
#[test]
fn follows_workspace_lint_inheritance() -> Result<()> {
    let manifest = "[package]\nname = \"test-macros\"\n\n[lints]\nworkspace = true\n";
    let level = clippy_lint_level(manifest, WORKSPACE_MANIFEST, "disallowed_methods")?;
    ensure!(
        level.as_deref() == Some("deny"),
        "inherited level should be deny, found {level:?}"
    );
    Ok(())
}

/// Scenario: a manifest inherits workspace lints but the workspace table has
/// dropped the lint.
/// Invariant: no level is reported, so weakening the shared table after #439
/// fails the contract instead of passing through the inheritance path.
#[test]
fn reports_no_level_when_the_workspace_table_drops_the_lint() -> Result<()> {
    let manifest = "[package]\nname = \"test-macros\"\n\n[lints]\nworkspace = true\n";
    let workspace =
        "[workspace]\nmembers = [\".\"]\n\n[workspace.lints.clippy]\npedantic = \"warn\"\n";
    let level = clippy_lint_level(manifest, workspace, "disallowed_methods")?;
    ensure!(level.is_none(), "expected no level, found {level:?}");
    Ok(())
}

/// Scenario: a `lint` recipe is read from a Makefile that also names Clippy in
/// a comment, in a variable, and in another target.
/// Invariant: only the recipe's own uncommented commands are returned, as
/// written, so none of those three can satisfy the coverage requirement.
#[test]
fn reads_only_the_targets_own_uncommented_commands() -> Result<()> {
    let makefile = concat!(
        "CLIPPY_FLAGS ?= --all-targets -- -D warnings\n",
        "DECOY = $(CARGO) clippy --all-features\n",
        "\n",
        "lint: check-static-regexes ## Run Clippy\n",
        "\t# $(CARGO) clippy --manifest-path test-macros/Cargo.toml $(CLIPPY_FLAGS)\n",
        "\t@cargo clippy $(CLIPPY_FLAGS)\n",
        "\n",
        "typecheck:\n",
        "\tcargo clippy --manifest-path test-macros/Cargo.toml\n",
    );
    let commands = recipe_commands(makefile, "lint")?;
    ensure!(
        commands == vec!["cargo clippy $(CLIPPY_FLAGS)".to_owned()],
        "expected the single uncommented lint command, found {commands:?}"
    );
    Ok(())
}

/// Scenario: each shape a recipe command can take is judged for whether it runs
/// Clippy.
/// Invariant: only a command whose first word names Cargo and whose subcommand
/// is `clippy` counts. A command that merely mentions Clippy carries every flag
/// and manifest path the contract looks for while linting nothing, so a
/// substring search would accept a gate that has been switched off.
#[rstest]
#[case::variable_reference("$(CARGO) clippy $(CLIPPY_FLAGS)", true)]
#[case::bare_cargo("cargo clippy --all-targets", true)]
#[case::absolute_path("/usr/local/bin/cargo clippy --all-targets", true)]
#[case::toolchain_override("$(CARGO) +nightly clippy --all-targets", true)]
#[case::echoed("echo $(CARGO) clippy $(CLIPPY_FLAGS)", false)]
#[case::no_op_builtin(": $(CARGO) clippy $(CLIPPY_FLAGS)", false)]
#[case::echoed_subcommand("echo clippy --all-targets", false)]
#[case::another_subcommand("$(CARGO) build --all-targets", false)]
#[case::unrelated_executable("$(MDLINT) clippy", false)]
#[case::unknown_variable("$(NOT_DEFINED) clippy", false)]
#[case::empty("", false)]
fn recognizes_only_executable_clippy_invocations(#[case] command: &str, #[case] expected: bool) {
    let makefile = concat!(
        "CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)\n",
        "MDLINT ?= markdownlint-cli2\n",
        "CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings\n",
    );
    assert_eq!(
        is_cargo_clippy_invocation(makefile, command),
        expected,
        "`{command}` should {} count as a Clippy invocation",
        if expected { "" } else { "not" }
    );
}

/// Scenario: package coverage is judged for each shape of Clippy command.
/// Invariant: a bare command covers only the root package, a `--manifest-path`
/// command covers only the package it names, and a `--workspace` command covers
/// both, which is the shape issue #439 leaves behind.
#[rstest]
#[case::bare_covers_root("cargo clippy --all-targets", "Cargo.toml", true)]
#[case::bare_misses_macros("cargo clippy --all-targets", "test-macros/Cargo.toml", false)]
#[case::manifest_path_misses_root(
    "cargo clippy --manifest-path test-macros/Cargo.toml",
    "Cargo.toml",
    false
)]
#[case::manifest_path_covers_macros(
    "cargo clippy --manifest-path test-macros/Cargo.toml",
    "test-macros/Cargo.toml",
    true
)]
#[case::workspace_covers_root("cargo clippy --workspace", "Cargo.toml", true)]
#[case::workspace_covers_macros("cargo clippy --workspace", "test-macros/Cargo.toml", true)]
fn judges_package_coverage_by_command_shape(
    #[case] command: &str,
    #[case] manifest: &str,
    #[case] expected: bool,
) {
    assert_eq!(
        covers_package(command, manifest),
        expected,
        "`{command}` coverage of {manifest} should be {expected}"
    );
}
