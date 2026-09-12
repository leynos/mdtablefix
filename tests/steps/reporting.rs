//! Step definitions for `tests/features/check_mode.feature` and
//! `tests/features/diff_mode.feature`.
//!
//! Every step drives the real binary through `assert_cmd`, so the feature
//! specifies the command-line contract rather than a reimplementation of it.
//! The scenario's state is a `Slot`-bearing struct, which is the mutable state
//! pattern `rstest-bdd` supports without the ICE-prone `&mut World` world.
//!
//! The bindings themselves are in `tests/bdd_reporting.rs`, which must declare
//! this module before them: step registration happens at macro-expansion time,
//! so a binding expanded first would not yet see these definitions.

use std::{fs, path::Path, time::SystemTime};

use assert_cmd::Command;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, then, when};
use tempfile::TempDir;

/// A ragged table, which every mode must agree needs reformatting.
pub const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// The same table already aligned, which no mode may change.
///
/// The formatter's own bytes: cells are padded to the delimiter row's width,
/// so `| A | B |` would itself be drift.
pub const CLEAN: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// What one run of the binary produced.
#[derive(Clone, Debug)]
pub struct Run {
    /// The process exit status, or `-1` when the process was signalled.
    pub status: i32,
    /// Standard output as text.
    pub stdout: String,
    /// Standard error as text.
    pub stderr: String,
}

/// A directory's observable identity: each entry's name, bytes, and
/// modification time.
pub type Fingerprint = Vec<(String, Vec<u8>, Option<SystemTime>)>;

/// The state one scenario accumulates.
///
/// Each field is a `Slot`, so a step borrows the whole state immutably and
/// fills one slot, which is what lets `Given`, `When`, and `Then` share data
/// without a mutable borrow crossing a step boundary.
#[derive(Default, ScenarioState)]
pub struct ReportingState {
    /// The directory the scenario's files live in, created on first use.
    dir: Slot<TempDir>,
    /// The files named on the command line, in argument order.
    files: Slot<Vec<String>>,
    /// The directory's identity immediately before the run.
    before: Slot<Fingerprint>,
    /// The most recent run.
    run: Slot<Run>,
    /// Every run's standard output, when a scenario repeats the run.
    outputs: Slot<Vec<String>>,
}

/// The scenario's directory, created on first use.
fn directory_path(state: &ReportingState) -> std::path::PathBuf {
    state
        .dir
        .get_or_insert_with(|| tempfile::tempdir().expect("temporary directory"))
        .path()
        .to_path_buf()
}

/// Writes `content` as `name` and registers it in argument order.
fn create_file(state: &ReportingState, name: &str, content: &str) {
    fs::write(directory_path(state).join(name), content).expect("write fixture");
    state
        .files
        .get_or_insert_with(Vec::new)
        .push(name.to_string());
}

/// Captures the directory's entry names, bytes, and modification times.
fn fingerprint(directory: &Path) -> Fingerprint {
    let mut entries: Fingerprint = fs::read_dir(directory)
        .expect("read the directory")
        .map(|entry| {
            let entry = entry.expect("read a directory entry");
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).expect("read a fixture"),
                entry
                    .metadata()
                    .expect("read fixture metadata")
                    .modified()
                    .ok(),
            )
        })
        .collect();
    entries.sort();
    entries
}

/// The most recent run, which every `Then` step reads.
fn last_run(state: &ReportingState) -> Run {
    state
        .run
        .get()
        .expect("the scenario must run mdtablefix before asserting on it")
}

#[given("a Markdown file {name:string} that is already formatted")]
fn already_formatted(state: &ReportingState, name: String) { create_file(state, &name, CLEAN); }

#[given("a Markdown file {name:string} with an unaligned table")]
fn unaligned_table(state: &ReportingState, name: String) { create_file(state, &name, RAGGED); }

#[given("a Markdown file {name:string} already formatted with CRLF endings")]
fn crlf_formatted(state: &ReportingState, name: String) {
    create_file(state, &name, &CLEAN.replace('\n', "\r\n"));
}

#[given("a Markdown file {name:string} with a byte-order mark and an unaligned table")]
fn marked_unaligned(state: &ReportingState, name: String) {
    create_file(state, &name, &format!("\u{FEFF}{RAGGED}"));
}

#[given("a path {name:string} that does not exist")]
fn missing_path(state: &ReportingState, name: String) {
    state.files.get_or_insert_with(Vec::new).push(name);
}

