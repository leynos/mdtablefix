//! The process-level failure contracts: exit status and stream split.
//!
//! The companion to [`cli_config_binary`], which covers what a run formats.
//! Both read the same fixture and scrub the same variables; this half covers
//! the cases that must fail, because a diagnostic a user cannot see, or one
//! printed to standard output, is a different contract from the formatting
//! itself.
//!
//! Every assertion here names the stream, not just the status: a configuration
//! diagnostic on standard output would corrupt the document a pipeline is
//! reading, and a status alone would not notice.

#[path = "support/cli_config_fixture.rs"]
mod fixture;

#[cfg(unix)]
#[path = "support/permission_denial.rs"]
mod permission_denial;

#[cfg(unix)]
#[path = "support/candidate_refusals.rs"]
mod candidate_refusals;

use fixture::{Fixture, RAGGED, assert_clean_success, assert_failure};
#[cfg(unix)]
use permission_denial::{allow_read, deny_read, permissions_are_ignored};

/// A selected file that is absent is an error on standard error, exit 2.
#[test]
fn a_missing_selected_file_exits_two_with_no_standard_output() {
    let fixture = Fixture::new();
    let missing = fixture.root().join("absent.toml");

    let run = fixture.run_selected(&missing, &["--check", "drift.md"], "");

    assert_failure(&run, 2, &["required configuration file not found"]);
}

/// A selected file that is not valid TOML is an error, exit 2.
#[test]
fn a_malformed_selected_file_exits_two_with_no_standard_output() {
    let fixture = Fixture::new();
    let config = fixture.write("broken.toml", "wrap = \n");

    let run = fixture.run_selected(&config, &["--check", "drift.md"], "");

    assert_failure(
        &run,
        2,
        &["cannot resolve configuration", "TOML parse error"],
    );
}

/// An `extends` parent outside the working directory is refused unread.
///
/// The parent is deliberately *not* valid TOML: were it read, the loader would
/// quote its first line in the diagnostic, which is the disclosure this
/// refusal exists to prevent. Its bytes stand in for a credentials file. The
/// assertion that the marker never appears is therefore the contract, not an
/// incidental detail — a pass that merely checked the exit status would be
/// satisfied by the very leak being closed.
///
/// The parent lives in a second temporary directory *beside* the fixture tree,
/// and the `..` in the configuration file therefore really does climb out of
/// the working directory to a file that exists. Both temporary directories
/// share a parent, which is what lets one relative path reach the other. A
/// parent written inside the tree would leave the `..` pointing at nothing,
/// and the refusal could then be mistaken for a resolution failure that never
/// read anything because there was nothing to read.
#[test]
fn an_extends_parent_outside_the_tree_is_refused_without_being_read() {
    let fixture = Fixture::new();
    let neighbouring = tempfile::tempdir().expect("create a directory beside the fixture tree");
    let neighbour = neighbouring
        .path()
        .file_name()
        .expect("the temporary directory has a name")
        .to_str()
        .expect("the name is valid UTF-8");
    std::fs::write(
        neighbouring.path().join("credentials"),
        "aws_secret_access_key = hunter2\n",
    )
    .expect("write the credential file");
    fixture.write(
        ".mdtablefix.toml",
        format!("extends = \"../{neighbour}/credentials\"\nwrap = true\n"),
    );

    // The non-vacuity check: resolved from the working directory, the relative
    // parent really is a readable file. Without this the run would fail for the
    // uninteresting reason that there was nothing to leak.
    let reachable = fixture
        .root()
        .join("..")
        .join(neighbour)
        .join("credentials");
    assert!(
        reachable.is_file(),
        "the parent must be readable, or the refusal proves nothing: {}",
        reachable.display()
    );

    let run = fixture.run(&[], "a...b\n");

    assert_failure(&run, 2, &["does not support"]);
    // The disclosure the refusal exists to prevent, checked separately because
    // it is an absence rather than another needle.
    assert!(
        !run.stderr.contains("hunter2"),
        "the parent's content must never reach the log: {}",
        run.stderr
    );
}

/// The refusal is keyed on the parsed structure, not on the text.
///
/// A file that mentions the key in a comment or as a nested value is ordinary
/// configuration, and refusing it would make an unexplained failure out of a
/// document the schema ignores.
#[test]
fn a_mention_of_extends_without_the_key_loads_normally() {
    let fixture = Fixture::new();
    fixture.write(
        ".mdtablefix.toml",
        "# this repository does not use extends\n[notes]\nextends = \"unused\"\n",
    );

    let run = fixture.run(&[], "a...b\n");

    assert_clean_success(&run, "a...b\n");
}

/// A malformed project file is reported even when a user file loaded.
///
/// The framework classifies an automatic candidate as optional and withholds an
/// optional failure whenever any scope loaded, so without the promotion in
/// `src/config.rs` this run would exit 0 on the user settings and never mention
/// the broken file. That is the destructive surprise the adoption exists to
/// prevent: the user would be formatted by a rule set they cannot see, taken
/// from a file that is not the one they are editing.
///
/// The user file is written under `XDG_CONFIG_HOME` and the project file at the
/// fixture root, which is the child's working directory.
#[test]
fn a_malformed_project_file_is_reported_beside_a_valid_user_file() {
    let fixture = Fixture::new();
    fixture.write("xdg/mdtablefix/config.toml", "ellipsis = true\n");
    fixture.write(".mdtablefix.toml", "wrap = \n");

    let run = fixture.run(&[], "a...b\n");

    assert_failure(&run, 2, &[".mdtablefix.toml", "TOML parse error"]);
}

