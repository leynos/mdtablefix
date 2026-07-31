//! Regression coverage for the `check-static-regexes` lint guard.
//!
//! The guard lives in `scripts/check-static-regexes.sh` (invoked by the
//! Makefile's `check-static-regexes` target). It rejects hand-rolled static
//! regular expressions that bypass the `lazy_regex!` convention by wrapping
//! `Regex::new` directly in a supported lazy-wrapper constructor.
//!
//! These tests exercise the script directly so the Makefile and the tests
//! share a single source of truth for the scan. Every supported wrapper form
//! is asserted to be rejected, a clean fixture is asserted to pass, and a
//! ripgrep scan failure is asserted to propagate.
//!
//! Fixtures live under `tests/data/static_regex/` with a `.rs.txt` extension
//! so the guard (which scans `*.rs`) does not match the fixtures in place; the
//! tests copy each fixture into a temporary directory as a `.rs` file before
//! scanning.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use rstest::rstest;
use tempfile::TempDir;

/// The diagnostic emitted when a prohibited declaration is found.
const PROHIBITED_DIAGNOSTIC: &str = "static regular expressions must use lazy_regex!";

/// Every lazy-wrapper constructor form the guard must reject when it directly
/// wraps `Regex::new`. Each label maps to a fixture under
/// `tests/data/static_regex/` exercising one supported spelling:
///
///   * `lazylock_direct`           — `LazyLock::new(|| Regex::new(...))`
///   * `lazylock_qualified`        — `std::sync::LazyLock::new(|| Regex::new(...))`
///   * `lazylock_move`             — `LazyLock::new(move || Regex::new(...))`
///   * `once_cell_lazy_direct`     — `Lazy::new(|| Regex::new(...))`
///   * `once_cell_lazy_qualified`  — `once_cell::sync::Lazy::new(|| Regex::new(...))`
///   * `once_cell_lazy_move`       — `once_cell::sync::Lazy::new(move || Regex::new(...))`
const PROHIBITED_FORMS: &[&str] = &[
    "lazylock_direct",
    "lazylock_qualified",
    "lazylock_move",
    "once_cell_lazy_direct",
    "once_cell_lazy_qualified",
    "once_cell_lazy_move",
];

fn manifest_dir() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

fn script_path() -> PathBuf { manifest_dir().join("scripts/check-static-regexes.sh") }

fn fixture(label: &str) -> String {
    let path = manifest_dir().join(format!("tests/data/static_regex/{label}.rs.txt"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

/// Materialize `label`'s fixture as a `.rs` file inside a fresh temp directory.
fn scan_dir_with(label: &str) -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");
    std::fs::write(dir.path().join(format!("{label}.rs")), fixture(label))
        .expect("failed to write fixture into temp dir");
    dir
}

/// Run the guard against `scan_dir`, optionally overriding the `RG` ripgrep
/// command.
///
/// `rg` is the raw `RG` value, so it may carry arguments (for example
/// `rg --pcre2`); the guard splits it on whitespace. Passing `None` clears any
/// ambient `RG` so default-path runs exercise the guard's own `rg` default
/// deterministically.
fn run_guard(scan_dir: &Path, rg: Option<&str>) -> std::process::Output {
    let mut cmd = Command::new(script_path());
    cmd.arg(scan_dir);
    match rg {
        Some(rg) => cmd.env("RG", rg),
        None => cmd.env_remove("RG"),
    };
    cmd.output()
        .expect("failed to execute check-static-regexes.sh")
}

/// Write `script` to `<dir>/<name>` and mark it executable.
fn write_stub(dir: &Path, name: &str, script: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, script).expect("failed to write stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("failed to chmod stub");
    }
    path
}

#[rstest]
fn rejects_prohibited_lazy_wrapper_form(#[values(0, 1, 2, 3, 4, 5)] index: usize) {
    let label = PROHIBITED_FORMS[index];
    let dir = scan_dir_with(label);

    let output = run_guard(dir.path(), None);

    assert_eq!(
        output.status.code(),
        Some(1),
        "form `{label}` should be rejected with status 1"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(PROHIBITED_DIAGNOSTIC),
        "form `{label}` should emit the prohibited diagnostic, got: {stdout}"
    );
}

#[test]
fn accepts_clean_sources() {
    // The sanctioned `lazy_regex!` idiom plus an unrelated non-static
    // `Regex::new` call that must not trip the guard.
    let dir = scan_dir_with("clean");

    let output = run_guard(dir.path(), None);

    assert_eq!(
        output.status.code(),
        Some(0),
        "clean sources should pass; stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn propagates_ripgrep_scan_failure() {
    let dir = TempDir::new().expect("failed to create temp dir");
    // A stub standing in for ripgrep that fails with a distinctive status.
    let stub = write_stub(dir.path(), "rg-stub.sh", "#!/bin/sh\nexit 3\n");

    let output = run_guard(dir.path(), Some(&stub.display().to_string()));

    assert_eq!(
        output.status.code(),
        Some(3),
        "a ripgrep scan failure should propagate its exit status"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to scan Rust sources (rg exit 3)"),
        "scan failure should emit the scan-failure diagnostic, got: {stderr}"
    );
}

/// An `RG` override may carry arguments — the Makefile's `$(RG)` expansion
/// supported this before the scan moved into the script, and `check-ripgrep`
/// still validates only `$(firstword $(RG))`. The guard must therefore split
/// `RG` on whitespace and forward the extra arguments to ripgrep ahead of its
/// own, rather than treating the whole value as one executable name.
#[test]
fn preserves_arguments_supplied_through_rg() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let argv_log = dir.path().join("argv.txt");
    // A stub that records its argv, then reports "no matches" so the guard
    // takes its clean-scan path.
    let stub = write_stub(
        dir.path(),
        "rg-stub.sh",
        &format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\nexit 1\n",
            argv_log.display()
        ),
    );

    let output = run_guard(dir.path(), Some(&format!("{} --pcre2", stub.display())));

    assert_eq!(
        output.status.code(),
        Some(0),
        "an argument-bearing RG override must still run; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let argv: Vec<String> = std::fs::read_to_string(&argv_log)
        .expect("stub should have recorded its argv")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        argv.first().map(String::as_str),
        Some("--pcre2"),
        "the RG override's own arguments must be forwarded first, got: {argv:?}"
    );
    assert!(
        argv.contains(&"-U".to_owned()) && argv.contains(&"--glob".to_owned()),
        "the guard's own ripgrep arguments must follow, got: {argv:?}"
    );
    assert_eq!(
        argv.last().map(String::as_str),
        Some(dir.path().to_str().expect("temp dir path should be UTF-8")),
        "the scan directory must remain the final argument, got: {argv:?}"
    );
}
