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

use fixture::{Fixture, RAGGED, assert_clean_success, assert_failure};

/// The ragged table above, already aligned: what a formatting run must produce.
///
/// Used by every case that checks output rather than a status or a stream, and
/// only by this binary, so it lives here rather than in the shared fixture.
const CLEAN: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// One line per transformation, each written so that switching it on changes
/// the line and switching it off leaves it alone.
///
/// Every transformation switch the configuration schema carries has a witness
/// here, so a default that arrived enabled is visible per key rather than as
/// one opaque diff. The eight transformations a run reaches from standard
/// input are all present: `wrap`, `renumber`, `breaks`, `ellipsis`, `fences`,
/// `footnotes`, `code_emphasis` and `headings`. Every one defaults to off on
/// the command line, which is why the answer below is the witness unchanged,
/// and [`every_transformation_switch_changes_its_witness`] proves that each
/// witness line is one its switch really edits.
///
/// The three switches that do *not* appear are `git`, `include_untracked` and
/// `allow_conflicted`: all describe which files a run reads, not what it does
/// to a stream, so a run reading standard input cannot witness them. They are
/// covered by the precedence table in `src/config_tests.rs`, and `git`
/// additionally by [`a_configured_git_selection_runs_without_the_flag`].
const DEFAULTS_WITNESS: &str = concat!(
    "A paragraph that is short.\n",
    "\n",
    "A paragraph that is long enough to be reflowed by the wrapping pass, which measures each \
     line against eighty columns and breaks it apart.\n",
    "\n",
    "1. First\n",
    "1. Second\n",
    "\n",
    "***\n",
    "\n",
    "An ellipsis...\n",
    "\n",
    "~~~~\nfenced\n~~~~\n",
    "\n",
    "See the docs.1 for details.\n",
    "\n",
    "a**`b`**c\n",
    "\n",
    "Setext heading\n",
    "==============\n",
);

/// What [`DEFAULTS_WITNESS`] formats to when every default holds.
///
/// Spelled out rather than derived from the witness, so a reader can see that
/// nothing moved: every transformation is off, so `***`, `...`, `~~~~`,
/// `docs.1`, `` a**`b`**c `` and the Setext pair all come back untouched. The
/// one transformation that edits by default is table alignment, and the
/// witness deliberately holds no table — [`RAGGED`] covers that separately.
///
/// The witness therefore equals the answer byte-for-byte, which is the claim
/// the case makes. What stops that from being vacuous is that each line is
/// *changed* by its own switch: `--wrap` breaks the long paragraph,
/// `--renumber` makes the second item `2.`, `--breaks` rewrites `***`,
/// `--ellipsis` writes `…`, `--fences` writes backticks, `--footnotes` writes
/// `docs.[^1]`, `--code-emphasis` writes `` a`b`c ``, and `--headings` writes
/// `# Setext heading`. A default that arrived enabled anywhere would show up
/// here as a difference between the two constants.
const DEFAULTS_ANSWER: &str = concat!(
    "A paragraph that is short.\n",
    "\n",
    "A paragraph that is long enough to be reflowed by the wrapping pass, which measures each \
     line against eighty columns and breaks it apart.\n",
    "\n",
    "1. First\n",
    "1. Second\n",
    "\n",
    "***\n",
    "\n",
    "An ellipsis...\n",
    "\n",
    "~~~~\nfenced\n~~~~\n",
    "\n",
    "See the docs.1 for details.\n",
    "\n",
    "a**`b`**c\n",
    "\n",
    "Setext heading\n",
    "==============\n",
);

/// A run with no configuration at all formats exactly as it did before.
///
/// No selector, no configuration variable, and no candidate anywhere in the
/// tree the child searches: discovery runs and finds nothing, which is the only
/// spelling that exercises the absence path. Naming an empty selected file
/// would instead short-circuit discovery and prove only that the loader can
/// read a file that states nothing.
///
/// The input is one line per transformation a default could have flipped, so a
/// switch that arrived enabled, or a default that narrowed, changes the output.
/// `RAGGED` is deliberately not used: it distinguishes alignment and nothing
/// else.
#[test]
fn a_run_with_no_configuration_at_all_formats_as_it_always_did() {
    let fixture = Fixture::new();

    let run = fixture.run(&[], DEFAULTS_WITNESS);

    assert_clean_success(&run, DEFAULTS_ANSWER);
}

