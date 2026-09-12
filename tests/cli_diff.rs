//! Integration tests for `--diff`: determinism, independence from the
//! surrounding files, the read-only guarantee, and the closed-pipe early exit.
//!
//! These read badly as prose, which is why they are not in
//! `tests/features/diff_mode.feature`: determinism needs ten runs compared
//! against each other, order-independence needs the same file rendered alone
//! and beside a sibling, the read-only guarantee is asserted against a
//! directory snapshot, and the pipe test needs a child that cannot finish
//! writing.

use std::{fs, path::Path, time::SystemTime};

use assert_cmd::Command;
use tempfile::tempdir;

/// A ragged table that every mode must agree needs reformatting.
const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// A second ragged table, so two files' diffs are distinguishable.
const ALT_RAGGED: &str = "|A|B|C|\n|---|---|---|\n|1|2|3|\n";

/// [`RAGGED`] without its final terminator, which the formatter adds back.
const UNTERMINATED: &str = "|A|B|\n|---|---|\n|1|2|";

/// The same table already aligned, which no mode may change.
///
/// These are the formatter's own bytes, not a hand-written approximation:
/// every cell is padded to the delimiter row's width, so `| A | B |` would
/// itself be reported as drift.
const CLEAN: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// The hunk a three-line replacement produces, which is all a ragged table is.
///
/// A hunk header's line numbers depend on where the table sits, so the
/// above-threshold test looks for the deleted lines instead.
const HUNK: &str = "@@ -1,3 +1,3 @@";

/// How many tables the spread fixture holds, and how much prose separates
/// them. Four tables of three lines between three runs of 340 prose lines puts
/// the fixture above the line-count threshold with changes far apart.
const TABLES: usize = 4;
const PROSE_PER_TABLE: usize = 340;

/// How many fixtures [`a_closed_pipe_is_a_successful_early_exit`] writes, and
/// how much padding each name carries.
///
/// The pair is chosen per platform because the limits that bound it pull in
/// opposite directions: the run must write past what the pipe holds for the
/// child to still be blocked when the read end closes, and the names are what
/// the output is made of. Unix has a 64 KiB pipe and room for a 93 KiB command
/// line; Windows refuses a command line past 32 767 characters with
/// `ERROR_FILENAME_EXCED_RANGE`, so the Unix pair cannot even be spawned there,
/// and its pipe is created with a size hint of zero — the system default, not
/// the Unix 64 KiB — so a far smaller run fills it. `tests/cli_check.rs` sizes
/// its own fixture the same way, for the same reason.
#[cfg(unix)]
const CLOSED_PIPE_FILES: usize = 500;
#[cfg(unix)]
const CLOSED_PIPE_PADDING: usize = 180;
#[cfg(not(unix))]
const CLOSED_PIPE_FILES: usize = 250;
#[cfg(not(unix))]
const CLOSED_PIPE_PADDING: usize = 80;

/// The spread fixture's first table line, as the deletions side presents it.
const FIRST_TABLE_DELETION: &str = "-|A0|B0|";

/// A prose line as the deletions side would present it.
///
/// Prose survives a default rewrite untouched, so a prose line on the
/// deletions side means the diff replaced a region it should have matched.
const PROSE_DELETION: &str = "-prose line";

/// How many times the determinism test repeats the same invocation.
const REPEATS: usize = 10;

/// A file above the line-count threshold whose changes are far apart.
///
/// A single changed table is cheap to diff whatever the threshold says: with
/// a common prefix and suffix around it, trimming leaves the line-diff
/// algorithm a three-line range, so neither algorithm has real work to do.
/// Spreading the tables across the whole file leaves that trimming nothing to
/// remove, which is the only shape in which the threshold's choice of
/// algorithm is visible.
fn spread_out() -> String {
    let mut lines = Vec::new();
    let mut prose = 0;
    for table in 0..TABLES {
        lines.push(format!("|A{table}|B{table}|"));
        lines.push(String::from("|---|---|"));
        lines.push(format!("|{table}0|{table}1|"));
        for _ in 0..PROSE_PER_TABLE {
            lines.push(format!("prose line {prose}"));
            prose += 1;
        }
    }
    let mut content = lines.join("\n");
    content.push('\n');
    content
}

/// Runs the binary with `args` in `directory` and returns its raw output.
///
/// Paths are passed relative to `directory`, which is also the working
/// directory, so the diff headers name the files as the user wrote them
/// rather than by an absolute path the temporary directory invented.
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

