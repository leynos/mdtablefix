//! Tests for how the settings layers compose.
//!
//! One fixed `MapEnv` drives both halves of resolution — discovery looks names
//! up, while the merge layer enumerates the `MDTABLEFIX_` prefix — so nothing
//! here reads or mutates the process environment. See
//! `docs/adrs/0012-environment-seam-taxonomy.md`.
//!
//! The precedence table names every key the schema carries, in both
//! polarities. Nine switches and the `md_exts` list state `true` (or a value)
//! in the file and the opposite in the environment, so they witness the file
//! applying, the environment overriding the file, and — in the `AllThree`
//! cells, which write all three layers — the command line overriding the
//! environment. `headings` carries the opposite polarity, so it witnesses the
//! environment applying on its own and overriding an explicit file `false`.
//! The table is generated rather than sampled: the merge is emitted from the
//! schema, so a key left out of it would never fail, it would simply never be
//! exercised, and [`expected`] states each cell's value for a reader to check
//! against the documented order.
//!
//! [`a_typed_false_outranks_a_configured_true`] covers the direction a bare
//! switch cannot state. Every switch accepts an optional `=<BOOL>` value, so a
//! configured `true` is overridden by typing `--flag=false`; that test checks
//! each switch separately, because each declares the value separately and one
//! that lost the declaration would accept the flag while ignoring the word.
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
///
/// Every field of [`ConfigFile`](super::ConfigFile) appears here. A key left out
/// would be one whose layering nothing states: the merge is generated from the
/// schema, so an omission would not fail, it would simply never be exercised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    /// The `wrap` switch.
    Wrap,
    /// The `renumber` switch.
    Renumber,
    /// The `breaks` switch.
    Breaks,
    /// The `ellipsis` switch.
    Ellipsis,
    /// The `fences` switch.
    Fences,
    /// The `footnotes` switch.
    Footnotes,
    /// The `code_emphasis` switch.
    CodeEmphasis,
    /// The `headings` switch.
    Headings,
    /// The `md_exts` list.
    MdExts,
    /// The `git` switch.
    Git,
    /// The `include_untracked` switch.
    IncludeUntracked,
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

/// The file a test writes when it needs `key` configured *true*.
///
/// Every switch states `true`. [`file_contents`] states `headings` as `false`
/// so the precedence table exercises both polarities, but a case that types
/// `false` and asserts `false` needs this spelling: with the file already
/// saying `false`, the assertion would hold whether or not the typed value had
/// been read.
fn true_file(key: Key) -> &'static str {
    match key {
        Key::Headings => "headings = true\n",
        other => file_contents(other),
    }
}

/// The file a row writes, in TOML, for `key`.
///
/// Every switch states `true` except `headings`, which states `false` so the
/// table exercises both polarities rather than only one.
fn file_contents(key: Key) -> &'static str {
    match key {
        Key::Wrap => "wrap = true\n",
        Key::Renumber => "renumber = true\n",
        Key::Breaks => "breaks = true\n",
        Key::Ellipsis => "ellipsis = true\n",
        Key::Fences => "fences = true\n",
        Key::Footnotes => "footnotes = true\n",
        Key::CodeEmphasis => "code_emphasis = true\n",
        Key::Headings => "headings = false\n",
        Key::MdExts => "md_exts = [\"toml\"]\n",
        Key::Git => "git = true\n",
        Key::IncludeUntracked => "include_untracked = true\n",
    }
}

/// The environment variable a row sets for `key`, and its value.
///
/// The value is the opposite of the file's, so a cell where the environment
/// did not apply is distinguishable from one where it applied and agreed.
fn variable(key: Key) -> (&'static str, &'static str) {
    match key {
        Key::Wrap => ("MDTABLEFIX_WRAP", "false"),
        Key::Renumber => ("MDTABLEFIX_RENUMBER", "false"),
        Key::Breaks => ("MDTABLEFIX_BREAKS", "false"),
        Key::Ellipsis => ("MDTABLEFIX_ELLIPSIS", "false"),
        Key::Fences => ("MDTABLEFIX_FENCES", "false"),
        Key::Footnotes => ("MDTABLEFIX_FOOTNOTES", "false"),
        Key::CodeEmphasis => ("MDTABLEFIX_CODE_EMPHASIS", "false"),
        Key::Headings => ("MDTABLEFIX_HEADINGS", "true"),
        Key::MdExts => ("MDTABLEFIX_MD_EXTS", "[rst, xml]"),
        Key::Git => ("MDTABLEFIX_GIT", "false"),
        Key::IncludeUntracked => ("MDTABLEFIX_INCLUDE_UNTRACKED", "false"),
    }
}

/// The flags a row types, which outrank both a file and the environment.
///
/// Every switch is typed in its bare form, which means `true`; the separate
/// [`a_typed_false_outranks_a_configured_true`] covers the `=<BOOL>` form.
fn typed_flags(key: Key) -> &'static [&'static str] {
    match key {
        Key::Wrap => &["--wrap"],
        Key::Renumber => &["--renumber"],
        Key::Breaks => &["--breaks"],
        Key::Ellipsis => &["--ellipsis"],
        Key::Fences => &["--fences"],
        Key::Footnotes => &["--footnotes"],
        Key::CodeEmphasis => &["--code-emphasis"],
        Key::Headings => &["--headings"],
        Key::MdExts => &["--md-exts", "yaml"],
        Key::Git => &["--git"],
        Key::IncludeUntracked => &["--include-untracked"],
    }
}

