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
//! add `.ONESHELL:` to the Makefile
//!   -> no_construct_can_mask_a_failing_clippy_command
//!      .ONESHELL puts the whole recipe in one shell, where only the last
//!      command's status is reported
//! chain the two Clippy commands on one line with `;`
//!   -> no_construct_can_mask_a_failing_clippy_command
//!      the Clippy command [...] chains another with `;`
//! ```
//!
//! `.ONESHELL:` paired with `-e` in `.SHELLFLAGS` was also applied, and passes:
//! the rule is about the status reaching Make, not about the construct.
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

/// Scenario: the `lint` recipe's shape is judged for whether a failing Clippy
/// command can be reported as success.
/// Invariant: no construct that would mask an exit status is present, so a
/// failure in the first invocation still fails the target rather than being
/// overwritten by the second one's success.
///
/// Make runs each recipe line in its own shell and stops at the first non-zero
/// status, which is why the recipe needs no `|| exit 1`. Two constructs break
/// that. `.ONESHELL` puts the whole recipe in one shell, where only the last
/// command's status is reported unless `.SHELLFLAGS` carries `-e`; and a `;`
/// chain does the same within one line. Either would let a broken policy pass
/// the gate.
///
/// Verified by execution on 2026-09-07, not by reading the recipe. With a
/// `std::env::var` call in the root package and the `test-macros` invocation
/// last, `make lint` exited 2 and never reached the second command. Adding
/// `.ONESHELL:` to the same Makefile made the identical tree exit 0, which is
/// the regression this test exists to catch.
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
        ".ONESHELL puts the whole recipe in one shell, where only the last command's status is \
         reported; pair it with -e in .SHELLFLAGS, or give every Clippy command its own `|| exit \
         1`"
    );
    for command in recipe_commands(MAKEFILE, "lint")? {
        ensure!(
            !(is_cargo_clippy_invocation(MAKEFILE, &command) && command.contains(';')),
            "the Clippy command `{command}` chains another with `;`, so a failure in the first \
             would be reported as the second's success"
        );
    }
    Ok(())
}
