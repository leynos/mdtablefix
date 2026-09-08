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

#![allow(
    dead_code,
    reason = "this module is shared by four test binaries through #[path]; each uses a different \
              subset of the readers, so `unused here` is not a defect"
)]

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

/// One command from a Make recipe, with the prefixes that change its meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeCommand {
    /// The command text, with the `@` and `+` prefixes removed.
    ///
    /// Those two change when Make echoes or runs a command, not whether its
    /// failure counts, so they are noise to every caller here.
    pub text: String,
    /// Whether Make was told to ignore this command's exit status.
    ///
    /// The `-` prefix is kept as a flag rather than stripped, because a gate
    /// whose command cannot fail the target is a gate that does not gate.
    pub ignores_errors: bool,
}

/// Return the commands of `target`'s recipe, exactly as written.
///
/// A recipe line is tab-indented; the recipe ends at the first line that is
/// neither tab-indented nor blank. Comment lines are dropped, so a
/// commented-out command cannot satisfy a coverage requirement.
///
/// A line ending in a backslash continues onto the next one and the pair is
/// returned as a single command, because that is what Make does: it passes the
/// whole continued line to one shell. Reading the physical lines separately
/// would let a Clippy call wrapped in a never-taken branch, written as
/// `if false; then` on the first line, the invocation on the second, and
/// `; fi` on the third, look like a bare invocation on a line of its own, and
/// so certify a recipe that lints nothing.
///
/// Variables are left unexpanded so a caller can judge the executable and the
/// argument order from what the recipe actually says. Expanding first would
/// replace `$(CARGO)` with a shell fragment and lose the token boundary that
/// makes the first word identifiable. Use [`expand_make_variables`] afterwards
/// when the flags matter.
pub fn recipe_commands(makefile: &str, target: &str) -> Result<Vec<RecipeCommand>> {
    let prefix = format!("{target}:");
    let body = makefile
        .lines()
        .skip_while(|line| !line.starts_with(&prefix))
        .skip(1);
    let mut commands = Vec::new();
    let mut pending: Option<RecipeCommand> = None;
    for line in body {
        let Some(text) = line.strip_prefix('\t') else {
            if line.trim().is_empty() {
                continue;
            }
            break;
        };
        let text = text.trim_end();
        let continues = text.ends_with('\\');
        let text = text.strip_suffix('\\').unwrap_or(text).trim();
        if let Some(started) = pending.as_mut() {
            started.text.push(' ');
            started.text.push_str(text);
        } else {
            // Make accepts the prefixes in any order and any number.
            let body = text.trim_start_matches(['@', '-', '+']);
            let ignores_errors = text[..text.len() - body.len()].contains('-');
            pending = Some(RecipeCommand {
                text: body.trim().to_owned(),
                ignores_errors,
            });
        }
        if !continues && let Some(command) = pending.take() {
            push_command(&mut commands, &command);
        }
    }
    // A recipe whose last line ends in a backslash is malformed, but the
    // command it began is still part of the recipe and must be judged.
    if let Some(command) = pending {
        push_command(&mut commands, &command);
    }
    ensure!(
        !commands.is_empty(),
        "the {target} target should have a recipe"
    );
    Ok(commands)
}

/// Add `command` to `commands` unless it is blank or a shell comment.
fn push_command(commands: &mut Vec<RecipeCommand>, command: &RecipeCommand) {
    let text = command.text.trim();
    if text.is_empty() || text.starts_with('#') {
        return;
    }
    commands.push(RecipeCommand {
        text: text.to_owned(),
        ignores_errors: command.ignores_errors,
    });
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
