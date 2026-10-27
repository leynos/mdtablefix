//! Fixtures shared by the configuration tests.
//!
//! A module of its own so both test modules build the same isolated tree and
//! drive resolution through the same env-source seam. Every item is
//! `pub(super)` because the only modules that use them are `super`'s
//! descendants.
//!
//! Nothing here reads or mutates the process environment: one `MapEnv` is
//! cloned into both shared sources, so discovery and the merge layer see the
//! same map without the test touching the developer's own configuration. See
//! `docs/adrs/0012-environment-seam-taxonomy.md`.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::{ArgMatches, CommandFactory, FromArgMatches};
use ortho_config::{EnvSource, MapEnv, ScanEnvSource};
use tempfile::TempDir;

use super::{ConfigError, Settings};
use crate::command::Cli;

/// A temporary tree holding every directory discovery consults.
pub(super) struct Fixture {
    /// Deleted when the test ends.
    directory: TempDir,
}

impl Fixture {
    /// Creates the empty directory tree discovery will search.
    pub(super) fn new() -> Self {
        let directory = tempfile::tempdir().expect("create a temporary directory");
        let fixture = Self { directory };
        for name in ["home", "xdg", "dirs", "appdata", "local"] {
            std::fs::create_dir_all(fixture.base(name)).expect("create a fixture directory");
        }

        fixture
    }

    /// The tree this fixture owns.
    pub(super) fn root(&self) -> &Path { self.directory.path() }

    /// A named directory inside the tree.
    pub(super) fn base(&self, name: &str) -> PathBuf { self.root().join(name) }

    /// The environment discovery reads, with every base directory pointing into
    /// the tree so the host's own configuration cannot be reached.
    ///
    /// `XDG_CONFIG_DIRS` is set rather than omitted: an absent or all-empty
    /// list falls back to `/etc/xdg`, which is the host's business, not a
    /// test's.
    pub(super) fn environment(&self) -> MapEnv {
        MapEnv::new()
            .with_var("HOME", self.base("home"))
            .with_var("USERPROFILE", self.base("home"))
            .with_var("XDG_CONFIG_HOME", self.base("xdg"))
            .with_var("XDG_CONFIG_DIRS", self.base("dirs"))
            .with_var("APPDATA", self.base("appdata"))
            .with_var("LOCALAPPDATA", self.base("local"))
    }

    /// Writes `contents` where automatic discovery finds it as a user file.
    ///
    /// `XDG_CONFIG_HOME` is the User scope, so this file is applied after any
    /// System-scope file and before the Project one.
    pub(super) fn discover(&self, contents: &str) {
        let path = self.base("xdg").join("mdtablefix").join("config.toml");
        write(&path, contents);
    }

    /// Writes `contents` where automatic discovery finds it as a system file.
    ///
    /// `XDG_CONFIG_DIRS` is the System scope, the least-preferred of the three.
    /// A test cannot place a file in the Project scope, because the framework
    /// always derives that root from the process working directory; the
    /// process-level suite covers that scope instead.
    pub(super) fn discover_system(&self, contents: &str) {
        let path = self.base("dirs").join("mdtablefix").join("config.toml");
        write(&path, contents);
    }

    /// Writes `contents` to a free-standing file for the selector to name.
    pub(super) fn select(&self, contents: &str) -> PathBuf {
        let path = self.root().join("selected.toml");
        write(&path, contents);

        path
    }

    /// The environment of a run whose selector names a file holding `contents`.
    ///
    /// The three steps that build it — the fixed base map, a file to name, and
    /// the variable that names it — are the same in every case that drives a
    /// failure through the selector, so they are written once here rather than
    /// four lines at a time in each test.
    ///
    /// A case that needs the selector to name something other than a file it can
    /// write, such as an absent path or the empty string, builds its environment
    /// directly instead: neither is a *contents* a test can pass here.
    pub(super) fn selecting(&self, contents: &str) -> MapEnv {
        let mut environment = self.environment();
        environment.insert("MDTABLEFIX_CONFIG_PATH", self.select(contents));

        environment
    }
}

/// Writes `contents` to `path`, creating its parent directories.
pub(super) fn write(path: &Path, contents: &str) {
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
pub(super) fn resolve(argv: &[&str], environment: &MapEnv) -> Result<Settings, ConfigError> {
    let (cli, matches) = parse(argv);
    let lookup: Arc<dyn EnvSource> = Arc::new(environment.clone());
    let scan: Arc<dyn ScanEnvSource> = Arc::new(environment.clone());

    Settings::resolve_with_sources(&cli, &matches, lookup, scan)
}
