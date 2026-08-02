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
//!
//! Paths are [`camino`] UTF-8 types and every filesystem operation goes
//! through a [`cap_std::fs_utf8::Dir`] capability scoped to the directory it
//! touches, so reads and writes cannot stray outside the manifest or the
//! temporary directory they belong to. [`TempDir`] still provides the isolated
//! directories and [`Command`] still runs the guard; only the path and
//! filesystem layers change.

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
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

/// Adapt an ambient [`std::path::Path`] — as produced by [`TempDir::path`] —
/// into a UTF-8 path, failing loudly rather than lossily if it is not UTF-8.
fn utf8(path: &std::path::Path) -> &Utf8Path {
    Utf8Path::from_path(path).expect("temporary directory path should be UTF-8")
}

/// Open a filesystem capability scoped to `dir`.
///
/// Every subsequent operation names a path relative to this handle, so it
/// cannot reach outside `dir`.
fn open_dir(dir: &Utf8Path) -> Dir {
    Dir::open_ambient_dir(dir, ambient_authority())
        .unwrap_or_else(|e| panic!("failed to open directory {dir}: {e}"))
}

/// The crate root, used as the capability root for reading fixtures.
fn manifest_dir() -> Utf8PathBuf { Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// The guard script under test.
fn script_path() -> Utf8PathBuf { manifest_dir().join("scripts/check-static-regexes.sh") }

/// Read `label`'s fixture through a capability scoped to the crate root.
fn fixture(label: &str) -> String {
    let relative = format!("tests/data/static_regex/{label}.rs.txt");
    open_dir(&manifest_dir())
        .read_to_string(&relative)
        .unwrap_or_else(|e| panic!("failed to read fixture {relative}: {e}"))
}

/// Materialize `label`'s fixture as a `.rs` file inside a fresh temp directory.
fn scan_dir_with(label: &str) -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");
    open_dir(utf8(dir.path()))
        .write(format!("{label}.rs"), fixture(label))
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
fn run_guard(scan_dir: &Utf8Path, rg: Option<&str>) -> std::process::Output {
    let mut cmd = Command::new(script_path());
    cmd.arg(scan_dir);
    match rg {
        Some(rg) => cmd.env("RG", rg),
        None => cmd.env_remove("RG"),
    };
    cmd.output()
        .expect("failed to execute check-static-regexes.sh")
}

/// Write `script` to `<dir>/<name>`, mark it executable, and return its path.
///
/// Both operations go through a capability scoped to `dir`, so `name` is
/// resolved relative to that directory rather than against ambient authority.
fn write_stub(dir: &Utf8Path, name: &str, script: &str) -> Utf8PathBuf {
    let handle = open_dir(dir);
    handle.write(name, script).expect("failed to write stub");
    #[cfg(unix)]
    {
        use cap_std::fs::{Permissions, PermissionsExt};
        handle
            .set_permissions(name, Permissions::from_mode(0o755))
            .expect("failed to chmod stub");
    }
    dir.join(name)
}

#[rstest]
fn rejects_prohibited_lazy_wrapper_form(#[values(0, 1, 2, 3, 4, 5)] index: usize) {
    let label = PROHIBITED_FORMS[index];
    let dir = scan_dir_with(label);

    let output = run_guard(utf8(dir.path()), None);

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

    let output = run_guard(utf8(dir.path()), None);

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
    let scan_dir = utf8(dir.path());
    // A stub standing in for ripgrep that fails with a distinctive status.
    let stub = write_stub(scan_dir, "rg-stub.sh", "#!/bin/sh\nexit 3\n");

    let output = run_guard(scan_dir, Some(stub.as_str()));

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
///
/// The `single_quote` case pins the stub's own quoting: the log path is derived
/// from `$0` inside the script rather than interpolated into its source, so a
/// `TMPDIR` containing a single quote cannot break the recording. (The
/// directory name deliberately omits whitespace, since `RG` is split on it.)
#[rstest]
#[case::plain("scan")]
#[case::single_quote("scan's")]
fn preserves_arguments_supplied_through_rg(#[case] dir_name: &str) {
    const STUB_NAME: &str = "rg-stub.sh";

    let root = TempDir::new().expect("failed to create temp dir");
    let root_dir = utf8(root.path());
    open_dir(root_dir)
        .create_dir(dir_name)
        .expect("failed to create scan dir");
    let dir = root_dir.join(dir_name);

    // A stub that records its argv, then reports "no matches" so the guard
    // takes its clean-scan path. It writes beside itself via `"$0.argv"`, so no
    // path is interpolated into the script source.
    let stub = write_stub(
        &dir,
        STUB_NAME,
        "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > \"$0.argv\"\nexit 1\n",
    );

    let output = run_guard(&dir, Some(&format!("{stub} --pcre2")));

    assert_eq!(
        output.status.code(),
        Some(0),
        "an argument-bearing RG override must still run; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let argv: Vec<String> = open_dir(&dir)
        .read_to_string(format!("{STUB_NAME}.argv"))
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
        Some(dir.as_str()),
        "the scan directory must remain the final argument, got: {argv:?}"
    );
}

// ---------------------------------------------------------------------------
// Property-based coverage
//
// The fixtures above pin the specific spellings named in the issue. The guard's
// real contract, though, is a syntax-matching invariant over a whole family of
// declarations: either supported wrapper, any module qualification, any
// `\s*`-legal whitespace (including newlines, since the scan runs with `-U`),
// with or without `move`, and with or without a braced closure body. The
// properties below generate across that space and assert rejection, and
// generate across clearly-sanctioned forms and assert acceptance.
//
// Each case writes several generated declarations into one directory and runs
// the guard once, then asserts every generated file is named in the output.
// That keeps process spawns proportional to cases rather than declarations
// while still checking each declaration individually.
// ---------------------------------------------------------------------------

/// Whitespace runs for positions where the guard's pattern allows `\s*`.
fn optional_ws() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["", " ", "  ", "\t", "\n", "\n    "]).prop_map(str::to_owned)
}

