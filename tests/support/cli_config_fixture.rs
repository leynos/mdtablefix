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
//! none. And every `MDTABLEFIX_*` variable the schema can read is removed from
//! the child, so a developer who exported one does not change what a test sees.
//!
//! The environment is scrubbed variable by variable rather than with
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

/// Every variable the configuration layer can read, removed from each child.
///
/// The selector is listed first because it is the one variable that changes
/// which file is read rather than what a file says. The rest mirror the twelve
/// keys `ConfigFile` carries, spelled the way `CsvEnv` derives them: the
/// `MDTABLEFIX_` prefix, then the schema key in upper case.
const CONFIGURATION_VARIABLES: [&str; 13] = [
    "MDTABLEFIX_CONFIG_PATH",
    "MDTABLEFIX_WRAP",
    "MDTABLEFIX_RENUMBER",
    "MDTABLEFIX_BREAKS",
    "MDTABLEFIX_ELLIPSIS",
    "MDTABLEFIX_FENCES",
    "MDTABLEFIX_FOOTNOTES",
    "MDTABLEFIX_CODE_EMPHASIS",
    "MDTABLEFIX_HEADINGS",
    "MDTABLEFIX_MD_EXTS",
    "MDTABLEFIX_GIT",
    "MDTABLEFIX_INCLUDE_UNTRACKED",
    "MDTABLEFIX_ALLOW_CONFLICTED",
];

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
        for name in CONFIGURATION_VARIABLES {
            command.env_remove(name);
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
