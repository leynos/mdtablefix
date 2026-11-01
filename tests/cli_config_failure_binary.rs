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

use fixture::{Fixture, RAGGED, assert_clean_success, assert_failure};
#[cfg(unix)]
use permission_denial::{allow_read, deny_read, permissions_are_ignored};

/// A selected file that is absent is an error on standard error, exit 2.
#[test]
fn a_missing_selected_file_exits_two_with_no_standard_output() {
    let fixture = Fixture::new();
    let missing = fixture.root().join("absent.toml");

    let run = fixture.run_selected(&missing, &["--check", "drift.md"], "");

    assert_failure(&run, 2, "required configuration file not found");
}

/// A selected file that is not valid TOML is an error, exit 2.
#[test]
fn a_malformed_selected_file_exits_two_with_no_standard_output() {
    let fixture = Fixture::new();
    let config = fixture.write("broken.toml", "wrap = \n");

    let run = fixture.run_selected(&config, &["--check", "drift.md"], "");

    assert_eq!(run.status, 2);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr.contains("cannot resolve configuration"),
        "stderr: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains("TOML parse error"),
        "the diagnostic names the parse failure: {}",
        run.stderr
    );
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

    assert_eq!(
        run.status, 2,
        "stdout: {} stderr: {}",
        run.stdout, run.stderr
    );
    assert_eq!(run.stdout, "", "the diagnostic belongs on stderr");
    assert!(
        run.stderr.contains(".mdtablefix.toml"),
        "the diagnostic names the broken file: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains("TOML parse error"),
        "stderr: {}",
        run.stderr
    );
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

    assert_failure(&run, 2, "expected a sequence");
}

/// A value the extension parser rejects names its key, exit 2.
///
/// The list passes the merge — it is a sequence of strings — and is rejected
/// afterwards by the same parser `--md-exts` uses, which is why the diagnostic
/// names the key rather than relaying a TOML or type error.
///
/// The run supplies a positional path, so the configured list is never used for
/// selection. It is still rejected: every setting is validated whether or not
/// the resolved mode consults it, because a configuration file that is wrong is
/// worth fixing even on a run that would not have read it — and because a
/// validation that depended on the mode would let a typo surface only on some
/// invocations.
#[test]
fn a_rejected_extension_names_its_key() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "md_exts = [\"a.b\"]\n");
    fixture.write("notes.md", RAGGED);

    let run = fixture.run_selected(&config, &["notes.md"], "");

    assert_failure(&run, 2, "invalid configuration value for md_exts");
}

/// A selected file that exists but cannot be read is an error, exit 2.
///
/// The missing-file case above is satisfied by a loader that refuses only what
/// is absent. This one is the other half: the path exists, so any check that
/// stops at metadata passes, and only a real failed read produces the status.
/// `ExplicitMode::RequiredExclusive` is what makes it fatal — a named file is a
/// statement of intent, so a file the tool cannot honour is not one it may
/// silently replace with the defaults.
///
/// The mode is `0o000` rather than a read-only directory, because the
/// configuration path is *read* rather than written. A process that ignores
/// permission bits — root, or an unprivileged one holding `CAP_DAC_OVERRIDE` —
/// cannot induce the failure at all; that is reported as a skip rather than
/// asserted, since an assertion that never fails would be vacuous.
#[cfg(unix)]
#[test]
fn an_unreadable_selected_file_exits_two_with_no_standard_output() {
    let fixture = Fixture::new();
    let config = fixture.write("locked.toml", "ellipsis = true\n");
    fixture.write("drift.md", RAGGED);
    deny_read(&config);

    let run = fixture.run_selected(&config, &["drift.md"], "");
    let permissions_ignored = permissions_are_ignored(&config);
    allow_read(&config);

    if permissions_ignored {
        eprintln!("skipped: this process ignores permission bits, so the read cannot fail");
        return;
    }

    assert_eq!(
        run.status, 2,
        "stdout: {} stderr: {}",
        run.stdout, run.stderr
    );
    assert_eq!(run.stdout, "", "the diagnostic belongs on stderr");
    assert!(
        run.stderr.contains("locked.toml"),
        "the diagnostic names the unreadable file: {}",
        run.stderr
    );
    assert!(
        run.stderr.to_lowercase().contains("permission denied"),
        "the diagnostic states why the read failed: {}",
        run.stderr
    );
    assert_eq!(
        std::fs::read_to_string(fixture.root().join("drift.md")).expect("read the input"),
        RAGGED,
        "a run that could not load its configuration must not touch the input"
    );
}

/// An unreadable discovered file is reported even when a valid one loaded.
///
/// The promotion in `src/config.rs` exists for exactly this shape: the
/// framework classifies an automatic candidate as optional and withholds the
/// failure whenever any scope loaded, so without promotion a locked project
/// file beside a readable user file would leave the run formatting on settings
/// the user cannot see. A malformed file proves the same path; an *unreadable*
/// one proves it for the failure mode a permissions mistake actually produces,
/// where the bytes are not even available to parse.
///
/// The user file is the read-only one, so the precedence order is the real
/// one: the project file that cannot be read is the *nearer* scope, and a run
/// that ignored it would silently apply the user file instead. The input is a
/// named file rather than standard input, so the case can also show that a run
/// which fails before reading its configuration does not rewrite what it was
/// asked to format.
#[cfg(unix)]
#[test]
fn an_unreadable_project_file_is_reported_beside_a_valid_user_file() {
    let fixture = Fixture::new();
    fixture.write("xdg/mdtablefix/config.toml", "ellipsis = true\n");
    let project = fixture.write(".mdtablefix.toml", "wrap = true\n");
    fixture.write("drift.md", RAGGED);
    deny_read(&project);

    let run = fixture.run(&["drift.md"], "");
    let permissions_ignored = permissions_are_ignored(&project);
    allow_read(&project);

    if permissions_ignored {
        eprintln!("skipped: this process ignores permission bits, so the read cannot fail");
        return;
    }

    assert_eq!(
        run.status, 2,
        "stdout: {} stderr: {}",
        run.stdout, run.stderr
    );
    assert_eq!(run.stdout, "", "the diagnostic belongs on stderr");
    assert!(
        run.stderr.contains(".mdtablefix.toml"),
        "the diagnostic names the unreadable file: {}",
        run.stderr
    );
    assert!(
        run.stderr.to_lowercase().contains("permission denied"),
        "the diagnostic states why the read failed: {}",
        run.stderr
    );
    assert_eq!(
        std::fs::read_to_string(fixture.root().join("drift.md")).expect("read the input"),
        RAGGED,
        "a run that could not load its configuration must not touch the input"
    );
}

/// An empty extension list is rejected rather than selecting nothing.
///
/// The process-level half of the unit case: `md_exts = []` is valid TOML and
/// every one of its zero members parses, so only the explicit length check
/// catches it. Without this, a `--git` run would select nothing and report
/// that as a successful, empty run — indistinguishable from "no files need
/// changing", which is exactly the silence a configuration mistake should not
/// be able to produce.
///
/// The positional path is supplied for the same reason as the rejected-value
/// case above: every setting is validated whether or not the resolved mode
/// consults it.
#[test]
fn an_empty_extension_list_names_its_key() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "md_exts = []\n");
    fixture.write("notes.md", RAGGED);

    let run = fixture.run_selected(&config, &["notes.md"], "");

    assert_failure(
        &run,
        2,
        "invalid configuration value for md_exts: the extension list must name at least one",
    );
}
