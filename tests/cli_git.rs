//! End-to-end tests for the `--git` command-line surface.
//!
//! The behavioural scenarios live in `tests/features/git_file_selection.feature`
//! and drive the real binary through `assert_cmd`. This file covers what a
//! scenario cannot state as behaviour: the grammar of the flags themselves, the
//! `--help` rendering, and the properties a reader of a terminal depends on.
//!
//! Every fixture is a real repository built with real `git`, with the ambient
//! configuration neutralised, because the developer's own `core.excludesFile`
//! would otherwise leak into what is selected.

use rstest::rstest;

#[path = "support/cli_git_fixture.rs"]
mod fixture;

// `run_in` is reached from the fixture by every `Fixture::run` call, but the
// only *direct* use here is the check below, which exists only where
// `ExitStatus` renders itself the same way. Importing it unconditionally made
// the Windows build fail `-D unused-imports`, where that test is compiled out.
#[cfg(unix)]
use fixture::run_in;
use fixture::{CLEAN, Fixture, RAGGED, command_in};

/// REQ-GIT-003: `--md-exts` replaces the default set rather than adding to it,
/// a leading dot is optional, and surrounding whitespace is ignored.
#[rstest]
#[case(None, "docs/guide.md\nnotes.markdown\nrules.mdc\n")]
#[case(Some("mdc"), "rules.mdc\n")]
#[case(Some(".MD"), "docs/guide.md\n")]
#[case(Some(" .MD , mdc "), "docs/guide.md\nrules.mdc\n")]
fn md_exts_replaces_the_default_set(#[case] exts: Option<&str>, #[case] expected: &str) {
    let fixture = Fixture::new();
    fixture.track("docs/guide.md", RAGGED);
    fixture.track("notes.markdown", RAGGED);
    fixture.track("rules.mdc", RAGGED);

    let mut args = vec!["--git", "--list-files"];
    if let Some(exts) = exts {
        args.extend_from_slice(&["--md-exts", exts]);
    }
    let run = fixture.run(&args);

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, expected, "with --md-exts {exts:?}");
}

/// A value that could never match is refused as the command line is parsed.
///
/// `mdc.` and `tar.gz` are compared against the segment after a path's final
/// dot, which for `guide.mdc.` is nothing at all and for `notes.tar.gz` is
/// `gz`, so accepting either would end in an empty selection the user had no
/// way to explain from the flag they wrote.
#[rstest]
#[case("mdc.")]
#[case("tar.gz")]
fn a_dotted_extension_is_rejected(#[case] exts: &str) {
    let fixture = Fixture::new();
    fixture.track("docs/guide.md", RAGGED);
    fixture.track("rules.mdc", RAGGED);

    let run = fixture.run(&["--git", "--list-files", "--md-exts", exts]);

    assert_eq!(run.status, 2, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty(), "stdout: {:?}", run.stdout);
    assert!(
        run.stderr
            .contains(&format!("extension \"{exts}\" contains a dot")),
        "the rejection must name the value and the reason: {}",
        run.stderr
    );
}

/// REQ-GIT-004: `--git` and positional file arguments are mutually exclusive,
/// and the rejection is a `clap` error rather than a run-time one.
#[test]
fn rejects_git_with_explicit_files() {
    let fixture = Fixture::new();
    fixture.track("notes.md", RAGGED);

    let run = fixture.run(&["--git", "notes.md"]);

    assert_eq!(run.status, 2, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty(), "stdout: {:?}", run.stdout);
    assert!(
        run.stderr.contains("--git") && run.stderr.contains("cannot be used with"),
        "the rejection must name the conflict: {}",
        run.stderr
    );
}

/// The other half of REQ-GIT-005: `--in-place` with no selection at all is
/// still an error, so `--git` is a way to satisfy the requirement rather than
/// a way to bypass it.
#[test]
fn in_place_without_inputs_is_rejected() {
    let fixture = Fixture::new();

    let run = fixture.run(&["--in-place"]);

    assert_eq!(run.status, 2, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty(), "stdout: {:?}", run.stdout);
}

/// REQ-GIT-005: `--in-place` is satisfied by `--git` alone, and rewrites the
/// files the selection names without being told them.
#[test]
fn in_place_is_satisfied_by_git() {
    let fixture = Fixture::new();
    fixture.track("docs/guide.md", RAGGED);
    fixture.track("src/lib.rs", RAGGED);

    let run = fixture.run(&["--git", "--in-place"]);

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(fixture.read("docs/guide.md"), CLEAN);
    assert_eq!(
        fixture.read("src/lib.rs"),
        RAGGED,
        "a file outside the extension set must be left alone"
    );
}

