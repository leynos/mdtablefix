//! Unit tests for private fence helper behaviour.

use rstest::{fixture, rstest};

use super::{attachment::AttachmentOutcome, *};

#[fixture]
fn out() -> Vec<String> { return Vec::new(); }

#[fixture]
fn tracker() -> FenceTracker { return FenceTracker::new(); }

#[rstest]
#[case(vec!["```"])]
#[case(vec!["", "```"])]
#[case(vec!["", "", "```"])]
fn attach_to_next_fence_attaches_to_unlabelled_fence(
    #[case] raw_lines: Vec<&str>,
    mut out: Vec<String>,
    mut tracker: FenceTracker,
) {
    let lines: Vec<String> = raw_lines.into_iter().map(str::to_string).collect();
    let mut lines = lines.iter().peekable();

    let outcome = attach_to_next_fence(&mut lines, "rust", "", &mut out, "Rust", &mut tracker);

    assert_eq!(outcome, AttachmentOutcome::Attached);
    assert_eq!(out, vec!["```rust".to_string()]);
    assert!(lines.next().is_none());
    assert!(tracker.in_fence());
}

#[rstest]
#[case(vec!["", "plain text"], vec!["Rust", ""], Some("plain text"))]
#[case(vec!["", "```python"], vec!["Rust", ""], Some("```python"))]
#[case(vec![], vec!["Rust"], None)]
fn attach_to_next_fence_preserves_specifier_when_no_attachment_occurs(
    #[case] raw_lines: Vec<&str>,
    #[case] expected_out: Vec<&str>,
    #[case] expected_next: Option<&str>,
    mut out: Vec<String>,
    mut tracker: FenceTracker,
) {
    let lines: Vec<String> = raw_lines.into_iter().map(str::to_string).collect();
    let mut lines = lines.iter().peekable();

    let outcome = attach_to_next_fence(&mut lines, "rust", "", &mut out, "Rust", &mut tracker);

    let expected_out: Vec<String> = expected_out.into_iter().map(str::to_string).collect();
    assert_eq!(outcome, AttachmentOutcome::Preserved);
    assert_eq!(out, expected_out);
    assert_eq!(lines.next().map(String::as_str), expected_next);
    assert!(!tracker.in_fence());
}

#[rstest]
#[case("```", "  ", "  ```rust")]
#[case("  ```", "    ", "    ```rust")]
#[case("\t```", "  ", "\t```rust")]
fn attach_to_next_fence_applies_indent_selection(
    #[case] fence_line: &str,
    #[case] spec_indent: &str,
    #[case] expected: &str,
    mut out: Vec<String>,
    mut tracker: FenceTracker,
) {
    let lines = [fence_line.to_string()];
    let mut lines = lines.iter().peekable();

    let outcome = attach_to_next_fence(
        &mut lines,
        "rust",
        spec_indent,
        &mut out,
        "Rust",
        &mut tracker,
    );

    assert_eq!(outcome, AttachmentOutcome::Attached);
    assert_eq!(out, vec![expected.to_string()]);
    assert!(lines.next().is_none());
    assert!(tracker.in_fence());
}
