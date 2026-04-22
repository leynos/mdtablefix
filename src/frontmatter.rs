//! YAML frontmatter detection and preservation.
//!
//! This module provides a helper to detect and split a leading YAML frontmatter
//! block from a Markdown document. The frontmatter block is defined as starting
//! with a line containing exactly `---` (the YAML opener) and ending with a line
//! containing `---` or `...` with optional trailing whitespace (the YAML closer).
//! Only a block at the very beginning of the document counts as frontmatter.

/// Splits the input into a leading YAML frontmatter prefix and the remaining body.
///
/// A valid frontmatter block must:
/// - Start with the first line being exactly `---`
/// - End with a line that is `---` or `...` with optional trailing whitespace before any body
///   content (matching is done after `trim_end()`)
///
/// If no valid closer is found, the entire input is returned as the body with an
/// empty prefix. This preserves existing behaviour for malformed or non-frontmatter
/// documents.
///
/// # Examples
///
/// ```ignore
/// use crate::frontmatter::split_leading_yaml_frontmatter;
///
/// let lines = vec![
///     "---".to_string(),
///     "title: Example".to_string(),
///     "---".to_string(),
///     "# Heading".to_string(),
/// ];
/// let (prefix, body) = split_leading_yaml_frontmatter(&lines);
/// assert_eq!(prefix.len(), 3);
/// assert_eq!(body.len(), 1);
/// assert_eq!(body[0], "# Heading");
/// ```
#[must_use]
pub(crate) fn split_leading_yaml_frontmatter(lines: &[String]) -> (&[String], &[String]) {
    if lines.is_empty() {
        return (&[], &[]);
    }

    // First line must be exactly the YAML opener (no leading/trailing whitespace)
    if lines[0] != "---" {
        return (&[], lines);
    }

    // Look for a closing delimiter after the opener
    // Only trim trailing whitespace to preserve leading whitespace
    // (indented lines inside YAML block scalars should not be treated as closers)
    for (idx, line) in lines.iter().enumerate().skip(1) {
        let trimmed_end = line.trim_end();
        if trimmed_end == "---" || trimmed_end == "..." {
            // Found valid closer - split after this line
            let split_at = idx + 1;
            return (&lines[..split_at], &lines[split_at..]);
        }
    }

    // No valid closer found - treat as ordinary Markdown
    (&[], lines)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    /// Helper to convert `&[&str]` → `Vec<String>`.
    fn s(v: &[&str]) -> Vec<String> { v.iter().copied().map(str::to_string).collect() }

    struct PrefixEmptyCase {
        lines: Vec<String>,
        body_is_empty: bool,
        check_body_equality: bool,
    }

    struct FrontmatterSplitCase {
        lines: Vec<String>,
        prefix_len: usize,
        body_len: usize,
        prefix_spot_checks: Vec<(usize, &'static str)>,
        body_spot_check: Option<&'static str>,
    }

    /// Cases where `prefix` is empty (no frontmatter detected).
    #[rstest]
    #[case::empty_input_returns_empty_slices(PrefixEmptyCase { lines: s(&[]), body_is_empty: true, check_body_equality: false })]
    #[case::no_frontmatter_returns_empty_prefix(PrefixEmptyCase { lines: s(&["# Heading", "Some text"]), body_is_empty: false, check_body_equality: true })]
    #[case::unmatched_opener_treated_as_body(PrefixEmptyCase { lines: s(&["---", "Some text", "More text"]), body_is_empty: false, check_body_equality: false })]
    #[case::indented_opener_not_recognized(PrefixEmptyCase { lines: s(&["  ---", "title: Example", "  ---"]), body_is_empty: false, check_body_equality: false })]
    #[case::later_dash_block_not_frontmatter(PrefixEmptyCase { lines: s(&["# Heading", "", "---", "Not frontmatter", "---"]), body_is_empty: false, check_body_equality: false })]
    #[case::indented_closer_not_recognized(PrefixEmptyCase { lines: s(&["---", "title: Example", "  ---  ", "# Heading"]), body_is_empty: false, check_body_equality: false })]
    fn prefix_empty_cases(#[case] case: PrefixEmptyCase) {
        let (prefix, body) = split_leading_yaml_frontmatter(&case.lines);
        assert!(prefix.is_empty());
        if case.body_is_empty {
            assert!(body.is_empty());
        } else if case.check_body_equality {
            assert_eq!(body, &case.lines);
        } else {
            assert!(!body.is_empty());
        }
    }

    /// Cases where frontmatter is detected (non-empty `prefix`).
    #[rstest]
    #[case::detects_frontmatter_with_triple_dash_closer(FrontmatterSplitCase { lines: s(&["---", "title: Example", "author: Test", "---", "# Heading", "Body text"]), prefix_len: 4, body_len: 2, prefix_spot_checks: vec![(0, "---"), (3, "---")], body_spot_check: Some("# Heading") })]
    #[case::detects_frontmatter_with_triple_dot_closer(FrontmatterSplitCase { lines: s(&["---", "title: Example", "...", "# Heading"]), prefix_len: 3, body_len: 1, prefix_spot_checks: vec![(2, "...")], body_spot_check: Some("# Heading") })]
    #[case::frontmatter_with_empty_body(FrontmatterSplitCase { lines: s(&["---", "title: Example", "---"]), prefix_len: 3, body_len: 0, prefix_spot_checks: vec![], body_spot_check: None })]
    #[case::frontmatter_only_no_body(FrontmatterSplitCase { lines: s(&["---", "---"]), prefix_len: 2, body_len: 0, prefix_spot_checks: vec![(1, "---")], body_spot_check: None })]
    #[case::trailing_whitespace_on_closer_is_trimmed(FrontmatterSplitCase { lines: s(&["---", "title: Example", "---  ", "# Heading"]), prefix_len: 3, body_len: 1, prefix_spot_checks: vec![], body_spot_check: None })]
    #[case::multiline_yaml_values_preserved(FrontmatterSplitCase { lines: s(&["---", "description: |", "  This is a multi-line", "  YAML value", "---", "# Content"]), prefix_len: 5, body_len: 1, prefix_spot_checks: vec![], body_spot_check: Some("# Content") })]
    fn frontmatter_split_cases(#[case] case: FrontmatterSplitCase) {
        let (prefix, body) = split_leading_yaml_frontmatter(&case.lines);
        assert_eq!(prefix.len(), case.prefix_len);
        assert_eq!(body.len(), case.body_len);
        for (idx, expected) in case.prefix_spot_checks {
            assert_eq!(prefix[idx], expected);
        }
        if let Some(expected) = case.body_spot_check {
            assert_eq!(body[0], expected);
        }
    }
}
