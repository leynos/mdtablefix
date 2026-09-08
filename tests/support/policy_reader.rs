//! Readers that judge a Clippy configuration, a package manifest and a
//! Make recipe against the environment-access policy.
//!
//! The Makefile parsing these build on lives in `make_reader.rs`; see that
//! module for why the two are separate. A consumer of this module declares
//! both.

use anyhow::{Context, Result};
use toml::{Table, Value};

use crate::make_reader::{RecipeCommand, make_assignment};

/// Return the `path` field of every `disallowed-methods` entry in `clippy.toml`.
pub fn disallowed_method_paths(configuration: &str) -> Result<Vec<String>> {
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
pub fn clippy_lint_level(
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

/// Return why `command`'s exit status would not reach Make, if it would not.
///
/// Make reports a recipe line's status only when nothing between the command
/// and the shell's exit swallows it. Four constructs do, and each would let a
/// failing gate pass:
///
/// * the `-` recipe prefix, which tells Make to ignore the status outright;
/// * a `;` chain, where only the last command's status is reported;
/// * a `||` fallback such as `|| true`, which substitutes a success. `|| exit 1` is the only
///   exception, since it re-raises the failure rather than hiding it, and every fallback in a chain
///   must be that one: in `cmd || true || exit 1` the shell never reaches the `exit 1`;
/// * a pipeline, whose status is the last stage's, not the command's.
///
/// Returns `None` when the status does reach Make.
pub fn status_masking_construct(command: &RecipeCommand) -> Option<&'static str> {
    if command.ignores_errors {
        return Some("carries Make's `-` prefix, so its failure is ignored");
    }
    if command.text.contains(';') {
        return Some("chains another command with `;`, so only the last status is reported");
    }
    // Every fallback is judged, not just the last. In `cmd || true || exit 1`
    // the last one is `exit 1`, but `true` succeeds first, so the shell never
    // reaches it and the line exits 0 with `cmd` having failed.
    if command
        .text
        .split("||")
        .skip(1)
        .any(|fallback| fallback.trim() != "exit 1")
    {
        return Some("has a `||` fallback other than `exit 1`, which substitutes a success");
    }
    // A `|` that is not part of `||` opens a pipeline.
    if command.text.replace("||", "").contains('|') {
        return Some("is piped, so the reported status is the last stage's");
    }
    None
}

/// Return whether `command` lints `manifest`'s package.
///
/// A `--workspace` run covers every member, so it covers both packages once
/// issue #439 lands. Until then the root package is the one a command with no
/// `--manifest-path` selects, and `test-macros` needs its manifest named.
pub fn covers_package(command: &str, manifest: &str) -> bool {
    if command.contains("--workspace") {
        return true;
    }
    match manifest {
        "Cargo.toml" => !command.contains("--manifest-path"),
        _ => command.contains(&format!("--manifest-path {manifest}")),
    }
}

/// Return whether `text` names the Cargo executable.
///
/// A bare `cargo` or any path ending in `/cargo` counts; nothing else does.
fn is_cargo_path(text: &str) -> bool { text == "cargo" || text.ends_with("/cargo") }

/// Return whether a Make variable's value resolves to the Cargo executable.
///
/// The value may be a shell fragment rather than a plain path. `$(CARGO)` in
/// this repository expands to an `$(or $(shell command -v cargo ...),...)`
/// lookup, so the value is split on the punctuation that separates its parts
/// and each piece is judged on its own.
fn value_names_cargo(value: &str) -> bool {
    value
        .split([' ', '\t', ',', '(', ')'])
        .any(|piece| is_cargo_path(piece.trim()))
}

/// Return whether `command` executes Cargo's `clippy` subcommand.
///
/// Both the executable and the argument position are checked, because a
/// substring search for `clippy` is satisfied by a command that never runs it:
/// `echo $(CARGO) clippy ...` and `: $(CARGO) clippy ...` both carry every flag
/// a recipe needs while linting nothing. So the first word must name Cargo,
/// resolving a `$(VARIABLE)` reference through the Makefile's own assignments,
/// and `clippy` must be the subcommand rather than a later argument. A leading
/// `+toolchain` override is skipped, since Cargo accepts one there.
pub fn is_cargo_clippy_invocation(makefile: &str, command: &str) -> bool {
    let mut words = command.split_whitespace();
    let Some(executable) = words.next() else {
        return false;
    };
    let names_cargo = match executable
        .strip_prefix("$(")
        .and_then(|r| r.strip_suffix(')'))
    {
        Some(variable) => make_assignment(makefile, variable).is_some_and(value_names_cargo),
        None => is_cargo_path(executable),
    };
    names_cargo && words.find(|word| !word.starts_with('+')) == Some("clippy")
}
