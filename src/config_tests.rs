//! Tests for how the settings layers compose.
//!
//! One fixed `MapEnv` drives both halves of resolution — discovery looks names
//! up, while the merge layer enumerates the `MDTABLEFIX_` prefix — so nothing
//! here reads or mutates the process environment. See
//! `docs/adrs/0012-environment-seam-taxonomy.md`.
//!
//! The precedence table splits the polarity of the boolean flags across keys,
//! because a boolean switch can only be typed *true*. `wrap`, `renumber`, and
//! `git` carry a file value of `true` and an environment value of `false`, so
//! they witness the file applying, the environment overriding the file, and —
//! in the `AllThree` cells, which write all three layers — the command line
//! overriding the environment. `headings` carries the opposite polarity, so it
//! witnesses the environment applying on its own and overriding an explicit
//! file `false`. No single key can witness every direction; the five together
//! do, and [`expected`] states each cell's value.
//!
//! One residual dependency: `ortho_config` always resolves a Project root from
//! the process working directory, and the builder method that would substitute
//! one is crate-private upstream. A cell asserting a compiled-in default
//! therefore also relies on this repository having no `.mdtablefix.toml` at its
//! root — and would fail loudly, not silently, if one appeared. Only the
//! dotfile is a Project candidate; a `mdtablefix/config.toml` there is not a
//! candidate at all, so its presence would change nothing.
//!
//! The error surface lives in `super::error_tests`, so this module
//! stays the precedence table it is named for.

use rstest::rstest;

use super::{
    Settings,
    test_support::{Fixture, resolve},
};

/// The settings keys the precedence table covers.
#[derive(Debug, Clone, Copy)]
enum Key {
    /// The `wrap` switch.
    Wrap,
    /// The `renumber` switch.
    Renumber,
    /// The `headings` switch.
    Headings,
    /// The `md_exts` list.
    MdExts,
    /// The `git` switch.
    Git,
}

/// How many layers state a value in one row of the precedence table.
#[derive(Debug, Clone, Copy)]
enum Scenario {
    /// No file and no `MDTABLEFIX_*` variable: the command-line defaults.
    Defaults,
    /// A configuration file only.
    FileOnly,
    /// An `MDTABLEFIX_*` variable only.
    EnvOnly,
    /// A configuration file and an `MDTABLEFIX_*` variable.
    FileAndEnv,
    /// Both of those, and a typed command-line flag as well.
    AllThree,
}

/// How a row's configuration file reaches the resolver.
#[derive(Debug, Clone, Copy)]
enum Supply {
    /// Automatic discovery under `XDG_CONFIG_HOME`.
    Discovery,
    /// The `MDTABLEFIX_CONFIG_PATH` selector.
    Selector,
}

/// The file a row writes, in TOML, for `key`.
fn file_contents(key: Key) -> &'static str {
    match key {
        Key::Wrap => "wrap = true\n",
        Key::Renumber => "renumber = true\n",
        Key::Headings => "headings = false\n",
        Key::MdExts => "md_exts = [\"toml\"]\n",
        Key::Git => "git = true\n",
    }
}

/// The environment variable a row sets for `key`, and its value.
fn variable(key: Key) -> (&'static str, &'static str) {
    match key {
        Key::Wrap => ("MDTABLEFIX_WRAP", "false"),
        Key::Renumber => ("MDTABLEFIX_RENUMBER", "false"),
        Key::Headings => ("MDTABLEFIX_HEADINGS", "true"),
        Key::MdExts => ("MDTABLEFIX_MD_EXTS", "[rst, xml]"),
        Key::Git => ("MDTABLEFIX_GIT", "false"),
    }
}

/// The flags a row types, which outrank both a file and the environment.
fn typed_flags(key: Key) -> &'static [&'static str] {
    match key {
        Key::Wrap => &["--wrap"],
        Key::Renumber => &["--renumber"],
        Key::Headings => &["--headings"],
        Key::MdExts => &["--md-exts", "yaml"],
        Key::Git => &["--git"],
    }
}

