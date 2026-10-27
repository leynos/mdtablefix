//! The process-level configuration contracts: exit status and stream split.
//!
//! The in-process tables in `src/config_tests.rs` prove which layer wins. This
//! file proves what a user, a script, or a CI job observes, which only a child
//! process can show: the status the shell sees and which stream each line went
//! to. Each fixture is a real file on disk, and every child's configuration base
//! directories point into a temporary tree, so the developer's own
//! `~/.config/mdtablefix/config.toml` cannot decide a result.
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

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

/// A ragged table, which every formatting mode must agree needs reformatting.
const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// The same table already aligned, which no mode may change.
const CLEAN: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

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
struct Run {
    /// The exit status, or `-1` when the process was signalled.
    status: i32,
    /// Standard output as text.
    stdout: String,
    /// Standard error as text.
    stderr: String,
}

/// A temporary tree with a scrubbed configuration environment.
struct Fixture {
    /// Deleted when the test ends.
    directory: TempDir,
}

impl Fixture {
    /// Creates the tree and the directories discovery would consult.
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

    /// Writes `contents` to `name`, creating its parent directories.
    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.root().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create the file's parent directory");
        }
        std::fs::write(&path, contents).expect("write a fixture");

        path
    }

    /// An empty configuration file, whose contents state nothing.
    fn empty_config(&self) -> PathBuf { self.write("empty.toml", "") }

    /// The child process, scrubbed and pointed at the fixture's directories.
    ///
    /// An absent or all-empty `XDG_CONFIG_DIRS` would fall back to `/etc/xdg`,
    /// which belongs to the host, so it is set rather than removed. `HOME` and
    /// `USERPROFILE` are both set because discovery prefers whichever is
    /// non-empty and treats the other as a fallback.
    fn command(&self) -> Command {
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
    fn finish(command: &mut Command) -> Run {
        let output = command.output().expect("run the binary");

        Run {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Runs the binary in the tree with `args`, feeding `stdin`.
    fn run(&self, args: &[&str], stdin: &str) -> Run {
        Self::finish(self.command().args(args).write_stdin(stdin))
    }

    /// Runs the binary with `args`, from the configuration file `config` names.
    fn run_selected(&self, config: &Path, args: &[&str], stdin: &str) -> Run {
        Self::finish(
            self.command()
                .env("MDTABLEFIX_CONFIG_PATH", config)
                .args(args)
                .write_stdin(stdin),
        )
    }
}

/// A run with no configuration formats exactly as it did before the adoption.
///
/// The output is pinned character for character, so a changed default — a
/// switch that arrives enabled, or an extension list that narrows — fails here
/// rather than in a downstream repository.
#[test]
fn a_run_with_no_configuration_formats_as_it_always_did() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();

    let run = fixture.run_selected(&empty, &[], RAGGED);

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, CLEAN);
    assert!(run.stderr.is_empty(), "stderr: {}", run.stderr);
}

/// A discovered file can switch a transformation on, and nothing else changes.
#[test]
fn a_discovered_file_supplies_a_setting() {
    let fixture = Fixture::new();
    fixture.write("xdg/mdtablefix/config.toml", "ellipsis = true\n");

    let run = fixture.run(&[], "a...b\n");

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "a…b\n");
    assert!(run.stderr.is_empty(), "stderr: {}", run.stderr);
}

/// A selected file can switch a transformation on, and nothing else changes.
#[test]
fn a_selected_file_supplies_a_setting() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "ellipsis = true\n");

    let run = fixture.run_selected(&config, &[], "a...b\n");

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "a…b\n");
    assert!(run.stderr.is_empty(), "stderr: {}", run.stderr);
}

/// A selected file that is absent is an error on standard error, exit 2.
#[test]
fn a_missing_selected_file_exits_two_with_no_standard_output() {
    let fixture = Fixture::new();
    let missing = fixture.root().join("absent.toml");

    let run = fixture.run_selected(&missing, &["--check", "drift.md"], "");

    assert_eq!(run.status, 2);
    assert_eq!(run.stdout, "", "the diagnostic belongs on stderr");
    assert!(
        run.stderr.contains("required configuration file not found"),
        "stderr: {}",
        run.stderr
    );
}

/// A selected file that is not valid TOML is an error on standard error, exit 2.
#[test]
fn a_malformed_selected_file_exits_two_with_no_standard_output() {
    let fixture = Fixture::new();
    let config = fixture.write("broken.toml", "wrap = \n");

    let run = fixture.run_selected(&config, &["--check", "drift.md"], "");

    assert_eq!(run.status, 2);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr.contains("cannot resolve configuration"),
        "stderr: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains("TOML parse error"),
        "the diagnostic names the parse failure: {}",
        run.stderr
    );
}

/// An environment value the merge cannot type is an error, exit 2.
///
/// A bare word is a plain string in the provider's dialect, and a string is not
/// a sequence, so this is the shape a user reaches by writing a list without its
/// brackets.
#[test]
fn an_untypable_environment_value_exits_two() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();

    let mut command = fixture.command();
    let run = Fixture::finish(
        command
            .env("MDTABLEFIX_CONFIG_PATH", &empty)
            .env("MDTABLEFIX_MD_EXTS", "rst xml")
            .args(["--git", "--list-files"]),
    );

    assert_eq!(run.status, 2);
    assert!(run.stdout.is_empty(), "stdout: {}", run.stdout);
    assert!(
        run.stderr.contains("expected a sequence"),
        "stderr: {}",
        run.stderr
    );
}