/// Every switch that has a witness line really rewrites that line.
///
/// The negative control for the case above. If a witness line were inert —
/// because the syntax it probes had changed, or because the pass had stopped
/// reaching it — then "the default leaves it alone" would be true of any
/// implementation, including one whose defaults had all flipped. This proves
/// the opposite direction for each line separately, so the pair together say
/// that the defaults are *exactly* off rather than merely looking untouched.
///
/// `--wrap` reflows rather than edits one line, and `--fences` edits two, so
/// the assertion is that the output differs from the unswitched run, not that
/// it differs by one line.
#[rstest::rstest]
#[case("--wrap")]
#[case("--renumber")]
#[case("--breaks")]
#[case("--ellipsis")]
#[case("--fences")]
#[case("--footnotes")]
#[case("--code-emphasis")]
#[case("--headings")]
fn every_transformation_switch_changes_its_witness(#[case] flag: &str) {
    let fixture = Fixture::new();

    let bare = fixture.run(&[], DEFAULTS_WITNESS);
    let switched = fixture.run(&[flag], DEFAULTS_WITNESS);

    assert_clean_success(&bare, DEFAULTS_ANSWER);
    assert_eq!(switched.status, 0, "stderr: {}", switched.stderr);
    assert!(switched.stderr.is_empty(), "stderr: {}", switched.stderr);
    assert_ne!(
        switched.stdout, bare.stdout,
        "{flag} left the witness untouched, so the line it is meant to witness proves nothing"
    );
}

/// An empty selected file is not the same run, and still formats unchanged.
///
/// Kept beside the case above because the two are easy to confuse and prove
/// different things: this one shows a selected file that supplies nothing
/// leaves every default in place, while absence is proved by the case above.
#[test]
fn an_empty_selected_file_leaves_every_default_in_place() {
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

    assert_failure(&run, 2, "--md-exts requires --git");
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

    assert_failure(&run, 2, "git ls-files");
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

    assert_failure(&run, 2, "required arguments were not provided");
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

/// A typed `--flag=false` turns a configured switch off, exit 0.
///
/// The interface issue #666 named: a repository records its rule set once and
/// an invocation switches one rule off. The file says `ellipsis = true`, which
/// [`a_discovered_file_supplies_a_setting`] proves applies, and the command
/// line says `--ellipsis=false`, so an implementation that ignored the value
/// would print the ellipsis and fail here.
#[test]
fn a_typed_false_turns_a_configured_switch_off() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "ellipsis = true\n");

    let run = fixture.run_selected(&config, &["--ellipsis=false"], "a...b\n");

    assert_clean_success(&run, "a...b\n");
}

/// A bare switch still means `true`, and `--flag=true` means the same.
///
/// The additive half of the syntax: the value is optional, so a command line
/// written before it existed is unchanged. Both spellings are checked against
/// the same input, so a parser that treated the presence of `=` as significant
/// would fail one of them.
#[test]
fn a_bare_switch_and_a_typed_true_agree() {
    let fixture = Fixture::new();

    let bare = fixture.run(&["--ellipsis"], "a...b\n");
    let typed = fixture.run(&["--ellipsis=true"], "a...b\n");

    assert_clean_success(&bare, "a…b\n");
    assert_clean_success(&typed, "a…b\n");
}

/// A non-boolean value for a switch is still a usage error, exit 2.
///
/// `--wrap=80` was rejected before the optional value existed and must stay
/// rejected: the flag names a transformation, not a width. The diagnostic now
/// comes from the boolean parser rather than from "takes no value", so the
/// status and the stream are the contract and the wording is not pinned.
#[test]
fn a_non_boolean_switch_value_is_a_usage_error() {
    let fixture = Fixture::new();

    let run = fixture.run(&["--wrap=80"], "alpha beta\n");

    assert_failure(&run, 2, "'80'");
}
