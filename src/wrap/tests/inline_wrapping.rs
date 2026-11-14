//! Tests for the inline wrapping pipeline.
//!
//! These cases exercise `wrap_preserving_code` and
//! `attach_punctuation_to_previous_line` from `crate::wrap::inline`, covering
//! hard-break preservation, bracket references, whitespace-only folds, and
//! carry-whitespace handling. The suite uses `proptest` for generated wrapping
//! cases and `rstest` for fixture-driven cases.
//!
//! Two families moved out to keep this module under the file-size ceiling: the
//! citation cases to [`super::inline_citations`], and every link punctuation
//! case — leading and trailing alike — to [`super::inline_link_punctuation`].

use rstest::rstest;

use super::{
    super::{inline::attach_punctuation_to_previous_line, wiring::wrap_preserving_code},
    TRAILING_PUNCTUATION_CHARS,
};

proptest::proptest! {
    #[test]
    fn wrap_preserving_code_keeps_supported_punctuation_with_links(
        punctuation_index in 0..TRAILING_PUNCTUATION_CHARS.len(),
        pad_len in 0usize..24,
        wrap_width in 24usize..96,
        url_slug in "[a-z]{3,12}",
    ) {
        let punctuation = TRAILING_PUNCTUATION_CHARS[punctuation_index];
        let prefix = "lead ".repeat(pad_len);
        let link = format!("[link](docs/{url_slug}.md){punctuation}");
        let input = format!("{prefix}{link} trailing words force wrapping");
        let lines = wrap_preserving_code(&input, wrap_width);

        assert!(
            lines.iter().any(|line| line.contains(&link)),
            "expected {link:?} to stay attached in {lines:?}",
        );
        assert!(
            lines.iter().all(|line| line.trim() != punctuation.to_string()),
            "punctuation was orphaned in {lines:?}",
        );
    }

    #[test]
    fn wrap_preserving_code_keeps_supported_punctuation_with_code_spans(
        punctuation_index in 0..TRAILING_PUNCTUATION_CHARS.len(),
        pad_len in 0usize..24,
        wrap_width in 24usize..96,
    ) {
        let punctuation = TRAILING_PUNCTUATION_CHARS[punctuation_index];
        let prefix = "lead ".repeat(pad_len);
        let code_span = format!("`code-{pad_len}`{punctuation}");
        let input = format!("{prefix}{code_span} trailing words force wrapping");
        let lines = wrap_preserving_code(&input, wrap_width);

        assert!(
            lines.iter().any(|line| line.contains(&code_span)),
            "expected {code_span:?} to stay attached in {lines:?}",
        );
        assert!(
            lines.iter().all(|line| line.trim() != punctuation.to_string()),
            "punctuation was orphaned in {lines:?}",
        );
    }
}

#[test]
fn attach_punctuation_appends_to_previous_code_line() {
    let mut lines = vec!["wrap `code`".to_owned()];
    let current = String::new();
    assert!(attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        "!",
    ));
    assert_eq!(lines, vec!["wrap `code`!".to_owned()]);
}

#[test]
fn attach_punctuation_requires_empty_current_buffer() {
    let mut lines = vec!["`code`".to_owned()];
    let current = " pending".to_owned();
    assert!(!attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        "!",
    ));
    assert_eq!(lines, vec!["`code`".to_owned()]);
}

#[test]
fn attach_punctuation_ignores_non_code_suffix() {
    let mut lines = vec!["plain text".to_owned()];
    let current = String::new();
    assert!(!attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        ".",
    ));
    assert_eq!(lines, vec!["plain text".to_owned()]);
}

/// A break made at a run of spaces keeps none of them at the line end: two or
/// more would render as a hard break the source never wrote (issue #561).
#[test]
fn wrap_preserving_code_trims_consecutive_whitespace_at_breaks() {
    let lines = wrap_preserving_code("alpha  beta   gamma", 8);
    assert_eq!(
        lines,
        vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]
    );
}

#[test]
fn wrap_preserving_code_keeps_colon_suffixed_footnote_reference_with_prose() {
    let input = concat!(
        "The Lighthouse \"Progressive Web App\" category specifically checks for the technical ",
        "requirements of a PWA. The audits are grouped into subcategories [^96]:",
    );
    let lines = wrap_preserving_code(input, 80);

    assert_eq!(
        lines,
        vec![
            "The Lighthouse \"Progressive Web App\" category specifically checks for the",
            "technical requirements of a PWA. The audits are grouped into",
            "subcategories [^96]:",
        ]
    );
    assert!(lines.iter().all(|line| !line.starts_with("[^96]:")));
}

