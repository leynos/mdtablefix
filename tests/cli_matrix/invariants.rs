//! Independent output invariants for CLI matrix snapshots.

use std::fs;

use super::{LogicalCase, TransformFlag, fixture_path};

const ELLIPSIS_UTF8: &[u8] = b"\xE2\x80\xA6";

/// Asserts output properties that prove enabled transforms changed matching input.
pub(crate) fn assert_transform_invariants(logical: &LogicalCase, stdout: &[u8]) {
    let fixture = fs::read_to_string(fixture_path(logical.fixture)).expect("read matrix fixture");
    let output = String::from_utf8_lossy(stdout);

    if logical.flags.contains(&TransformFlag::Ellipsis) && fixture.contains("...") {
        // Validates `--ellipsis`: textual dots in the fixture must become a Unicode ellipsis.
        assert!(
            stdout
                .windows(ELLIPSIS_UTF8.len())
                .any(|window| window == ELLIPSIS_UTF8),
            "{} should contain a UTF-8 Unicode ellipsis",
            logical.id,
        );
    }
    if logical.flags.contains(&TransformFlag::Fences) && fixture_has_fence_candidate(&fixture) {
        // Validates `--fences`: eligible fence input must be normalised to backtick fences.
        assert!(
            output.contains("```"),
            "{} should contain a backtick fenced code block delimiter",
            logical.id,
        );
    }
    if logical.flags.contains(&TransformFlag::Renumber) {
        for marker in unordered_fixture_markers(&fixture) {
            // Validates `--renumber`: out-of-sequence fixture markers must not survive.
            assert!(
                !output.lines().any(|line| line.starts_with(&marker)),
                "{} should not retain unordered list marker {marker:?}",
                logical.id,
            );
        }
    }
}

fn fixture_has_fence_candidate(fixture: &str) -> bool {
    fixture.lines().any(|line| {
        line.trim_start().starts_with("```")
            || line.trim_start().starts_with("~~~")
            || line.starts_with("    ")
    })
}

fn unordered_fixture_markers(fixture: &str) -> Vec<String> {
    let mut markers = Vec::new();
    for (expected, line) in (1..).zip(fixture.lines().filter_map(ordered_marker)) {
        if line != expected {
            markers.push(format!("{line}. "));
        }
    }
    markers
}

fn ordered_marker(line: &str) -> Option<usize> {
    let (digits, rest) = line.split_once('.')?;
    rest.starts_with(' ')
        .then_some(digits)
        .filter(|digits| !digits.is_empty())?
        .parse()
        .ok()
}
