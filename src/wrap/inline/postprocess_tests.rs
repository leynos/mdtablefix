//! Unit tests for inline post-wrap normalization.
//!
//! These tests compile as a child module of `postprocess`, so they can cover
//! private helpers while keeping test-only cases out of the production module.

use rstest::rstest;

use super::{
    super::fragment::{FragmentKind, InlineFragment},
    *,
};

fn fragment(text: &str) -> InlineFragment { InlineFragment::new(text.into()) }

#[test]
fn inline_fragment_whitespace_space() {
    let fragment = InlineFragment::new(" ".into());
    assert_eq!(fragment.kind, FragmentKind::Whitespace);
    assert!(fragment.is_whitespace());
    assert!(!fragment.is_atomic());
    assert_eq!(fragment.width, 1);
}

#[test]
fn inline_fragment_whitespace_tab() {
    let fragment = InlineFragment::new("\t".into());
    assert_eq!(fragment.kind, FragmentKind::Whitespace);
}

#[test]
fn inline_fragment_inline_code() {
    let fragment = InlineFragment::new("`foo`".into());
    assert_eq!(fragment.kind, FragmentKind::InlineCode);
    assert!(fragment.is_atomic());
    assert!(!fragment.is_whitespace());
    assert!(!fragment.is_plain());
}

#[test]
fn inline_fragment_link() {
    let fragment = InlineFragment::new("[text](url)".into());
    assert_eq!(fragment.kind, FragmentKind::Link);
    assert!(fragment.is_atomic());
}

#[test]
fn inline_fragment_plain() {
    let fragment = InlineFragment::new("word".into());
    assert_eq!(fragment.kind, FragmentKind::Plain);
    assert!(fragment.is_plain());
    assert!(!fragment.is_atomic());
}

#[test]
fn merge_keeps_content_lines_unchanged() {
    let lines = vec![vec![fragment("hello")], vec![fragment("world")]];
    assert_eq!(merge_whitespace_only_lines(&lines, 80), lines);
}

#[test]
fn merge_carries_whitespace_forward() {
    let lines = vec![
        vec![fragment("hello")],
        vec![fragment(" ")],
        vec![fragment("world")],
    ];
    assert_eq!(
        merge_whitespace_only_lines(&lines, 80),
        vec![
            vec![fragment("hello")],
            vec![fragment(" "), fragment("world")],
        ]
    );
}

#[test]
fn merge_moves_inline_code_tail_before_single_space() {
    let lines = vec![
        vec![fragment("plain"), fragment("`code`")],
        vec![fragment(" ")],
        vec![fragment("tail")],
    ];
    let merged = merge_whitespace_only_lines(&lines, 80);

    assert_eq!(merged[0], vec![fragment("plain")]);
    assert_eq!(
        merged[1],
        vec![fragment("`code`"), fragment(" "), fragment("tail")]
    );
}

#[test]
fn merge_leaves_inline_code_tail_when_carry_would_exceed_width() {
    let lines = vec![
        vec![fragment("plain"), fragment("`long-code-tail`")],
        vec![fragment(" ")],
        vec![fragment("wide continuation")],
    ];
    let merged = merge_whitespace_only_lines(&lines, 20);

    assert_eq!(
        merged[0],
        vec![fragment("plain"), fragment("`long-code-tail`")]
    );
    assert_eq!(merged[1], vec![fragment("wide continuation")]);
}

#[test]
fn merge_moves_inline_code_tail_at_exact_width_boundary() {
    let lines = vec![
        vec![fragment("plain"), fragment("`code`")],
        vec![fragment(" ")],
        vec![fragment("tail")],
    ];
    let width = fragment("`code`").width + 1 + fragment("tail").width;
    let merged = merge_whitespace_only_lines(&lines, width);

    assert_eq!(merged[0], vec![fragment("plain")]);
    assert_eq!(
        merged[1],
        vec![fragment("`code`"), fragment(" "), fragment("tail")]
    );
}

