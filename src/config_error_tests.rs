//! Tests for what resolution does with a configuration source it cannot use.
//!
//! Every case here drives the same env-source seam as the precedence table in
//! [`super::config_tests`], and none of them touches the process environment.
//! They are split from that module because the table is about *which* layer
//! wins and these are about the *failure* vocabulary — a different contract,
//! and one that would otherwise grow the table's file past the line limit.

use std::path::PathBuf;

use super::{
    ConfigError,
    test_support::{Fixture, resolve, write},
};

/// A selected file that does not exist is an error, not a silent skip.
#[test]
fn a_missing_selected_file_is_an_error() {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();
    environment.insert("MDTABLEFIX_CONFIG_PATH", fixture.root().join("absent.toml"));

    let error = resolve(&["mdtablefix"], &environment).expect_err("the file is absent");

    assert!(
        error
            .to_string()
            .contains("required configuration file not found"),
        "{error}"
    );
}

/// A selected file that declares no `extends` key still loads.
///
/// The positive control for the `extends` refusal: a refusal that fired on
/// every selected file, rather than only on a declaration, would fail here.
#[test]
fn a_selected_file_without_extends_still_loads() {
    let fixture = Fixture::new();
    let environment = fixture.selecting("wrap = true\n");

    let settings = resolve(&["mdtablefix"], &environment).expect("the file loads");

    assert!(settings.opts.wrap);
}

/// A selected file that is not valid TOML is reported as a configuration error.
#[test]
fn a_malformed_selected_file_is_an_error() {
    let fixture = Fixture::new();
    let environment = fixture.selecting("wrap = \n");

    let error = resolve(&["mdtablefix"], &environment).expect_err("the file is malformed");

    assert!(
        error.to_string().contains("cannot resolve configuration"),
        "{error}"
    );
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
    fixture.discover("ellipsis = true\n");
    fixture.discover_system("wrap = \n");

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
    fixture.discover("ellipsis = true\n");

    let settings = resolve(&["mdtablefix"], &fixture.environment()).expect("the search is quiet");

    assert!(settings.opts.ellipsis);
}

/// A value of the wrong type is rejected rather than coerced.
#[test]
fn a_value_of_the_wrong_type_is_an_error() {
    let fixture = Fixture::new();
    let environment = fixture.selecting("wrap = \"notabool\"\n");

    let error = resolve(&["mdtablefix"], &environment).expect_err("the value is not a boolean");

    assert!(error.to_string().contains("expected a boolean"), "{error}");
}

/// An extension the parser would reject names the key it came from.
///
/// A configured list did not pass through clap's `value_parser`, so this is the
/// only place the user learns which setting to fix.
#[test]
fn an_extension_the_parser_rejects_names_its_key() {
    let fixture = Fixture::new();
    let environment = fixture.selecting("md_exts = [\"a.b\"]\n");

    let error = resolve(&["mdtablefix"], &environment).expect_err("the extension is invalid");

    assert!(
        matches!(&error, ConfigError::InvalidValue { key, .. } if key == "md_exts"),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "invalid configuration value for md_exts: extension \"a.b\" contains a dot"
    );
}

/// An empty extension list is rejected rather than selecting nothing.
///
/// The list is valid TOML and every one of its zero members parses, so only an
/// explicit length check catches it. Left alone it would build an empty filter
/// and a `--git` run would report a selection of nothing — a result the user
/// could not explain from the file they wrote, and one they would more likely
/// read as "no files need changing" than as a mistake.
///
/// The command line cannot reach this state, because `--md-exts` takes at least
/// one value, which is why the guard belongs here rather than in the parser.
#[test]
fn an_empty_extension_list_is_an_error() {
    let fixture = Fixture::new();
    let environment = fixture.selecting("md_exts = []\n");

    let error = resolve(&["mdtablefix"], &environment).expect_err("the list names no extension");

    assert!(
        matches!(&error, ConfigError::InvalidValue { key, .. } if key == "md_exts"),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "invalid configuration value for md_exts: the extension list must name at least one \
         extension"
    );
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

/// The refusal applies to a discovered file, not only a selected one.
#[test]
fn an_extends_key_in_a_discovered_file_is_refused() {
    let fixture = Fixture::new();
    fixture.discover("extends = \"base.toml\"\n");

    let error = resolve(&["mdtablefix"], &fixture.environment())
        .expect_err("a discovered inheritance is refused");

    assert!(
        matches!(&error, ConfigError::UnsupportedExtends { .. }),
        "{error:?}"
    );
}

/// A candidate reached through a symlink is judged by its target's contents.
///
/// The pre-scan and the loader must agree about which file a path names. They
/// agree only because both canonicalise before reading: a read that did not
/// resolve the link first would fail here, and a failure is reported as "no
/// `extends`", which would let the loader follow the link and expand the chain.
///
/// The target is deliberately valid TOML that declares `extends`, so the file
/// is loadable by the loader and refused only by this module. A malformed file
/// would prove nothing, because the loader would reject it too.
#[cfg(unix)]
#[test]
fn a_candidate_reached_through_a_symlink_is_judged_by_its_target() {
    let fixture = Fixture::new();
    let target = fixture.root().join("dotfiles.toml");
    write(&target, "extends = \"somewhere.toml\"\n");
    let link = fixture.base("xdg").join("mdtablefix").join("config.toml");
    std::fs::create_dir_all(link.parent().expect("the link has a parent"))
        .expect("create the configuration directory");
    std::os::unix::fs::symlink(&target, &link).expect("create the symlink");
    let environment = fixture.environment();

    let error = resolve(&["mdtablefix"], &environment)
        .expect_err("a symlinked declaration is refused like any other");

    assert!(
        matches!(&error, ConfigError::UnsupportedExtends { .. }),
        "the pre-scan must judge the target, not the link: {error:?}"
    );
}

///
/// The check reads the parsed top-level table rather than searching the text,
/// so a comment or a string mentioning the key is ordinary content. Without
/// this case a grep-shaped implementation would pass every case above.
#[test]
fn the_word_extends_in_a_comment_is_not_a_declaration() {
    let fixture = Fixture::new();
    let environment = fixture.selecting("# this file does not use extends\nwrap = true\n");

    let settings = resolve(&["mdtablefix"], &environment).expect("the comment is not a key");

    assert!(settings.opts.wrap);
}

/// A nested `extends` belongs to no layer loader and is left alone.
///
/// Only a top-level key starts a chain — the loader asks the merged view for
/// `extends` — so a table that happens to have that name under a section is a
/// value the schema ignores, like any other unknown key.
#[test]
fn a_nested_extends_key_is_not_a_declaration() {
    let fixture = Fixture::new();
    let environment = fixture.selecting("wrap = true\n[section]\nextends = \"base.toml\"\n");

    let settings = resolve(&["mdtablefix"], &environment).expect("the nested key is inert");

    assert!(settings.opts.wrap);
}

/// A key the schema does not know is ignored, so a newer file still loads.
#[test]
fn an_unknown_key_is_ignored() {
    let fixture = Fixture::new();
    let environment = fixture.selecting("unknown = 1\nwrap = true\n");

    let settings = resolve(&["mdtablefix"], &environment).expect("the file resolves");

    assert!(settings.opts.wrap);
}

/// An empty selector value is unset, so automatic discovery still runs.
#[test]
fn an_empty_selector_value_leaves_discovery_in_charge() {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();
    fixture.discover("wrap = true\n");
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
