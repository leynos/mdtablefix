//! `wrap_text` regression tests covering code spans, link preservation,
//! whitespace normalization, and basic overflow guards.

use mdtablefix::wrap::{Token, tokenize_markdown, wrap_text};
use proptest::prelude::*;
use rstest::rstest;
use unicode_width::UnicodeWidthStr;

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

#[test]
fn wrap_text_preserves_unicode_code_span_authored_break_as_a_fixed_point() {
    let input = lines_vec!["é `abcd", "efgh` x"];
    let wrapped = wrap_text(&input, 7);

    // The authored break *inside* the overlong span survives as the seam
    // between "`abcd" and "efgh`". The prose on either side does not stay
    // attached, because re-attaching it would exceed the width; the
    // documented contract retains span boundaries, not surrounding prose.
    assert_eq!(wrapped, lines_vec!["é", "`abcd", "efgh`", "x"]);
    assert!(
        wrapped
            .iter()
            .all(|line| UnicodeWidthStr::width(line.as_str()) <= 7)
    );
    assert_eq!(wrap_text(&wrapped, 7), wrapped);
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
fn wrap_text_oversized_code_span_stays_intact() {
    let code_span = concat!(
        "`fn find_interrupted_session(base_dir: &Utf8Path, owner: &str, ",
        "repository: &str, pr_number: u64) -> Result<Option<SessionState>, IntakeError>`",
    );
    let input = lines_vec![format!("1. Implement {code_span} now.")];
    let wrapped = wrap_text(&input, 80);

    assert!(wrapped.join("\n").contains(code_span));
    assert!(wrapped.iter().all(|line| !line.trim().is_empty()));
    assert!(wrapped.iter().skip(1).all(|line| line.starts_with("   ")));
}

#[rstest]
#[case::escaped_option(r"`Ensure the manifest exists or pass \`--file\` with the correct path.`")]
#[case::escaped_inner_word(r"`word.\`inner\`.rest`")]
fn tokenize_markdown_keeps_escaped_backtick_code_span_atomic(#[case] span: &str) {
    let tokens = tokenize_markdown(span);

    assert_eq!(
        tokens,
        vec![Token::Code {
            raw: span,
            fence: "`",
            code: &span[1..span.len() - 1],
        }]
    );
}

/// Escaped backtick code spans must survive wrapping intact, whether they
/// appear inside a list item or a paragraph. The `contains` check guards the
/// atomic span specifically, while the snapshot guards the complete
/// line-breaking structure. The explicit snapshot names match the committed
/// `.snap` fixtures so they are reused regardless of the test function name.
#[rstest]
#[case::list_item(
    concat!(
        r"- Message: `Ensure the manifest exists or pass \`--file\` with the correct path.` ",
        "The docs should pin that wording."
    ),
    r"`Ensure the manifest exists or pass \`--file\` with the correct path.`",
    "wrap_text_keeps_escaped_backtick_code_span_atomic_in_list_item"
)]
#[case::paragraph(
    concat!(
        r"Document `word.\`inner\`.rest` carefully because wrapping near the ",
        "line boundary must keep the inline code span intact."
    ),
    r"`word.\`inner\`.rest`",
    "wrap_text_keeps_escaped_backtick_code_span_atomic_in_paragraph"
)]
fn wrap_text_keeps_escaped_backtick_code_span_atomic(
    #[case] source: &str,
    #[case] expected_span: &str,
    #[case] snapshot_name: &str,
) {
    let input = lines_vec![source];
    let wrapped = wrap_text(&input, 80);
    let rendered = wrapped.join("\n");

    // Structural guard: the atomic span must survive wrapping.
    assert!(
        rendered.contains(expected_span),
        "expected atomic span {expected_span:?} to survive wrapping, got: {rendered}"
    );
    assert!(wrapped.iter().all(|line| line.width() <= 80));

    // Snapshot: verifies complete line-breaking structure.
    insta::with_settings!(
        {
            snapshot_path => "../../tests/snapshots",
            prepend_module_to_snapshot => false,
        },
        {
            insta::assert_snapshot!(snapshot_name, rendered);
        }
    );
}

#[test]
fn test_tokenize_backslash_terminated_code_span() {
    let tokens = tokenize_markdown(r"Install to `C:\path\bin\` and add");
    let code_tokens = tokens
        .iter()
        .filter(|token| matches!(token, Token::Code { .. }))
        .collect::<Vec<_>>();

    assert_eq!(code_tokens.len(), 1);
    assert!(matches!(
        code_tokens[0],
        Token::Code {
            raw: r"`C:\path\bin\`",
            ..
        }
    ));
}

#[test]
fn tokenize_markdown_backslash_terminated_span_not_swallowed_by_later_literal_fence() {
    let tokens = tokenize_markdown(r"`C:\path\bin\` and then run `cmd`");
    let code_tokens = tokens
        .iter()
        .filter(|token| matches!(token, Token::Code { .. }))
        .collect::<Vec<_>>();

    assert_eq!(code_tokens.len(), 2);
    assert!(matches!(
        code_tokens[0],
        Token::Code {
            raw: r"`C:\path\bin\`",
            ..
        }
    ));
    assert!(matches!(code_tokens[1], Token::Code { raw: "`cmd`", .. }));
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
    assert_eq!(wrapped.join(" "), input[0]);
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

proptest! {
    /// A wrapped overlong Unicode code span must never come back changed by a
    /// second pass, must always respect the requested width, and must keep the
    /// authored break *inside* the span as its seam.
    ///
    /// The claim is deliberately narrower than "the whole input is a fixed
    /// point". `wrap_text` retains authored boundaries inside an overlong span,
    /// but re-attaches surrounding prose only when it still fits; asserting the
    /// wider claim fails for every input where the prose cannot be re-attached.
    #[test]
    fn prop_wrap_text_preserves_unicode_overlong_code_span_boundaries(
        left in "\\p{L}{1,8}",
        right in "\\p{L}{1,8}",
    ) {
        let input = vec![
            format!("é `é{left}"),
            format!("界{right}` x"),
        ];
        let width = input
            .iter()
            .map(|line| UnicodeWidthStr::width(line.as_str()))
            .max()
            .expect("the generated source always has two lines");
        let code_span = format!("`é{left} 界{right}`");
        let code_span_width = UnicodeWidthStr::width(code_span.as_str());

        prop_assert!(
            code_span_width > width,
            "generated code span width {code_span_width} must exceed line width {width}"
        );

        let output = wrap_text(&input, width);
        prop_assert!(
            output
                .iter()
                .all(|line| UnicodeWidthStr::width(line.as_str()) <= width),
            "output must respect width {width}: {output:?}"
        );
        prop_assert_eq!(wrap_text(&output, width), output.clone());

        // The authored break is the boundary between "`é{left}" and
        // "界{right}`". It survives as a line seam even though the prose around
        // it is re-wrapped independently.
        let first_piece = format!("`é{left}");
        let second_piece = format!("界{right}`");
        let seam = output.iter().position(|line| line == &first_piece);
        prop_assert!(
            matches!(seam, Some(index) if output.get(index + 1) == Some(&second_piece)),
            "authored span seam {first_piece:?}/{second_piece:?} must be adjacent in {output:?}"
        );
    }
}
