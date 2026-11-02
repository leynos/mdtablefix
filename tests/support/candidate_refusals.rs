//! The two candidates the loader refuses before it reads a byte.
//!
//! The `extends` pre-scan in `src/config_extends.rs` exists to read the files
//! the loader is about to read, so that no parent named by `extends` is ever
//! opened. "The files the loader is about to read" is therefore a contract
//! between two pieces of code, and these are its process-level cases: the ones
//! that need a working directory, or a process that must terminate, to be
//! visible at all.
//!
//! Both were once wrong in the same direction. The pre-scan canonicalized
//! before asking whether the loader would read the candidate at all, so it
//! followed a link out of the checkout and opened an entry the loader rejects
//! unread. A repository can commit either shape, which is why they are pinned
//! here rather than left to inspection.
//!
//! A module of its own for the same reason as `permission_denial`: each
//! integration-test binary compiles its own copy of a `#[path]` module, and
//! `-D warnings` makes every item this file never calls a `dead_code` error, so
//! a helper only this file needs must live in a file only this file declares.

use std::time::Duration;

use super::fixture::Fixture;

/// How long a run may take before the test calls it a hang.
///
/// The FIFO case waits forever if the guard is removed, and a test that hangs
/// is worse than one that fails: it stalls the suite instead of naming the
/// defect. This is a generous ceiling for a binary that formats one small
/// file, and it is only reached on the regression it exists to catch.
const HANG_CEILING: Duration = Duration::from_secs(20);

/// A project candidate the loader refuses is reported by the loader, exit 2.
///
/// A `.mdtablefix.toml` symlink climbing out of the checkout is the shape a
/// repository could commit. The target is valid TOML declaring `extends`, so
/// the failure is discriminating: a pre-scan that canonicalized before asking
/// the capability whether the candidate was readable would have reported this
/// as its own `extends` refusal, having read a file outside the tree. The
/// loader refuses it path-only, and that is the diagnostic that must appear.
#[test]
fn an_escaping_project_symlink_is_refused_by_the_loader() {
    let fixture = Fixture::new();
    let outside = fixture.write("outside/elsewhere.toml", "extends = \"p.toml\"\n");
    let project = fixture.root().join(".mdtablefix.toml");
    std::os::unix::fs::symlink(&outside, &project).expect("create the symlink");
    assert!(
        outside.is_file(),
        "the target must exist, or the refusal proves nothing"
    );

    let run = fixture.run(&["--check", "drift.md"], "");

    assert_eq!(
        run.status, 2,
        "stdout: {} stderr: {}",
        run.stdout, run.stderr
    );
    assert!(
        !run.stderr.contains("extends"),
        "the pre-scan must not have read the target: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains("path led outside"),
        "the loader's own refusal must be the one reported: {}",
        run.stderr
    );
}

/// A candidate that is not a regular file is refused, and does not block.
///
/// A FIFO is the case that hangs rather than fails when the candidate is
/// opened instead of statted: opening one for reading waits for a writer that
/// never comes. The loader stats first and reports the entry, so the run must
/// terminate with the ordinary file diagnostic.
#[test]
fn a_project_fifo_is_refused_without_blocking() {
    let fixture = Fixture::new();
    let project = fixture.root().join(".mdtablefix.toml");
    let status = std::process::Command::new("mkfifo")
        .arg(&project)
        .status()
        .expect("run mkfifo");
    assert!(status.success(), "mkfifo failed: {status}");

    // Bounded, because a run that blocks never returns to be asserted on.
    // `assert_cmd` kills the child at the deadline, so the status becomes the
    // signal's rather than a passing code.
    let run = Fixture::finish(
        fixture
            .command()
            .args(["--check", "drift.md"])
            .timeout(HANG_CEILING),
    );

    assert_eq!(
        run.status, 2,
        "the run must fail, not succeed or hang: stdout: {} stderr: {}",
        run.stdout, run.stderr
    );
    assert!(
        run.stderr.contains("not a regular file"),
        "the loader names the reason: {}",
        run.stderr
    );
}