/// The dependencies `clap` cannot express, checked after parsing instead.
///
/// Measured on clap 4.6.6: `requires = "git"` accepts `--list-files notes.md`,
/// because the positional satisfies the requirement the flag was supposed to
/// impose. Every one of these must therefore be rejected by the post-parse
/// check, with the exit status and usage footer of a parser error.
#[rstest]
#[case(&["--include-untracked"], "--include-untracked")]
#[case(&["--include-untracked=false", "notes.md"], "--include-untracked")]
#[case(&["--allow-conflicted"], "--allow-conflicted")]
#[case(&["--allow-conflicted=false", "notes.md"], "--allow-conflicted")]
#[case(&["--list-files", "notes.md"], "--list-files")]
// The flag is judged the same way as the switches above, and its default is why:
// the check is keyed on where the value came from rather than on whether one is
// present.
#[case(&["--md-exts", "md"], "--md-exts")]
fn a_git_only_flag_without_git_is_rejected(#[case] args: &[&str], #[case] flag: &str) {
    let fixture = Fixture::new();

    let run = fixture.run(args);

    assert_eq!(run.status, 2, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty(), "stdout: {:?}", run.stdout);
    assert!(
        run.stderr.contains(&format!("{flag} requires --git")),
        "the rejection must name the flag: {}",
        run.stderr
    );
}

/// The other direction: a flag that resolves to false because nobody typed it
/// must not demand `--git`, and a typed `=false` given *with* `--git` is
/// accepted.
///
/// The first half is what the rejection cases cannot show: a value-based check
/// would pass it too, so it stands as the control that the provenance check did
/// not simply reject everything. The second half is the pairing the rejection
/// cases deliberately omit, and it proves the `=false` rejection above is about
/// `--git` rather than about the value.
#[rstest]
#[case(&["notes.md"])]
#[case(&["--git", "--include-untracked=false"])]
#[case(&["--git", "--allow-conflicted=false"])]
fn a_git_only_flag_typed_false_is_accepted_with_git(#[case] args: &[&str]) {
    let fixture = Fixture::new();
    fixture.track("notes.md", RAGGED);

    let run = fixture.run(args);

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
}

/// The default extension set is still the default when the flag is absent from
/// a successful run, which is what makes the replacement above observable.
#[test]
fn the_default_extension_set_is_md_mdc_and_markdown() {
    let fixture = Fixture::new();
    fixture.track("guide.md", RAGGED);
    fixture.track("rules.mdc", RAGGED);
    fixture.track("notes.markdown", RAGGED);
    fixture.track("prose.txt", RAGGED);

    let run = fixture.run(&["--git", "--list-files"]);

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout, "guide.md\nnotes.markdown\nrules.mdc\n",
        "the default set is md, mdc, and markdown; `prose.txt` is tracked and must not be listed"
    );
}

/// REQ-GIT-010: `--list-files` reads no file's content.
///
/// A tracked file whose bytes are not UTF-8 is the evidence with teeth: it is
/// listed successfully, where any mode that read it would fail to decode it.
/// The property is discharged without depending on file permissions, so the
/// test means the same thing when it runs as another user.
#[test]
fn listing_a_file_does_not_read_its_content() {
    let fixture = Fixture::new();
    fixture.track("docs/guide.md", b"\xff\xfe not UTF-8 \xff");

    let run = fixture.run(&["--git", "--list-files"]);

    assert_eq!(
        run.status, 0,
        "listing must not read the file: {}",
        run.stderr
    );
    assert_eq!(run.stdout, "docs/guide.md\n");
}

/// A `--git` failure outside a repository is one deliberate line, naming the
/// command the user could run and relaying git's own diagnostic beside it.
///
/// The fixture directory is outside any repository, which this suite already
/// requires: the feature file's "outside a Git repository" scenario turns on
/// the same property, and `make mutants` points `TMPDIR` outside the worktree
/// so that it holds there too.
///
/// Unix only, because the relayed status is rendered by `ExitStatus`, which
/// reads `exit status: 12` here and something else on Windows.
#[cfg(unix)]
#[test]
fn a_git_failure_is_reported_as_one_deliberate_line() {
    let elsewhere = tempfile::tempdir().expect("create temporary directory");

    let run = run_in(elsewhere.path(), &["--git"]);

    assert_eq!(run.status, 2, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty(), "stdout: {:?}", run.stdout);
    assert!(
        run.stderr
            .starts_with("mdtablefix: `git ls-files` failed with exit status: "),
        "the diagnostic must open with this tool's own wording: {}",
        run.stderr
    );
    assert_eq!(
        run.stderr.lines().count(),
        1,
        "git's diagnostic must be relayed as one line: {}",
        run.stderr
    );
}

/// The flags this milestone added are documented, and the help text as a whole
/// is reviewed rather than skimmed: a new option changes the rendering of every
/// line below it.
#[test]
fn help_documents_the_git_flags() {
    // The help rendering does not depend on the working directory, so the
    // fixture's hardened environment is reused without building a repository.
    let output = command_in(&std::env::temp_dir())
        .arg("--help")
        .output()
        .expect("run mdtablefix --help");
    assert!(output.status.success(), "--help must succeed");

    let help = String::from_utf8(output.stdout).expect("the help text is UTF-8");
    // Windows names the binary `mdtablefix.exe`, and clap prints the program
    // as `argv[0]` spells it. The snapshot is about the flags and the whole
    // rendering below them, so the platform's suffix is removed rather than
    // recorded.
    let help = help.replace("mdtablefix.exe", "mdtablefix");
    for flag in [
        "--git",
        "--include-untracked",
        "--md-exts",
        "--list-files",
        "--allow-conflicted",
    ] {
        assert!(help.contains(flag), "{flag} must appear in --help");
    }

    insta::with_settings!({
        snapshot_path => "snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!("cli_git_help", help);
    });
}
