//! Helpers that stage fixtures for, and run, the `check-static-regexes` guard.

use std::{io, process::Command};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

/// Adapt an ambient [`std::path::Path`] — as produced by [`TempDir::path`] —
/// into a UTF-8 path, failing loudly rather than lossily if it is not UTF-8.
pub fn utf8(path: &std::path::Path) -> io::Result<&Utf8Path> {
    Utf8Path::from_path(path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("non-UTF-8 temporary path: {}", path.display()),
        )
    })
}

/// Open a filesystem capability scoped to `dir`.
///
/// Every subsequent operation names a path relative to this handle, so it
/// cannot reach outside `dir`.
pub fn open_dir(dir: &Utf8Path) -> io::Result<Dir> {
    Dir::open_ambient_dir(dir, ambient_authority())
}

/// The crate root, used as the capability root for reading fixtures.
pub fn manifest_dir() -> Utf8PathBuf { Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// The guard script under test.
pub fn script_path() -> Utf8PathBuf { manifest_dir().join("scripts/check-static-regexes.sh") }

/// Read `label`'s fixture through a capability scoped to the crate root.
pub fn fixture(label: &str) -> io::Result<String> {
    let relative = format!("tests/data/static_regex/{label}.rs.txt");
    open_dir(&manifest_dir())?.read_to_string(&relative)
}

/// Materialize `label`'s fixture as a `.rs` file inside a fresh temp directory.
pub fn scan_dir_with(label: &str) -> io::Result<TempDir> {
    let dir = TempDir::new()?;
    open_dir(utf8(dir.path())?)?.write(format!("{label}.rs"), fixture(label)?)?;
    Ok(dir)
}

/// Run the guard against `scan_dir`, optionally overriding the `RG` ripgrep
/// command.
///
/// `rg` is the raw `RG` value, so it may carry arguments (for example
/// `rg --pcre2`); the guard splits it on whitespace. Passing `None` clears any
/// ambient `RG` so default-path runs exercise the guard's own `rg` default
/// deterministically. The guard runs with `scan_dir` as its working
/// directory, so a stub in it can be named `./<name>`, free of the
/// whitespace the guard splits `RG` on (see `script_stubs::stub_command`).
pub fn run_guard(scan_dir: &Utf8Path, rg: Option<&str>) -> io::Result<std::process::Output> {
    let mut cmd = Command::new(script_path());
    cmd.arg(scan_dir).current_dir(scan_dir);
    match rg {
        Some(rg) => cmd.env("RG", rg),
        None => cmd.env_remove("RG"),
    };
    cmd.output()
}
