//! Coupling tests for `try_couple_bracketed_reference` opener validation.
//!
//! Only a `[` opener introduces a bare numeric bracket reference, so the helper
//! must leave other opening punctuation as ordinary prose even when the token
//! that follows closes like a reference.

use rstest::rstest;

use super::{SpanKind, try_couple_bracketed_reference};

fn tokens(opener: &str, reference: &str) -> Vec<String> {
    vec![opener.to_string(), reference.to_string()]
}

#[rstest]
#[case("[", "1]", Some((SpanKind::BracketedRef, 2)))]
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
