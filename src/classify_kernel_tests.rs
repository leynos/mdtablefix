//! Unit tests for the production structural scanner kernel.

use proptest::prelude::*;
use rstest::rstest;

use crate::classify_kernel::{ClassifyCtxKernel, LineClass, OpenFence, classify_seq};

/// Runs the production kernel for one UTF-8 test line.
fn classify(line: &str, ctx: &ClassifyCtxKernel) -> LineClass {
    classify_seq(&line.chars().collect::<Vec<_>>(), ctx).class
}

/// Checks representative structural classes and close grammar boundaries.
#[rstest]
#[case("2024 revenue", LineClass::ParagraphText)]
#[case("# heading", LineClass::AtxHeading)]
#[case("#123", LineClass::ParagraphText)]
#[case("####### heading", LineClass::ParagraphText)]
#[case("|---|---|", LineClass::TableDelimiter)]
#[case("|  |  |", LineClass::TableRow)]
#[case("```rust", LineClass::FenceMarker)]
#[case("___", LineClass::ThematicBreak)]
#[case("- item", LineClass::ListItem)]
#[case("123456789. item", LineClass::ListItem)]
#[case("1234567890. item", LineClass::ParagraphText)]
#[case("   ", LineClass::Blank)]
#[case(">", LineClass::Blank)]
#[case(">   ", LineClass::Blank)]
#[case("    code", LineClass::Literal)]
#[case("\t_\t_\t_\t", LineClass::Literal)]
#[case(">     > code", LineClass::Literal)]
#[case("> \ttext", LineClass::ParagraphText)]
#[case("\u{00a0}", LineClass::ParagraphText)]
fn classifies_structural_lines(#[case] line: &str, #[case] expected: LineClass) {
    assert_eq!(classify(line, &ClassifyCtxKernel::default()), expected);
}

/// Checks that Setext classification requires prefix agreement.
#[rstest]
#[case(true, LineClass::SetextUnderline)]
#[case(false, LineClass::ThematicBreak)]
fn classifies_setext_according_to_prefix_agreement(
    #[case] prefix_agrees: bool,
    #[case] expected: LineClass,
) {
    let context = ClassifyCtxKernel::following(LineClass::ParagraphText, prefix_agrees);

    assert_eq!(classify("---", &context), expected);
}

/// Checks matching and non-matching fence contents against open-fence state.
#[rstest]
#[case("```", LineClass::FenceMarker)]
#[case("``", LineClass::Literal)]
#[case("~~~", LineClass::Literal)]
#[case("", LineClass::Literal)]
#[case("    ```", LineClass::Literal)]
fn classifies_lines_inside_an_open_fence(#[case] line: &str, #[case] expected: LineClass) {
    let context = ClassifyCtxKernel::in_fence(OpenFence::new('`', 3));

    assert_eq!(classify(line, &context), expected);
}

proptest! {
    /// Keeps every scalar body offset inside its input sequence.
    #[test]
    fn body_start_stays_within_its_character_sequence(chars in prop::collection::vec(any::<char>(), 0..128)) {
        let classified = classify_seq(&chars, &ClassifyCtxKernel::default());

        prop_assert!(classified.body_start.0 <= chars.len());
    }
}
