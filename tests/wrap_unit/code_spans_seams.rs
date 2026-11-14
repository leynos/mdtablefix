//! Seam tests for authored breaks adjacent to multibyte code-span content.
//!
//! `code_spans.rs` covers the shape where the preserved seam is ASCII on both
//! sides. These cases reach the shape it cannot: a preserved code-span seam
//! whose left piece ends in a multibyte scalar.
//!
//! The fallback only engages when **every authored line already fits** and the
//! joined span exceeds the width. An input where a line overflows defers to the
//! greedy wrapper and never reaches boundary preservation, so the widths below
//! are chosen deliberately rather than incidentally.

use mdtablefix::wrap::wrap_text;
use unicode_width::UnicodeWidthStr;

/// Assert the output honours `width`, is a fixed point under a second pass, and
/// preserves the given seam as two adjacent lines.
///
/// The seam check is what distinguishes boundary preservation from ordinary
/// greedy wrapping: a joined span would appear as one overlong line.
fn assert_preserved_seam(output: &[String], width: usize, left: &str, right: &str) {
    assert!(
        output
            .iter()
            .all(|line| UnicodeWidthStr::width(line.as_str()) <= width),
        "output must respect width {width}: {output:?}"
    );
    assert_eq!(
        wrap_text(output, width),
        output,
        "second pass must be a fixed point"
    );

    let seam = output.iter().position(|line| line == left);
    assert!(
        matches!(seam, Some(index) if output.get(index + 1).is_some_and(|line| line == right)),
        "expected preserved seam {left:?}/{right:?} adjacent in {output:?}"
    );
}

/// The authored break inside the span follows the multibyte scalar `é`.
///
/// The wider span (13 columns) cannot fit the width, while both authored lines
/// (8 columns each) do, which is what makes the fallback engage rather than
/// defer. Observed output is recorded exactly rather than inferred.
#[test]
fn wrap_text_preserves_authored_break_immediately_after_multibyte_inside_span() {
    let input = lines_vec!["é `abcdé", "fghij` x"];
    let width = 8;

    let wrapped = wrap_text(&input, width);

    assert_eq!(wrapped, lines_vec!["é", "`abcdé", "fghij`", "x"]);
    assert_preserved_seam(&wrapped, width, "`abcdé", "fghij`");
}

/// The multibyte scalar sits immediately before the seam rather than in the
/// prose, so the boundary offset itself follows a multibyte character.
#[test]
fn wrap_text_preserves_authored_break_after_multibyte_code_content() {
    let input = lines_vec!["`abcé", "def` x"];
    let width = 6;

    let wrapped = wrap_text(&input, width);

    assert_eq!(wrapped, lines_vec!["`abcé", "def`", "x"]);
    assert_preserved_seam(&wrapped, width, "`abcé", "def`");
}

/// A prefixed paragraph with a multibyte prefix reaches the synthetic
/// edge-space trim, which must drop only the space contributed by joining.
///
/// The prefilter (`has_potential_synthetic_edge_space`) is purely textual, so a
/// multibyte prefix is not what gates this path; the multibyte content is what
/// makes the offsets differ from character positions. Both edges are covered:
/// a join space left of the closing fence, and one right of the opening fence.
///
/// Observed output is recorded, not inferred. The contrast that gives these
/// cases meaning is that an *authored* edge space survives — see
/// `wrap_text_keeps_authored_code_span_edge_space_beside_multibyte_prefix`.
#[rstest::rstest]
#[case::before_closer(lines_vec!["- éé `", "  foo ` 末"], "- éé `foo ` 末")]
#[case::after_opener(lines_vec!["- éé ` foo", "  ` 末"], "- éé ` foo` 末")]
fn wrap_text_trims_synthetic_code_span_edge_space_beside_multibyte_prefix(
    #[case] input: Vec<String>,
    #[case] expected_trimmed: &str,
) {
    let width = 16;

    let wrapped = wrap_text(&input, width);

    assert_eq!(wrapped, lines_vec![expected_trimmed]);
    assert!(
        wrapped
            .iter()
            .all(|line| UnicodeWidthStr::width(line.as_str()) <= width)
    );
    assert_eq!(wrap_text(&wrapped, width), wrapped);
}

/// The mirror of the synthetic-space cases: a space the author actually typed
/// inside the fence must survive joining, so the trim is not a blanket collapse.
#[test]
fn wrap_text_keeps_authored_code_span_edge_space_beside_multibyte_prefix() {
    let input = lines_vec!["- é `foo ", "  bar` 末"];

    let wrapped = wrap_text(&input, 20);

    assert_eq!(wrapped, lines_vec!["- é `foo  bar` 末"]);
}
