//! Tests for what resolution does with a configuration source it cannot use.
//!
//! Every case here drives the same env-source seam as the precedence table in
//! [`super::config_tests`], and none of them touches the process environment.
//! They are split from that module because the table is about *which* layer
//! wins and these are about what a source may and may not carry — the failure
//! vocabulary, plus the one setting configuration is refused — a different
//! contract, and one that would otherwise grow the table's file past the line
//! limit.

// `Path` is named only by the symlink and FIFO installers, which are Unix-only.
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

use super::{
    ConfigError,
    test_support::{Fixture, Scope, resolve, write},
};

/// A selected file the `extends` refusal must not reject still loads.
///
/// These are the positive controls for the refusal, and each rules out a
/// different over-eager implementation. A refusal that fired on every selected
/// file fails the plain case; one that searched the text rather than the parsed
/// table fails the comment case; one that matched the key at any depth fails
/// the nested case; and one that rejected an unrecognized key fails the unknown
/// case, which is what a newer version's file would carry. Every file here
/// states `wrap = true` at the top level, so a load that silently applied
/// nothing would fail too.
#[rstest::rstest]
#[case::plain("wrap = true\n")]
#[case::comment("# this file does not use extends\nwrap = true\n")]
#[case::nested("wrap = true\n[section]\nextends = \"base.toml\"\n")]
#[case::unknown_key("unknown = 1\nwrap = true\n")]
fn a_selected_file_without_a_declaration_still_loads(#[case] contents: &'static str) {
    let fixture = Fixture::new();
    let environment = fixture.selecting(contents);

    let settings = resolve(&["mdtablefix"], &environment).expect("the file is not refused");

    assert!(settings.opts.wrap);
}

/// A selected file the merge cannot use is reported, not silently skipped.
///
/// Two shapes, both reaching the same `ConfigError::Merge`: TOML that does not
/// parse, and a value whose type the schema cannot accept. A file is selected
/// rather than discovered in both cases, so the failure is the file's own
/// rather than a promotion of a withheld automatic-candidate error.
#[rstest::rstest]
#[case::malformed("wrap = \n", "cannot resolve configuration")]
#[case::wrong_type("wrap = \"notabool\"\n", "expected a boolean")]
fn a_selected_file_that_cannot_be_merged_is_an_error(
    #[case] contents: &'static str,
    #[case] fragment: &'static str,
) {
    let fixture = Fixture::new();
    let environment = fixture.selecting(contents);

    let error = resolve(&["mdtablefix"], &environment).expect_err("the file cannot be merged");

    assert!(error.to_string().contains(fragment), "{error}");
}

/// A malformed discovered file is reported even when another scope loaded.
///
/// The framework classifies an automatic candidate as *optional*, and withholds
/// an optional failure whenever any scope loaded — so a broken project file
/// beside a valid user file would otherwise leave the run to proceed on the
/// user settings alone. Formatting with a rule set the user did not ask for is
/// the destructive surprise the adoption exists to prevent, so the withheld
/// error is promoted. This is the regression guard for that promotion.
///
/// The user file is written first and the system file second, so the user scope
/// is the one that loaded: a promotion that only fired for the last scope
/// walked would fail here.
#[test]
fn a_malformed_discovered_file_is_reported_beside_a_valid_one() {
    let fixture = Fixture::new();
    fixture.place(Scope::User, "ellipsis = true\n");
    fixture.place(Scope::System, "wrap = \n");

    let error = resolve(&["mdtablefix"], &fixture.environment())
        .expect_err("the malformed discovery candidate is reported");

    let rendered = error.to_string();
    assert!(
        rendered.contains("cannot resolve configuration"),
        "{rendered}"
    );
    assert!(rendered.contains("TOML parse error"), "{rendered}");
}

/// An absent discovered candidate contributes nothing, however many load.
///
/// The counterpart to the promotion above: the framework reports *no* error for
/// a location that holds no file, so promoting the withheld ones must not turn
/// a normal search into a failure. Without this case an over-eager promotion
/// would look like a correct one.
#[test]
fn an_absent_discovered_candidate_is_not_an_error() {
    let fixture = Fixture::new();
    fixture.place(Scope::User, "ellipsis = true\n");

    let settings = resolve(&["mdtablefix"], &fixture.environment()).expect("the search is quiet");

    assert!(settings.opts.ellipsis);
}