/// Whitespace runs for positions requiring at least one space, such as the
/// gap after a `move` keyword.
fn required_ws() -> impl Strategy<Value = String> {
    prop::sample::select(vec![" ", "  ", "\t", "\n", "\n    "]).prop_map(str::to_owned)
}

/// A possibly-empty module qualification such as `once_cell::sync::`.
fn qualifier() -> impl Strategy<Value = String> {
    prop::collection::vec("[a-z][a-z0-9_]{0,6}", 0..3).prop_map(|segments| {
        segments.iter().fold(String::new(), |mut acc, segment| {
            acc.push_str(segment);
            acc.push_str("::");
            acc
        })
    })
}

prop_compose! {
    /// A static declaration that wraps `Regex::new` in a supported lazy
    /// wrapper, and so must always be rejected.
    fn prohibited_declaration()(
        name in "[A-Z][A-Z0-9_]{0,7}",
        wrapper in prop::sample::select(vec!["LazyLock", "Lazy"]),
        wrapper_qualifier in qualifier(),
        regex_qualifier in qualifier(),
        pattern in "[a-z0-9]{1,8}",
        after_eq in optional_ws(),
        after_new in optional_ws(),
        after_paren in optional_ws(),
        after_closure in optional_ws(),
        move_gap in prop::option::of(required_ws()),
        braced in any::<bool>(),
    ) -> String {
        let move_kw = move_gap.map_or_else(String::new, |gap| format!("move{gap}"));
        let (open, close) = if braced { ("{ ", " }") } else { ("", "") };
        format!(
            "static {name}: {wrapper}<Regex> ={after_eq}{wrapper_qualifier}{wrapper}::new\
             {after_new}({after_paren}{move_kw}||{after_closure}{open}\
             {regex_qualifier}Regex::new(\"{pattern}\").unwrap(){close});\n"
        )
    }
}

prop_compose! {
    /// A declaration the guard must leave alone: the sanctioned macro, a
    /// non-static binding, a supported wrapper around a non-regex value, an
    /// unsupported wrapper, or a function reference rather than a closure.
    fn clean_declaration()(
        name in "[A-Z][A-Z0-9_]{0,7}",
        pattern in "[a-z0-9]{1,8}",
        form in 0_usize..5,
    ) -> String {
        match form {
            0 => format!("static {name}: LazyLock<Regex> = lazy_regex!(\"{pattern}\");\n"),
            1 => format!("fn build_{name}() {{ let re = Regex::new(\"{pattern}\").unwrap(); }}\n"),
            2 => format!("static {name}: LazyLock<String> = LazyLock::new(|| String::new());\n"),
            3 => format!("static {name}: OnceLock<Regex> = OnceLock::new();\n"),
            _ => format!("static {name}: LazyLock<Regex> = LazyLock::new(make_{name});\n"),
        }
    }
}

/// Write each declaration to its own `.rs` file and scan the directory once.
fn scan_generated(declarations: &[String]) -> (TempDir, std::process::Output) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let handle = open_dir(utf8(dir.path()));
    for (index, declaration) in declarations.iter().enumerate() {
        handle
            .write(format!("case{index}.rs"), declaration)
            .expect("failed to write generated source");
    }
    let output = run_guard(utf8(dir.path()), None);
    (dir, output)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Every declaration in the supported wrapper family is rejected, whatever
    /// its qualification, whitespace, `move` keyword, or closure body shape.
    #[test]
    fn rejects_generated_prohibited_declarations(
        declarations in prop::collection::vec(prohibited_declaration(), 1..5),
    ) {
        let (_dir, output) = scan_generated(&declarations);

        prop_assert_eq!(
            output.status.code(),
            Some(1),
            "generated declarations should be rejected: {:?}",
            declarations
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        prop_assert!(stdout.contains(PROHIBITED_DIAGNOSTIC));
        // Assert per declaration, so one matching file cannot mask the rest.
        for (index, declaration) in declarations.iter().enumerate() {
            prop_assert!(
                stdout.contains(&format!("case{index}.rs")),
                "declaration {} went undetected: {:?}\nguard output: {stdout}",
                index,
                declaration
            );
        }
    }

    /// Sanctioned and unrelated declarations never trip the guard.
    #[test]
    fn accepts_generated_clean_declarations(
        declarations in prop::collection::vec(clean_declaration(), 1..5),
    ) {
        let (_dir, output) = scan_generated(&declarations);

        prop_assert_eq!(
            output.status.code(),
            Some(0),
            "clean declarations should pass: {:?}\nguard output: {}",
            declarations,
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

// Bounded model checking is unsuitable here because the guard is a ripgrep
// pattern over unbounded Rust source text rather than a bounded state machine;
// property testing exercises that input domain directly.
