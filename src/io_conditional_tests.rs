//! Tests for conditional file replacement.
//!
//! Conditional outcomes stay in their own module so the general replacement
//! tests remain within the repository's file-size limit.

use super::*;

/// A conditional replacement writes when the target still holds what was read.
///
/// The positive control for the decline test below, and for `rewrite`'s use of
/// the same entry point: without it, an implementation that never replaced
/// anything would satisfy both.
#[test]
fn conditional_replacement_replaces_a_matching_target() {
    let dir = tempdir().expect("create temporary directory");
    let file = dir.path().join("sample.md");
    let original = "|A|B|\n|1|2|";
    let formatted = "| A | B |\n| 1 | 2 |\n";
    fs::write(&file, original).expect("write fixture");
    let root = Utf8Path::from_path(dir.path()).expect("the temporary directory is UTF-8");
    let capability =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");

    let replaced =
        replace_file_if_unchanged(&capability, Utf8Path::new("sample.md"), original, formatted)
            .expect("replace the target that still matches");

    assert!(replaced, "a target holding what was read is replaced");
    assert_eq!(fs::read_to_string(&file).expect("read target"), formatted);
    assert_eq!(entry_names(dir.path()), vec!["sample.md"]);
}

/// A target another writer reached first is left exactly as that writer left
/// it, and the caller is told so rather than given an error.
#[test]
fn conditional_replacement_declines_a_target_that_moved_on() {
    let dir = tempdir().expect("create temporary directory");
    let file = dir.path().join("sample.md");
    let read = "|A|B|\n|1|2|";
    let moved_on = "|X|Y|\n|3|4|";
    fs::write(&file, moved_on).expect("write the other writer's version");
    let root = Utf8Path::from_path(dir.path()).expect("the temporary directory is UTF-8");
    let capability =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");

    let replaced = replace_file_if_unchanged(
        &capability,
        Utf8Path::new("sample.md"),
        read,
        "| A | B |\n| 1 | 2 |\n",
    )
    .expect("a declined replacement is not an error");

    assert!(!replaced, "the target no longer holds the text read");
    assert_eq!(
        fs::read_to_string(&file).expect("read target"),
        moved_on,
        "the other writer's text must survive untouched"
    );
    assert_eq!(
        entry_names(dir.path()),
        vec!["sample.md"],
        "a declined replacement must remove the temporary file it wrote"
    );
}

/// A failed cleanup after a declined replacement is reported to the caller.
#[test]
fn conditional_replacement_reports_a_failed_cleanup() {
    let dir = tempdir().expect("create temporary directory");
    let file = dir.path().join("sample.md");
    let expected = "|A|B|\n|1|2|";
    let moved_on = "|X|Y|\n|3|4|";
    fs::write(&file, moved_on).expect("write the other writer's version");
    let root = Utf8Path::from_path(dir.path()).expect("the temporary directory is UTF-8");
    let capability =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");
    let _cleanup = cleanup_failure_seam::arm();

    let error = replace_file_if_unchanged(
        &capability,
        Utf8Path::new("sample.md"),
        expected,
        "| A | B |\n| 1 | 2 |\n",
    )
    .expect_err("a failed cleanup must be reported");

    assert!(error.to_string().contains("cleanup failure seam"));
    assert_eq!(fs::read_to_string(&file).expect("read target"), moved_on);
}

/// A target that cannot be read back at all is an error rather than a decline.
///
/// A caller that asked for a conditional replacement must not be told it
/// succeeded, or that the condition failed, when the question could not be put.
#[test]
fn conditional_replacement_reports_a_target_it_cannot_read_back() {
    let dir = tempdir().expect("create temporary directory");
    let file = dir.path().join("sample.md");
    fs::write(&file, "|A|B|\n|1|2|").expect("write fixture");
    let root = Utf8Path::from_path(dir.path()).expect("the temporary directory is UTF-8");
    let capability =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");
    fs::remove_file(&file).expect("remove the target before the comparison");

    let error = replace_file_if_unchanged(
        &capability,
        Utf8Path::new("sample.md"),
        "|A|B|\n|1|2|",
        "| A | B |\n| 1 | 2 |\n",
    )
    .expect_err("a target that cannot be read back cannot be replaced");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        entry_names(dir.path()),
        Vec::<String>::new(),
        "the temporary file must not survive the error"
    );
}