/// Runs `args` `REPEATS` times and returns the first run's standard output,
/// having asserted that every run exited `expected_status` and printed exactly
/// those bytes.
///
/// Repetition alone would be satisfied by ten identical *empty* outputs, which
/// is exactly what a rejected `--diff` flag produces, so the caller is expected
/// to assert something about the returned output as well.
fn repeated_output(directory: &Path, args: &[&str], expected_status: i32) -> String {
    let mut first: Option<String> = None;
    for attempt in 0..REPEATS {
        let output = run_in(directory, args);
        assert_eq!(
            status_of(&output),
            expected_status,
            "attempt {attempt} exited {} rather than {expected_status}: {}",
            status_of(&output),
            stderr_of(&output)
        );
        let stdout = stdout_of(&output);
        match &first {
            None => first = Some(stdout),
            Some(expected) => assert_eq!(
                &stdout, expected,
                "attempt {attempt} differed from the first run"
            ),
        }
    }
    first.expect("REPEATS is not zero, so at least one run happened")
}

/// `INV-DETERMINISTIC`: the same invocation prints the same bytes every time.
///
/// Nothing in the diff path may consult the clock or a hash seed: the headers
/// carry no timestamps, and the line-diff algorithm is chosen by a line count
/// rather than by a wall-clock budget.
#[test]
fn diff_is_deterministic_across_ten_runs() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("ragged.md"), RAGGED).expect("write fixture");

    let stdout = repeated_output(dir.path(), &["--diff", "ragged.md"], 1);

    assert!(
        stdout.contains(HUNK),
        "ten identical empty outputs would be vacuous: {stdout:?}"
    );
}

/// `INV-DETERMINISTIC` above the degradation threshold, where the renderer
/// switches from Myers to Patience.
///
/// The line count is the *only* thing that chooses between the two algorithms,
/// and this is the case where a wall-clock budget would have been tempting,
/// since it is the one long enough to look slow. The localisation assertion is
/// what rejects that budget: a deadline that trips makes the renderer replace
/// the region wholesale rather than diff it, and the prose that no rewrite can
/// touch would come back as deleted.
#[test]
fn diff_is_deterministic_above_the_threshold() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("spread.md"), spread_out()).expect("write fixture");

    let stdout = repeated_output(dir.path(), &["--diff", "spread.md"], 1);

    assert!(
        stdout.contains(FIRST_TABLE_DELETION),
        "the tables must be diffed, not skipped: {stdout:?}"
    );
    let replaced = stdout.matches(PROSE_DELETION).count();
    assert_eq!(
        replaced,
        0,
        "the diff must localise the change to the tables; {replaced} of the {} untouched prose \
         lines came back as deleted, which is a wholesale replacement rather than a diff",
        TABLES * PROSE_PER_TABLE
    );
}

/// `INV-DETERMINISTIC`: a file's diff depends only on that file's bytes, not
/// on which other files were named beside it or in what order.
///
/// Asserting this by concatenation rather than by comparing two runs of the
/// same set keeps the claim precise: the two-file output is exactly the two
/// one-file outputs, so neither the presence nor the position of a sibling can
/// change what a file's own diff says.
#[test]
fn diff_is_independent_of_its_siblings() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("alpha.md"), RAGGED).expect("write fixture");
    fs::write(dir.path().join("bravo.md"), ALT_RAGGED).expect("write fixture");

    let alpha = run_in(dir.path(), &["--diff", "alpha.md"]);
    let bravo = run_in(dir.path(), &["--diff", "bravo.md"]);
    assert_eq!(status_of(&alpha), 1, "{}", stderr_of(&alpha));
    assert_eq!(status_of(&bravo), 1, "{}", stderr_of(&bravo));
    let alone_alpha = stdout_of(&alpha);
    let alone_bravo = stdout_of(&bravo);
    assert_ne!(
        alone_alpha, alone_bravo,
        "the two fixtures must produce different diffs, or this test is vacuous"
    );

    let together = run_in(dir.path(), &["--diff", "alpha.md", "bravo.md"]);
    let reversed = run_in(dir.path(), &["--diff", "bravo.md", "alpha.md"]);

    assert_eq!(
        stdout_of(&together),
        format!("{alone_alpha}{alone_bravo}"),
        "a file's diff must not depend on its siblings"
    );
    assert_eq!(
        stdout_of(&reversed),
        format!("{alone_bravo}{alone_alpha}"),
        "reversing the arguments must reverse the blocks and nothing else"
    );
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

