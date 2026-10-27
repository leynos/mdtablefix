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
    test_support::{Fixture, resolve},
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

/// A selected file that is not valid TOML is reported as a configuration error.
#[test]
fn a_malformed_selected_file_is_an_error() {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();
    environment.insert("MDTABLEFIX_CONFIG_PATH", fixture.select("wrap = \n"));

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
    let mut environment = fixture.environment();
    environment.insert(
        "MDTABLEFIX_CONFIG_PATH",
        fixture.select("wrap = \"notabool\"\n"),
    );

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
    let mut environment = fixture.environment();
    environment.insert(
        "MDTABLEFIX_CONFIG_PATH",
        fixture.select("md_exts = [\"a.b\"]\n"),
    );

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

/// A key the schema does not know is ignored, so a newer file still loads.
#[test]
fn an_unknown_key_is_ignored() {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();
    environment.insert(
        "MDTABLEFIX_CONFIG_PATH",
        fixture.select("unknown = 1\nwrap = true\n"),
    );

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
    let mut environment = fixture.environment();
    environment.insert("MDTABLEFIX_CONFIG_PATH", fixture.select("git = true\n"));

    let settings = resolve(&["mdtablefix", "notes.md"], &environment).expect("the layers resolve");

    assert!(!settings.git);
    assert_eq!(settings.files, vec![PathBuf::from("notes.md")]);
}
