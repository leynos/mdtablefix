//! Coverage for the readers behind the environment-access policy contract.
//!
//! `tests/env_access_policy.rs` runs those readers against this repository,
//! which exercises only the spellings it uses today. These cases exercise the
//! rest against inline fixtures: `[lints] workspace = true` inheritance and a
//! level given as a table, which issues #438 and #439 introduce; a single
//! `--workspace` Clippy command, which #439 leaves behind; and the command
//! shapes that mention Clippy without running it.
//!
//! They live apart from the contract so neither file outgrows the repository's
//! 400-line limit, and so a failure says plainly whether a reader broke or the
//! repository drifted.

use anyhow::{Result, ensure};
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

/// Scenario: a Clippy configuration is read for its disallowed method paths.
/// Invariant: every entry's `path` is returned in order, and an entry missing
/// that field is skipped rather than failing the read, so a malformed entry
/// cannot masquerade as a ban.
#[test]
fn reads_every_disallowed_method_path() -> Result<()> {
    let configuration = concat!(
        "disallowed-methods = [\n",
        "  { path = \"std::env::var\", reason = \"inject an environment reader\" },\n",
        "  { reason = \"an entry with no path\" },\n",
        "  \"std::env::set_var\",\n",
        "]\n",
    );
    let paths = disallowed_method_paths(configuration)?;
    ensure!(
        paths == vec!["std::env::var".to_owned()],
        "expected the one entry carrying a path, found {paths:?}"
    );
    Ok(())
}

/// Scenario: a Clippy configuration declares no `disallowed-methods` array.
/// Invariant: the read fails rather than reporting an empty ban list, so a
/// configuration that has lost the table cannot look like one that bans
/// nothing by design.
#[test]
fn rejects_a_configuration_without_disallowed_methods() {
    let error = disallowed_method_paths("cognitive-complexity-threshold = 9\n")
        .expect_err("a configuration with no disallowed-methods should fail the read");
    assert!(
        format!("{error}").contains("disallowed-methods"),
        "the error should name the missing table, found: {error}"
    );
}

/// Scenario: a recipe command carrying Make variable references is expanded.
/// Invariant: a known reference resolves, a reference defined in terms of
/// another resolves too, and an undefined reference is left intact so it shows
/// up in a failure message rather than vanishing.
#[rstest]
#[case::known("$(FLAGS)", "--all-targets -- -D warnings")]
#[case::nested("$(EXTENDED)", "--all-targets -- -D warnings --all-features")]
#[case::undefined("$(MISSING) --all-targets", "$(MISSING) --all-targets")]
#[case::mixed("cargo clippy $(FLAGS)", "cargo clippy --all-targets -- -D warnings")]
#[case::literal("cargo clippy", "cargo clippy")]
fn expands_make_variable_references(#[case] command: &str, #[case] expected: &str) {
    let makefile = concat!(
        "FLAGS ?= --all-targets -- -D warnings\n",
        "EXTENDED = $(FLAGS) --all-features\n",
    );
    assert_eq!(
        expand_make_variables(makefile, command),
        expected,
        "`{command}` should expand to `{expected}`"
    );
}
