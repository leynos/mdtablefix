//! Denying a read so the caller can prove the tool reports it.
//!
//! A separate module from `cli_config_fixture` because of how `#[path]` spends
//! its items: each integration-test file that declares the fixture compiles its
//! own copy, and `-D warnings` makes every item that copy's file never calls a
//! `dead_code` error. The fixture is shared by two binaries, but only the
//! failure binary denies a read, so a helper living beside the fixture would be
//! dead in the other one — and the suite would not compile. Keeping the
//! permission helpers here means only the file that uses them pays for them.
//!
//! The distinction the helpers turn on is *read* versus *write*: a
//! configuration file is opened, so denying it is a mode of `0o000` on the file
//! itself. The atomic-rewrite suite denies a different thing — a write to a file
//! that stays readable — with a read-only parent directory, and cannot be
//! reused for this.
//!
//! Every item here is safe: whether a denied file can still be opened is
//! observed by opening it, rather than inferred from the process's user ID.

use std::path::Path;

/// Whether the permission bits on `path` are ignored for this process.
///
/// Call this **after** [`deny_read`] and **before** [`allow_read`]: the probe is
/// the denial itself. A process that can open a `0o000` file cannot induce the
/// failure a permission-denied test asserts, because the read it expects to
/// fail succeeds. Each caller reports that as a skip, because an assertion that
/// is never reached is not evidence.
///
/// The probe is behavioural rather than a check for user ID 0, because root is
/// not the only process that bypasses permission bits: a container runner may
/// grant `CAP_DAC_OVERRIDE` or `CAP_DAC_READ_SEARCH` to an unprivileged user,
/// and an identity check would report "not root, the read will fail" for a read
/// that then succeeds, failing the test for a reason unrelated to the tool.
pub fn permissions_are_ignored(path: &Path) -> bool { std::fs::File::open(path).is_ok() }

/// Removes every permission bit from `path`, making it unreadable.
///
/// A configuration file is *read*, so the mode that denies it is `0o000` rather
/// than the read-only-directory mode the atomic-rewrite suite uses to deny a
/// *write* to a file that is still readable.
///
/// # Panics
///
/// Panics when the mode cannot be set, which is a broken fixture rather than a
/// failed expectation.
pub fn deny_read(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000))
        .expect("make the file unreadable");
}

/// Restores owner read and write, so the tree can be listed and removed.
///
/// # Panics
///
/// Panics when the mode cannot be set, which would leave the temporary
/// directory undeletable and is a fixture failure rather than a test result.
pub fn allow_read(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .expect("restore the file mode");
}