/// Runs the binary once against the scenario's files, without recording it.
///
/// Separated from the step so a scenario that repeats the run can invoke it
/// without re-recording the pre-run fingerprint, which only the first run can
/// meaningfully capture.
fn run_once(state: &ReportingState, flags: &str) -> Run {
    let directory = directory_path(state);
    let files = state.files.get().unwrap_or_default();
    let output = Command::cargo_bin("mdtablefix")
        .expect("cargo binary")
        .current_dir(&directory)
        .args(flags.split_whitespace())
        .args(&files)
        .output()
        .expect("run mdtablefix");

    Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[when("mdtablefix runs with {flags:string} against those files")]
fn runs_against_files(state: &ReportingState, flags: String) {
    state.before.set(fingerprint(&directory_path(state)));
    state.run.set(run_once(state, &flags));
}

#[when("mdtablefix runs with {flags:string} against those files ten times")]
fn runs_ten_times(state: &ReportingState, flags: String) {
    let mut outputs = Vec::new();
    for _ in 0..10 {
        let run = run_once(state, &flags);
        outputs.push(run.stdout.clone());
        state.run.set(run);
    }
    state.outputs.set(outputs);
}

#[then("the exit status is {expected:i32}")]
fn exit_status_is(state: &ReportingState, expected: i32) {
    let run = last_run(state);
    assert_eq!(run.status, expected, "exit status, stderr: {}", run.stderr);
}

#[then("standard output is empty")]
fn stdout_is_empty(state: &ReportingState) {
    let run = last_run(state);
    assert!(
        run.stdout.is_empty(),
        "standard output was {:?}",
        run.stdout
    );
}

#[then("standard output is {expected:string}")]
fn stdout_is(state: &ReportingState, expected: String) {
    let run = last_run(state);
    assert_eq!(run.stdout, format!("{expected}\n"), "standard output");
}

#[then("standard output lists {first:string} before {second:string}")]
fn stdout_lists_before(state: &ReportingState, first: String, second: String) {
    let run = last_run(state);
    let position = |needle: &str| {
        run.stdout
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} is not reported in {:?}", run.stdout))
    };
    assert!(
        position(&first) < position(&second),
        "{first} must be reported before {second}: {:?}",
        run.stdout
    );
}

/// The unified-diff header names the file on both sides, with no timestamps,
/// so the output is stable and a consumer can recover the path.
#[then("the diff header names {name:string} on both sides")]
fn diff_header_names_both_sides(state: &ReportingState, name: String) {
    let run = last_run(state);
    let header = format!("--- {name}\n+++ {name}\n");
    assert!(
        run.stdout.contains(&header),
        "the diff must name {name} on both sides: {:?}",
        run.stdout
    );
}

#[then("the diff contains a hunk header")]
fn diff_contains_a_hunk_header(state: &ReportingState) {
    let run = last_run(state);
    assert!(
        run.stdout.lines().any(|line| line.starts_with("@@ ")),
        "the diff must contain a hunk header: {:?}",
        run.stdout
    );
}

/// Repetition alone would be satisfied by ten identical *empty* outputs, which
/// is exactly what a rejected `--diff` flag produces, so the step also requires
/// the repeated output to be something.
#[then("every run produced identical standard output")]
fn every_run_produced_identical_output(state: &ReportingState) {
    let outputs = state
        .outputs
        .get()
        .expect("the scenario must repeat the run before comparing its outputs");
    let first = outputs.first().expect("ten runs produce ten outputs");
    assert!(
        !first.is_empty(),
        "ten identical empty outputs would be vacuous: {}",
        last_run(state).stderr
    );
    for (index, output) in outputs.iter().enumerate() {
        assert_eq!(output, first, "run {index} differed from the first run");
    }
}

#[then("the summary reads {expected:string}")]
fn summary_reads(state: &ReportingState, expected: String) {
    let run = last_run(state);
    assert_eq!(run.stderr, format!("{expected}\n"), "standard error");
}

#[then("standard error mentions {text:string}")]
fn stderr_mentions(state: &ReportingState, text: String) {
    let run = last_run(state);
    assert!(
        run.stderr.contains(&text),
        "standard error must mention {text:?}: {:?}",
        run.stderr
    );
}

#[then("the summary reports {count:usize} file could not be read")]
fn summary_reports_unreadable(state: &ReportingState, count: usize) {
    let run = last_run(state);
    let noun = if count == 1 { "file" } else { "files" };
    let phrase = format!("{count} {noun} could not be read");
    assert!(
        run.stderr.contains(&phrase),
        "standard error must contain {phrase:?}: {:?}",
        run.stderr
    );
}

#[then("the working directory is byte-identical")]
fn directory_unchanged(state: &ReportingState) {
    let before = state
        .before
        .get()
        .expect("the run must record the directory's identity first");
    let after = fingerprint(&directory_path(state));
    assert_eq!(after, before, "the working directory must not change");
}

#[then("{name:string} is reformatted")]
fn is_reformatted(state: &ReportingState, name: String) {
    let content =
        fs::read_to_string(directory_path(state).join(&name)).expect("read the rewritten file");
    assert_eq!(content, CLEAN, "{name} must hold the formatted table");
}
