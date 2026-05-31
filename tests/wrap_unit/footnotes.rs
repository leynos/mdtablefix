//! `wrap_text` tests covering inline GFM footnote-reference handling.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

/// Keep the pre-split snapshot layout: file names live in `tests/snapshots/`
/// and use the legacy `wrap_unit__<name>` prefix without the module
/// component, so existing reviewed snapshots remain authoritative.
macro_rules! footnote_snapshot {
    ($name:expr, $value:expr) => {
        insta::with_settings!(
            {
                snapshot_path => "../snapshots",
                prepend_module_to_snapshot => false,
            },
            { insta::assert_snapshot!($name, $value) }
        );
    };
}

fn assert_footnote_reference_is_intact(output: &[String], marker: &str) {
    let rendered = output.join("\n");
    assert!(rendered.contains(marker));
    assert!(!rendered.contains("[\n"));
    assert!(!rendered.contains("\n^"));
}

#[rstest]
#[case("[^4]")]
#[case("[^25]")]
#[case("[^note]")]
fn wrap_text_preserves_inline_footnote_references(#[case] marker: &str) {
    let input = lines_vec![format!(
        concat!(
            "This sentence has enough preceding text to make the formatter choose ",
            "a bad wrap point near this reference ",
            "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.",
            "{} This sentence follows the reference marker.",
        ),
        marker,
    )];

    let wrapped = wrap_text(&input, 80);

    assert_footnote_reference_is_intact(&wrapped, marker);
    assert!(
        wrapped
            .iter()
            .any(|line| line.contains(&format!(".{marker}")))
    );
}

#[test]
fn wrap_text_snapshots_inline_footnote_reference_outputs() {
    let input = lines_vec![concat!(
        "This sentence has enough preceding text to make the formatter choose ",
        "a bad wrap point near this reference ",
        "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.",
        "[^4] This sentence follows the reference marker.",
    )];

    footnote_snapshot!(
        "inline_footnote_reference_wrap",
        wrap_text(&input, 80).join("\n")
    );
}

#[rstest]
#[case(
    concat!(
        "To assert specific outcomes, use the standard macros `assert!`, `assert_eq!`, and ",
        "`assert_ne!`.[^3] This sentence follows the reference marker.",
    ),
    "[^3]",
    "`assert_ne!`.[^3]",
    "inline_footnote_reference_after_code_wrap"
)]
#[case(
    concat!(
        "See the standard macros (`assert!`, `assert_eq!`, and `assert_ne!`).[^3] ",
        "This sentence follows the reference marker.",
    ),
    "[^3]",
    "`assert_ne!`).[^3]",
    "inline_footnote_reference_after_opener_coupled_code_wrap"
)]
fn wrap_text_keeps_footnote_reference_with_preceding_atomic_span(
    #[case] paragraph: &str,
    #[case] marker: &str,
    #[case] expected_snippet: &str,
    #[case] snapshot_name: &str,
) {
    let input = lines_vec![paragraph];
    let wrapped = wrap_text(&input, 80);

    assert_footnote_reference_is_intact(&wrapped, marker);
    assert!(wrapped.iter().any(|line| line.contains(expected_snippet)));

    footnote_snapshot!(snapshot_name, wrapped.join("\n"));
}
