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
///   content (matching is done after `trim_end()`)}
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
    use super::*;

    #[test]
    fn empty_input_returns_empty_slices() {
        let lines: Vec<String> = vec![];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert!(prefix.is_empty());
        assert!(body.is_empty());
    }

    #[test]
    fn no_frontmatter_returns_empty_prefix() {
        let lines = vec!["# Heading".to_string(), "Some text".to_string()];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert!(prefix.is_empty());
        assert_eq!(body, &lines);
    }

    #[test]
    fn detects_frontmatter_with_triple_dash_closer() {
        let lines = vec![
            "---".to_string(),
            "title: Example".to_string(),
            "author: Test".to_string(),
            "---".to_string(),
            "# Heading".to_string(),
            "Body text".to_string(),
        ];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert_eq!(prefix.len(), 4);
        assert_eq!(prefix[0], "---");
        assert_eq!(prefix[3], "---");
        assert_eq!(body.len(), 2);
        assert_eq!(body[0], "# Heading");
    }

    #[test]
    fn detects_frontmatter_with_triple_dot_closer() {
        let lines = vec![
            "---".to_string(),
            "title: Example".to_string(),
            "...".to_string(),
            "# Heading".to_string(),
        ];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert_eq!(prefix.len(), 3);
        assert_eq!(prefix[2], "...");
        assert_eq!(body.len(), 1);
        assert_eq!(body[0], "# Heading");
    }

    #[test]
    fn unmatched_opener_treated_as_body() {
        // A --- line without a closer is not frontmatter
        let lines = vec![
            "---".to_string(),
            "Some text".to_string(),
            "More text".to_string(),
        ];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert!(prefix.is_empty());
        assert_eq!(body.len(), 3);
    }

    #[test]
    fn frontmatter_with_empty_body() {
        let lines = vec![
            "---".to_string(),
            "title: Example".to_string(),
            "---".to_string(),
        ];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert_eq!(prefix.len(), 3);
        assert!(body.is_empty());
    }

    #[test]
    fn frontmatter_only_no_body() {
        let lines = vec!["---".to_string(), "---".to_string()];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert_eq!(prefix.len(), 2);
        assert!(body.is_empty());
    }

    #[test]
    fn indented_opener_not_recognized() {
        // The opener must be exactly "---" at the start (no leading/trailing whitespace)
        let lines = vec![
            "  ---".to_string(),
            "title: Example".to_string(),
            "  ---".to_string(),
        ];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        // Indented opener is not recognized as frontmatter
        assert!(
            prefix.is_empty(),
            "indented opener should not be recognized"
        );
        assert_eq!(body.len(), 3);
    }

    #[test]
    fn later_dash_block_not_frontmatter() {
        // Only the leading block counts
        let lines = vec![
            "# Heading".to_string(),
            String::new(),
            "---".to_string(),
            "Not frontmatter".to_string(),
            "---".to_string(),
        ];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert!(prefix.is_empty());
        assert_eq!(body.len(), 5);
    }

    #[test]
    fn indented_closer_not_recognized() {
        // Indented closers are not recognized (to preserve YAML block scalars)
        let lines = vec![
            "---".to_string(),
            "title: Example".to_string(),
            "  ---  ".to_string(),
            "# Heading".to_string(),
        ];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        // The indented --- is not treated as a closer
        assert!(prefix.is_empty());
        assert_eq!(body.len(), 4);
    }

    #[test]
    fn trailing_whitespace_on_closer_is_trimmed() {
        // The closer can have trailing whitespace
        let lines = vec![
            "---".to_string(),
            "title: Example".to_string(),
            "---  ".to_string(),
            "# Heading".to_string(),
        ];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert_eq!(prefix.len(), 3);
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn multiline_yaml_values_preserved() {
        let lines = vec![
            "---".to_string(),
            "description: |".to_string(),
            "  This is a multi-line".to_string(),
            "  YAML value".to_string(),
            "---".to_string(),
            "# Content".to_string(),
        ];
        let (prefix, body) = split_leading_yaml_frontmatter(&lines);
        assert_eq!(prefix.len(), 5);
        assert_eq!(body.len(), 1);
        assert_eq!(body[0], "# Content");
    }
}
