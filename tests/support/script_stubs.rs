//! Stub scripts for subprocess tests: written through a directory capability
//! and run through `sh`, never executed directly (#586).

use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Returns an `RG` value that runs `stub` through `sh` rather than executing it.
///
/// The stub is written by this process; a test thread that forks while it is
/// open for writing leaves a child holding a write descriptor until it
/// execs, and executing the file in that window fails with `ETXTBSY` (#586).
/// The stub is named relative to the guard's working directory, the scan
/// directory it lives in, because the guard splits `RG` on whitespace and an
/// absolute temporary path may contain some.
pub fn stub_command(stub: &Utf8Path) -> String {
    format!("sh ./{}", stub.file_name().unwrap_or(stub.as_str()))
}

/// Write `script` to `<dir>/<name>`, mark it executable, and return its path.
///
/// Both operations go through a capability scoped to `dir`, so `name` is
/// resolved relative to that directory rather than against ambient authority.
pub fn write_stub(dir: &Utf8Path, name: &str, script: &str) -> io::Result<Utf8PathBuf> {
    let handle = Dir::open_ambient_dir(dir, ambient_authority())?;
    handle.write(name, script)?;
    #[cfg(unix)]
    {
        use cap_std::fs::{Permissions, PermissionsExt};
        handle.set_permissions(name, Permissions::from_mode(0o755))?;
    }
    Ok(dir.join(name))
}
