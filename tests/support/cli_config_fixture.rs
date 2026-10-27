//! Fixtures shared by the process-level configuration tests.
//!
//! A module of its own so both integration-test files build the same isolated
//! tree and scrub the same variables, and so neither file grows past the line
//! limit. Each test binary compiles alone, so this is declared with `#[path]`
//! from each of them rather than shared by Cargo.
//!
//! Two boundaries keep the child environment honest. `MDTABLEFIX_CONFIG_PATH`
//! short-circuits automatic discovery, so a case that wants no configuration at
//! all names a free-standing empty file rather than relying on the host having
//! none. And every variable the provider could read is removed from the child,
//! so a developer who exported one does not change what a test sees.
//!
//! The second boundary is a prefix match rather than a list. The provider
//! upper-cases a name before comparing it, so it reads any spelling of the
//! prefix: measured on 2026-10-27, `MDTABLEFIX_ELLIPSIS=true`,
//! `mdtablefix_ellipsis=true` and `MdTaBlEfIx_ElLiPsIs=true` each enabled the
//! transformation, and a lower-case `mdtablefix_config_path` selected its file
//! too. Removing one spelling of each key would therefore leave the others
//! readable on any platform where names are case-sensitive, so the scrub
//! enumerates the environment and removes every name that carries the prefix in
//! any case.
//!
//! The environment is otherwise inherited rather than cleared with
//! `env_clear`: Windows needs `SystemRoot` and `PATH` to create a process and to
//! find `git`, and this suite runs on both platforms.
//!
//! Every item is used by at least one including binary, because an item this
//! module defines and an including binary does not use is dead code under
//! `-D warnings`.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

/// A ragged table, which every formatting mode must agree needs reformatting.
///
/// Only the shared shapes live here. A fixture used by one binary stays in that
/// binary, because an item this module defines and an including binary does not
/// use is dead code under `-D warnings`.
pub const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// The prefix every configuration variable shares, in lower case.
///
/// Wide enough to cover the selector and each of the twelve keys
/// `ConfigFile` carries, which is deliberate: the provider matches the prefix
/// itself, so a match on a full name would miss a key the schema gains later.
const CONFIGURATION_PREFIX: &str = "mdtablefix_";

/// Whether `name` could be a configuration variable, in any spelling.
///
/// Compared in lower case because the provider upper-cases a name before
/// matching it, so every spelling of the prefix reaches a setting. Only the
/// prefix is tested, not a key, so a variable this fixture has never heard of
/// is still removed.
fn carries_configuration_prefix(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy()
        .to_lowercase()
        .starts_with(CONFIGURATION_PREFIX)
}

/// What one child process produced.
pub struct Run {
    /// The exit status, or `-1` when the process was signalled.
    pub status: i32,
    /// Standard output as text.
    pub stdout: String,
    /// Standard error as text.
    pub stderr: String,
}

/// A temporary tree with a scrubbed configuration environment.
pub struct Fixture {
    /// Deleted when the test ends.
    directory: TempDir,
}

impl Fixture {
    /// Creates the tree and the directories discovery would consult.
    pub fn new() -> Self {
        let directory = tempfile::tempdir().expect("create a temporary directory");
        let fixture = Self { directory };
        for name in ["home", "xdg", "dirs", "appdata", "local"] {
            std::fs::create_dir_all(fixture.base(name)).expect("create a fixture directory");
        }

        fixture
    }

    /// The tree this fixture owns.
    ///
    /// It is also the child's working directory, which is what puts a file
    /// written at the tree root in the Project discovery scope.
    pub fn root(&self) -> &Path { self.directory.path() }

    /// A named directory inside the tree.
    fn base(&self, name: &str) -> PathBuf { self.root().join(name) }

    /// Writes `contents` to `name`, creating its parent directories.
    pub fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.root().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create the file's parent directory");
        }
        std::fs::write(&path, contents).expect("write a fixture");

        path
    }

    /// An empty configuration file, whose contents state nothing.
    pub fn empty_config(&self) -> PathBuf { self.write("empty.toml", "") }

    /// The child process, scrubbed and pointed at the fixture's directories.
    ///
    /// An absent or all-empty `XDG_CONFIG_DIRS` would fall back to `/etc/xdg`,
    /// which belongs to the host, so it is set rather than removed. `HOME` and
    /// `USERPROFILE` are both set because discovery prefers whichever is
    /// non-empty and treats the other as a fallback.
    ///
    /// The scrub removes a variable by name for every inherited name that could
    /// be one, which is why it reads the process environment: the set is defined
    /// by a prefix, so a fixed list of spellings cannot cover it on a platform
    /// where names are case-sensitive. This is the composition root for the
    /// child's environment, and it has no caller that could be handed the
    /// parent's instead — the whole point is that a developer's exported
    /// variable cannot change what a test observes.
    #[expect(
        clippy::disallowed_methods,
        reason = "the fixture is the composition root for its child's environment"
    )]
    pub fn command(&self) -> Command {
        let mut command = Command::cargo_bin("mdtablefix").expect("cargo binary");
        command
            .current_dir(self.root())
            .env("HOME", self.base("home"))
            .env("USERPROFILE", self.base("home"))
            .env("XDG_CONFIG_HOME", self.base("xdg"))
            .env("XDG_CONFIG_DIRS", self.base("dirs"))
            .env("APPDATA", self.base("appdata"))
            .env("LOCALAPPDATA", self.base("local"));
        for (name, _) in std::env::vars_os() {
            if carries_configuration_prefix(&name) {
                command.env_remove(name);
            }
        }

        command
    }

    /// Runs a prepared command and decodes what it produced.
    pub fn finish(command: &mut Command) -> Run {
        let output = command.output().expect("run the binary");

        Run {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Runs the binary in the tree with `args`, feeding `stdin`.
    pub fn run(&self, args: &[&str], stdin: &str) -> Run {
        Self::finish(self.command().args(args).write_stdin(stdin))
    }

    /// Runs the binary with `args`, from the configuration file `config` names.
    pub fn run_selected(&self, config: &Path, args: &[&str], stdin: &str) -> Run {
        Self::finish(
            self.command()
                .env("MDTABLEFIX_CONFIG_PATH", config)
                .args(args)
                .write_stdin(stdin),
        )
    }
}

/// Asserts that `run` succeeded and printed `expected` with nothing on stderr.
///
/// Every successful case in the process-level suites makes the same three
/// claims, and stating them once keeps each test down to what it is actually
/// about: which configuration source supplied the setting.
pub fn assert_clean_success(run: &Run, expected: &str) {
    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, expected);
    assert!(run.stderr.is_empty(), "stderr: {}", run.stderr);
}

/// Asserts that `run` failed with `status`, naming `needle` on standard error.
///
/// A failing run is three claims at once: the status a script branches on, that
/// standard output stayed empty, and that the diagnostic says what went wrong.
/// The empty standard output is not decoration — a diagnostic printed there
/// would corrupt the document a pipeline is reading — so the three are stated
/// together rather than left to each test to remember.
///
/// `needle` is a fragment rather than the whole message because the framework's
/// own text is not this crate's to pin; what a test asserts is that this tool
/// passed the failure on rather than swallowing it.
pub fn assert_failure(run: &Run, status: i32, needle: &str) {
    assert_eq!(run.status, status, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty(), "stdout: {}", run.stdout);
    assert!(run.stderr.contains(needle), "stderr: {}", run.stderr);
}
