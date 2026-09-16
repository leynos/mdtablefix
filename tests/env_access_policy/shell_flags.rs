//! Fixtures for the `.SHELLFLAGS` half of the recipe guard.
//!
//! The Makefile this repository ships sets neither `.ONESHELL` nor
//! `.SHELLFLAGS`, so the assertion in the parent file exercises the guard in
//! one direction only: it passes because there is nothing to judge. These cases
//! supply the Makefiles that would have to be judged, in both directions, since
//! a guard nothing can fail is a guard nobody has tested.

use anyhow::{Result, ensure};
use rstest::rstest;

use super::{aborts_on_error, declares_one_shell, effective_shellflags, shell_aborts_on_error};

/// Scenario: a Makefile that puts its whole recipe in one shell is read for
/// whether that shell aborts on the first failure.
///
/// Invariant: only a final `.SHELLFLAGS` value carrying `-e` says it does.
/// A variable whose name merely begins with `.SHELLFLAGS` is an ordinary
/// variable; an assignment a later one replaces is not the value Make runs
/// with; and an unset variable leaves Make's default `-c`, which does not
/// abort.
///
/// Mutation proof (2026-09-15), each applied alone to
/// [`effective_shellflags`] and reverted:
///
/// - restoring the prefix search, `strip_prefix(".SHELLFLAGS")` over every line with `.any(...)`,
///   passes `similarly_named_variable` and `later_assignment_removes_it`, which is the whole of
///   this rule;
/// - reporting the first assignment rather than the last fails `later_assignment_adds_it`;
/// - treating `+=` as a plain assignment fails `appended_after_errexit`, whose appended flag is not
///   the one that aborts. `appended_later` does not discriminate that mutation, because its
///   appended flag is `-e` itself;
/// - treating `?=` as a plain assignment fails `conditional_after_a_value`.
///
/// And on [`aborts_on_error`], each applied alone and reverted: comparing a
/// whole token to `-e`, as the guard did before, fails `bundled_with_pipefail`
/// and `named_option`; reporting any token containing the letter `e` fails
/// `a_different_named_option`, whose argument is `pipefail`.
#[rstest]
#[case::unset("lint:\n\tcargo clippy\n", false)]
#[case::plain(".SHELLFLAGS := -e -c\n", true)]
#[case::similarly_named_variable(".SHELLFLAGS_NOTE := -e is missing here\n", false)]
#[case::later_assignment_removes_it(".SHELLFLAGS := -e -c\n.SHELLFLAGS := -c\n", false)]
#[case::later_assignment_adds_it(".SHELLFLAGS := -c\n.SHELLFLAGS := -e -c\n", true)]
#[case::appended_later(".SHELLFLAGS := -c\n.SHELLFLAGS += -e\n", true)]
#[case::appended_after_errexit(".SHELLFLAGS := -e -c\n.SHELLFLAGS += -u\n", true)]
#[case::conditional_after_a_value(".SHELLFLAGS := -c\n.SHELLFLAGS ?= -e -c\n", false)]
#[case::conditional_when_unset(".SHELLFLAGS ?= -e -c\n", true)]
#[case::trailing_comment(".SHELLFLAGS := -c # -e would go here\n", false)]
#[case::recipe_line("lint:\n\t.SHELLFLAGS := -e -c\n", false)]
#[case::bundled_with_pipefail(".SHELLFLAGS := -eo pipefail -c\n", true)]
#[case::named_option(".SHELLFLAGS := -o errexit -c\n", true)]
#[case::a_different_named_option(".SHELLFLAGS := -o pipefail -c\n", false)]
#[case::a_bundle_without_it(".SHELLFLAGS := -uc\n", false)]
fn only_a_final_value_carrying_dash_e_aborts(
    #[case] makefile: &str,
    #[case] aborts: bool,
) -> Result<()> {
    let read = shell_aborts_on_error(makefile);
    ensure!(
        read == aborts,
        "expected aborts={aborts} for {makefile:?}, effective flags {:?}",
        effective_shellflags(makefile)
    );
    Ok(())
}

/// Scenario: one `.SHELLFLAGS` value is read for whether it aborts.
///
/// Invariant: `errexit` counts however it is spelled, and nothing else does.
/// Stated apart from the Makefile fold above so that a change to either half
/// fails on its own terms.
#[rstest]
#[case::dash_e("-e -c", true)]
#[case::bundle("-eo pipefail -c", true)]
#[case::named("-o errexit -c", true)]
#[case::default_only("-c", false)]
#[case::another_named_option("-o pipefail -c", false)]
#[case::plus_e("+e -c", false)]
#[case::an_argument_containing_e("-c errexit", false)]
fn errexit_counts_however_it_is_spelled(#[case] flags: &str, #[case] aborts: bool) -> Result<()> {
    ensure!(
        aborts_on_error(flags) == aborts,
        "expected aborts={aborts} for {flags:?}"
    );
    Ok(())
}

/// Scenario: a Makefile is read for whether it declares `.ONESHELL`.
///
/// Invariant: the declaration is recognised however Make would parse it, and
/// nothing else is. Make separates a target from its colon with optional
/// whitespace, so `.ONESHELL :` puts the recipe in one shell just as
/// `.ONESHELL:` does; a test for the two characters together reads the spaced
/// spelling as an ordinary line and skips the `.SHELLFLAGS` guard entirely.
///
/// Mutation proof (2026-09-15), each applied alone to [`declares_one_shell`]
/// and reverted: restoring `starts_with(".ONESHELL:")` fails `spaced_colon`
/// and `tab_before_colon`; dropping the colon test altogether, so any line
/// beginning with the name counts, fails `similarly_named_variable`.
#[rstest]
#[case::absent("lint:\n\tcargo clippy\n", false)]
#[case::plain(".ONESHELL:\n", true)]
#[case::spaced_colon(".ONESHELL :\n", true)]
#[case::tab_before_colon(".ONESHELL\t:\n", true)]
#[case::indented("  .ONESHELL:\n", true)]
#[case::with_a_prerequisite(".ONESHELL: lint\n", true)]
#[case::similarly_named_variable(".ONESHELL_NOTE := not a declaration\n", false)]
#[case::mentioned_in_a_comment("# .ONESHELL would go here\n", false)]
fn one_shell_is_recognised_however_make_would_parse_it(
    #[case] makefile: &str,
    #[case] declared: bool,
) -> Result<()> {
    ensure!(
        declares_one_shell(makefile) == declared,
        "expected declared={declared} for {makefile:?}"
    );
    Ok(())
}
