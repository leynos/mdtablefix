//! CLI wrapping option tests.
//!
//! Validates that the `--wrap` command-line flag correctly triggers text
//! wrapping behaviour when processing Markdown content through the `mdtablefix`
//! binary.

use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};
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
    insta::assert_snapshot!(
        "issue_329_wrap_fences_ellipsis_preserve_fenced_content",
        output
    );
    Ok(())
}

fn fence_marker_strategy() -> impl Strategy<Value = String> {
    (prop_oneof![Just('`'), Just('~')], 3usize..=6)
        .prop_map(|(marker, len)| std::iter::repeat_n(marker, len).collect())
}

fn fence_info_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just("sql".to_owned()),
        Just("json".to_owned()),
        Just("json payload".to_owned()),
        Just("{#example .sample}".to_owned()),
        printable_ascii_line_strategy(0..24),
    ]
}

fn printable_ascii_line_strategy(len: std::ops::Range<usize>) -> impl Strategy<Value = String> {
    prop::collection::vec(0x20u8..=0x7e, len)
        .prop_map(|bytes| bytes.into_iter().map(char::from).collect())
}

fn fence_like_line_strategy(marker: char, marker_len: usize) -> BoxedStrategy<String> {
    let opposite_marker = if marker == '`' { '~' } else { '`' };
    let opposite = (3usize..=8)
        .prop_map(move |len| std::iter::repeat_n(opposite_marker, len).collect::<String>());
    let shorter_same = if marker_len > 3 {
        (3usize..marker_len)
            .prop_map(move |len| std::iter::repeat_n(marker, len).collect::<String>())
            .boxed()
    } else {
        Just(format!("{marker}{marker}")).boxed()
    };

    prop_oneof![opposite, shorter_same].boxed()
}

fn closes_active_fence(line: &str, marker: char, marker_len: usize) -> bool {
    let trimmed = line.trim_start();
    let run_len = trimmed.chars().take_while(|ch| *ch == marker).count();
    run_len >= marker_len && trimmed.chars().nth(run_len).is_none_or(|ch| ch != marker)
}

fn fenced_body_strategy(marker: char, marker_len: usize) -> impl Strategy<Value = Vec<String>> {
    let arbitrary_line = printable_ascii_line_strategy(0..80)
        .prop_filter("body line must not close active fence", move |line| {
            !closes_active_fence(line, marker, marker_len)
        });
    prop::collection::vec(
        prop_oneof![
            Just("-- Payload example...".to_owned()),
            Just("{...}".to_owned()),
            Just("VALUES ('00000000-0000-0000-0000-000000000001', 'default');".to_owned()),
            fence_like_line_strategy(marker, marker_len),
            arbitrary_line.boxed(),
        ],
        1..=8,
    )
}

#[test]
fn combined_flags_preserve_generated_fenced_bodies() {
    let strategy = (0usize..=3, fence_marker_strategy(), fence_info_strategy()).prop_flat_map(
        |(indent, marker, info)| {
            let marker_char = marker
                .chars()
                .next()
                .expect("fence marker strategy emits a non-empty marker");
            let marker_len = marker.len();
            (
                Just(indent),
                Just(marker),
                Just(info),
                fenced_body_strategy(marker_char, marker_len),
            )
        },
    );
    let mut runner = TestRunner::new(Config {
        cases: 96,
        ..Config::default()
    });

    runner
        .run(&strategy, |(indent, marker, info, body_lines)| {
            let indent = " ".repeat(indent);
            let info_suffix = if info.is_empty() {
                String::new()
            } else {
                format!(" {info}")
            };
            let body = body_lines.join("\n");
            let input = format!("{indent}{marker}{info_suffix}\n{body}\n{indent}{marker}\n");
            let assertion = run_cli_with_stdin(ISSUE_329_COMBINED_FLAGS, &input)
                .map_err(|err| TestCaseError::fail(err.to_string()))?;
            let success = assertion.success();
            let output = String::from_utf8_lossy(&success.get_output().stdout);
            let output_lines = output.lines().collect::<Vec<_>>();
            let output_body = output_lines
                .get(1..output_lines.len().saturating_sub(1))
                .ok_or_else(|| TestCaseError::fail(format!("missing fence body:\n{output}")))?
                .join("\n");

            prop_assert_eq!(
                output_body,
                body,
                "fenced body changed after combined flags:\ninput:\n{}\noutput:\n{}",
                input,
                output
            );
            Ok(())
        })
        .expect("generated fenced bodies are preserved by combined flags");
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