/// A configured extension list this tool cannot use names the key it came from.
///
/// A configured list did not pass through clap's `value_parser`, so this is the
/// only place the user learns which setting to fix; the key appears in both the
/// variant and the rendered message. Two entries are reachable from a file and
/// not from the command line, which is why the checks live here:
///
/// - a member the parser rejects, which the typed `--md-exts` would have refused at parse time; and
/// - an empty list, which is valid TOML whose every member (there are none) parses, so only an
///   explicit length check catches it. Left alone it would build an empty filter and a `--git` run
///   would report a selection of nothing — a result the user could not explain from the file they
///   wrote, and one they would more likely read as "no files need changing" than as a mistake.
///   `--md-exts` takes at least one value, so the command line cannot reach the state.
#[rstest::rstest]
#[case::unparseable(
    "md_exts = [\"a.b\"]\n",
    "invalid configuration value for md_exts: extension \"a.b\" contains a dot"
)]
#[case::empty(
    "md_exts = []\n",
    "invalid configuration value for md_exts: the extension list must name at least one extension"
)]
fn a_configured_extension_list_names_the_key_it_came_from(
    #[case] contents: &'static str,
    #[case] message: &'static str,
) {
    let fixture = Fixture::new();
    let environment = fixture.selecting(contents);

    let error = resolve(&["mdtablefix"], &environment).expect_err("the list is unusable");

    assert!(
        matches!(&error, ConfigError::InvalidValue { key, .. } if key == "md_exts"),
        "{error:?}"
    );
    assert_eq!(error.to_string(), message);
}

/// An `extends` key is refused before the parent is opened.
///
/// The parent is written beside the declaring file and is valid TOML, so the
/// refusal cannot be explained by a path that resolves to nothing or by a
/// parent the loader could not parse: the only reason this run fails is that
/// the key itself is refused. The diagnostic names the declaring file and
/// never the parent, so nothing derived from the parent's bytes can reach it.
///
/// The `..` form of the disclosure is covered at the process level, where the
/// parent really does live outside the working directory.
#[test]
fn an_extends_key_is_refused_before_the_parent_is_read() {
    let fixture = Fixture::new();
    let parent = fixture.root().join("parent-that-must-not-be-read.toml");
    write(&parent, "wrap = true\n");
    let environment = fixture.selecting("extends = \"parent-that-must-not-be-read.toml\"\n");

    let error = resolve(&["mdtablefix"], &environment).expect_err("inheritances are refused");

    let rendered = error.to_string();
    assert!(
        matches!(&error, ConfigError::UnsupportedExtends { .. }),
        "{error:?}"
    );
    assert!(rendered.contains("does not support"), "{rendered}");
    assert!(
        !rendered.contains("parent-that-must-not-be-read"),
        "the diagnostic must not name the parent: {rendered}"
    );
}

/// A file and the environment cannot turn off the conflict guard.
///
/// `allow_conflicted` is the one setting the schema deliberately omits; see
/// [`schema::CONFIGURED_ARGUMENTS`](super::schema::CONFIGURED_ARGUMENTS). A
/// committed `.mdtablefix.toml` that switched it on would let an
/// `--git --in-place` run rewrite text across unresolved conflict markers in a
/// tree the user only checked out, so both non-command-line layers are checked
/// here and neither may reach it.
///
/// The flag itself still works, which is what makes this a narrowing rather
/// than a loss: the last case types it and expects it to be honoured. It lives
/// here rather than beside the precedence table because it is the same claim
/// as the refusals above — a source the run reads, and a key it may not carry.
#[rstest::rstest]
#[case::file("allow_conflicted = true\n", None, false)]
#[case::environment(
    "",
    Some(("MDTABLEFIX_ALLOW_CONFLICTED", "true")),
    false
)]
#[case::command_line("", None, true)]
fn only_the_command_line_can_allow_a_conflicted_rewrite(
    #[case] contents: &'static str,
    #[case] variable: Option<(&'static str, &'static str)>,
    #[case] expected: bool,
) {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();
    if !contents.is_empty() {
        environment.insert(
            "MDTABLEFIX_CONFIG_PATH",
            fixture.place(Scope::Selected, contents),
        );
    }
    if let Some((name, value)) = variable {
        environment.insert(name, value);
    }

    let argv = if expected {
        vec!["mdtablefix", "--allow-conflicted"]
    } else {
        vec!["mdtablefix"]
    };

    let settings = resolve(&argv, &environment).expect("the layers resolve");

    assert_eq!(settings.allow_conflicted, expected);
}

/// The refusal applies to a discovered file, not only a selected one.
#[test]
fn an_extends_key_in_a_discovered_file_is_refused() {
    let fixture = Fixture::new();
    fixture.place(Scope::User, "extends = \"base.toml\"\n");

    let error = resolve(&["mdtablefix"], &fixture.environment())
        .expect_err("a discovered inheritance is refused");

    assert!(
        matches!(&error, ConfigError::UnsupportedExtends { .. }),
        "{error:?}"
    );
}

/// A candidate reached through a symlink is judged by its target's contents.
///
/// The pre-scan and the loader must agree about which file a path names. A
/// same-directory link stays inside the capability both open, so both follow
/// it; a read that stopped at the link itself would see no `extends` and let
/// the loader expand the chain.
///
/// The target is deliberately valid TOML that declares `extends`, so the file
/// is loadable by the loader and refused only by this module. A malformed file
/// would prove nothing, because the loader would reject it too.
#[cfg(unix)]
#[test]
fn a_candidate_reached_through_a_symlink_is_judged_by_its_target() {
    let fixture = Fixture::new();
    let link = fixture.base("xdg").join("mdtablefix").join("config.toml");
    let directory = link.parent().expect("the link has a parent").to_path_buf();
    std::fs::create_dir_all(&directory).expect("create the configuration directory");
    write(
        &directory.join("dotfiles.toml"),
        "extends = \"somewhere.toml\"\n",
    );
    // A relative target, because the capability both sides open follows one
    // inside its own directory and refuses an absolute path even when that
    // path names the same directory.
    std::os::unix::fs::symlink("dotfiles.toml", &link).expect("create the symlink");
    let environment = fixture.environment();

    let error = resolve(&["mdtablefix"], &environment)
        .expect_err("a symlinked declaration is refused like any other");

    assert!(
        matches!(&error, ConfigError::UnsupportedExtends { .. }),
        "the pre-scan must judge the target, not the link: {error:?}"
    );
}

