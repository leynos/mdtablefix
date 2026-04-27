//! Independent output invariants for CLI matrix snapshots.

use std::fs;

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
    let mut expected_by_indent: std::collections::BTreeMap<usize, usize> =
        std::collections::BTreeMap::new();

    for line in fixture.lines() {
        if line.trim().is_empty() {
            expected_by_indent.clear();
            continue;
        }
        let Some(marker) = ordered_marker(line) else {
            continue;
        };
        let indent_len = marker.indent.len();
        // Remove deeper-nested counters when we return to a shallower level.
        expected_by_indent.retain(|&k, _| k <= indent_len);
        let expected = *expected_by_indent.get(&indent_len).unwrap_or(&1);
        if marker.number != expected {
            markers.push(format!("{}{}. ", marker.indent, marker.number));
        }
        expected_by_indent.insert(indent_len, expected + 1);
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

#[cfg(test)]
mod tests {
    use super::{fixture_has_fence_candidate, ordered_marker, unordered_fixture_markers};

    #[test]
    fn ordered_marker_returns_none_for_plain_prose() {
        assert!(ordered_marker("plain prose").is_none());
    }

    #[test]
    fn ordered_marker_returns_none_without_dot_space_suffix() {
        assert!(ordered_marker("123 item").is_none());
    }

    #[test]
    fn ordered_marker_returns_indent_and_number() {
        let marker = ordered_marker("  3. item").expect("parse ordered marker");

        assert_eq!(marker.indent, "  ");
        assert_eq!(marker.number, 3);
    }

    #[test]
    fn ordered_marker_returns_none_for_empty_string() {
        assert!(ordered_marker("").is_none());
    }

    #[test]
    fn fixture_has_fence_candidate_detects_backtick_fence() {
        assert!(fixture_has_fence_candidate("```rust\ncode\n```"));
    }

    #[test]
    fn fixture_has_fence_candidate_detects_tilde_fence() {
        assert!(fixture_has_fence_candidate("~~~rust\ncode\n~~~"));
    }

    #[test]
    fn fixture_has_fence_candidate_detects_indented_code() {
        assert!(fixture_has_fence_candidate("    code"));
    }

    #[test]
    fn fixture_has_fence_candidate_rejects_plain_text() {
        assert!(!fixture_has_fence_candidate("plain prose\nmore prose"));
    }

    #[test]
    fn unordered_fixture_markers_accepts_numbered_flat_list() {
        assert!(unordered_fixture_markers("1. a\n2. b\n3. c").is_empty());
    }

    #[test]
    fn unordered_fixture_markers_flags_flat_out_of_order_marker() {
        assert_eq!(unordered_fixture_markers("1. a\n3. b"), ["3. "]);
    }

    #[test]
    fn unordered_fixture_markers_resets_counter_after_blank_line() {
        assert_eq!(unordered_fixture_markers("1. a\n\n3. b"), ["3. "]);
    }

    #[test]
    fn unordered_fixture_markers_tracks_nested_counters_independently() {
        let fixture = "1. outer\n   1. inner\n   2. inner2\n2. outer2";

        assert!(unordered_fixture_markers(fixture).is_empty());
    }

    #[test]
    fn unordered_fixture_markers_flags_nested_out_of_order_marker() {
        let fixture = "1. outer\n   1. inner\n   3. inner2\n2. outer2";

        assert_eq!(unordered_fixture_markers(fixture), ["   3. "]);
    }
}
