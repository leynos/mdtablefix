//! Tests for inline wrapping that preserves code spans and links.

use std::fmt::Write as _;

use rstest::rstest;

use super::{
    super::inline::{attach_punctuation_to_previous_line, wrap_preserving_code},
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

    #[test]
    fn wrap_preserving_code_keeps_generated_inline_citations_attached(
        wrap_width in 24usize..96,
        prefix_len in 0usize..32,
        citation_count in 1usize..6,
    ) {
        let citation = inline_citation_chain(citation_count);
        let expected_citation = format!("pattern{citation}");
        let input = format!(
            "{}{expected_citation} trailing words force wrapping",
            "lead ".repeat(prefix_len)
        );
        let lines = wrap_preserving_code(&input, wrap_width);

        assert_inline_citation_invariants(&lines, &expected_citation);
    }
}

#[test]
fn attach_punctuation_appends_to_previous_code_line() {
    let mut lines = vec!["wrap `code`".to_string()];
    let current = String::new();
    assert!(attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        "!",
    ));
    assert_eq!(lines, vec!["wrap `code`!".to_string()]);
}

#[test]
fn attach_punctuation_requires_empty_current_buffer() {
    let mut lines = vec!["`code`".to_string()];
    let current = " pending".to_string();
    assert!(!attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        "!",
    ));
    assert_eq!(lines, vec!["`code`".to_string()]);
}

#[test]
fn attach_punctuation_ignores_non_code_suffix() {
    let mut lines = vec!["plain text".to_string()];
    let current = String::new();
    assert!(!attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        ".",
    ));
    assert_eq!(lines, vec!["plain text".to_string()]);
}

#[test]
fn wrap_preserving_code_splits_after_consecutive_whitespace() {
    let lines = wrap_preserving_code("alpha  beta   gamma", 8);
    assert_eq!(
        lines,
        vec![
            "alpha  ".to_string(),
            "beta   ".to_string(),
            "gamma".to_string()
        ]
    );
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

fn citation_link_starts(expected_citation: &str) -> Vec<String> {
    let mut markers = Vec::new();
    let mut remaining = expected_citation;
    while let Some(open_index) = remaining.find('[') {
        let candidate = &remaining[open_index..];
        let Some(close_index) = candidate.find("](") else {
            break;
        };
        markers.push(candidate[..close_index + 2].to_string());
        remaining = &candidate[close_index + 2..];
    }
    markers
}

fn inline_citation_chain(citation_count: usize) -> String {
    let mut citation = String::new();
    for index in 1..=citation_count {
        write!(citation, "([{index}](https://example.com/ref{index}))")
            .expect("writing to String cannot fail");
    }
    citation
}

fn assert_inline_citation_invariants(lines: &[String], expected_citation: &str) {
    let citation_link_starts = citation_link_starts(expected_citation);
    assert!(
        !citation_link_starts.is_empty(),
        "expected citation fixture must contain at least one inline link",
    );
    assert!(
        lines.iter().any(|line| line.contains(expected_citation)),
        "expected citation to stay attached in {lines:?}",
    );
    assert!(
        lines.iter().all(|line| !line.ends_with('(')),
        "opening citation punctuation must not be stranded at line end: {lines:?}",
    );
    assert!(
        lines.iter().all(|line| {
            let trimmed = line.trim_start();
            citation_link_starts
                .iter()
                .all(|marker| !trimmed.starts_with(marker))
        }),
        "citation link must not start a continuation line: {lines:?}",
    );
    assert!(
        lines.iter().all(|line| line.trim() != ")("),
        "adjacent citation punctuation must not be orphaned: {lines:?}",
    );
}

#[rstest]
#[case(
    "The formatter keeps pattern([1](https://example.com/ref)) attached while wrapping.",
    32,
    "pattern([1](https://example.com/ref))"
)]
#[case(
    concat!(
        "The formatter keeps runtime([6](https://example.com/command))",
        "([7](https://example.com/event)) attached while wrapping."
    ),
    34,
    "runtime([6](https://example.com/command))([7](https://example.com/event))",
)]
fn wrap_preserving_code_keeps_inline_citation_links_attached(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected_citation: &str,
) {
    let lines = wrap_preserving_code(input, width);
    assert_inline_citation_invariants(&lines, expected_citation);
}

