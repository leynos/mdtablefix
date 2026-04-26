//! Independent output invariants for CLI matrix snapshots.

use std::{collections::HashMap, fs};

use anyhow::{Context as _, Result};

use super::{LogicalCase, TransformFlag, fixture_path};

const ELLIPSIS_UTF8: &[u8] = b"\xE2\x80\xA6";

struct OrderedMarker {
    indent: String,
    number: usize,
}

/// Asserts output properties that prove enabled transforms changed matching input.
pub(crate) fn assert_transform_invariants(logical: &LogicalCase, stdout: &[u8]) -> Result<()> {
    let fixture_path = fixture_path(logical.fixture);
    let fixture = fs::read_to_string(&fixture_path)
        .with_context(|| format!("read matrix fixture '{}'", fixture_path.display()))?;
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
    Ok(())
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
    let mut expected_by_indent = HashMap::new();

    for marker in fixture.lines().filter_map(ordered_marker) {
        let expected = expected_by_indent.entry(marker.indent.clone()).or_insert(1);
        if marker.number != *expected {
            markers.push(format!("{}{}. ", marker.indent, marker.number));
        }
        *expected += 1;
    }
    markers
}

fn ordered_marker(line: &str) -> Option<OrderedMarker> {
    let indent_length = line.len() - line.trim_start().len();
    let (digits, rest) = line[indent_length..].split_once('.')?;
    rest.starts_with(' ')
        .then_some(digits)
        .filter(|digits| !digits.is_empty())?
        .parse()
        .ok()
        .map(|number| OrderedMarker {
            indent: line[..indent_length].to_string(),
            number,
        })
}