#[test]
fn merge_trailing_whitespace_appended_to_last_line() {
    let lines = vec![vec![fragment("hello")], vec![fragment(" ")]];
    assert_eq!(
        merge_whitespace_only_lines(&lines, 80),
        vec![vec![fragment("hello"), fragment(" ")]]
    );
}

#[test]
fn merge_carries_multiple_consecutive_whitespace_lines_forward() {
    let lines = vec![
        vec![fragment("hello")],
        vec![fragment(" ")],
        vec![fragment("\t")],
        vec![fragment("world")],
    ];
    assert_eq!(
        merge_whitespace_only_lines(&lines, 80),
        vec![
            vec![fragment("hello")],
            vec![fragment(" "), fragment("\t"), fragment("world")]
        ]
    );
}

#[test]
fn merge_drops_single_space_before_atomic_starting_line() {
    let lines = vec![
        vec![fragment("alpha"), fragment("beta")],
        vec![fragment(" ")],
        vec![fragment("`code`")],
    ];
    assert_eq!(
        merge_whitespace_only_lines(&lines, 80),
        vec![
            vec![fragment("alpha"), fragment("beta")],
            vec![fragment("`code`")]
        ]
    );
}

#[test]
fn merge_empty_input_returns_empty_output() {
    assert!(merge_whitespace_only_lines(&[], 80).is_empty());
}

#[test]
fn rebalance_moves_atomic_tail_when_fits() {
    let mut lines = vec![
        vec![fragment("alpha"), fragment("`code`")],
        vec![fragment(" "), fragment("tail")],
    ];
    rebalance_atomic_tails(&mut lines, 80);
    assert_eq!(lines[0], vec![fragment("alpha")]);
    assert_eq!(
        lines[1],
        vec![fragment("`code`"), fragment(" "), fragment("tail")]
    );
}

#[rstest]
#[case::overflow_plain(
    vec![
        vec![fragment("alpha"), fragment("`code`")],
        vec![fragment(" "), fragment("plain")],
    ],
    11,
)]
#[case::next_starts_plain_only(
    vec![
        vec![fragment("alpha"), fragment("`code`")],
        vec![fragment("plain")],
    ],
    80,
)]
#[case::single_plain_fragment_line(
    vec![
        vec![fragment("tail")],
        vec![fragment(" "), fragment("beta")],
    ],
    20,
)]
#[case::single_atomic_fragment_line(
    vec![
        vec![fragment("`SessionState::write_sidecar(&self)`")],
        vec![fragment(" "), fragment("writes")],
    ],
    80,
)]
#[case::empty(Vec::new(), 10)]
#[case::single_line(vec![vec![fragment("alpha"), fragment("tail")]], 10)]
#[case::next_starts_space_then_atomic(
    vec![
        vec![fragment("alpha"), fragment("tail")],
        vec![fragment(" "), fragment("`code`")],
    ],
    20,
)]
fn rebalance_leaves_input_unchanged(
    #[case] mut lines: Vec<Vec<InlineFragment>>,
    #[case] width: usize,
) {
    let original = lines.clone();
    rebalance_atomic_tails(&mut lines, width);
    assert_eq!(lines, original);
}

#[test]
fn rebalance_moves_atomic_tail_at_exact_width_boundary() {
    let mut lines = vec![
        vec![fragment("alpha"), fragment("`tail`")],
        vec![fragment(" "), fragment("beta")],
    ];
    let width = line_width(&lines[1]) + lines[0].last().expect("tail exists").width;
    rebalance_atomic_tails(&mut lines, width);
    assert_eq!(lines[0], vec![fragment("alpha")]);
    assert_eq!(
        lines[1],
        vec![fragment("`tail`"), fragment(" "), fragment("beta")]
    );
}

#[test]
fn rebalance_moves_plain_tail_when_fits() {
    let mut lines = vec![
        vec![fragment("alpha"), fragment("tail")],
        vec![fragment(" "), fragment("beta")],
    ];
    rebalance_atomic_tails(&mut lines, 20);
    assert_eq!(lines[0], vec![fragment("alpha")]);
    assert_eq!(
        lines[1],
        vec![fragment("tail"), fragment(" "), fragment("beta")]
    );
}