#[test]
fn wrap_preserving_code_couples_opening_paren_before_inline_code() {
    let text = concat!(
        "- `src/cli/mod.rs` (240 lines): defines the `Cli` struct with ",
        "`#[derive(Parser, Serialize, Deserialize, OrthoConfig)]`, its subcommands ",
        "(`Commands` enum), and the `parse_with_localizer_from` function that creates ",
        "a localized clap command and parses arguments."
    );
    let lines = wrap_preserving_code(text, 80);
    for window in lines.windows(2) {
        assert!(
            !window[0].ends_with('('),
            "opening parenthesis must not be stranded at line end: {lines:?}"
        );
    }
}

#[rstest]
#[case("(`code`)", 10)]
#[case("[`code`]", 10)]
#[case("（`code`）", 10)]
#[case("「`code`」", 10)]
#[case("([label](url))", 10)]
#[case("[[label](url)]", 10)]
fn wrap_preserving_code_keeps_opening_bracket_with_inline_code(
    #[case] fragment: &str,
    #[case] width: usize,
) {
    let text = format!("prefix text {fragment} suffix.");
    let lines = wrap_preserving_code(&text, width);
    for line in &lines {
        if line.contains('`') || line.contains("](") {
            assert!(
                !line.ends_with('(')
                    && !line.ends_with('[')
                    && !line.ends_with('（')
                    && !line.ends_with('「'),
                "opening bracket must stay with atomic span on line: {line:?}"
            );
        }
    }
}

/// Prose and an inline code span that together fill 78 of the 80 columns, so a
/// following bracket reference cannot fit and must move as a unit.
///
/// This is the paragraph side of the issue #504 reproduction, where the
/// reference arrived on its own source line and the two were reflowed together.
const BRACKET_REFERENCE_SEAM_HEAD: &str = concat!(
    "aaaaa aaaaaa aaaa aa aaaa aamw jkxf ht abm iqy uxqdkre fz ioelg ",
    "**bold**`code`",
);

/// Asserts that `reference` survived wrapping intact.
fn assert_bracket_reference_whole(lines: &[String], reference: &str) {
    assert!(
        lines.iter().any(|line| line.contains(reference)),
        "expected {reference:?} to stay whole in {lines:?}",
    );
    assert!(
        lines.iter().all(|line| !line.ends_with('[')),
        "opening bracket must not be stranded at line end: {lines:?}",
    );
    assert!(
        lines.iter().all(|line| !line.trim_start().starts_with(']')),
        "reference tail must not be detached from its opener: {lines:?}",
    );
}

#[rstest]
#[case("[1]")]
#[case("[12]")]
fn wrap_preserving_code_keeps_bracket_reference_whole_after_code_span(#[case] reference: &str) {
    let text = format!("{BRACKET_REFERENCE_SEAM_HEAD} {reference}");
    let lines = wrap_preserving_code(&text, 80);

    assert_bracket_reference_whole(&lines, reference);
    assert_eq!(
        lines,
        vec![BRACKET_REFERENCE_SEAM_HEAD.to_owned(), reference.to_owned()]
    );
}

#[rstest]
#[case(11)]
#[case(12)]
#[case(13)]
#[case(15)]
#[case(16)]
fn wrap_preserving_code_keeps_bracket_reference_whole_across_widths(#[case] width: usize) {
    // The code span and the reference fill 18 columns together, so every width
    // under test is too narrow to hold both and forces the reference to move.
    let text = "**bold**`code` [1] and more prose to force a wrap.";
    let lines = wrap_preserving_code(text, width);

    assert!(
        lines.len() > 1,
        "expected wrapping at width {width}: {lines:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.contains('[') && !line.contains("[1]"))
    );
    assert_bracket_reference_whole(&lines, "[1]");
}

#[test]
fn wrap_preserving_code_keeps_reference_link_opening_bracket_with_label() {
    let input = concat!(
        "Implicit fixture injection removes the need for attributes in most cases. ",
        "[user guide][implicit-fixture-guide] · [trybuild][implicit-fixture-trybuild]",
    );
    let lines = wrap_preserving_code(input, 80);

    assert_eq!(
        lines,
        vec![
            "Implicit fixture injection removes the need for attributes in most cases.",
            concat!(
                "[user guide][implicit-fixture-guide] · ",
                "[trybuild][implicit-fixture-trybuild]",
            ),
        ]
    );
    assert!(lines.iter().all(|line| !line.ends_with('[')));
}

#[test]
fn wrap_preserving_code_glues_punctuation_after_code() {
    let lines = wrap_preserving_code("line with `code` !", 80);
    assert_eq!(lines, vec!["line with `code`!".to_owned()]);
}

