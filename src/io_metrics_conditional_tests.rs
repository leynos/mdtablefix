//! Metrics tests for conditional replacement outcomes.
//!
//! These cases live apart from the general metrics suite so both test modules
//! remain within the repository's file-size limit.

use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::*;

/// The text a conditional replacement in these cases was read as.
const ORIGINAL: &str = "|A|B|\n|1|2|";

/// The target both cases drive, reached only through a capability.
///
/// The temporary directory, the capability opened on it, and the fixture written
/// through that capability are one thing, made once: a case that rebuilt them
/// would be saying something about how a test reaches a directory rather than
/// about what the replacement does with one. Nothing here destructures the
/// value, so the guard that owns the directory outlives every read and write.
struct ConditionalTarget {
    /// Keeps the directory alive for as long as the capability is in use.
    _temporary: TempDir,
    /// The capability the replacement is handed, and the fixture written through.
    directory: Dir,
}

/// Creates the target `sample.md`, through the capability that replaces it.
#[test_macros::allow_fixture_expansion_lints]
#[fixture]
fn conditional_target() -> ConditionalTarget {
    let temporary = tempdir().expect("create temporary directory");
    let root =
        camino::Utf8Path::from_path(temporary.path()).expect("the temporary directory is UTF-8");
    let directory =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");
    directory
        .write(camino::Utf8Path::new("sample.md"), ORIGINAL)
        .expect("write the fixture through the capability");

    ConditionalTarget {
        _temporary: temporary,
        directory,
    }
}

/// A conditional replacement that finds the target unchanged from the text it
/// was read as writes it and records `success`, like any other replacement.
#[rstest]
fn a_conditional_replacement_of_a_matching_target_is_a_success(
    conditional_target: ConditionalTarget,
) {
    let capability = &conditional_target.directory;

    let (replaced, recorded) = recorded(|| {
        replace_file_if_unchanged(
            capability,
            camino::Utf8Path::new("sample.md"),
            ORIGINAL,
            "| A | B |\n| 1 | 2 |\n",
        )
    });

    replaced.expect("a matching target is replaced");
    assert_eq!(
        capability
            .read_to_string(camino::Utf8Path::new("sample.md"))
            .expect("read the target"),
        "| A | B |\n| 1 | 2 |\n"
    );
    assert_labels_are_bounded(&recorded);
    assert_eq!(
        outcome_count(&recorded, "success"),
        1,
        "a conditional replacement that wrote is a success: {recorded:?}"
    );
}

/// The other half of the case above: a target that moved on before the swap
/// records `unchanged` rather than `success` or `failure`.
#[rstest]
fn a_declined_replacement_after_the_target_moved_on_is_unchanged(
    conditional_target: ConditionalTarget,
) {
    let capability = &conditional_target.directory;
    capability
        .write(camino::Utf8Path::new("sample.md"), "|X|Y|\n|3|4|")
        .expect("write the other writer's version");

    let (replaced, recorded) = recorded(|| {
        replace_file_if_unchanged(
            capability,
            camino::Utf8Path::new("sample.md"),
            ORIGINAL,
            "| A | B |\n| 1 | 2 |\n",
        )
    });

    assert!(
        !replaced.expect("a declined replacement is not an error"),
        "the target no longer holds the text it was read as"
    );
    assert_labels_are_bounded(&recorded);
    assert_eq!(
        outcome_count(&recorded, "unchanged"),
        1,
        "a declined replacement is counted under its own outcome: {recorded:?}"
    );
    assert_eq!(
        outcome_samples(&recorded, "unchanged").len(),
        1,
        concat!(
            "a declined replacement is timed too, so stalls before the comparison are visible: ",
            "{recorded:?}"
        ),
        recorded = recorded
    );
    assert_eq!(
        outcome_count(&recorded, "success") + outcome_count(&recorded, "failure"),
        0,
        "a decline is neither a success nor a failure: {recorded:?}"
    );
}
