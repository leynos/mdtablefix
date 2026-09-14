//! Contract coverage for the environment-access lint policy.
//!
//! The policy has three parts and no single file holds all of them, so nothing
//! in the build connects them: `clippy.toml` names the prohibited methods, the
//! workspace lint table raises `clippy::disallowed_methods` to `deny` and
//! closes the `#[allow]` route around it, and the Makefile's `lint` recipe runs
//! Clippy over every member, every target, and every feature with warnings
//! denied. Drop any one part and the other two still look correct while the
//! policy stops being enforced. These tests tie the three together.
//!
//! They assert the policy's *shape*. `tests/env_access_enforcement.rs` asserts
//! that it *fires*, by running Clippy over a fixture package that calls all six
//! methods. Neither test subsumes the other: a configuration can have the right
//! shape and lint nothing, and a lint can fire while the gate that runs it has
//! stopped covering a package.
//!
//! Each package is still checked by name rather than assumed to inherit.
//! `[lints] workspace = true` is a line a member can lose without anything else
//! noticing, and losing it drops that member out of the policy while the
//! workspace table still reads correctly.
//!
//! The Makefile check parses the `lint` recipe rather than searching the file
//! for a command string, and judges each command as an invocation rather than
//! as text. A matching string in a comment, in another target, or covering only
//! the root package would satisfy a file-wide search while the gate no longer
//! enforces the policy; and a command such as `echo $(CARGO) clippy ...` would
//! satisfy a substring search for `clippy` while running no Clippy at all. So
//! the recipe is read as written, to judge the executable and the argument
//! order, and expanded afterwards, to judge the flags. Backslash continuations
//! are joined first, because Make passes a continued line to one shell: read as
//! separate physical lines, a Clippy call wrapped in `if false; then ... ; fi`
//! looks like a bare invocation on a line of its own.
//!
//! The policy itself is recorded in
//! `docs/adrs/0012-environment-seam-taxonomy.md`.
//!
//! Repository files are pulled in with `include_str!`, so deleting one is a
//! compile error rather than a silent skip, and the tests need no filesystem
//! access of their own. The parsing lives in `tests/support/lint_policy.rs`;
//! the readers there are also exercised against inline fixtures at the end of
//! this file.
//!
//! Mutation proof, re-run on 2026-09-14 after issue #439 made the two packages
//! one workspace. The mechanism these mutations were made against changed:
//! the lint level moved from a `[lints.clippy]` table in each manifest to one
//! `[workspace.lints.clippy]` table each member inherits, and the recipe's
//! second `--manifest-path test-macros/Cargo.toml` invocation gave way to the
//! `--workspace` one it always anticipated. Each mutation below was applied
//! alone to the current tree, run through the build, and reverted. Every one
//! failed, with the message shown:
//!
//! ```text
//! delete the std::env::set_var entry from clippy.toml
//!   -> clippy_configuration_disallows_every_environment_method
//!      clippy.toml must disallow std::env::set_var, found [...]
//! delete allow_attributes from the workspace lint table
//!   -> every_package_denies_the_policy_lints
//!      Cargo.toml must set clippy allow_attributes to deny, found None
//! delete `[lints] workspace = true` from the test-macros manifest, so it
//! stops inheriting
//!   -> every_package_denies_the_policy_lints
//!      test-macros/Cargo.toml must set clippy disallowed_methods to deny,
//!      found None
//! delete --workspace from CLIPPY_FLAGS
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      the lint target must run Clippy over test-macros/Cargo.toml, found
//!      ["... clippy --all-targets --all-features -- -D warnings"]
//! prefix the Clippy command with `echo`, so it runs nothing
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      the lint target must invoke Cargo Clippy
//! wrap the invocation in `if false; then ... ; fi` across continuation lines
//!   -> clippy_gate_denies_warnings_across_targets_and_features
//!      the lint target must invoke Cargo Clippy
//! append `|| true` to the invocation
//!   -> no_construct_can_mask_a_failing_clippy_command
//!      has a `||` fallback other than `exit 1`, which substitutes a success
//! give the invocation Make's `-` prefix
//!   -> no_construct_can_mask_a_failing_clippy_command
//!      carries Make's `-` prefix, so its failure is ignored
//! pipe the invocation into `tail -5`
//!   -> no_construct_can_mask_a_failing_clippy_command
//!      is piped, so the reported status is the last stage's
//! add `.ONESHELL:` to the Makefile
//!   -> no_construct_can_mask_a_failing_clippy_command
//!      .ONESHELL puts the whole recipe in one shell, where only the last
//!      command's status is reported
//! ```
//!
//! Three more, measured on 2026-09-06 against the two-invocation recipe this
//! replaced, and not re-run because the construct they exercise is gone or
//! unchanged: `-D warnings` and `--all-targets` deleted from `CLIPPY_FLAGS`
//! each fail `clippy_gate_denies_warnings_across_targets_and_features` on the
//! flag they name, and a second `lint:` target further down the Makefile fails
//! with "the lint target should be declared once, found 2 declarations".
//!
//! That last one is a live hole, not a hypothetical: GNU Make keeps the later
//! recipe for a target, warning that it overrides the earlier, so the added
//! `lint:` ran and the real one did not. Reading the first declaration would
//! have judged a recipe Make never runs.
//!
//! Two forms that keep the status intact were also applied, and pass:
//! `.ONESHELL:` paired with `-e` in `.SHELLFLAGS`, and an explicit
//! `|| exit 1`. The rule is about the status reaching Make, not about the
//! construct.
//!
//! Each of `|| true`, `|| true || exit 1`, the `-` prefix and the pipe was
//! confirmed to be a live hole before it was closed: with a `std::env::var`
//! call in the root package, `make lint` exited 0 under all four while this
//! file's tests passed.
//!
//! Enforcement itself was re-proven on 2026-09-14 through the single
//! `--workspace` invocation, which is the claim the restructure rests on. A
//! `std::env::var` call inside `allow_fixture_expansion_lints` in
//! `test-macros/src/lib.rs` fails `cargo clippy --workspace --all-targets
//! --all-features -- -D warnings` with "use of a disallowed method
//! `std::env::var`" and the configured reason "inject an environment reader",
//! so the member is linted in its own right rather than as a capped
//! dependency. Adding `#[allow(clippy::disallowed_methods)]` above it, the
//! obvious way to defeat the ban, fails in its own right with "#[allow]
//! attribute found" and "`allow` attribute without specifying a reason",
//! which shows the inherited hygiene denies reach the member too. Both were
//! reverted.
use anyhow::{Context, Result, bail, ensure};

