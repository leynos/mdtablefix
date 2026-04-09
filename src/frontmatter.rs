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
/// ```
/// use mdtablefix::frontmatter::split_leading_yaml_frontmatter;
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
pub fn split_leading_yaml_frontmatter(lines: &[String]) -> (&[String], &[String]) {
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

    /// Cases where `prefix` is empty (no frontmatter detected).
    #[rstest]
    #[case::empty_input_returns_empty_slices(
        s(&[]),
        true, // body_is_empty
        false // check_body_equality
    )]
    #[case::no_frontmatter_returns_empty_prefix(
        s(&["# Heading", "Some text"]),
        false,
        true // check body == input lines
    )]
    #[case::unmatched_opener_treated_as_body(
        s(&["---", "Some text", "More text"]),
        false,
        false
    )]
    #[case::indented_opener_not_recognized(
        s(&["  ---", "title: Example", "  ---"]),
        false,
        false
    )]
    #[case::later_dash_block_not_frontmatter(
        s(&["# Heading", "", "---", "Not frontmatter", "---"]),
        false,
        false
    )]
    #[case::indented_closer_not_recognized(
        s(&["---", "title: Example", "  ---  ", "# Heading"]),
        false,
        false
    )]
    fn prefix_empty_cases(
        #[case] lines: Vec<String>,
        #[case] body_is_empty: bool,
        #[case] check_body_equality: bool,
    ) {
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert!(prefix.is_empty());
        if body_is_empty {
            assert!(body.is_empty());
        } else if check_body_equality {
            assert_eq!(body, &lines);
        } else {
            assert!(!body.is_empty());
        }
    }

    /// Cases where frontmatter is detected (non-empty `prefix`).
    #[rstest]
    #[case::detects_frontmatter_with_triple_dash_closer(
        s(&["---", "title: Example", "author: Test", "---", "# Heading", "Body text"]),
        4,      // prefix_len
        2,      // body_len
        Some((0, "---")),
        Some((3, "---")),
        Some("# Heading")
    )]
    #[case::detects_frontmatter_with_triple_dot_closer(
        s(&["---", "title: Example", "...", "# Heading"]),
        3,
        1,
        Some((2, "...")),
        None,
        Some("# Heading")
    )]
    #[case::frontmatter_with_empty_body(
        s(&["---", "title: Example", "---"]),
        3,
        0,
        None,
        None,
        None
    )]
    #[case::frontmatter_only_no_body(
        s(&["---", "---"]),
        2,
        0,
        Some((1, "---")),
        None,
        None
    )]
    #[case::trailing_whitespace_on_closer_is_trimmed(
        s(&["---", "title: Example", "---  ", "# Heading"]),
        3,
        1,
        None,
        None,
        None
    )]
    #[case::multiline_yaml_values_preserved(
        s(&["---", "description: |", "  This is a multi-line", "  YAML value", "---", "# Content"]),
        5,
        1,
        None,
        None,
        Some("# Content")
    )]
    fn frontmatter_split_cases(
        #[case] lines: Vec<String>,
        #[case] prefix_len: usize,
        #[case] body_len: usize,
        #[case] prefix_spot_check: Option<(usize, &str)>,
        #[case] prefix_spot_check_2: Option<(usize, &str)>,
        #[case] body_spot_check: Option<&str>,
    ) {
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert_eq!(prefix.len(), prefix_len);
        assert_eq!(body.len(), body_len);
        if let Some((idx, expected)) = prefix_spot_check {
            assert_eq!(prefix[idx], expected);
        }
        if let Some((idx, expected)) = prefix_spot_check_2 {
            assert_eq!(prefix[idx], expected);
        }
        if let Some(expected) = body_spot_check {
            assert_eq!(body[0], expected);
        }
    }
}
