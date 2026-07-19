//! CLI wrapping option tests.
//!
//! Validates that the `--wrap` command-line flag correctly triggers text
//! wrapping behaviour when processing Markdown content through the `mdtablefix`
//! binary.

use proptest::prelude::*;
use rstest::rstest;

use super::cli_stdin::run_cli_with_stdin;

const ISSUE_329_COMBINED_FLAGS: &[&str] =
    &["--wrap", "--renumber", "--breaks", "--ellipsis", "--fences"];

#[test]
fn test_cli_wrap_option() -> Result<(), Box<dyn std::error::Error>> {
    let input = "This line is deliberately made much longer than eighty columns so that the \
                 wrapping algorithm is forced to insert a soft line-break somewhere in the middle \
                 of the paragraph when the --wrap flag is supplied.";
    let assertion = run_cli_with_stdin(&["--wrap"], &format!("{input}\n"))?;
    let success = assertion.success();
    let text = String::from_utf8_lossy(&success.get_output().stdout);
    assert!(
        text.lines().count() > 1,
        "expected wrapped output on multiple lines"
    );
    assert!(text.lines().all(|l| l.len() <= 80));
    Ok(())
}

#[test]
fn test_cli_wrap_rejects_parameter() -> Result<(), Box<dyn std::error::Error>> {
    run_cli_with_stdin(&["--wrap=80"], "alpha beta\n")?
        .failure()
        .stderr(predicates::str::contains("unexpected value '80'"));
    Ok(())
}

/// Verifies `--wrap` reflows Markdown paragraphs while respecting inline code spans.
#[rstest(
    paragraph,
    expected_lines,
    case::standard(
        concat!(
            "This paragraph demonstrates how reflow respects inline code while ensuring the ",
            "entire `mdtablefix --wrap --columns 80` invocation remains intact when crossing ",
            "the boundary for readability in documentation examples.",
        ),
        &[
            "This paragraph demonstrates how reflow respects inline code while ensuring the",
            "entire `mdtablefix --wrap --columns 80` invocation remains intact when crossing",
            "the boundary for readability in documentation examples.",
        ],
    ),
    case::bulleted(
        concat!(
            "- This bullet demonstrates how reflow respects inline code while ensuring the ",
            "entire `mdtablefix --wrap --columns 80` invocation stays intact when crossing ",
            "the boundary for documentation readability.",
        ),
        &[
            "- This bullet demonstrates how reflow respects inline code while ensuring the",
            "  entire `mdtablefix --wrap --columns 80` invocation stays intact when crossing",
            "  the boundary for documentation readability.",
        ],
    ),
    case::numbered(
        concat!(
            "1. This numbered example demonstrates how reflow respects inline code while ensuring the ",
            "entire `mdtablefix --wrap --columns 80` invocation stays intact when crossing ",
            "the boundary for documentation readability.",
        ),
        &[
            "1. This numbered example demonstrates how reflow respects inline code while",
            "   ensuring the entire `mdtablefix --wrap --columns 80` invocation stays intact",
            "   when crossing the boundary for documentation readability.",
        ],
    ),
)]
fn test_cli_wrap_reflows_markdown(
    paragraph: &str,
    expected_lines: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut input = paragraph.to_owned();
    input.push('\n');
    let assertion = run_cli_with_stdin(&["--wrap"], &input)?;
    let success = assertion.success();
    let output = String::from_utf8_lossy(&success.get_output().stdout);
    assert!(
        output.ends_with('\n'),
        "expected wrapped output to retain trailing newline",
    );
    assert_eq!(output.lines().collect::<Vec<_>>(), expected_lines);
    Ok(())
}

#[rstest]
#[case::single(
    concat!(
        "This paragraph keeps pattern([1](https://github.com/leynos/mdtablefix/pull/url)) ",
        "attached while the command-line wrapper reflows surrounding prose."
    ),
    "pattern([1](https://github.com/leynos/mdtablefix/pull/url))",
)]
#[case::adjacent(
    concat!(
        "This paragraph keeps pattern([1](https://github.com/leynos/mdtablefix/pull/url))",
        "([2](https://github.com/leynos/mdtablefix/issues/325)) attached while wrapping."
    ),
    concat!(
        "pattern([1](https://github.com/leynos/mdtablefix/pull/url))",
        "([2](https://github.com/leynos/mdtablefix/issues/325))"
    ),
)]
fn test_cli_wrap_keeps_inline_citation_links_attached(
    #[case] paragraph: &str,
    #[case] expected_citation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let assertion = run_cli_with_stdin(&["--wrap"], &format!("{paragraph}\n"))?;
    let success = assertion.success();
    let output = String::from_utf8_lossy(&success.get_output().stdout);
    let lines = output.lines().collect::<Vec<_>>();
    let link_starts = citation_link_starts(expected_citation);

    assert!(
        output.contains(expected_citation),
        "expected citation to stay attached in CLI output: {output}",
    );
    assert!(
        lines.iter().all(|line| !line.ends_with('(')),
        "opening citation punctuation must not be stranded: {lines:?}",
    );
    assert!(
        lines.iter().all(|line| {
            let trimmed = line.trim_start();
            link_starts
                .iter()
                .all(|marker| !trimmed.starts_with(marker))
        }),
        "citation link must not start a continuation line: {lines:?}",
    );
    assert!(
        lines.iter().all(|line| line.trim() != ")("),
        "adjacent citation punctuation must not be orphaned: {lines:?}",
    );
    Ok(())
}

