//! Reads a Makefile for whether one shell runs a whole recipe, and whether
//! that shell stops at the first failure.
//!
//! Split from `tests/env_access_policy.rs` under the repository's 400-line
//! cap. The parent's recipe guard asks these questions; `shell_flags.rs`
//! drives them against fixture Makefiles in both directions.

/// Assignment operators Make recognises, longest first so `::=` is not read as
/// `:=`, nor `:=` as `=`.
const ASSIGNMENTS: [&str; 5] = ["::=", ":=", "+=", "?=", "="];

/// Return the effective value of `.SHELLFLAGS`, or `None` if it is never set.
///
/// Two things a prefix search got wrong, both of which let `.ONESHELL` mask a
/// failing Clippy command while this contract stayed green. `.SHELLFLAGS_NOTE`
/// begins with the variable's name and is an ordinary variable that says
/// nothing about the shell; and Make's later assignment wins, so an earlier
/// `.SHELLFLAGS = -e -c` followed by `.SHELLFLAGS = -c` leaves no `-e` in the
/// shell Make actually runs. The name is therefore matched exactly and the
/// assignments are folded in file order.
///
/// `+=` appends to what is there and `?=` assigns only when unset, as Make
/// defines them. A tab makes a line a recipe line rather than an assignment, so
/// those are skipped; a trailing comment is not part of the value. Line
/// continuations are not joined, which would understate a value split across
/// lines rather than overstate it.
pub(super) fn effective_shellflags(makefile: &str) -> Option<String> {
    makefile
        .lines()
        .filter_map(shellflags_assignment)
        .fold(None, apply_assignment)
}

/// Return the operator and argument of a `.SHELLFLAGS` assignment on `line`.
///
/// The name is matched exactly, so `.SHELLFLAGS_NOTE` is an ordinary variable
/// and says nothing about the shell. A tab makes a line a recipe line rather
/// than an assignment, and a trailing comment is not part of the value.
fn shellflags_assignment(line: &str) -> Option<(&'static str, &str)> {
    if line.starts_with('\t') {
        return None;
    }
    let rest = line.trim_start().strip_prefix(".SHELLFLAGS")?.trim_start();
    let (operator, argument) = ASSIGNMENTS
        .iter()
        .find_map(|operator| rest.strip_prefix(operator).map(|rest| (*operator, rest)))?;
    Some((
        operator,
        argument.split('#').next().unwrap_or_default().trim(),
    ))
}

/// Fold one assignment into the value so far, as Make defines the operators.
///
/// `+=` appends to what is there, `?=` assigns only when unset, and the rest
/// replace. Folding in file order is what makes the last assignment the one
/// that counts.
fn apply_assignment(value: Option<String>, assignment: (&str, &str)) -> Option<String> {
    let (operator, argument) = assignment;
    match operator {
        "+=" => Some(match value {
            Some(existing) if existing.is_empty() => argument.to_owned(),
            Some(existing) => format!("{existing} {argument}"),
            None => argument.to_owned(),
        }),
        "?=" => value.or_else(|| Some(argument.to_owned())),
        _ => Some(argument.to_owned()),
    }
}

/// Return whether the shell Make runs a recipe in aborts on the first failure.
///
/// Judged on the value Make ends up with. Make's default `.SHELLFLAGS` is
/// `-c`, so an unset variable is not an aborting shell.
///
/// `-e` is not the only spelling. A single-dash bundle carries each of its
/// letters as a separate option, so `-eo pipefail -c` sets `errexit` as surely
/// as `-e -c` does, and that bundle is the form this estate's Makefiles use.
/// `-o errexit` sets it by name. Comparing whole tokens to `-e` called the
/// bundle non-aborting, which would fail a Makefile that is correct: a contract
/// wrong in that direction gets deleted rather than obeyed.
pub(super) fn aborts_on_error(flags: &str) -> bool {
    let tokens: Vec<&str> = flags.split_whitespace().collect();
    tokens.iter().enumerate().any(|(index, token)| {
        if let Some(letters) = token.strip_prefix('-') {
            if letters.starts_with('-') {
                return false;
            }
            if letters == "o" {
                return tokens.get(index + 1) == Some(&"errexit");
            }
            return letters.contains('e');
        }
        false
    })
}

/// Return whether the Makefile's effective `.SHELLFLAGS` abort on failure.
pub(super) fn shell_aborts_on_error(makefile: &str) -> bool {
    effective_shellflags(makefile).is_some_and(|flags| aborts_on_error(&flags))
}

/// Return whether the Makefile declares the `.ONESHELL` special target.
///
/// Make parses `.ONESHELL` as a target, and a target may be separated from its
/// colon by whitespace, so `.ONESHELL :` enables one-shell recipes exactly as
/// `.ONESHELL:` does. The name and the colon are therefore matched separately.
/// A test for the two characters together reads the spaced spelling as an
/// ordinary line, leaves the `.SHELLFLAGS` guard unasked, and lets a failing
/// Clippy command be masked by a later successful one: the whole of what this
/// guard exists to prevent.
///
/// A line that merely begins with the name is not a declaration, so
/// `.ONESHELL_NOTE := ...` reads as the ordinary variable it is.
pub(super) fn declares_one_shell(makefile: &str) -> bool {
    makefile.lines().any(|line| {
        line.trim_start()
            .strip_prefix(".ONESHELL")
            .is_some_and(|rest| rest.trim_start().starts_with(':'))
    })
}
