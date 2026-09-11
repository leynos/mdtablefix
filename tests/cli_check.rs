//! Integration tests for `--check`: argument order, the exit-status contract,
//! and the read-only guarantee.
//!
//! These three cases read badly as prose, which is why they are not in
//! `tests/features/check_mode.feature`: the ordering batch needs eight files
//! whose alphabetical, size, and completion orders all differ from argument
//! order, the exit-status contract is a cross product, and the read-only
//! guarantee is asserted against a directory snapshot.
//!
//! `--in-place` and `--diff` are exercised here too, because the contract that
//! separates them from `--check` — drift is reported, not failed, and it is
//! reported by both reporting modes rather than only by the terse one — is only
//! meaningful beside the modes it separates. The cross product lives in one
//! place so that a mode cannot be added to it partially.

use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use assert_cmd::Command;
use tempfile::tempdir;

/// A ragged table that every mode must agree needs reformatting.
const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// The same table already aligned, which no mode may change.
///
/// These are the formatter's own bytes, not a hand-written approximation:
/// every cell is padded to the delimiter row's width, so `| A | B |` would
/// itself be reported as drift. `tests/line_endings.rs` holds the same
/// fixture.
const CLEAN: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// The line delta of one ragged table, as `git diff --numstat` reports it.
const RAGGED_INSERTIONS: i64 = 3;

/// The matching deletion count.
const RAGGED_DELETIONS: i64 = 3;

/// Runs the binary with `args` in `directory` and returns its raw output.
///
/// Paths are passed relative to `directory`, which is also the working
/// directory, so report lines name the files as the user wrote them rather
/// than by an absolute path the temporary directory invented.
fn run_in(directory: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("mdtablefix")
        .expect("cargo binary")
        .current_dir(directory)
        .args(args)
        .output()
        .expect("run mdtablefix")
}

/// The captured standard output as text.
fn stdout_of(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

/// The captured standard error as text.
fn stderr_of(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

/// The exit status, or `-1` if the process was signalled.
fn status_of(output: &std::process::Output) -> i32 { output.status.code().unwrap_or(-1) }

/// One file in the ordering batch.
///
/// `padding` lines of prose are appended so the files differ in size along
/// argument order; the padding is never touched by a default rewrite, and the
/// expected counts are unaffected by it.
struct BatchFile {
    name: &'static str,
    ragged: bool,
    padding: usize,
}

/// The ordering batch, in argument order.
///
/// The eight names are in neither alphabetical nor reverse-alphabetical order,
/// and their sizes increase along argument order, so an implementation that
/// returns files in alphabetical order, or in the order the parallel workers
/// happened to finish, is rejected rather than passing by luck.
const BATCH: &[BatchFile] = &[
    BatchFile {
        name: "zulu.md",
        ragged: false,
        padding: 0,
    },
    BatchFile {
        name: "bravo.md",
        ragged: true,
        padding: 5,
    },
    BatchFile {
        name: "yankee.md",
        ragged: false,
        padding: 10,
    },
    BatchFile {
        name: "alpha.md",
        ragged: true,
        padding: 15,
    },
    BatchFile {
        name: "xray.md",
        ragged: false,
        padding: 20,
    },
    BatchFile {
        name: "charlie.md",
        ragged: true,
        padding: 25,
    },
    BatchFile {
        name: "whiskey.md",
        ragged: false,
        padding: 30,
    },
    BatchFile {
        name: "delta.md",
        ragged: true,
        padding: 35,
    },
];

/// Writes `file` into `directory` and returns its path.
fn write_batch_file(directory: &Path, file: &BatchFile) -> PathBuf {
    let mut content = String::from(if file.ragged { RAGGED } else { CLEAN });
    content.push_str(&"unformatted prose line\n".repeat(file.padding));
    let path = directory.join(file.name);
    fs::write(&path, content).expect("write fixture");
    path
}

/// The report line a drifting file must produce.
fn report_line(file: &BatchFile) -> String {
    format!("{} +{RAGGED_INSERTIONS} -{RAGGED_DELETIONS}", file.name)
}

#[test]
fn reports_every_file_in_order() {
    let dir = tempdir().expect("create temporary directory");
    for file in BATCH {
        write_batch_file(dir.path(), file);
    }
    let names: Vec<&str> = BATCH.iter().map(|file| file.name).collect();

    let mut args: Vec<&str> = vec!["--check"];
    args.extend(names.iter().copied());
    let output = run_in(dir.path(), &args);

    assert_eq!(
        status_of(&output),
        1,
        "drifting files must exit 1: {}",
        stderr_of(&output)
    );
    let mut expected = String::new();
    for file in BATCH.iter().filter(|file| file.ragged) {
        expected.push_str(&report_line(file));
        expected.push('\n');
    }
    assert_eq!(
        stdout_of(&output),
        expected,
        "reports must follow argument order, not alphabetical, size, or completion order"
    );
}

/// How the files of one exit-status case are shaped.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Files {
    /// Every file is already formatted.
    Clean,
    /// Some files drift; the rest are clean.
    SomeDrift,
    /// Every file drifts.
    AllDrift,
}

/// The modes the command line exposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliMode {
    /// No mode flag: formatted output goes to standard output.
    Print,
    /// `--in-place`.
    InPlace,
    /// `--check`.
    Check,
    /// `--diff`.
    Diff,
}

