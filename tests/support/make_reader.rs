//! Readers for Make variables and recipe commands.
//!
//! Split from `policy_reader.rs` along the line between reading a Makefile
//! and judging what it says. The split is by consumer as much as by
//! subject: `tests/lint_policy_properties.rs` needs the parsing and nothing
//! else, and a module carrying readers a binary does not use would need a
//! dead-code suppression, which is the attribute this whole policy exists
//! to keep out of the repository.
//!
//! Every reader returns a `Result` or an `Option`. None panics, so a
//! malformed fixture surfaces as a test failure with context.

use anyhow::{Result, ensure};

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
/// Fails if the target is declared more than once. GNU Make keeps the later
/// recipe, so reading the first would judge one that never runs.
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
    let declarations = makefile
        .lines()
        .filter(|line| line.starts_with(&prefix))
        .count();
    // GNU Make keeps the *later* recipe for a target and warns that it is
    // overriding the earlier one, so a second `lint:` further down the file
    // would run instead of this one. Reading the first would then validate a
    // recipe Make never runs. Rejecting the duplicate is stronger than picking
    // the last, since a Makefile with two recipes for one gate is a mistake in
    // its own right.
    ensure!(
        declarations == 1,
        concat!(
            "the {target} target should be declared once, found {declarations} declarations; ",
            "GNU Make would run the last and this contract would judge the first"
        ),
        target = target,
        declarations = declarations
    );
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