/// The value `scenario` must resolve to for `key`.
///
/// Written out cell by cell rather than derived from the winning layer, so a
/// reader can check the table against the documented precedence order without
/// running it.
///
/// The nine switches that state `true` in the file share one arm, because their
/// table is the same: the file applies alone and under a typed flag, and the
/// environment's `false` wins wherever it is set. Stating that once keeps the
/// two deliberate contrasts below it — `headings`, whose file value is `false`,
/// and the `md_exts` list — readable as the differences they are.
///
/// The outer match is on `key` alone, not on the pair, so a key added to
/// [`Key`] without a cell here is a compile error rather than a silently
/// uncovered row.
fn expected(key: Key, scenario: Scenario) -> &'static str {
    match key {
        Key::Wrap
        | Key::Renumber
        | Key::Breaks
        | Key::Ellipsis
        | Key::Fences
        | Key::Footnotes
        | Key::CodeEmphasis
        | Key::Git
        | Key::IncludeUntracked => match scenario {
            Scenario::Defaults | Scenario::EnvOnly | Scenario::FileAndEnv => "false",
            Scenario::FileOnly | Scenario::AllThree => "true",
        },
        Key::Headings => match scenario {
            Scenario::Defaults | Scenario::FileOnly => "false",
            Scenario::EnvOnly | Scenario::FileAndEnv | Scenario::AllThree => "true",
        },
        Key::MdExts => match scenario {
            Scenario::Defaults => "md,mdc,markdown",
            Scenario::FileOnly => "toml",
            Scenario::EnvOnly | Scenario::FileAndEnv => "rst,xml",
            Scenario::AllThree => "yaml",
        },
    }
}

/// The value `settings` resolved `key` to, rendered for comparison.
fn observed(key: Key, settings: &Settings) -> String {
    match key {
        Key::Wrap => settings.opts.wrap.to_string(),
        Key::Renumber => settings.opts.renumber.to_string(),
        Key::Breaks => settings.opts.breaks.to_string(),
        Key::Ellipsis => settings.opts.ellipsis.to_string(),
        Key::Fences => settings.opts.fences.to_string(),
        Key::Footnotes => settings.opts.footnotes.to_string(),
        Key::CodeEmphasis => settings.opts.code_emphasis.to_string(),
        Key::Headings => settings.opts.headings.to_string(),
        Key::MdExts => settings.md_exts.iter().collect::<Vec<_>>().join(","),
        Key::Git => settings.git.to_string(),
        Key::IncludeUntracked => settings.include_untracked.to_string(),
    }
}

/// Every key resolves to the most-preferred layer that states it.
///
/// 11 keys by 5 source combinations by 2 file supplies, so a layer that was
/// silently dropped, or one that outranked the wrong neighbour, changes at
/// least one assertion. Each file supply is exercised because the selector
/// short-circuits automatic discovery, and the two must compose identically.
#[rstest]
fn a_key_resolves_to_the_most_preferred_layer_that_states_it(
    #[values(
        Key::Wrap,
        Key::Renumber,
        Key::Breaks,
        Key::Ellipsis,
        Key::Fences,
        Key::Footnotes,
        Key::CodeEmphasis,
        Key::Headings,
        Key::MdExts,
        Key::Git,
        Key::IncludeUntracked
    )]
    key: Key,
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

/// A typed `--flag=false` outranks a configured `true`.
///
/// The direction the precedence table reaches only with a value: the table
/// types each switch bare, which says `true`, so a configured `true` could be
/// overridden there only by an environment `false`. Every switch is checked
/// here, because each declares the optional value separately and one that lost
/// the declaration would silently accept the flag while ignoring the word.
///
/// The configuration states `true` for the key and the command line then
/// states `false`, so a run that ignored the typed value would resolve the
/// configured `true` and fail here.
#[rstest]
fn a_typed_false_outranks_a_configured_true(
    #[values(
        Key::Wrap,
        Key::Renumber,
        Key::Breaks,
        Key::Ellipsis,
        Key::Fences,
        Key::Footnotes,
        Key::CodeEmphasis,
        Key::Headings,
        Key::Git,
        Key::IncludeUntracked
    )]
    key: Key,
) {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();
    environment.insert("MDTABLEFIX_CONFIG_PATH", fixture.select(true_file(key)));

    let typed = typed_flags(key);
    let negated = format!("{}=false", typed[0]);
    let argv = ["mdtablefix", negated.as_str()];

    let settings = resolve(&argv, &environment).expect("the layers resolve");

    assert_eq!(observed(key, &settings), "false");
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
/// than a loss: the last case types it and expects it to be honoured.
#[rstest]
#[case::file("allow_conflicted = true\n", None, false)]
#[case::environment(
    "",
    Some(("MDTABLEFIX_ALLOW_CONFLICTED", "true")),
    false
)]
#[case::command_line("", None, true)]
fn only_the_command_line_can_allow_a_conflicted_rewrite(
    #[case] contents: &str,
    #[case] variable: Option<(&str, &str)>,
    #[case] expected: bool,
) {
    let fixture = Fixture::new();
    let mut environment = fixture.environment();
    if !contents.is_empty() {
        environment.insert("MDTABLEFIX_CONFIG_PATH", fixture.select(contents));
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