/// `INV-NOWRITE` for the verbose rendering: `--diff` reads, and reports a file
/// it could not read, without touching anything else in the directory.
///
/// The snapshot is what the `ReadOnlyDir` type cannot prove on its own: an
/// ambient write, a backup file, or a lock file would all show up here and
/// nowhere else.
#[test]
fn directory_snapshot_unchanged() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("clean.md"), CLEAN).expect("write fixture");
    fs::write(dir.path().join("ragged.md"), RAGGED).expect("write fixture");
    let before = snapshot(dir.path());

    let output = run_in(
        dir.path(),
        &["--diff", "clean.md", "ragged.md", "missing.md"],
    );

    assert_eq!(status_of(&output), 2, "an unreadable file must exit 2");
    assert!(
        stderr_of(&output).contains("missing.md"),
        "the run must name the unreadable file, so the error is a read failure rather than a \
         rejected command line: {}",
        stderr_of(&output)
    );
    assert!(
        stdout_of(&output).contains(HUNK),
        "one unreadable file must not suppress the diff of the readable ones: {:?}",
        stdout_of(&output)
    );
    assert_eq!(
        snapshot(dir.path()),
        before,
        "--diff must leave entry set, lengths, and modification times untouched"
    );
}

/// A file whose last line carries no terminator is diffed with the marker that
/// says so, on the deletions side.
///
/// The original is ragged as well as unterminated, so this is the diff a user
/// would see rather than a single-line replacement: the marker sits between
/// the deleted lines and the added ones, and the additions side carries none,
/// because the formatter terminates every line it writes — see
/// `write_unified_diff`'s contract.
#[test]
fn an_unterminated_original_is_marked() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("ragged.md"), UNTERMINATED).expect("write fixture");

    let output = run_in(dir.path(), &["--diff", "ragged.md"]);

    assert_eq!(status_of(&output), 1, "drift must exit 1");
    assert_eq!(
        stdout_of(&output),
        "--- ragged.md\n+++ ragged.md\n@@ -1,3 +1,3 @@\n-|A|B|\n-|---|---|\n-|1|2|\n\\ No newline \
         at end of file\n+| A   | B   |\n+| --- | --- |\n+| 1   | 2   |\n"
    );
}

/// A closed pipe must not turn into a panic, which would exit `101` — a status
/// the tool does not document.
///
/// This is the plan's `mdtablefix --diff *.md | head` shape, sized like the
/// `--check` one in `tests/cli_check.rs`: every file drifts, so the run would
/// exit `1` if it completed, and enough reports are queued to overflow a pipe
/// buffer, so the child is still writing when the read end closes. Exit `0` is
/// therefore reachable only through the early exit.
/// [`CLOSED_PIPE_FILES`] and [`CLOSED_PIPE_PADDING`] carry the size and why it
/// differs by platform.
#[test]
fn a_closed_pipe_is_a_successful_early_exit() {
    use std::{
        io::Read as _,
        process::{Command, Stdio},
    };

    let dir = tempdir().expect("create temporary directory");
    let names: Vec<String> = (0..CLOSED_PIPE_FILES)
        .map(|index| format!("{index:0>3}-{}.md", "d".repeat(CLOSED_PIPE_PADDING)))
        .collect();
    for name in &names {
        fs::write(dir.path().join(name), RAGGED).expect("write fixture");
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_mdtablefix"))
        .arg("--diff")
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

    // On Unix the reports are past what the pipe holds, so the child is blocked
    // in `write` when the read end closes and cannot have completed the run.
    // Windows cannot be held to the same standard: its reports are bounded by
    // the command-line cap described on the constants above, so the child may
    // finish before the parent's read end closes, and `1` — drift found — is a
    // documented status rather than a defect. A panic or a crash is not
    // documented on either platform.
    #[cfg(unix)]
    assert_eq!(
        output.status.code(),
        Some(0),
        "a closed pipe is an early exit, not a failure: {stderr}"
    );
    #[cfg(not(unix))]
    assert!(
        matches!(output.status.code(), Some(0 | 1)),
        "a closed pipe is an early exit or a completed run, never an undocumented status: {stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "the write failure is handled, not panicked on: {stderr}"
    );
}