/// The value `scenario` must resolve to for `key`.
///
/// Written out cell by cell rather than derived from the winning layer, so a
/// reader can check the table against the documented precedence order without
/// running it. Arms sharing a body are grouped: for the three switches whose
/// file value is `true` and environment value `false`, "the file applies" and
/// "nothing applies" both render as `false`, and stating them once makes the
/// polarity difference from `headings` — whose file value is `false` — visible.
fn expected(key: Key, scenario: Scenario) -> &'static str {
    match (key, scenario) {
        (
            Key::Wrap | Key::Renumber | Key::Git,
            Scenario::Defaults | Scenario::EnvOnly | Scenario::FileAndEnv,
        )
        | (Key::Headings, Scenario::Defaults | Scenario::FileOnly) => "false",
        (Key::Wrap | Key::Renumber | Key::Git, Scenario::FileOnly | Scenario::AllThree)
        | (Key::Headings, Scenario::EnvOnly | Scenario::FileAndEnv | Scenario::AllThree) => "true",
        (Key::MdExts, Scenario::Defaults) => "md,mdc,markdown",
        (Key::MdExts, Scenario::FileOnly) => "toml",
        (Key::MdExts, Scenario::EnvOnly | Scenario::FileAndEnv) => "rst,xml",
        (Key::MdExts, Scenario::AllThree) => "yaml",
    }
}

/// The value `settings` resolved `key` to, rendered for comparison.
fn observed(key: Key, settings: &Settings) -> String {
    match key {
        Key::Wrap => settings.opts.wrap.to_string(),
        Key::Renumber => settings.opts.renumber.to_string(),
        Key::Headings => settings.opts.headings.to_string(),
        Key::MdExts => settings.md_exts.iter().collect::<Vec<_>>().join(","),
        Key::Git => settings.git.to_string(),
    }
}

/// Every key resolves to the most-preferred layer that states it.
///
/// 5 keys by 5 source combinations by 2 file supplies, so a layer that was
/// silently dropped, or one that outranked the wrong neighbour, changes at
/// least one assertion. Each file supply is exercised because the selector
/// short-circuits automatic discovery, and the two must compose identically.
#[rstest]
fn a_key_resolves_to_the_most_preferred_layer_that_states_it(
    #[values(Key::Wrap, Key::Renumber, Key::Headings, Key::MdExts, Key::Git)] key: Key,
    #[values(
        Scenario::Defaults,
        Scenario::FileOnly,
        Scenario::EnvOnly,
        Scenario::FileAndEnv,
        Scenario::AllThree
    )]
    scenario: Scenario,
    #[values(Supply::Discovery, Supply::Selector)] supply: Supply,
) {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();

    if matches!(
        scenario,
        Scenario::FileOnly | Scenario::FileAndEnv | Scenario::AllThree
    ) {
        let contents = file_contents(key);
        match supply {
            Supply::Discovery => fixture.discover(contents),
            Supply::Selector => {
                environment.insert("MDTABLEFIX_CONFIG_PATH", fixture.select(contents));
            }
        }
    }

    if matches!(
        scenario,
        Scenario::EnvOnly | Scenario::FileAndEnv | Scenario::AllThree
    ) {
        let (name, value) = variable(key);
        environment.insert(name, value);
    }

    let mut argv = vec!["mdtablefix"];
    if matches!(scenario, Scenario::AllThree) {
        argv.extend_from_slice(typed_flags(key));
    }

    let settings = resolve(&argv, &environment).expect("the layers resolve");

    assert_eq!(observed(key, &settings), expected(key, scenario));
}

/// Scopes stack in precedence order, so a user file outranks a system one.
///
/// The precedence table above exercises the candidate list within one scope;
/// this states the other ordering, which `AutomaticMode::StackScopes` adds.
/// The two run in opposite directions, so a run that got one right can still
/// have the other inverted.
#[rstest]
#[case("ellipsis = true\n", "ellipsis = false\n", "a...b\n")]
#[case("ellipsis = false\n", "ellipsis = true\n", "a…b\n")]
fn a_user_file_outranks_a_system_file(
    #[case] system: &str,
    #[case] user: &str,
    #[case] expected: &str,
) {
    let fixture = Fixture::new();
    fixture.discover_system(system);
    fixture.discover(user);

    let settings = resolve(&["mdtablefix"], &fixture.environment()).expect("the layers resolve");

    let rendered = if settings.opts.ellipsis {
        "a…b\n"
    } else {
        "a...b\n"
    };
    assert_eq!(rendered, expected);
}
