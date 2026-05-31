//! Unit tests for `wrap_text`.
//!
//! This module covers the core wrapping behaviour for prose and the regression
//! guards for issue `#261`, ensuring verbatim code blocks remain untouched.

#[macro_use]
#[path = "common/mod.rs"]
mod common;

use mdtablefix::{process_stream, wrap::wrap_text};
use rstest::rstest;
use unicode_width::UnicodeWidthStr;

fn assert_footnote_reference_is_intact(output: &[String], marker: &str) {
    let rendered = output.join("\n");
    assert!(rendered.contains(marker));
    assert!(!rendered.contains("[\n"));
    assert!(!rendered.contains("\n^"));
}

#[rstest]
#[case(
    lines_vec![
        "- Decision: Make `make kani` a Kani command smoke check using `cargo kani",
        "  --version` until real harnesses land.",
    ],
    "`cargo kani --version`"
)]
#[case(
    lines_vec![
        "1. Users select a theme via (`CLI >",
        "   environment > config file >",
        "   defaults`) parsing.",
    ],
    "(`CLI > environment > config file > defaults`)"
)]
fn wrap_text_joins_cross_line_code_spans(#[case] input: Vec<String>, #[case] expected: &str) {
    let rendered = wrap_text(&input, 80).join("\n");
    assert!(rendered.contains(expected));
}

#[rstest]
#[case("- Release `4.1.1", "  rc1` candidate.", "`4.1.1 rc1`", "`4.1.1` rc1")]
#[case("- Version `1.2", "  beta` works.", "`1.2 beta`", "`1.2` beta")]
fn wrap_text_joins_split_version_code_spans_without_inserting_fence(
    #[case] first: &str,
    #[case] second: &str,
    #[case] expected_span: &str,
    #[case] invalid_span: &str,
) {
    let input = lines_vec![first, second];
    let rendered = wrap_text(&input, 80).join("\n");
    assert!(
        rendered.contains(expected_span),
        "expected joined span {expected_span:?}, got: {rendered}"
    );
    assert!(
        !rendered.contains(invalid_span),
        "must not synthesise a closing fence before {invalid_span:?}, got: {rendered}"
    );
}

#[test]
fn wrap_text_joins_indented_ordered_list_code_span_continuation() {
    let input = lines_vec!["10. Use `cargo kani", "    --version` for smoke checks."];
    let rendered = wrap_text(&input, 80).join("\n");
    assert!(rendered.contains("`cargo kani"));
    assert!(rendered.contains("    --version` for smoke checks."));
    assert!(!rendered.contains("`cargo kani --version`"));
}

#[test]
fn wrap_text_preserves_hyphenated_words() {
    let input = lines_vec!["A word that is very-long-word indeed"];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(
        wrapped,
        lines_vec!["A word that is", "very-long-word", "indeed"]
    );
    assert_eq!(wrapped.join(" "), input[0]);
}

#[test]
fn wrap_text_breaks_between_space_separated_code_spans() {
    let input = lines_vec![concat!(
        "The file loader selects the parser based on the extension ",
        "(`.toml`, `.json`, `.json5`, `.yaml`, `.yml`). When the `json5` ",
        "feature is active, both `.json` and `.json5` files are parsed ",
        "using the JSON5 format."
    )];
    let wrapped = wrap_text(&input, 80);

    for line in &wrapped {
        assert!(
            UnicodeWidthStr::width(line.as_str()) <= 80,
            "line too wide ({} cols): {line:?}",
            UnicodeWidthStr::width(line.as_str())
        );
    }

    assert!(
        wrapped[0].ends_with("`.toml`,") || wrapped[0].ends_with("`.json`,"),
        "expected first line to break inside the code-span list, got: {:?}",
        wrapped[0]
    );
}

#[test]
fn wrap_text_does_not_insert_spaces_in_hyphenated_words() {
    let input = lines_vec![concat!(
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Donec tincidunt ",
        "elit-sed fermentum congue. Vivamus dictum nulla sed consectetur ",
        "volutpat."
    )];
    let wrapped = wrap_text(&input, 80);
    assert_eq!(
        wrapped,
        lines_vec![
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Donec tincidunt",
            "elit-sed fermentum congue. Vivamus dictum nulla sed consectetur volutpat."
        ]
    );
}