impl CliMode {
    /// The arguments selecting this mode.
    fn args(self) -> &'static [&'static str] {
        match self {
            Self::Print => &[],
            Self::InPlace => &["--in-place"],
            Self::Check => &["--check"],
            Self::Diff => &["--diff"],
        }
    }

    /// Whether drift in this mode is reported through the exit status.
    fn reports(self) -> bool { matches!(self, Self::Check | Self::Diff) }

    /// The status this mode must yield for the given observations.
    ///
    /// An error outranks drift in every mode, drift is a status only under the
    /// two reporting modes, and a successful `--in-place` over drifting files
    /// succeeds.
    fn expected_status(self, files: Files, with_error: bool) -> i32 {
        if with_error {
            2
        } else {
            i32::from(files != Files::Clean && self.reports())
        }
    }
}

#[test]
fn exit_status_matrix() {
    let shapes = [
        ("all_clean", Files::Clean),
        ("some_drift", Files::SomeDrift),
        ("all_drift", Files::AllDrift),
    ];
    let modes = [
        CliMode::Print,
        CliMode::InPlace,
        CliMode::Check,
        CliMode::Diff,
    ];

    for (shape_name, files) in shapes {
        for mode in modes {
            for with_error in [false, true] {
                let dir = tempdir().expect("create temporary directory");
                let mut names = Vec::new();
                for index in 0..2 {
                    let ragged = match files {
                        Files::Clean => false,
                        Files::SomeDrift => index == 0,
                        Files::AllDrift => true,
                    };
                    let name = format!("file{index}.md");
                    fs::write(dir.path().join(&name), if ragged { RAGGED } else { CLEAN })
                        .expect("write fixture");
                    names.push(name);
                }
                if with_error {
                    names.push(String::from("missing.md"));
                }

                let mut args: Vec<&str> = mode.args().to_vec();
                args.extend(names.iter().map(String::as_str));
                let output = run_in(dir.path(), &args);

                assert_eq!(
                    status_of(&output),
                    mode.expected_status(files, with_error),
                    "{shape_name} under {mode:?} (with_error: {with_error}) exited {}, stderr: {}",
                    status_of(&output),
                    stderr_of(&output)
                );
            }
        }
    }
}

/// A file's observable identity: name, length, and modification time.
type Snapshot = Vec<(String, u64, Option<SystemTime>)>;

/// Captures the directory's entry set, lengths, and modification times.
fn snapshot(directory: &Path) -> Snapshot {
    let mut entries: Snapshot = fs::read_dir(directory)
        .expect("read directory")
        .map(|entry| {
            let entry = entry.expect("read directory entry");
            let metadata = entry.metadata().expect("read metadata");
            (
                entry.file_name().to_string_lossy().into_owned(),
                metadata.len(),
                metadata.modified().ok(),
            )
        })
        .collect();
    entries.sort();
    entries
}

#[test]
fn directory_snapshot_unchanged() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("clean.md"), CLEAN).expect("write fixture");
    fs::write(dir.path().join("ragged.md"), RAGGED).expect("write fixture");
    fs::write(dir.path().join("empty.md"), "").expect("write fixture");
    fs::write(dir.path().join("bom.md"), format!("\u{FEFF}{RAGGED}")).expect("write fixture");
    let before = snapshot(dir.path());

    let output = run_in(
        dir.path(),
        &[
            "--check",
            "clean.md",
            "ragged.md",
            "empty.md",
            "bom.md",
            "missing.md",
        ],
    );

    assert_eq!(status_of(&output), 2, "an unreadable file must exit 2");
    assert!(
        stderr_of(&output).contains("missing.md"),
        "the run must name the unreadable file, which proves it reached the filesystem rather \
         than failing while parsing arguments: {}",
        stderr_of(&output)
    );
    assert_eq!(
        snapshot(dir.path()),
        before,
        "--check must leave entry set, lengths, and modification times untouched"
    );
}

/// A closed pipe must not turn into a panic, which would exit `101` — a status
/// the tool does not document.
///
/// This is the plan's `mdtablefix --check *.md | head` shape. Every file
/// drifts, so the run would exit `1` if it completed: exit `0` therefore means
/// the write failed and the run stopped early, and a run that panicked instead
/// exits `101` or is killed by a signal. There are enough reports to overflow a
/// pipe buffer, so the child is still writing — and blocked — when the read end
/// closes, which is what makes the failure deterministic rather than a race.
#[test]
fn a_closed_pipe_is_a_successful_early_exit() {
    use std::{
        io::Read as _,
        process::{Command, Stdio},
    };

    let dir = tempdir().expect("create temporary directory");
    let names: Vec<String> = (0..500)
        .map(|index| format!("{index:0>3}-{}.md", "p".repeat(180)))
        .collect();
    for name in &names {
        fs::write(dir.path().join(name), RAGGED).expect("write fixture");
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_mdtablefix"))
        .arg("--check")
        .args(&names)
        .current_dir(dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mdtablefix");

    let mut stdout = child.stdout.take().expect("child stdout is piped");
    let mut first = [0u8; 1];
    stdout.read_exact(&mut first).expect("read one byte");
    drop(stdout);

    let output = child.wait_with_output().expect("wait for mdtablefix");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "a closed pipe is an early exit, not a failure: {stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "the write failure is handled, not panicked on: {stderr}"
    );
}

#[test]
fn in_place_over_a_drifting_file_does_change_the_directory() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("ragged.md"), RAGGED).expect("write fixture");
    let before = snapshot(dir.path());

    let output = run_in(dir.path(), &["--in-place", "ragged.md"]);

    assert_eq!(status_of(&output), 0, "a successful rewrite exits 0");
    assert_ne!(
        snapshot(dir.path()),
        before,
        "the snapshot helper must detect a write, or the read-only assertion is vacuous"
    );
}