#[test]
fn wrap_preserving_code_breaks_between_inline_code_spans() {
    let text = "Extensions (`.toml`, `.json`, `.json5`, `.yaml`, `.yml`).";
    // Width 35 sits between the width of the `.json` and `.json5` prefixes,
    // forcing the wrapper to decide whether it can break between separate
    // inline code spans that are spaced apart.
    let lines = wrap_preserving_code(text, 35);
    assert_eq!(
        lines,
        vec![
            "Extensions (`.toml`, `.json`,".to_owned(),
            "`.json5`, `.yaml`, `.yml`).".to_owned(),
        ]
    );
}

#[test]
fn wrap_preserving_code_retains_punctuation_after_separate_spans() {
    let text = "Alpha `code` `more`, trailing.";
    let lines = wrap_preserving_code(text, 18);
    assert_eq!(
        lines,
        vec!["Alpha `code`".to_owned(), "`more`, trailing.".to_owned(),]
    );
}

#[rstest]
#[case("alpha beta", 5, &["alpha", "beta"])]
#[case("alpha  beta", 5, &["alpha", "beta"])]
#[case("alpha `beta`", 5, &["alpha", "`beta`"])]
fn wrap_preserving_code_strips_leading_carry_whitespace(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected: &[&str],
) {
    let lines = wrap_preserving_code(input, width);
    assert_eq!(
        lines,
        expected.iter().map(|&s| s.to_owned()).collect::<Vec<_>>()
    );
    for line in lines.iter().skip(1) {
        assert!(
            !line.starts_with(' '),
            "continuation lines must not begin with carry whitespace: {line:?}"
        );
    }
}

#[rstest]
#[case("  alpha beta", 8, &["  alpha", "beta"])]
fn wrap_preserving_code_preserves_intentional_leading_whitespace_on_first_line(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected: &[&str],
) {
    let lines = wrap_preserving_code(input, width);
    assert_eq!(
        lines,
        expected.iter().map(|&s| s.to_owned()).collect::<Vec<_>>()
    );
    assert!(
        lines[0].starts_with("  "),
        "first line must preserve intentional leading whitespace"
    );
    for line in lines.iter().skip(1) {
        assert!(
            !line.starts_with(' '),
            "continuation lines must not begin with carry whitespace: {line:?}"
        );
    }
}

#[rstest]
#[case("trail  ", 80, &["trail  "])]
#[case("`code span`  ", 12, &["`code span`  "])]
#[case("foo  ", 3, &["foo  "])]
#[case("x  ", 1, &["x  "])]
fn preserves_trailing_spaces(#[case] input: &str, #[case] width: usize, #[case] expected: &[&str]) {
    let out = wrap_preserving_code(input, width);
    assert_eq!(
        out,
        expected.iter().map(|&s| s.to_owned()).collect::<Vec<_>>()
    );
}

#[rstest]
#[case("aaaaaaaaaaaa", 5, &["aaaaaaaaaaaa"])] // forced flush without split
#[case("abcde", 3, &["abcde"])]
#[case("`codespan`", 6, &["`codespan`"])]
fn no_split_forced_flush_no_trim(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected: &[&str],
) {
    let out = wrap_preserving_code(input, width);
    assert_eq!(
        out,
        expected.iter().map(|&s| s.to_owned()).collect::<Vec<_>>()
    );
}

/// A wrap of whitespace-only content can fold away to no lines at all.
///
/// `fit_fragment` used to treat only a *single* grouped line as "nothing to
/// flush yet", so an empty layout reached `flush_boundary_link`, which
/// subtracted two from a length of zero and panicked. A lone space reaches it
/// at every width, which makes this the smallest form of the defect.
#[rstest]
#[case::lone_space_at_width_one(" ", 1)]
#[case::lone_space_at_default_width(" ", 80)]
#[case::two_spaces("  ", 1)]
fn wrap_preserving_code_handles_whitespace_only_content(#[case] input: &str, #[case] width: usize) {
    let lines = wrap_preserving_code(input, width);

    assert!(
        lines.iter().all(|line| line.trim().is_empty()),
        "whitespace-only input {input:?} must not gain visible content, got {lines:?}",
    );
}

/// The same empty-layout path is reached when leading prose is trimmed of its
/// trailing spaces but keeps a word, so the word must survive intact.
///
/// This is the shape `reattach_prose` actually passes at the leading edge, and
/// it is the case that makes the defect a production path rather than a
/// whitespace-only curiosity.
#[test]
fn wrap_preserving_code_keeps_trimmed_leading_prose() {
    let lines = wrap_preserving_code(" a", 1);

    assert_eq!(lines, [" a"]);
}