/// A reporting mode keeps its split: findings on stdout, summary on stderr.
///
/// The drift is real, so the exit status is 1 rather than 2 — a configuration
/// error is the only failure that exits 2 without reporting a file.
#[test]
fn check_reports_drift_on_standard_output_and_sums_on_standard_error() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();
    fixture.write("drift.md", RAGGED);

    let run = fixture.run_selected(&empty, &["--check", "drift.md"], "");

    assert_eq!(run.status, 1);
    assert_eq!(run.stdout, "drift.md +3 -3\n");
    assert_eq!(run.stderr, "1 file would be reformatted.\n");
}

/// `--md-exts` typed without `--git` is a usage error that exits 2.
///
/// This is the negative half of the typed-flag contract: the flag's *provenance*
/// decides, so a value supplied by a file or the environment does not trigger
/// it. The positive half is [`a_configured_extension_list_is_not_a_typed_flag`].
#[test]
fn a_typed_extension_list_without_git_exits_two() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();

    let run = fixture.run_selected(&empty, &["--md-exts", "toml"], "");

    assert_eq!(run.status, 2);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr.contains("--md-exts requires --git"),
        "stderr: {}",
        run.stderr
    );
}

/// A configured extension list is not a typed flag, so it needs no `--git`.
///
/// The same value that fails on the command line succeeds here, because nothing
/// the user typed brought it. The run deliberately formats a standard input
/// rather than asking for `--version`: clap answers a version request before
/// `Settings::resolve` is reached, so that spelling would pass whether or not
/// the typed-flag rule were correct.
#[test]
fn a_configured_extension_list_is_not_a_typed_flag() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "md_exts = [\"toml\"]\n");

    let run = fixture.run_selected(&config, &[], RAGGED);

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, CLEAN);
    assert!(run.stderr.is_empty(), "stderr: {}", run.stderr);
}

/// A configured `git` drives selection on a bare invocation.
///
/// The reverse of the flag contract: the setting is not typed, so clap's
/// `inputs` group is not satisfied by it, yet the resolved settings select from
/// the repository. Outside one, that selection fails with this tool's own
/// diagnostic rather than falling back to standard input no one offered.
#[test]
fn a_configured_git_selection_runs_without_the_flag() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "git = true\n");

    let run = fixture.run_selected(&config, &[], RAGGED);

    assert_eq!(run.status, 2);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr.contains("git ls-files"),
        "stderr: {}",
        run.stderr
    );
}

/// A configured `git` does not satisfy clap's `inputs` group.
///
/// `--list-files` is a git-only flag, so typing it without `--git` is a usage
/// error however the setting arrived. A configuration file cannot make a
/// command line valid.
#[test]
fn a_configured_git_selection_does_not_satisfy_the_inputs_group() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "git = true\n");

    let run = fixture.run_selected(&config, &["--list-files"], "");

    assert_eq!(run.status, 2);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr.contains("required arguments were not provided"),
        "stderr: {}",
        run.stderr
    );
}

/// A positional path outranks a configured `git`, and no repository is touched.
///
/// The file is formatted from its path, so a configured `git` must not turn the
/// run into a repository query that would fail outside one.
#[test]
fn a_positional_path_outranks_a_configured_git_selection() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "git = true\n");
    fixture.write("notes.md", RAGGED);

    let run = fixture.run_selected(&config, &["notes.md"], "");

    assert_eq!(run.status, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, CLEAN);
}

/// A value the extension parser rejects names its key, exit 2.
///
/// The list passes the merge — it is a sequence of strings — and is rejected
/// afterwards by the same parser `--md-exts` uses, which is why the diagnostic
/// names the key rather than relaying a TOML or type error.
#[test]
fn a_rejected_extension_names_its_key() {
    let fixture = Fixture::new();
    let config = fixture.write("selected.toml", "md_exts = [\"a.b\"]\n");

    let run = fixture.run_selected(&config, &[], RAGGED);

    assert_eq!(run.status, 2);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr
            .contains("invalid configuration value for md_exts"),
        "stderr: {}",
        run.stderr
    );
}

/// The help rendering gains no configuration surface.
///
/// A `--help` that grew a configuration section or a new flag would be a
/// user-visible change the issue did not ask for; the snapshot file pins the
/// full text, and this states the claim where the adoption's own tests live.
#[test]
fn help_still_describes_only_the_command_line() {
    let fixture = Fixture::new();
    let empty = fixture.empty_config();

    let run = fixture.run_selected(&empty, &["--help"], "");

    assert_eq!(run.status, 0);
    assert!(
        run.stdout.starts_with("Reflow broken markdown tables"),
        "{}",
        run.stdout
    );
    assert!(
        !run.stdout.contains("MDTABLEFIX_CONFIG_PATH"),
        "{}",
        run.stdout
    );
    assert!(!run.stdout.contains("configuration"), "{}", run.stdout);
}
