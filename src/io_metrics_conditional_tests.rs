//! Metrics tests for conditional replacement outcomes.
//!
//! These cases live apart from the general metrics suite so both test modules
//! remain within the repository's file-size limit.

use super::*;

/// A conditional replacement that finds the target unchanged from the text it
/// was read as writes it and records `success`, like any other replacement.
#[test]
fn a_conditional_replacement_of_a_matching_target_is_a_success() {
    let dir = tempdir().expect("create temporary directory");
    let _file = fixture(&dir);
    let root = camino::Utf8Path::from_path(dir.path()).expect("the temporary directory is UTF-8");
    let capability = cap_std::fs_utf8::Dir::open_ambient_dir(root, cap_std::ambient_authority())
        .expect("open the directory capability");

    let (replaced, recorded) = recorded(|| {
        replace_file_if_unchanged(
            &capability,
            camino::Utf8Path::new("sample.md"),
            "|A|B|\n|1|2|",
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
#[test]
fn a_declined_replacement_after_the_target_moved_on_is_unchanged() {
    let dir = tempdir().expect("create temporary directory");
    let _file = fixture(&dir);
    let root = camino::Utf8Path::from_path(dir.path()).expect("the temporary directory is UTF-8");
    let capability = cap_std::fs_utf8::Dir::open_ambient_dir(root, cap_std::ambient_authority())
        .expect("open the directory capability");
    capability
        .write(camino::Utf8Path::new("sample.md"), "|X|Y|\n|3|4|")
        .expect("write the other writer's version");

    let (replaced, recorded) = recorded(|| {
        replace_file_if_unchanged(
            &capability,
            camino::Utf8Path::new("sample.md"),
            "|A|B|\n|1|2|",
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