/// A valid project file beside a valid user file is not an error.
///
/// The negative control for the case above: a promotion that reported every
/// automatic candidate, rather than only the ones that failed to load, would
/// fail here. Both files load, so nothing is withheld and nothing is reported.
#[test]
fn a_valid_project_file_beside_a_valid_user_file_is_quiet() {
    let fixture = Fixture::new();
    fixture.write("xdg/mdtablefix/config.toml", "ellipsis = true\n");
    fixture.write(".mdtablefix.toml", "wrap = true\n");

    let run = fixture.run(&[], "a...b\n");

    assert_clean_success(&run, "a…b\n");
}

/// An environment value the merge cannot type is an error, exit 2.
///
/// A bare word is a plain string in the provider's dialect, and a string is not
/// a sequence, so this is the shape a user reaches by writing a list without its
/// brackets.
#[test]
fn an_untypable_environment_value_exits_two() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();

    let mut command = fixture.command();
    let run = Fixture::finish(
        command
            .env("MDTABLEFIX_CONFIG_PATH", &empty)
            .env("MDTABLEFIX_MD_EXTS", "rst xml")
            .args(["--git", "--list-files"]),
    );

    assert_failure(&run, 2, &["expected a sequence"]);
}

/// A configured extension list the tool cannot use names its key, exit 2.
///
/// Both cases pass the merge — the list is a sequence of strings — and are
/// rejected afterwards by the same parser `--md-exts` uses, which is why the
/// diagnostic names the key rather than relaying a TOML or type error. A
/// rejected member is one the typed `--md-exts` would have refused at parse
/// time. An empty list is valid TOML whose every member — there are none —
/// parses, so only the explicit length check catches it; without that check a
/// `--git` run would select nothing and report it as a successful, empty run,
/// which is indistinguishable from "no files need changing" — exactly the
/// silence a configuration mistake should not be able to produce.
///
/// The run supplies a positional path, so the configured list is never used for
/// selection. It is still rejected: every setting is validated whether or not
/// the resolved mode consults it, because a configuration file that is wrong is
/// worth fixing even on a run that would not have read it — and because a
/// validation that depended on the mode would let a typo surface only on some
/// invocations.
#[rstest::rstest]
#[case::rejected("md_exts = [\"a.b\"]\n", "invalid configuration value for md_exts")]
#[case::empty(
    "md_exts = []\n",
    "invalid configuration value for md_exts: the extension list must name at least one"
)]
fn an_unusable_extension_list_names_its_key(
    #[case] contents: &'static str,
    #[case] needle: &'static str,
) {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", contents);
    fixture.write("notes.md", RAGGED);

    let run = fixture.run_selected(&config, &["notes.md"], "");

    assert_failure(&run, 2, &[needle]);
}

/// A configuration file that exists but cannot be read is an error, exit 2.
///
/// A selected file is the case the missing-file test above is satisfied by: a
/// loader that refuses only what is absent passes there, while here the path
/// exists and only a real failed read produces the status.
/// `ExplicitMode::RequiredExclusive` is what makes it fatal — a named file is a
/// statement of intent, so a file the tool cannot honour is not one it may
/// silently replace with the defaults.
///
/// A discovered project file is the other shape, and it proves the promotion in
/// `src/config.rs`: the framework classifies an automatic candidate as optional
/// and withholds the failure whenever any scope loaded, so a locked project file
/// beside a readable user file would otherwise leave the run formatting on
/// settings the user cannot see. The user file is the readable one, so the
/// precedence order is the real one — the locked file is the *nearer* scope —
/// and a run that ignored it would silently apply the user file instead.
///
/// The mode is `0o000` rather than a read-only directory, because the
/// configuration path is *read* rather than written. A process that ignores
/// permission bits — root, or an unprivileged one holding `CAP_DAC_OVERRIDE` —
/// cannot induce the failure at all; that is reported as a skip rather than
/// asserted, since an assertion that never fails would be vacuous.
///
/// Both cases rewrite in place. A print-mode run would leave the input alone
/// after a successful configuration load too, so an unchanged file there would
/// prove nothing about the failure; `--in-place` makes the assertion
/// discriminating — a run that had resolved its configuration would rewrite
/// the document, and only a failure before the driver can leave it intact.
#[cfg(unix)]
#[rstest::rstest]
#[case::selected("selected.locked.toml", true)]
#[case::discovered(".mdtablefix.toml", false)]
fn an_unreadable_configuration_file_is_an_error_exit_two(
    #[case] locked: &'static str,
    #[case] is_selected: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new();
    fixture.write("xdg/mdtablefix/config.toml", "ellipsis = true\n");
    let path = fixture.write(locked, "wrap = true\n");
    fixture.write("drift.md", RAGGED);
    let flags = ["--in-place", "drift.md"];
    deny_read(&path);

    let run = if is_selected {
        fixture.run_selected(&fixture.root().join(locked), &flags, "")
    } else {
        fixture.run(&flags, "")
    };
    let permissions_ignored = permissions_are_ignored(&path);
    allow_read(&path);

    if permissions_ignored {
        eprintln!("skipped: this process ignores permission bits, so the read cannot fail");
        return Ok(());
    }

    assert_failure(&run, 2, &[locked]);
    assert!(
        run.stderr.to_lowercase().contains("permission denied"),
        "the diagnostic states why the read failed: {}",
        run.stderr
    );
    let untouched = std::fs::read_to_string(fixture.root().join("drift.md"))?;
    assert_eq!(
        untouched, RAGGED,
        "a run that could not load its configuration must not touch the input"
    );

    Ok(())
}
