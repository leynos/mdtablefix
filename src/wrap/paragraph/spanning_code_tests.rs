//! Exact-output checks for source boundaries inside overlong code spans.
//!
//! These pin the seam and prose-reattachment behaviour that the property tests
//! in `spanning_code_props.rs` generalise. They were extracted from an inline
//! `mod tests` block so `spanning_code.rs` stays inside the repository's
//! 400-line limit, matching how `boundary_tests` is already declared.

use super::{
    ProseEdge,
    conforming_source_lines_for_overlong_span,
    join_with_boundaries,
    reattach_prose,
};

#[rstest::rstest]
#[case::after_code_content("`éab", "cd`", 5)]
#[case::immediately_after_code_content("`é", "abc`", 3)]
fn preserves_multibyte_code_span_source_boundaries(
    #[case] first: &str,
    #[case] second: &str,
    #[case] expected_boundary: usize,
) {
    let segments = [(first.to_owned(), false), (second.to_owned(), false)];
    let (joined, boundaries) = join_with_boundaries(&segments);

    assert_eq!(joined, format!("{first} {second}"));
    assert_eq!(boundaries, [expected_boundary]);
    assert_eq!(
        conforming_source_lines_for_overlong_span(&segments, "", 4),
        Some(vec![first.to_owned(), second.to_owned()]),
    );
}

#[test]
fn joins_prose_at_the_requested_preserved_span_edge() {
    let mut leading = vec!["`code`".to_owned()];
    reattach_prose(&mut leading, "before ", 20, ProseEdge::Leading);
    assert_eq!(leading, ["before `code`"]);

    let mut trailing = vec!["`code`".to_owned()];
    reattach_prose(&mut trailing, " after", 20, ProseEdge::Trailing);
    assert_eq!(trailing, ["`code` after"]);
}

#[test]
fn wraps_trimmed_prose_at_the_requested_preserved_span_edge() {
    let mut leading = vec!["`code".to_owned(), "span`".to_owned()];
    reattach_prose(&mut leading, "before ", 5, ProseEdge::Leading);
    assert_eq!(leading, ["before", "`code", "span`"]);

    let mut trailing = vec!["`code".to_owned(), "span`".to_owned()];
    reattach_prose(&mut trailing, " after", 5, ProseEdge::Trailing);
    assert_eq!(trailing, ["`code", "span`", "after"]);
}