#[test]
fn wrap_text_preserves_code_spans() {
    let input = lines_vec![concat!(
        "with their own escaping rules. On Windows, scripts default to `powershell -Command` ",
        "unless the manifest's `interpreter` field overrides the setting."
    )];
    let wrapped = wrap_text(&input, 60);
    assert_eq!(
        wrapped,
        lines_vec![
            "with their own escaping rules. On Windows, scripts default",
            "to `powershell -Command` unless the manifest's",
            "`interpreter` field overrides the setting."
        ]
    );
}

#[test]
fn wrap_text_normalizes_whitespace_only_lines() {
    let input = vec![String::new(), "   ".to_string(), "\t\t".to_string()];
    let wrapped = wrap_text(&input, 80);
    assert_eq!(wrapped, vec![String::new(), String::new(), String::new()]);
}

#[test]
fn wrap_text_treats_whitespace_only_lines_as_paragraph_breaks() {
    let input = vec!["foo".to_string(), "   ".to_string(), "bar".to_string()];
    let wrapped = wrap_text(&input, 80);
    assert_eq!(
        wrapped,
        vec!["foo".to_string(), String::new(), "bar".to_string()]
    );
}

#[test]
fn wrap_text_uses_display_width_for_unicode_indent() {
    let input = vec!["　a b".to_string()];
    let wrapped = wrap_text(&input, 4);
    assert_eq!(wrapped, vec!["　a".to_string(), "　b".to_string()]);
}

#[test]
fn wrap_text_multiple_code_spans() {
    let input = lines_vec!["combine `foo bar` and `baz qux` in one line"];
    let wrapped = wrap_text(&input, 25);
    assert_eq!(
        wrapped,
        lines_vec!["combine `foo bar` and", "`baz qux` in one line"]
    );
}

#[test]
fn wrap_text_nested_backticks() {
    let input = lines_vec!["Use `` `code` `` to quote backticks"];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(
        wrapped,
        lines_vec!["Use `` `code` `` to", "quote backticks"]
    );
}

#[test]
fn wrap_text_unmatched_backticks() {
    let input = lines_vec!["This has a `dangling code span."];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(wrapped, lines_vec!["This has a", "`dangling code span."]);
}

#[test]
fn wrap_text_preserves_links() {
    let input = lines_vec![
        "`falcon-pachinko` is an extension library for the",
        "[Falcon](https://falcon.readthedocs.io) web framework. It adds a structured",
        "approach to asynchronous WebSocket routing and background worker integration."
    ];
    let wrapped = wrap_text(&input, 80);
    let joined = wrapped.join("\n");
    assert_eq!(joined.matches("https://").count(), 1);
    assert!(
        wrapped
            .iter()
            .any(|l| l.contains("https://falcon.readthedocs.io"))
    );
}

#[test]
fn wrap_text_keeps_trailing_spaces_for_blockquote_final_line() {
    // "> " is the prefix; available width = 10 - 2 = 8.
    let input = lines_vec!["> word1 word2  "];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(wrapped, lines_vec!["> word1", "> word2  "]);
}

#[test]
fn wrap_text_keeps_trailing_spaces_for_bullet_final_line() {
    // "- " is the prefix; continuation lines are indented with two spaces.
    let input = lines_vec!["- word1 word2  "];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(wrapped, lines_vec!["- word1", "  word2  "]);
}

#[test]
fn wrap_text_preserves_indented_hash_as_text() {
    let input = lines_vec!["Paragraph intro.", "    # code", "Continuation."];
    let wrapped = wrap_text(&input, 40);
    assert_eq!(input, wrapped);
}

