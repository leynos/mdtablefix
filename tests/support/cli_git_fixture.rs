//! A real `git` repository for the end-to-end `--git` command-line tests.
//!
//! Two integration-test files drive the binary against a repository — the flag
//! grammar file and the mode-with-no-input file — and both need the same
//! primitive: a temporary tree with `git` initialized, the ambient
//! configuration neutralized, and a way to run the binary in it. Duplicating
//! that would mean two fixtures that drift, and one of them silently testing a
//! different environment.
//!
//! Each integration-test binary compiles its own copy of a `#[path]` module, so
//! an item declared here that either including file never calls is `dead_code`
//! under `-D warnings`. Every item below is reached by both files: both stage a
//! file, run the binary through [`Fixture`], and assert on [`Run`]; the mode
//! file also uses [`command_in`] to feed standard input. What differs between
//! the files is the expected text, so those literals stay with the assertions
//! that make them.

use std::{path::Path, process::Command as ProcessCommand};

use assert_cmd::Command;
use tempfile::TempDir;

/// A ragged table, which every mode must agree needs reformatting.
pub const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// The same table already aligned, which no mode may change.
pub const CLEAN: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// What one run of the binary produced.
pub struct Run {
    /// The process exit status, or `-1` when the process was signalled.
    pub status: i32,
    /// Standard output as text.
    pub stdout: String,
    /// Standard error as text.
    pub stderr: String,
}

/// A fixture repository in a temporary directory.
pub struct Fixture {
    /// Kept alive for the length of the test; dropping it deletes the tree.
    directory: TempDir,
}

impl Fixture {
    /// Creates an empty repository whose default branch is `main`.
    ///
    /// # Panics
    ///
    /// Panics when the temporary directory cannot be created or `git init`
    /// fails, which is a broken environment rather than a failed expectation.
    pub fn new() -> Self {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let fixture = Self { directory };
        fixture.git(&["init", "--quiet", "-b", "main"]);

        fixture
    }

    /// Runs `git` in the repository, requiring it to succeed.
    ///
    /// # Panics
    ///
    /// Panics when `git` is absent from `PATH` or exits non-zero, because a
    /// fixture that could not be built cannot support the test's claim.
    fn git(&self, args: &[&str]) {
        let output = self
            .git_command(args)
            .output()
            .expect("run git; the fixture needs it on PATH");
        assert!(
            output.status.success(),
            "git {args:?} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// The fixture's hardened `git` invocation.
    ///
    /// `GIT_CONFIG_GLOBAL=/dev/null` removes the developer's configuration
    /// along with their identity, so the fixture supplies the identity through
    /// the environment rather than through a file inside the repository — a
    /// file the selection could then read.
    fn git_command(&self, args: &[&str]) -> ProcessCommand {
        let mut command = ProcessCommand::new("git");
        command
            .current_dir(self.root())
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("HOME", self.root())
            .env("LC_ALL", "C")
            .env("LANGUAGE", "")
            .env("GIT_AUTHOR_NAME", "mdtablefix tests")
            .env("GIT_AUTHOR_EMAIL", "tests@example.invalid")
            .env("GIT_COMMITTER_NAME", "mdtablefix tests")
            .env("GIT_COMMITTER_EMAIL", "tests@example.invalid");

        command
    }

    /// The repository root.
    pub fn root(&self) -> &Path { self.directory.path() }

    /// Writes `name` with `content` and stages it.
    ///
    /// Writing and staging are one step: a file left untracked is invisible to
    /// `--git`, which is the opposite of what a test here needs.
    ///
    /// # Panics
    ///
    /// Panics when the file cannot be written or `git add` fails, which is a
    /// fixture failure.
    pub fn track(&self, name: &str, content: impl AsRef<[u8]>) {
        let path = self.root().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create the fixture's directory");
        }
        std::fs::write(&path, content).expect("write a fixture");
        self.git(&["add", "--", name]);
    }

    /// Reads `name` as text.
    ///
    /// # Panics
    ///
    /// Panics when the file cannot be read, which is a fixture failure.
    pub fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.root().join(name)).expect("read a fixture")
    }

    /// Runs the binary in the repository root.
    ///
    /// # Panics
    ///
    /// Panics when the binary cannot be spawned, which is a broken build
    /// rather than a failed expectation.
    pub fn run(&self, args: &[&str]) -> Run { run_in(self.root(), args) }
}

/// Runs the binary in `directory` and collects what it produced.
///
/// # Panics
///
/// Panics when the binary cannot be spawned, which is a broken build rather
/// than a failed expectation.
pub fn run_in(directory: &Path, args: &[&str]) -> Run {
    let output = command_in(directory)
        .args(args)
        .output()
        .expect("run mdtablefix");

    Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// A hardened invocation of the binary, positioned in `directory`.
///
/// Exposed as a builder rather than only through [`run_in`] because a case that
/// supplies standard input needs to call `write_stdin` between building the
/// command and running it. The environment is the one every other run here
/// gets, so a case that reaches for the builder is not thereby testing a
/// different machine from its neighbours.
///
/// Every directory the binary could discover a configuration file in is
/// pointed at `directory`, so discovery finds whatever the test wrote there
/// and nothing of the developer's. `XDG_CONFIG_DIRS` is set rather than
/// omitted because an absent or all-empty value falls back to `/etc/xdg`,
/// which belongs to the host. `HOME` and `USERPROFILE` are both set because
/// discovery prefers whichever is non-empty and treats the other as a
/// fallback.
///
/// The `MDTABLEFIX_*` variables are removed by prefix rather than by name.
/// The set is defined by a prefix, so a fixed list of spellings cannot cover
/// it, and an exported `MDTABLEFIX_WRAP=true` would otherwise change what a
/// `--git` case observes. This is the one place the child's environment is
/// composed, so the scan belongs here and nowhere else in this module.
///
/// # Panics
///
/// Panics when the binary cannot be found, which is a broken build rather than
/// a failed expectation.
#[expect(
    clippy::disallowed_methods,
    reason = "the fixture is the composition root for its child's environment"
)]
pub fn command_in(directory: &Path) -> Command {
    let mut command = Command::cargo_bin("mdtablefix").expect("cargo binary");
    command
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("HOME", directory)
        .env("USERPROFILE", directory)
        .env("XDG_CONFIG_HOME", directory)
        .env("XDG_CONFIG_DIRS", directory)
        .env("APPDATA", directory)
        .env("LOCALAPPDATA", directory)
        .env("LC_ALL", "C")
        .env("LANGUAGE", "");
    for (name, _) in std::env::vars_os() {
        if carries_configuration_prefix(&name) {
            command.env_remove(name);
        }
    }

    command
}

/// Whether `name` is one of the binary's configuration variables.
///
/// The comparison is case-insensitive because the two platforms this suite
/// runs on disagree about case in environment names, and a variable the binary
/// would read as `MDTABLEFIX_WRAP` must be removed however the shell spelled
/// it.
fn carries_configuration_prefix(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy()
        .to_lowercase()
        .starts_with("mdtablefix_")
}
