//! Coupling tests for `try_couple_bracketed_reference` opener validation.
//!
//! Only a `[` opener introduces a bare numeric bracket reference, so the helper
//! must leave other opening punctuation as ordinary prose even when the token
//! that follows closes like a reference.

use rstest::rstest;

use super::{SpanCursor, SpanKind, try_couple_bracketed_reference};

fn tokens(opener: &str, reference: &str) -> Vec<String> {
    vec![opener.to_owned(), reference.to_owned()]
}

#[rstest]
#[case("[", "1]", true)]
#[case("[", "[1]", true)]
#[case("[", "12].", true)]
#[case("(", "1]", false)]
#[case("\"", "12]", false)]
#[case("（", "1]", false)]
#[case("[", "[^1]", false)]
fn couples_square_bracket_openers_only(
    #[case] opener: &str,
    #[case] reference: &str,
    #[case] expected: bool,
) {
    let tokens = tokens(opener, reference);
    let mut cursor = SpanCursor {
        tokens: &tokens,
        end: 0,
        width: 0,
        kind: SpanKind::General,
    };

    assert_eq!(try_couple_bracketed_reference(&mut cursor), expected);
    if expected {
        assert_eq!(cursor.end, 2);
        assert_eq!(cursor.kind, SpanKind::BracketedRef);
    }
}

/// A `[` that follows another `[` is still the opener of the reference that
/// closes it, so the coupling fires from a bracket run's second token.
///
/// Declining it there separates `[[1]]` into `[` and `[1]]`, which the wrapper
/// is then free to break between, stranding the first bracket at a line end:
/// the shape issue #504 recorded. The cases are called at the index the
/// tokenizer reaches them at rather than at zero.
#[rstest]
#[case(&["[", "[", "1]"], true)]
#[case(&["[", "[", "1]]"], true)]
fn couples_the_reference_after_a_bracket_run(#[case] token_text: &[&str], #[case] expected: bool) {
    let tokens = token_text
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut cursor = SpanCursor {
        tokens: &tokens,
        end: 1,
        width: 0,
        kind: SpanKind::General,
    };

    assert_eq!(try_couple_bracketed_reference(&mut cursor), expected);
    assert_eq!(cursor.end, 3);
    assert_eq!(cursor.kind, SpanKind::BracketedRef);
}