/// Extracts link-start markers from `expected_citation`.
///
/// Given an `expected_citation: &str`, this helper returns a `Vec<String>` of
/// derived marker prefixes such as `"[1]("`. The CLI citation tests use those
/// markers for dynamic assertions instead of hard-coded citation text, avoiding
/// false negatives when citation content or ordering changes.
fn citation_link_starts(expected_citation: &str) -> Vec<String> {
    let mut markers = Vec::new();
    let mut remaining = expected_citation;
    while let Some(start) = remaining.find('[') {
        let after_start = &remaining[start..];
        if let Some(end) = after_start.find("](") {
            markers.push(after_start[..end + 2].to_owned());
        }
        remaining = &after_start[1..];
    }
    markers
}

/// Ensures `--wrap` preserves an explicit language specifier on fences.
#[test]
fn test_cli_wrap_preserves_language() -> Result<(), Box<dyn std::error::Error>> {
    let input = "```rust\nfn main() {}\n```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Accepts an optional space between the fence marker and language.
#[test]
fn test_cli_wrap_preserves_language_with_space() -> Result<(), Box<dyn std::error::Error>> {
    let input = "``` rust\nfn main() {}\n```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Validates handling of opening fences without language specifiers.
#[test]
fn test_cli_wrap_preserves_plain_fence() -> Result<(), Box<dyn std::error::Error>> {
    let input = "```\ncode\n```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Ensures `--wrap` preserves indented fenced code blocks.
#[test]
fn test_cli_wrap_preserves_indented_fence() -> Result<(), Box<dyn std::error::Error>> {
    let input = "    ```rust\n    fn main() {}\n    ```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Ensures `--wrap` preserves tildes as fence markers with language.
#[test]
fn test_cli_wrap_preserves_tilde_fence_with_language() -> Result<(), Box<dyn std::error::Error>> {
    let input = "~~~python\nprint('Hello, world!')\n~~~\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Ensures `--wrap` preserves tildes as fence markers without language.
#[test]
fn test_cli_wrap_preserves_tilde_fence_without_language() -> Result<(), Box<dyn std::error::Error>>
{
    let input = "~~~\nno language here\n~~~\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Opening with four backticks should ignore inner triple backticks.
#[test]
fn test_cli_wrap_preserves_four_backticks_and_ignores_inner_triple()
-> Result<(), Box<dyn std::error::Error>> {
    let input = "````rust\n```\nfn main() {}\n````\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Retains extended info strings including attributes and options.
#[test]
fn test_cli_wrap_preserves_extended_info_string() -> Result<(), Box<dyn std::error::Error>> {
    let input = "``` rust linenums {style=monokai}\ncode\n```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Accepts four or more tildes as fence markers.
#[test]
fn test_cli_wrap_preserves_tilde_with_four_markers() -> Result<(), Box<dyn std::error::Error>> {
    let input = "~~~~python\nprint('hi')\n~~~~\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Ensures `--wrap` preserves inline code spans with embedded quotes.
#[test]
fn test_cli_wrap_preserves_inline_code_span_with_quotes() -> Result<(), Box<dyn std::error::Error>>
{
    let input = concat!(
        r#"- **Imperative (Avoid):** `When I type "user@example.com" into the "email"
  field and click the "submit" button` A declarative style describes the user's
  intent and the system's behaviour—the "what." It abstracts away the
  implementation details.[^18]
"#,
        r#"- **Declarative (Prefer):** `When the user logs in with valid credentials`
"#,
    );
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    let success = assertion.success();
    let output = String::from_utf8_lossy(&success.get_output().stdout);
    assert!(
        output.contains("user@example.com"),
        "email literal must remain intact after wrapping"
    );
    assert!(
        output.contains("field and click the \"submit\" button"),
        "submit step text must remain intact after wrapping"
    );
    assert!(
        output.contains("`When the user logs in with valid credentials`"),
        "second inline code span must remain intact"
    );
    Ok(())
}

/// Protects issue `#329`: combined format flags must not rewrite fenced content.
#[test]
fn test_cli_wrap_fences_ellipsis_preserve_fenced_content() -> Result<(), Box<dyn std::error::Error>>
{
    let input = include_str!("../data/issue_329_wrap_fences_ellipsis_input.txt");
    let assertion = run_cli_with_stdin(ISSUE_329_COMBINED_FLAGS, input)?;
    let success = assertion.success();
    let output = String::from_utf8_lossy(&success.get_output().stdout);
    insta::with_settings!({
        snapshot_path => "../snapshots",
    }, {
        insta::assert_snapshot!(
            "issue_329_wrap_fences_ellipsis_preserve_fenced_content",
            output
        );
    });
    Ok(())
}

#[test]
fn combined_flags_preserve_generated_fenced_bodies() {
    let strategy = super::cli_issue_329_property::fenced_block_strategy();
    let mut runner = super::cli_issue_329_property::fenced_block_runner();

    runner
        .run(&strategy, |block| {
            let assertion = run_cli_with_stdin(ISSUE_329_COMBINED_FLAGS, &block.input)
                .map_err(|err| TestCaseError::fail(err.to_string()))?;
            let success = assertion.success();
            let output = String::from_utf8_lossy(&success.get_output().stdout);
            let output_lines = output.lines().collect::<Vec<_>>();

            let located = super::cli_issue_329_property::locate_fenced_block(&output_lines, &block)
                .map_err(TestCaseError::fail)?;
            let opening_without_indent = located
                .opening_line
                .strip_prefix(&block.indent)
                .ok_or_else(|| TestCaseError::fail("opening fence indentation changed"))?;
            let expected_info_suffix = if block.info_suffix.trim().is_empty() {
                ""
            } else {
                &block.info_suffix
            };
            prop_assert!(
                opening_without_indent.ends_with(expected_info_suffix),
                "opening fence info string changed after combined flags:\ninput:\n{}\noutput:\n{}",
                block.input,
                output
            );
            let marker_len = opening_without_indent.len() - expected_info_suffix.len();
            let output_marker = &opening_without_indent[..marker_len];
            let marker_char = output_marker
                .chars()
                .next()
                .ok_or_else(|| TestCaseError::fail("opening fence marker missing"))?;
            prop_assert!(
                matches!(marker_char, '`' | '~')
                    && marker_len >= 3
                    && output_marker.chars().all(|ch| ch == marker_char),
                "opening fence marker changed to an invalid delimiter:\ninput:\n{}\noutput:\n{}",
                block.input,
                output
            );
            let expected_closing = format!("{}{}", block.indent, output_marker);
            prop_assert!(
                located.closing_line == expected_closing,
                "closing fence line does not match output opening marker:\ninput:\n{}\noutput:\n{}",
                block.input,
                output
            );

            let output_body = located.body_lines.join("\n");

            prop_assert_eq!(
                output_body,
                block.body,
                "fenced body changed after combined flags:\ninput:\n{}\noutput:\n{}",
                block.input,
                output
            );
            Ok(())
        })
        .expect("generated fenced blocks are preserved by combined flags");
}

/// Whitespace-only fence suffixes are absent info strings and normalize away.
#[test]
fn combined_flags_normalize_whitespace_only_fence_suffix() -> Result<(), Box<dyn std::error::Error>>
{
    let input = "```  \n-- Payload example...\n```\n";
    let assertion = run_cli_with_stdin(ISSUE_329_COMBINED_FLAGS, input)?;
    assertion
        .success()
        .stdout("```\n-- Payload example...\n```\n");
    Ok(())
}

/// Ensures `--wrap` preserves emphasised step definition guidance with inline code spans.
#[test]
fn test_cli_wrap_preserves_step_definitions_guidance() -> Result<(), Box<dyn std::error::Error>> {
    let input = "- **Step Definitions:** Mirror the feature file structure in your `tests/steps/` \
                 directory.\n  Create a Rust module for each feature area (e.g., \
                 `tests/steps/authentication_steps.rs`,\n  `tests/steps/catalog_steps.rs`). This \
                 prevents having a single, massive step definition file\n  and makes it easier to \
                 find the code corresponding to a Gherkin step.\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    let success = assertion.success();
    let output = String::from_utf8_lossy(&success.get_output().stdout);
    for snippet in [
        "`tests/steps/`",
        "`tests/steps/authentication_steps.rs`",
        "`tests/steps/catalog_steps.rs`",
    ] {
        assert!(
            output.contains(snippet),
            "missing inline code span: {snippet}"
        );
    }
    Ok(())
}

/// Guards issue `#354` end-to-end: `mdtablefix --wrap` must keep an inline code
/// span that contains escaped backticks atomic when wrapping a list item, never
/// splitting it across lines and preserving the escaped backticks verbatim.
#[test]
fn test_cli_wrap_keeps_escaped_backtick_code_span_atomic_in_list_item()
-> Result<(), Box<dyn std::error::Error>> {
    let input = concat!(
        r"- Message: `Ensure the manifest exists or pass \`--file\` with the correct path.` ",
        "The docs should pin that wording.\n",
    );
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    let success = assertion.success();
    let output = String::from_utf8_lossy(&success.get_output().stdout);

    let span = r"`Ensure the manifest exists or pass \`--file\` with the correct path.`";
    // A contiguous, single-line match proves the span was never split across a
    // line break during wrapping.
    assert!(
        output.lines().any(|line| line.contains(span)),
        "escaped-backtick code span must stay atomic on one line: {output}",
    );
    // The list item is long enough to force wrapping onto several lines.
    assert!(
        output.lines().count() > 1,
        "expected the list item to wrap onto multiple lines: {output}",
    );
    assert!(
        output.lines().all(|line| line.len() <= 80),
        "every wrapped line must fit the default width: {output}",
    );
    Ok(())
}
