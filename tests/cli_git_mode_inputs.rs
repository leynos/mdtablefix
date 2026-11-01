//! What counts as an input for a mode flag.
//!
//! `--in-place`, `--check`, `--diff` and `--list-files` all require an input,
//! and the `mode` argument group says so. The group is satisfied by typing a
//! member of the `inputs` group, whatever value is typed with it — so a
//! `--git=false` alongside a mode flag satisfied the group while stating that
//! no Git selection was wanted. These tests pin the post-parse rule that closes
//! that gap, in both directions: the combinations that must be rejected, and
//! the ones that must still be accepted.
//!
//! A file of its own rather than more cases in [`cli_git`], because the two
//! files together would otherwise exceed the repository's line limit, and
//! because this is a different claim from the one that file makes: not what
//! `--git` selects, but what the mode group accepts as an input.

use rstest::rstest;

#[path = "support/cli_git_fixture.rs"]
mod fixture;

use fixture::{CLEAN, Fixture, RAGGED, command_in};

/// A mode flag needs an input of its own, and `--git=false` is not one.
///
/// The `mode` group's `requires("inputs")` is satisfied by typing `--git`,
/// whatever value is typed with it, so this path passed parsing while stating
/// that no Git selection was wanted. Standard input was then read and, in a
/// reporting mode, printed and exited 0 — a run that reports nothing and
/// leaves a script believing it passed. Before the optional `=<BOOL>` value
/// the state could not be expressed, which is why the group alone sufficed.
///
/// Input is supplied on standard input precisely because that is the fallback
/// under test: a run that read it would print the formatted table here.
///
/// `--list-files` is deliberately absent: it is caught a step earlier by the
/// `--list-files requires --git` rule, because reading paths is not something a
/// file argument can stand in for either. These three are the modes that
/// otherwise fell through to standard input.
#[rstest]
#[case("--check")]
#[case("--diff")]
#[case("--in-place")]
fn a_mode_flag_with_git_false_and_no_file_is_rejected(#[case] mode: &str) {
    let fixture = Fixture::new();

    let output = command_in(fixture.root())
        .args([mode, "--git=false"])
        .write_stdin(RAGGED)
        .output()
        .expect("run mdtablefix");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr must drive the status"
    );
    assert!(
        output.stdout.is_empty(),
        "a rejected run must print nothing to stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("{mode} requires --git or at least one file")),
        "the rejection must name the mode flag: {stderr}"
    );
}

/// The same mode flags are accepted, and acted on, when a real input is given.
///
/// The positive control for the case above: a rule that rejected every mode
/// flag, or that ignored the value of `--git`, would pass a test that only
/// asserted rejection. Each case also shows the accepted run did its work, so a
/// rule that accepted and then did nothing could not pass either.
/// `--list-files` lists the tracked file without reading it, and `--check`
/// reports no drift on an aligned file and exits 0. All of them act on
/// `notes.md`, which is the file the case names.
#[rstest]
#[case(&["--list-files", "--git=true"], "notes.md\n")]
#[case(&["--list-files", "--git"], "notes.md\n")]
#[case(&["--check", "notes.md"], "")]
fn a_mode_flag_with_a_real_input_does_its_work(
    #[case] args: &[&str],
    #[case] expected_stdout: &str,
) {
    let fixture = Fixture::new();
    fixture.track("notes.md", CLEAN);

    let run = fixture.run(args);

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, expected_stdout);
}

/// `--in-place` with a typed path rewrites it, which is the third kind of input
/// a mode accepts and the only one that changes a document.
///
/// The rejection above covers `--in-place` along with the reporting modes, so
/// the positive control has to cover it too — otherwise a rule that accepted
/// `--in-place --git=false` and dropped the write would still pass.
#[test]
fn in_place_rewrites_the_file_a_mode_flag_was_given() {
    let fixture = Fixture::new();
    fixture.track("notes.md", RAGGED);

    let run = fixture.run(&["--in-place", "notes.md"]);

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(fixture.read("notes.md"), CLEAN);
}
