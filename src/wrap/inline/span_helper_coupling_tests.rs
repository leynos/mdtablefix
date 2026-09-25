//! Coupling tests for `try_couple_bracketed_reference` opener validation.
//!
//! Only a `[` opener introduces a bare numeric bracket reference, so the helper
//! must leave other opening punctuation as ordinary prose even when the token
//! that follows closes like a reference.

use rstest::rstest;

use super::{SpanKind, try_couple_bracketed_reference};

fn tokens(opener: &str, reference: &str) -> Vec<String> {
    vec![opener.to_owned(), reference.to_owned()]
}

#[rstest]
#[case("[", "1]", Some((SpanKind::BracketedRef, 2)))]
#[case("[", "[1]", Some((SpanKind::BracketedRef, 2)))]
#[case("[", "12].", Some((SpanKind::BracketedRef, 2)))]
#[case("(", "1]", None)]
#[case("\"", "12]", None)]
#[case("（", "1]", None)]
#[case("[", "[^1]", None)]
fn couples_square_bracket_openers_only(
    #[case] opener: &str,
    #[case] reference: &str,
    #[case] expected: Option<(SpanKind, usize)>,
) {
    let mut width = 0;

    assert_eq!(
        try_couple_bracketed_reference(&tokens(opener, reference), 0, &mut width),
        expected
    );
}

/// A `[` that follows another `[` is still the opener of the reference that
/// closes it, so the coupling fires from a bracket run's second token.
///
/// Declining it there separates `[[1]]` into `[` and `[1]]`, which the wrapper
/// is then free to break between, stranding the first bracket at a line end:
/// the shape issue #504 recorded. The cases are called at the index the
/// tokenizer reaches them at rather than at zero.
#[rstest]
#[case(&["[", "[", "1]"], Some((SpanKind::BracketedRef, 3)))]
#[case(&["[", "[", "1]]"], Some((SpanKind::BracketedRef, 3)))]
fn couples_the_reference_after_a_bracket_run(
    #[case] token_text: &[&str],
    #[case] expected: Option<(SpanKind, usize)>,
) {
    let mut width = 0;
    let tokens = token_text
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    assert_eq!(
        try_couple_bracketed_reference(&tokens, 1, &mut width),
        expected
    );
}