#[path = "support/make_reader.rs"]
mod make_reader;

#[path = "support/policy_reader.rs"]
mod policy_reader;

use make_reader::{expand_make_variables, recipe_commands};
use policy_reader::{
    clippy_lint_level,
    covers_package,
    disallowed_method_paths,
    is_cargo_clippy_invocation,
    status_masking_construct,
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
/// Both members are listed by name even though the level now lives in one
/// `[workspace.lints.clippy]` table. `[lints] workspace = true` is a line a
/// member can lose on its own, and losing it drops that member out of the
/// policy while the workspace table still reads correctly.
/// [`clippy_lint_level`] follows the inheritance, so each entry is judged on
/// the level that actually applies to it.
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

/// Scenario: each member manifest is read for the level it gives each lint the
/// policy depends on.
/// Invariant: both members deny all three, whether declared in the package or
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
        .filter(|command| is_cargo_clippy_invocation(MAKEFILE, &command.text))
        .map(|command| expand_make_variables(MAKEFILE, &command.text))
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

/// Scenario: the `lint` recipe is judged for whether a failing Clippy command
/// could be reported as success.
/// Invariant: nothing between each Clippy command and Make swallows its exit
/// status, and the recipe as a whole is not put in one shell without `-e`, so a
/// failure in either invocation still fails the target.
///
/// Make runs each recipe line in its own shell and stops at the first non-zero
/// status, which is why the recipe needs no `|| exit 1`. Several constructs
/// break that, and each would let a broken policy pass a green gate:
/// `.ONESHELL` without `-e` in `.SHELLFLAGS`, Make's `-` prefix, a `;` chain,
/// a `|| true` fallback, and a pipeline.
///
/// Verified by execution on 2026-09-07, not by reading the recipe. With a
/// `std::env::var` call in the root package and the `test-macros` invocation
/// last, `make lint` exited 2 and never reached the second command. The same
/// tree exited 0 under each of `.ONESHELL:`, a `-` prefix, `|| true`, and a
/// pipe into `tail`, which is the class of regression this test exists to
/// catch.
#[test]
fn no_construct_can_mask_a_failing_clippy_command() -> Result<()> {
    let one_shell = MAKEFILE
        .lines()
        .any(|line| line.trim_start().starts_with(".ONESHELL:"));
    let errors_abort = MAKEFILE
        .lines()
        .filter_map(|line| line.strip_prefix(".SHELLFLAGS"))
        .any(|flags| flags.split_whitespace().any(|flag| flag == "-e"));
    ensure!(
        !one_shell || errors_abort,
        concat!(
            ".ONESHELL puts the whole recipe in one shell, where only the last command's ",
            "status is reported; pair it with -e in .SHELLFLAGS, or give every Clippy command ",
            "its own `|| exit 1`"
        )
    );
    for command in recipe_commands(MAKEFILE, "lint")? {
        if !is_cargo_clippy_invocation(MAKEFILE, &command.text) {
            continue;
        }
        if let Some(reason) = status_masking_construct(&command) {
            bail!("the Clippy command `{}` {reason}", command.text);
        }
    }
    Ok(())
}