/// A candidate the loader will refuse is not read by the pre-scan.
///
/// The loader asks its parent-directory capability for the candidate's
/// metadata before it canonicalizes anything, so a link that escapes the
/// directory, or an entry that is not a regular file, is refused without its
/// bytes being read. The pre-scan must do the same or it would disagree with
/// the loader about which files are in play.
///
/// This checks the symptom that made the divergence observable: the target is
/// valid TOML declaring `extends`, which the pre-scan used to report as its own
/// refusal. The loader never gets that far, so the error must be the loader's
/// own file error — and, crucially, must not be `UnsupportedExtends`.
///
/// A FIFO is the second case. It is not a regular file either, and a pre-scan
/// that opened it rather than statting it would block forever; the assertion
/// here would then never run, which is why this is worth pinning rather than
/// leaving to inspection.
#[cfg(unix)]
#[rstest::rstest]
#[case::escaping_symlink(escaping_symlink)]
#[case::fifo(fifo)]
fn a_candidate_the_loader_would_refuse_is_left_to_the_loader(#[case] install: fn(&Path, &Path)) {
    let fixture = Fixture::new();
    let target = fixture.base("outside").join("elsewhere.toml");
    write(&target, "extends = \"parent.toml\"\nwrap = true\n");
    let candidate = fixture.base("xdg").join("mdtablefix").join("config.toml");
    std::fs::create_dir_all(candidate.parent().expect("the candidate has a parent"))
        .expect("create the configuration directory");
    install(&candidate, &target);
    let environment = fixture.environment();

    let error = resolve(&["mdtablefix"], &environment)
        .expect_err("the loader refuses a candidate it will not read");

    assert!(
        !matches!(&error, ConfigError::UnsupportedExtends { .. }),
        "the pre-scan must not read this candidate: {error:?}"
    );
}

/// Installs a link that climbs out of the candidate's directory.
#[cfg(unix)]
fn escaping_symlink(candidate: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, candidate).expect("create the symlink");
}

/// Installs a FIFO, which is not a regular file.
#[cfg(unix)]
fn fifo(candidate: &Path, _target: &Path) {
    let status = std::process::Command::new("mkfifo")
        .arg(candidate)
        .status()
        .expect("run mkfifo");

    assert!(status.success(), "mkfifo failed: {status}");
}

/// An empty selector value is unset, so automatic discovery still runs.
#[test]
fn an_empty_selector_value_leaves_discovery_in_charge() {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();
    fixture.place(Scope::User, "wrap = true\n");
    environment.insert("MDTABLEFIX_CONFIG_PATH", "");

    let settings = resolve(&["mdtablefix"], &environment).expect("the file is discovered");

    assert!(settings.opts.wrap);
}

/// A positional path outranks a configured `git` selection.
///
/// The user's own statement of what to act on cannot be overridden by a file:
/// a configured `git = true` beside a typed path would otherwise silently
/// reformat a tree the user did not name.
#[test]
fn a_positional_path_outranks_a_configured_git_selection() {
    let fixture = Fixture::new();
    let environment = fixture.selecting("git = true\n");

    let settings = resolve(&["mdtablefix", "notes.md"], &environment).expect("the layers resolve");

    assert!(!settings.git);
    assert_eq!(settings.files, vec![PathBuf::from("notes.md")]);
}

/// Resolution reports one bounded outcome within its own span.
///
/// The span is what makes configuration resolution visible at all: without it
/// a slow or failing load is indistinguishable from any other startup cost.
/// The event is asserted rather than the span name because `traced_test`
/// exposes captured events, and the pair is what a subscriber actually sees.
///
/// The field is a category, not a value. A configuration file may hold
/// anything, and a diagnostic that echoed it would defeat the reason
/// `config_extends` refuses `extends` in the first place.
#[test]
#[test_macros::traced_test]
fn resolution_reports_its_outcome_as_a_bounded_category() {
    let fixture = Fixture::new();
    let environment = fixture.environment();

    resolve(&["mdtablefix"], &environment).expect("the defaults resolve");

    assert!(
        logs_contain("configuration resolution finished"),
        "resolution must report that it finished"
    );
    // Quoted, as the fmt subscriber renders a `&str` field. Asserting the
    // unquoted form would pass only if the field held a bare identifier.
    assert!(logs_contain("outcome=\"ok\""), "the outcome is a category");
}
