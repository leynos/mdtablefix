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
//! therefore also relies on this repository having no `.mdtablefix.toml` and no
//! `mdtablefix/config.toml` at its root — and would fail loudly, not silently,
//! if one appeared.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::{ArgMatches, CommandFactory, FromArgMatches};
use ortho_config::{EnvSource, MapEnv, ScanEnvSource};
use rstest::rstest;
use tempfile::TempDir;

use super::{ConfigError, Settings};
use crate::command::Cli;

/// A temporary tree holding every directory discovery consults.
struct Fixture {
    /// Deleted when the test ends.
    directory: TempDir,
}

impl Fixture {
    /// Creates the empty directory tree discovery will search.
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("create a temporary directory");
        let fixture = Self { directory };
        for name in ["home", "xdg", "dirs", "appdata", "local"] {
            std::fs::create_dir_all(fixture.base(name)).expect("create a fixture directory");
        }

        fixture
    }

    /// The tree this fixture owns.
    fn root(&self) -> &Path { self.directory.path() }

    /// A named directory inside the tree.
    fn base(&self, name: &str) -> PathBuf { self.root().join(name) }

    /// The environment discovery reads, with every base directory pointing into
    /// the tree so the host's own configuration cannot be reached.
    ///
    /// `XDG_CONFIG_DIRS` is set rather than omitted: an absent or all-empty
    /// list falls back to `/etc/xdg`, which is the host's business, not a
    /// test's.
    fn environment(&self) -> MapEnv {
        MapEnv::new()
            .with_var("HOME", self.base("home"))
            .with_var("USERPROFILE", self.base("home"))
            .with_var("XDG_CONFIG_HOME", self.base("xdg"))
            .with_var("XDG_CONFIG_DIRS", self.base("dirs"))
            .with_var("APPDATA", self.base("appdata"))
            .with_var("LOCALAPPDATA", self.base("local"))
    }

    /// Writes `contents` where automatic discovery finds it as a user file.
    fn discover(&self, contents: &str) {
        let path = self.base("xdg").join("mdtablefix").join("config.toml");
        write(&path, contents);
    }

    /// Writes `contents` to a free-standing file for the selector to name.
    fn select(&self, contents: &str) -> PathBuf {
        let path = self.root().join("selected.toml");
        write(&path, contents);

        path
    }
}

/// Writes `contents` to `path`, creating its parent directories.
fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the file's parent directory");
    }
    std::fs::write(path, contents).expect("write a configuration file");
}

/// Parses `argv` without consulting the process environment.
///
/// [`Cli::parse_validated`] is deliberately not used: it reads the real
/// environment's arguments and terminates the process on error. The two steps
/// it performs are repeated here so a test can supply its own argument vector.
fn parse(argv: &[&str]) -> (Cli, ArgMatches) {
    let matches = Cli::command()
        .try_get_matches_from(argv)
        .expect("the arguments parse");
    let cli = Cli::from_arg_matches(&matches).expect("the arguments convert");

    (cli, matches)
}

/// Resolves the settings against one fixed environment map.
///
/// The same map is cloned into both shared sources, which is the arrangement
/// `ortho_config` documents for a caller that wants discovery and the merge
/// layer to agree without touching the process environment.
fn resolve(argv: &[&str], environment: &MapEnv) -> Result<Settings, ConfigError> {
    let (cli, matches) = parse(argv);
    let lookup: Arc<dyn EnvSource> = Arc::new(environment.clone());
    let scan: Arc<dyn ScanEnvSource> = Arc::new(environment.clone());

    Settings::resolve_with_sources(&cli, &matches, lookup, scan)
}

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
