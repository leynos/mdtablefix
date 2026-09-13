//! Tests for conditional file replacement.
//!
//! Conditional outcomes stay in their own module so the general replacement
//! tests remain within the repository's file-size limit.

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::fixture;
use tempfile::{TempDir, tempdir};

use super::*;

/// The capability-scoped target each conditional replacement test drives.
struct ConditionalFixture {
    /// Keeps the directory alive while its capability is in use.
    _temporary: TempDir,
    /// The capability through which the test creates and reads its target.
    directory: Dir,
    /// The target relative to `directory`.
    target: Utf8PathBuf,
}

/// Creates a fresh target location without making an ambient filesystem path.
#[test_macros::allow_fixture_expansion_lints]
#[fixture]
fn conditional_fixture() -> ConditionalFixture {
    let temporary = tempdir().expect("create temporary directory");
    let root =
        camino::Utf8Path::from_path(temporary.path()).expect("the temporary directory is UTF-8");
    let directory =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");

    ConditionalFixture {
        _temporary: temporary,
        directory,
        target: Utf8PathBuf::from("sample.md"),
    }
}

/// Lists the fixture directory through its capability.
fn entry_names(directory: &Dir) -> Vec<String> {
    let mut names: Vec<String> = directory
        .read_dir(".")
        .expect("read the fixture directory")
        .map(|entry| {
            entry
                .expect("read fixture entry")
                .file_name()
                .expect("fixture entry has a UTF-8 name")
                .clone()
        })
        .collect();
    names.sort();
    names
}

/// A conditional replacement writes when the target still holds what was read.
///
/// The positive control for the decline test below, and for `rewrite`'s use of
/// the same entry point: without it, an implementation that never replaced
/// anything would satisfy both.
#[rstest::rstest]
fn conditional_replacement_replaces_a_matching_target(conditional_fixture: ConditionalFixture) {
    let original = "|A|B|\n|1|2|";
    let formatted = "| A | B |\n| 1 | 2 |\n";
    conditional_fixture
        .directory
        .write(&conditional_fixture.target, original)
        .expect("write fixture");

    let replaced = replace_file_if_unchanged(
        &conditional_fixture.directory,
        &conditional_fixture.target,
        original,
        formatted,
    )
    .expect("replace the target that still matches");

    assert!(replaced, "a target holding what was read is replaced");
    assert_eq!(
        conditional_fixture
            .directory
            .read_to_string(&conditional_fixture.target)
            .expect("read target"),
        formatted
    );
    assert_eq!(entry_names(&conditional_fixture.directory), ["sample.md"]);
}

/// A target another writer reached first is left exactly as that writer left
/// it, and the caller is told so rather than given an error.
#[rstest::rstest]
fn conditional_replacement_declines_a_target_that_moved_on(
    conditional_fixture: ConditionalFixture,
) {
    let read = "|A|B|\n|1|2|";
    let moved_on = "|X|Y|\n|3|4|";
    conditional_fixture
        .directory
        .write(&conditional_fixture.target, moved_on)
        .expect("write the other writer's version");

    let replaced = replace_file_if_unchanged(
        &conditional_fixture.directory,
        &conditional_fixture.target,
        read,
        "| A | B |\n| 1 | 2 |\n",
    )
    .expect("a declined replacement is not an error");

    assert!(!replaced, "the target no longer holds the text read");
    assert_eq!(
        conditional_fixture
            .directory
            .read_to_string(&conditional_fixture.target)
            .expect("read target"),
        moved_on,
        "the other writer's text must survive untouched"
    );
    assert_eq!(
        entry_names(&conditional_fixture.directory),
        ["sample.md"],
        "a declined replacement must remove the temporary file it wrote"
    );
}

/// A writer that lands between the swap's last comparison and its rename is
/// caught by that comparison rather than overwritten.
///
/// The window is the one no supported platform lets the swap close: no rename
/// compares contents, so without the comparison that follows the seam the
/// arriving writer's text would be renamed away and its work discarded. The
/// seam is what makes such a landing deterministic rather than a race, and the
/// case asserts the three things a decline promises: the replacement reports
/// that it wrote nothing, the arriving writer's text is what the target holds,
/// and no temporary file is left beside it.
#[rstest::rstest]
fn conditional_replacement_declines_a_writer_that_lands_inside_the_swap(
    conditional_fixture: ConditionalFixture,
) {
    let read = "|A|B|\n|1|2|";
    let intruder = "|X|Y|\n|3|4|";
    conditional_fixture
        .directory
        .write(&conditional_fixture.target, read)
        .expect("write fixture");
    let _armed = competing_writer_seam::arm(move |directory, path| {
        directory
            .write(path, intruder)
            .expect("the arriving write lands in the swap's window");
    });

    let replaced = replace_file_if_unchanged(
        &conditional_fixture.directory,
        &conditional_fixture.target,
        read,
        "| A | B |\n| 1 | 2 |\n",
    )
    .expect("a declined replacement is not an error");

    assert!(
        !replaced,
        "a swap another writer reached first replaces nothing"
    );
    assert_eq!(
        conditional_fixture
            .directory
            .read_to_string(&conditional_fixture.target)
            .expect("read target"),
        intruder,
        "the other writer's text must survive the declined swap"
    );
    assert_eq!(
        entry_names(&conditional_fixture.directory),
        ["sample.md"],
        "a declined replacement must remove the temporary file it wrote"
    );
}

/// A failed cleanup after a declined replacement is reported to the caller.
#[rstest::rstest]
fn conditional_replacement_reports_a_failed_cleanup(conditional_fixture: ConditionalFixture) {
    let expected = "|A|B|\n|1|2|";
    let moved_on = "|X|Y|\n|3|4|";
    conditional_fixture
        .directory
        .write(&conditional_fixture.target, moved_on)
        .expect("write the other writer's version");
    let _cleanup = cleanup_failure_seam::arm();

    let error = replace_file_if_unchanged(
        &conditional_fixture.directory,
        &conditional_fixture.target,
        expected,
        "| A | B |\n| 1 | 2 |\n",
    )
    .expect_err("a failed cleanup must be reported");

    assert!(error.to_string().contains("cleanup failure seam"));
    assert_eq!(
        conditional_fixture
            .directory
            .read_to_string(&conditional_fixture.target)
            .expect("read target"),
        moved_on
    );
}

/// A target that cannot be read back at all is an error rather than a decline.
///
/// A caller that asked for a conditional replacement must not be told it
/// succeeded, or that the condition failed, when the question could not be put.
#[rstest::rstest]
fn conditional_replacement_reports_a_target_it_cannot_read_back(
    conditional_fixture: ConditionalFixture,
) {
    conditional_fixture
        .directory
        .write(&conditional_fixture.target, "|A|B|\n|1|2|")
        .expect("write fixture");
    conditional_fixture
        .directory
        .remove_file(&conditional_fixture.target)
        .expect("remove the target before the comparison");

    let error = replace_file_if_unchanged(
        &conditional_fixture.directory,
        &conditional_fixture.target,
        "|A|B|\n|1|2|",
        "| A | B |\n| 1 | 2 |\n",
    )
    .expect_err("a target that cannot be read back cannot be replaced");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        entry_names(&conditional_fixture.directory),
        Vec::<String>::new(),
        "the temporary file must not survive the error"
    );
}
