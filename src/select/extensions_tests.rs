//! Grammar and matching tests for `--md-exts`.
//!
//! The message assertions are the user-visible half: the `--git --md-exts
//! md,,markdown` scenario in `tests/features/git_file_selection.feature`
//! asserts on `extension is empty` as it appears on stderr, so the wording
//! belongs to a test as well as to the feature file.

use camino::Utf8Path;
use rstest::rstest;

use super::{ExtensionFilter, ExtensionSpecError, InvalidCharacterKind, parse_extension};

#[test]
fn the_default_set_is_the_three_markdown_extensions() {
    let filter = ExtensionFilter::default();
    assert_eq!(filter.iter().collect::<Vec<_>>(), ["md", "mdc", "markdown"]);
    assert_eq!(filter.to_string(), "md, mdc, markdown");
}

#[rstest]
#[case("md", "md")]
#[case(".md", "md")]
#[case(".MD", "md")]
#[case("  .Markdown  ", "markdown")]
#[case("MDC", "mdc")]
fn one_extension_is_trimmed_unfolded_and_dedotted(#[case] value: &str, #[case] expected: &str) {
    assert_eq!(
        parse_extension(value).expect("a usable extension"),
        expected
    );
}

#[rstest]
#[case("", ExtensionSpecError::Empty)]
#[case("   ", ExtensionSpecError::Empty)]
#[case(".", ExtensionSpecError::DotOnly { value: ".".to_owned() })]
#[case(" . ", ExtensionSpecError::DotOnly { value: " . ".to_owned() })]
#[case(
    "md/markdown",
    ExtensionSpecError::InvalidCharacter {
        value: "md/markdown".to_owned(),
        kind: InvalidCharacterKind::PathSeparator,
    }
)]
#[case(
    "md\\markdown",
    ExtensionSpecError::InvalidCharacter {
        value: "md\\markdown".to_owned(),
        kind: InvalidCharacterKind::PathSeparator,
    }
)]
#[case(
    "md\0",
    ExtensionSpecError::InvalidCharacter {
        value: "md\0".to_owned(),
        kind: InvalidCharacterKind::Nul,
    }
)]
// A dot after the optional leading one, which no path's extension can equal.
#[case(
    "mdc.",
    ExtensionSpecError::InvalidCharacter {
        value: "mdc.".to_owned(),
        kind: InvalidCharacterKind::Dot,
    }
)]
#[case(
    "tar.gz",
    ExtensionSpecError::InvalidCharacter {
        value: "tar.gz".to_owned(),
        kind: InvalidCharacterKind::Dot,
    }
)]
// The leading dot is stripped before the check, so this is the same refusal as
// `mdc.` and is reported against the value as written.
#[case(
    ".md.",
    ExtensionSpecError::InvalidCharacter {
        value: ".md.".to_owned(),
        kind: InvalidCharacterKind::Dot,
    }
)]
fn an_unusable_extension_is_rejected_with_its_reason(
    #[case] value: &str,
    #[case] expected: ExtensionSpecError,
) {
    assert_eq!(
        parse_extension(value).expect_err("an unusable extension"),
        expected
    );
}

/// The exact string the feature file asserts on, kept honest by a test.
#[test]
fn the_empty_extension_message_is_the_one_the_feature_file_asserts() {
    assert_eq!(ExtensionSpecError::Empty.to_string(), "extension is empty");
}

/// `md,,markdown` reaches the parser as three values, the middle one empty:
/// this is the diagnostic the user sees for it.
#[test]
fn an_invalid_character_names_the_reason_and_the_value() {
    let error = parse_extension("md/markdown").expect_err("a path separator");
    assert_eq!(
        error.to_string(),
        "extension \"md/markdown\" contains a path separator"
    );
}

#[rstest]
#[case("docs/guide.md", true)]
#[case("docs/guide.MD", true)]
#[case("docs/guide.Markdown", true)]
#[case("notes.mdc", true)]
#[case("notes.mdx", false)]
#[case("docs/guide.md.bak", false)]
#[case("src/lib.rs", false)]
#[case("Makefile", false)]
// A leading dot with no second dot is a file name, not an extension: `Path`'s
// own reading, and the one that keeps `.md` from matching every dotfile.
#[case(".md", false)]
#[case("docs/.hidden", false)]
fn only_the_last_extension_counts_and_case_is_folded(#[case] path: &str, #[case] expected: bool) {
    let filter = ExtensionFilter::default();
    assert_eq!(filter.matches(Utf8Path::new(path)), expected, "{path}");
}

/// Why a configured dot is refused, stated as the fact the refusal rests on.
///
/// An extension is the segment after the *final* dot, so a value carrying one
/// is compared against something it can never equal. `a.mdc.` reports the empty
/// extension, because the segment after its final dot is nothing, and no
/// accepted value is empty — `--md-exts ""` is refused as surely as `mdc.` is.
/// The extension of `archive.tar.gz` is `gz`, which the value `tar.gz` never
/// equals, so a user who meant `gz` writes that and no more. Pinned here
/// because the parser's rule is only as durable as this behaviour, and because
/// this is what a reviewer should check the rule against.
#[rstest]
// Measured against `std::path::Path`, which `Utf8Path` delegates to.
#[case("a.mdc.", Some(""))]
#[case("a.b.c", Some("c"))]
#[case("archive.tar.gz", Some("gz"))]
#[case("docs/guide.md", Some("md"))]
#[case(".gitignore", None)]
fn a_paths_extension_is_the_segment_after_its_last_dot(
    #[case] path: &str,
    #[case] extension: Option<&str>,
) {
    assert_eq!(Utf8Path::new(path).extension(), extension, "{path}");
}

#[test]
fn a_filter_is_built_from_parsed_extensions_and_unfolds_its_own_duplicates() {
    let filter: ExtensionFilter = ["md", "MD", "mdc"].into_iter().map(str::to_owned).collect();
    assert_eq!(filter.iter().collect::<Vec<_>>(), ["md", "mdc"]);
    assert_eq!(filter.to_string(), "md, mdc");
}
