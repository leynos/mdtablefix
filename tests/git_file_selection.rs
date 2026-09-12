//! Scenario bindings for the `--git` specification.
//!
//! Each binding names one scenario in `tests/features/git_file_selection.feature`,
//! so the feature file stays the readable specification and the fixture work
//! lives in `tests/steps/git_selection.rs`. The declarations must come before
//! the bindings: the step registry is populated as macros expand, and strict
//! compile-time validation would otherwise report every step as missing.
//!
//! The symlink scenario is Unix only, matching its step definition: on Windows
//! `git` checks a symlink out as a plain file holding the target's path, so the
//! scenario would have no subject. Both the step and this binding carry the
//! same gate, so the registry and the feature file stay consistent rather than
//! one referring to a step the other does not define.

#[path = "steps/git_selection.rs"]
mod steps;

use rstest_bdd_macros::scenario;

use crate::steps::GitSelectionState;

/// The per-scenario state every generated test builds for itself.
#[test_macros::allow_fixture_expansion_lints]
#[rstest::fixture]
fn state() -> GitSelectionState { GitSelectionState::default() }

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Reformat tracked Markdown in place and nothing else"
)]
fn reformat_tracked_markdown_in_place(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Extend the selection to untracked files on request"
)]
fn extend_the_selection_to_untracked_files(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "List the selection without acting"
)]
fn list_the_selection_without_acting(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Print the formatted documents instead of writing them"
)]
fn print_the_formatted_documents(#[from(state)] _state: GitSelectionState) {}

#[cfg(unix)]
#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Never write through a symlink"
)]
fn never_write_through_a_symlink(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Restrict the selection to chosen extensions"
)]
fn restrict_the_selection_to_chosen_extensions(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Accept extensions written with a leading dot"
)]
fn accept_extensions_with_a_leading_dot(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Skip a tracked file deleted from the working tree"
)]
fn skip_a_tracked_file_deleted_from_the_working_tree(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Report a candidate that cannot be classified"
)]
fn report_a_candidate_that_cannot_be_classified(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Refuse to rewrite a conflicted file mid-merge"
)]
fn refuse_to_rewrite_a_conflicted_file(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Rewrite a conflicted file when explicitly allowed"
)]
fn rewrite_a_conflicted_file_when_allowed(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Report drift across the repository without changing it"
)]
fn report_drift_across_the_repository(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Report a diff across the repository without changing it"
)]
fn report_a_diff_across_the_repository(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Report a clean repository"
)]
fn report_a_clean_repository(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Exit successfully when nothing is selected"
)]
fn exit_successfully_when_nothing_is_selected(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Scope the selection to the current directory"
)]
fn scope_the_selection_to_the_current_directory(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Report a clear error outside a Git repository"
)]
fn report_a_clear_error_outside_a_repository(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Reject an unusable extension"
)]
fn reject_an_unusable_extension(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Reject combining --git with explicit file arguments"
)]
fn reject_combining_git_with_explicit_files(#[from(state)] _state: GitSelectionState) {}

#[scenario(
    path = "tests/features/git_file_selection.feature",
    name = "Reject --list-files without --git"
)]
fn reject_list_files_without_git(#[from(state)] _state: GitSelectionState) {}
