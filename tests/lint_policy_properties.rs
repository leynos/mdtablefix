//! Property coverage for the Make-policy readers.
//!
//! `tests/lint_policy_readers.rs` pins named cases: the exact spellings the
//! policy cares about, and the negative command shapes a gate must reject.
//! Those are examples. The readers also make claims over unbounded input, and
//! an example cannot carry a claim like "any order and any number": the
//! prefix cases there cover `@-` and `-@`, which is two of the sequences a
//! recipe may legally write.
//!
//! These properties carry the unbounded claims, each against an oracle simple
//! enough to be obviously right:
//!
//! * a recipe prefix sequence drawn from `@`, `+` and `-` is removed entirely, and `ignores_errors`
//!   is true exactly when the sequence contained a `-`;
//! * a command split across backslash continuations reads back as its fragments joined by one
//!   space;
//! * a defined `$(NAME)` reference expands to its value and an undefined one is left intact.
//!
//! Generated input stays inside the parser's contract. Malformed and negative
//! syntax is the named cases' job, because a generator cannot say which of two
//! wrong answers a malformed input deserves.

use proptest::prelude::*;

#[path = "support/make_reader.rs"]
mod make_reader;

use make_reader::{expand_make_variables, recipe_commands};

/// The command every generated recipe line carries.
///
/// Held constant so each property varies one thing: the prefixes, the
/// continuations, or the variable references.
const STABLE_COMMAND: &str = "cargo clippy --all-targets";

/// A bounded sequence of Make's recipe prefixes, possibly empty.
///
/// Make accepts `@` (silent), `+` (always run) and `-` (ignore errors) in any
/// order and any number. Six is enough to cover repetition and mixed order
/// while keeping shrinking quick.
fn prefix_sequence() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(vec!['@', '+', '-']), 0..6)
        .prop_map(|prefixes| prefixes.into_iter().collect())
}

/// A command fragment with no backslash, newline or recipe prefix.
///
/// Those three are excluded because each means something to the reader: a
/// backslash continues the line, a newline ends it, and a leading prefix is
/// stripped. A fragment carrying one would be testing a different property.
fn fragment() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z][a-z0-9_]{0,7}",
        ("[a-z][a-z0-9_]{0,5}", "[a-z0-9_]{1,5}").prop_map(|(head, tail)| format!("{head} {tail}")),
    ]
}

/// A Make variable name.
fn variable_name() -> impl Strategy<Value = String> { "[A-Z][A-Z0-9_]{0,7}".prop_map(String::from) }

/// A Make variable value with no `$`, parenthesis, newline or edge whitespace.
///
/// `$` and parentheses are excluded so the value cannot itself look like a
/// reference and be expanded on the second pass; edge whitespace because the
/// reader trims an assignment's value, which would put the oracle and the
/// reader in disagreement over something neither is really claiming.
fn variable_value() -> impl Strategy<Value = String> {
    "[a-z0-9][a-z0-9 _-]{0,10}[a-z0-9]".prop_map(String::from)
}

proptest! {
    /// Scenario: a `lint` recipe line carries an arbitrary sequence of Make's
    /// recipe prefixes before a fixed command.
    /// Invariant: the whole sequence is removed from the command text, and
    /// `ignores_errors` is true exactly when the sequence contained a `-`.
    /// Only `-` changes whether a failure counts; `@` and `+` change echoing
    /// and dry-run behaviour, so reporting either as ignore-errors would fail
    /// a healthy gate, and missing a `-` would pass a broken one.
    #[test]
    fn any_prefix_sequence_is_removed_and_only_a_dash_ignores_errors(
        prefixes in prefix_sequence(),
        spaced in any::<bool>(),
    ) {
        let gap = if spaced { " " } else { "" };
        let makefile = format!("lint:\n\t{prefixes}{gap}{STABLE_COMMAND}\n");

        let commands = recipe_commands(&makefile, "lint")
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(commands.len(), 1, "one line should read as one command");
        prop_assert_eq!(&commands[0].text, STABLE_COMMAND);
        prop_assert_eq!(
            commands[0].ignores_errors,
            prefixes.contains('-'),
            "ignore-errors should track the `-` in `{}`",
            prefixes
        );
    }

    /// Scenario: a `lint` recipe splits one command across backslash
    /// continuations.
    /// Invariant: the fragments read back as a single command, joined by one
    /// space. Make passes a continued line to one shell, so a reader that
    /// returned them separately would judge a fragment as though it were a
    /// command in its own right.
    #[test]
    fn continued_lines_read_back_as_one_joined_command(
        fragments in prop::collection::vec(fragment(), 1..6),
    ) {
        let mut recipe = String::from("lint:\n");
        for (index, piece) in fragments.iter().enumerate() {
            let last = index + 1 == fragments.len();
            recipe.push('\t');
            recipe.push_str(piece);
            if !last {
                recipe.push_str(" \\");
            }
            recipe.push('\n');
        }

        let commands = recipe_commands(&recipe, "lint")
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(commands.len(), 1, "continued lines should read as one command");
        prop_assert_eq!(&commands[0].text, &fragments.join(" "));
        prop_assert!(!commands[0].ignores_errors);
    }

    /// Scenario: a command references one defined and one undefined Make
    /// variable.
    /// Invariant: the defined reference becomes its value and the undefined one
    /// survives intact. Silently dropping an unresolved reference would erase
    /// it from the failure message that has to explain what the reader saw.
    #[test]
    fn a_defined_reference_expands_and_an_undefined_one_survives(
        defined in variable_name(),
        undefined in variable_name(),
        value in variable_value(),
    ) {
        // Distinct names, and neither a prefix of the other, so the reader's
        // line lookup cannot resolve one against the other's assignment.
        prop_assume!(!defined.starts_with(&undefined) && !undefined.starts_with(&defined));
        let makefile = format!("{defined} ?= {value}\n");

        prop_assert_eq!(
            expand_make_variables(&makefile, &format!("$({defined})")),
            value.clone()
        );
        prop_assert_eq!(
            expand_make_variables(&makefile, &format!("$({undefined})")),
            format!("$({undefined})")
        );
        prop_assert_eq!(
            expand_make_variables(&makefile, &format!("run $({defined}) $({undefined})")),
            format!("run {value} $({undefined})")
        );
    }

    /// Scenario: a variable is defined in terms of another.
    /// Invariant: both levels resolve. The reader runs exactly two expansion
    /// passes, which is as deep as this repository's Makefile goes, so this
    /// property deliberately generates two levels and no more.
    #[test]
    fn a_reference_defined_through_another_resolves_both_levels(
        inner in variable_name(),
        outer in variable_name(),
        value in variable_value(),
        tail in variable_value(),
    ) {
        prop_assume!(!inner.starts_with(&outer) && !outer.starts_with(&inner));
        let makefile = format!("{inner} ?= {value}\n{outer} = $({inner}) {tail}\n");

        prop_assert_eq!(
            expand_make_variables(&makefile, &format!("$({outer})")),
            format!("{value} {tail}")
        );
    }
}
