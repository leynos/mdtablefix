//! Readers for the repository's environment-access lint policy.
//!
//! `tests/env_access_policy.rs` asserts the policy; this module holds the
//! parsing it needs. The two are separated so neither file outgrows the
//! repository's 400-line limit, and so the readers can be exercised against
//! inline fixtures as well as against the repository's own files.
//!
//! Every reader returns a `Result`. None panics, so a malformed fixture
//! surfaces as a test failure with context rather than as a panic in a
//! helper.

use anyhow::{Context, Result, ensure};
use toml::{Table, Value};

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

/// Return `name`'s value from a simple Make assignment (`=`, `?=`, or `:=`).
///
/// Recursive and conditional assignments differ in when Make expands them, not
/// in the text they hold, so one reader covers all three.
pub fn make_assignment<'a>(makefile: &'a str, name: &str) -> Option<&'a str> {
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
pub fn expand_make_variables(makefile: &str, command: &str) -> String {
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
pub fn recipe_commands(makefile: &str, target: &str) -> Result<Vec<String>> {
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
pub fn covers_package(command: &str, manifest: &str) -> bool {
    if command.contains("--workspace") {
        return true;
    }
    match manifest {
        "Cargo.toml" => !command.contains("--manifest-path"),
        _ => command.contains(&format!("--manifest-path {manifest}")),
    }
}