#[test]
fn wrap_preserving_code_snapshots_single_inline_citation() {
    let lines = wrap_preserving_code(
        "The formatter keeps pattern([1](https://example.com/ref)) attached while wrapping.",
        32,
    );
    insta::assert_snapshot!(
        lines.join("\n"),
        @r###"
The formatter keeps
pattern([1](https://example.com/ref))
attached while wrapping.
"###
    );
}

#[test]
fn wrap_preserving_code_snapshots_adjacent_inline_citations() {
    let lines = wrap_preserving_code(
        concat!(
            "The formatter keeps runtime([6](https://example.com/command))",
            "([7](https://example.com/event)) attached while wrapping."
        ),
        34,
    );
    insta::assert_snapshot!(
        lines.join("\n"),
        @r###"
The formatter keeps
runtime([6](https://example.com/command))([7](https://example.com/event))
attached while wrapping.
"###
    );
}

#[test]
fn wrap_preserving_code_glues_punctuation_after_code() {
    let lines = wrap_preserving_code("line with `code` !", 80);
    assert_eq!(lines, vec!["line with `code`!".to_string()]);
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
            "Extensions (`.toml`, `.json`,".to_string(),
            "`.json5`, `.yaml`, `.yml`).".to_string(),
        ]
    );
}

#[test]
fn wrap_preserving_code_retains_punctuation_after_separate_spans() {
    let text = "Alpha `code` `more`, trailing.";
    let lines = wrap_preserving_code(text, 18);
    assert_eq!(
        lines,
        vec!["Alpha `code`".to_string(), "`more`, trailing.".to_string(),]
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
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
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
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
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
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
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
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
    );
}

#[rstest]
#[case("Check this [link](foo.md)!?", 11, "[link](foo.md)!?", "!?")]
#[case("Reference [doc](bar.md):", 9, "[doc](bar.md):", ":")]
#[case("See [note](baz.md)...", 3, "[note](baz.md)...", "...")]
#[case("Alert [warn](warn.md);", 5, "[warn](warn.md);", ";")]
fn wrap_preserving_code_keeps_trailing_link_punctuation(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected_link: &str,
    #[case] orphaned_punctuation: &str,
) {
    let lines = wrap_preserving_code(input, width);
    assert!(lines.len() > 1, "expected wrapping for {input:?}");
    assert!(
        lines.iter().any(|line| line.contains(expected_link)),
        "expected {expected_link:?} to stay attached in {lines:?}",
    );
    assert!(
        lines.iter().all(|line| line.trim() != orphaned_punctuation),
        "punctuation was orphaned in {lines:?}",
    );
}

#[test]
fn wrap_preserving_code_handles_leading_link_punctuation() {
    let input = concat!(
        "\"[Quoted link](quote.md)\" is important for understanding the ",
        "overall design because it provides context to the guidelines."
    );
    let lines = wrap_preserving_code(input, 80);
    assert!(lines.len() > 1, "expected wrapping for {input:?}");
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("\"[Quoted link](quote.md)")),
        "expected leading punctuation to stay with link in {lines:?}",
    );
    assert!(
        lines.iter().all(|line| line.trim() != "\""),
        "leading punctuation was orphaned in {lines:?}",
    );
}

#[test]
fn wrap_preserving_code_handles_leading_and_trailing_link_punctuation() {
    let input = concat!(
        "\"[Link](foo.md)!\" demonstrates punctuation around a link and ",
        "includes plenty of extra words to exceed the wrapping limit."
    );
    let lines = wrap_preserving_code(input, 80);
    assert!(lines.len() > 1, "expected wrapping for {input:?}");
    assert!(
        lines.iter().any(|line| line.contains("[Link](foo.md)!\"")),
        "expected trailing punctuation to stay with link in {lines:?}",
    );
    assert!(
        lines.iter().all(|line| line.trim() != "\""),
        "leading punctuation was orphaned in {lines:?}",
    );
}
