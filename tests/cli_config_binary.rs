//! The process-level configuration contracts: what a run formats and reports.
//!
//! The in-process tables in `src/config_tests.rs` prove which layer wins. This
//! file and [`cli_config_failure_binary`] prove what a user, a script, or a CI
//! job observes, which only a child process can show: the status the shell sees
//! and which stream each line went to. Each fixture is a real file on disk, and
//! every child's configuration base directories point into a temporary tree, so
//! the developer's own `~/.config/mdtablefix/config.toml` cannot decide a
//! result.
//!
//! This half covers *input selection* — which files a run acts on, and what it
//! reports — because a configured `git` is the setting that changes the input
//! source. The failure vocabulary, including the malformed files that exit 2,
//! lives in [`cli_config_failure_binary`].

#[path = "support/cli_config_fixture.rs"]
mod fixture;

use fixture::{Fixture, RAGGED, assert_clean_success};

/// The ragged table above, already aligned: what a formatting run must produce.
///
/// Used by every case that checks output rather than a status or a stream, and
/// only by this binary, so it lives here rather than in the shared fixture.
const CLEAN: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// A run with no configuration formats exactly as it did before the adoption.
///
/// The output is pinned character for character, so a changed default — a
/// switch that arrives enabled, or an extension list that narrows — fails here
/// rather than in a downstream repository.
#[test]
fn a_run_with_no_configuration_formats_as_it_always_did() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();

    let run = fixture.run_selected(&empty, &[], RAGGED);

    assert_clean_success(&run, CLEAN);
}

/// A discovered file can switch a transformation on, and nothing else changes.
#[test]
fn a_discovered_file_supplies_a_setting() {
    let fixture = Fixture::new();
    fixture.write("xdg/mdtablefix/config.toml", "ellipsis = true\n");

    let run = fixture.run(&[], "a...b\n");

    assert_clean_success(&run, "a…b\n");
}

/// A selected file can switch a transformation on, and nothing else changes.
#[test]
fn a_selected_file_supplies_a_setting() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "ellipsis = true\n");

    let run = fixture.run_selected(&config, &[], "a...b\n");

    assert_clean_success(&run, "a…b\n");
}

/// A reporting mode keeps its split: findings on stdout, summary on stderr.
///
/// The drift is real, so the exit status is 1 rather than 2 — a configuration
/// error is the only failure that exits 2 without reporting a file.
#[test]
fn check_reports_drift_on_standard_output_and_sums_on_standard_error() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();
    fixture.write("drift.md", RAGGED);

    let run = fixture.run_selected(&empty, &["--check", "drift.md"], "");

    assert_eq!(run.status, 1);
    assert_eq!(run.stdout, "drift.md +3 -3\n");
    assert_eq!(run.stderr, "1 file would be reformatted.\n");
}

/// `--md-exts` typed without `--git` is a usage error that exits 2.
///
/// This is the negative half of the typed-flag contract: the flag's *provenance*
/// decides, so a value supplied by a file or the environment does not trigger
/// it. The positive half is [`a_configured_extension_list_is_not_a_typed_flag`].
#[test]
fn a_typed_extension_list_without_git_exits_two() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();

    let run = fixture.run_selected(&empty, &["--md-exts", "toml"], "");

    assert_eq!(run.status, 2);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr.contains("--md-exts requires --git"),
        "stderr: {}",
        run.stderr
    );
}

/// A configured extension list is not a typed flag, so it needs no `--git`.
///
/// The same value that fails on the command line succeeds here, because nothing
/// the user typed brought it. The run deliberately formats a standard input
/// rather than asking for `--version`: clap answers a version request before
/// `Settings::resolve` is reached, so that spelling would pass whether or not
/// the typed-flag rule were correct.
#[test]
fn a_configured_extension_list_is_not_a_typed_flag() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "md_exts = [\"toml\"]\n");

    let run = fixture.run_selected(&config, &[], RAGGED);

    assert_clean_success(&run, CLEAN);
}

/// A configured `git` drives selection on a bare invocation.
///
/// The reverse of the flag contract: the setting is not typed, so clap's
/// `inputs` group is not satisfied by it, yet the resolved settings select from
/// the repository. Outside one, that selection fails with this tool's own
/// diagnostic rather than falling back to standard input no one offered.
#[test]
fn a_configured_git_selection_runs_without_the_flag() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "git = true\n");

    let run = fixture.run_selected(&config, &[], RAGGED);

    assert_eq!(run.status, 2);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr.contains("git ls-files"),
        "stderr: {}",
        run.stderr
    );
}

/// A configured `git` does not satisfy clap's `inputs` group.
///
/// `--list-files` is a git-only flag, so typing it without `--git` is a usage
/// error however the setting arrived. A configuration file cannot make a
/// command line valid.
#[test]
fn a_configured_git_selection_does_not_satisfy_the_inputs_group() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "git = true\n");

    let run = fixture.run_selected(&config, &["--list-files"], "");

    assert_eq!(run.status, 2);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr.contains("required arguments were not provided"),
        "stderr: {}",
        run.stderr
    );
}

/// A positional path outranks a configured `git`, and no repository is touched.
///
/// The file is formatted from its path, so a configured `git` must not turn the
/// run into a repository query that would fail outside one.
#[test]
fn a_positional_path_outranks_a_configured_git_selection() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "git = true\n");
    fixture.write("notes.md", RAGGED);

    let run = fixture.run_selected(&config, &["notes.md"], "");

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, CLEAN);
}

/// A project file outranks a user file, and both are read.
///
/// The scope stack is the other ordering from the candidate list within one
/// scope, and it is the one the users' guide's location table does not show: a
/// run from a directory holding `.mdtablefix.toml` applies that file after the
/// user file, so the nearer file wins. The project file is written at the
/// fixture root, which is also the child's working directory.
#[test]
fn a_project_file_outranks_a_user_file() {
    let fixture = Fixture::new();
    fixture.write("xdg/mdtablefix/config.toml", "ellipsis = true\n");
    fixture.write(".mdtablefix.toml", "ellipsis = false\n");

    let run = fixture.run(&[], "a...b\n");

    assert_clean_success(&run, "a...b\n");
}

/// The help rendering gains no configuration surface.
///
/// A `--help` that grew a configuration section or a new flag would be a
/// user-visible change the issue did not ask for; the snapshot file pins the
/// full text, and this states the claim where the adoption's own tests live.
#[test]
fn help_still_describes_only_the_command_line() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();

    let run = fixture.run_selected(&empty, &["--help"], "");

    assert_eq!(run.status, 0);
    assert!(
        run.stdout.starts_with("Reflow broken markdown tables"),
        "{}",
        run.stdout
    );
    assert!(
        !run.stdout.contains("MDTABLEFIX_CONFIG_PATH"),
        "{}",
        run.stdout
    );
    assert!(!run.stdout.contains("configuration"), "{}", run.stdout);
}