#[test]
fn wrap_text_flushes_before_heading() {
    let input = lines_vec!["Paragraph intro.", "# Heading", "Continuation."];
    let wrapped = wrap_text(&input, 40);
    assert_eq!(input, wrapped);
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

    insta::assert_snapshot!(
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

    insta::assert_snapshot!(snapshot_name, wrapped.join("\n"));
}

/// Guards issue `#261` by asserting both fenced and four-space indented shell
/// blocks remain byte-identical after `wrap_text` processes surrounding
/// Markdown.
#[rstest]
#[case(vec![
    "## Verification".to_string(),
    String::new(),
    "```bash".to_string(),
    "set -o pipefail".to_string(),
    "make check-fmt 2>&1 | tee /tmp/fmt.log".to_string(),
    "make lint 2>&1 | tee /tmp/lint.log".to_string(),
    "make test 2>&1 | tee /tmp/test.log".to_string(),
    "```".to_string(),
])]
#[case(vec![
    "## Verification".to_string(),
    String::new(),
    "    set -o pipefail".to_string(),
    "    make check-fmt 2>&1 | tee /tmp/fmt.log".to_string(),
    "    make lint 2>&1 | tee /tmp/lint.log".to_string(),
    "    make test 2>&1 | tee /tmp/test.log".to_string(),
])]
fn wrap_text_preserves_shell_block_after_heading(#[case] input: Vec<String>) {
    assert_eq!(wrap_text(&input, 80), input);
}

/// Guards issue `#261` by asserting fenced shell blocks remain byte-identical
/// even when the heading is immediately followed by the opening fence.
#[test]
fn wrap_text_preserves_fenced_shell_block_without_blank_line_after_heading() {
    let input = lines_vec![
        "## Verification",
        "```bash",
        "set -o pipefail",
        "make check-fmt 2>&1 | tee /tmp/fmt.log",
        "make lint 2>&1 | tee /tmp/lint.log",
        "make test 2>&1 | tee /tmp/test.log",
        "```",
    ];

    assert_eq!(wrap_text(&input, 80), input);
}

#[test]
fn wrap_text_does_not_overflow_after_tail_rebalancing() {
    let input = lines_vec!["a four five"];
    let wrapped = wrap_text(&input, 6);

    assert_eq!(wrapped.join(" "), "a four five");
    assert!(
        wrapped
            .iter()
            .all(|line| UnicodeWidthStr::width(line.as_str()) <= 6)
    );
}

#[test]
fn wrap_stream_couples_opening_paren_before_inline_code_in_list() {
    let input = lines_vec![concat!(
        "- `src/cli/mod.rs` (240 lines): defines the `Cli` struct with ",
        "`#[derive(Parser, Serialize, Deserialize, OrthoConfig)]`, its subcommands ",
        "(`Commands` enum), and the `parse_with_localizer_from` function that creates ",
        "a localized clap command and parses arguments."
    )];
    let output = process_stream(&input);
    for window in output.windows(2) {
        assert!(
            !window[0].ends_with('('),
            "opening parenthesis must not be stranded at line end: {output:?}"
        );
    }
}

#[rstest]
#[case("(`code`)", '(')]
#[case("[`code`]", '[')]
#[case("（`code`）", '（')]
#[case("「`code`」", '「')]
fn wrap_stream_keeps_opening_bracket_with_inline_code_in_list(
    #[case] fragment: &str,
    #[case] opener: char,
) {
    let input = lines_vec![format!(
        concat!(
            "- Leading prose that is long enough to force wrapping before {} and ",
            "trailing text."
        ),
        fragment,
    )];
    let output = process_stream(&input);
    for line in &output {
        if line.contains('`') {
            assert!(
                !line.ends_with(opener),
                "opening bracket must stay with inline code on line: {line:?}"
            );
        }
    }
}

#[test]
fn wrap_stream_future_attribute_punctuation() {
    let input = lines_vec![concat!(
        "- Test function (`#[awt]`) or a specific `#[future]` argument ",
        "(`#[future(awt)]`), tells `rstest` to automatically insert `.await` ",
        "calls for those futures."
    )];
    let output = process_stream(&input);
    assert_eq!(
        output,
        lines_vec![
            "- Test function (`#[awt]`) or a specific `#[future]` argument",
            "  (`#[future(awt)]`), tells `rstest` to automatically insert `.await` calls for",
            "  those futures."
        ]
    );
}
