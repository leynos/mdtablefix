//! `process_stream` tests covering inline-code coupling rules that depend on
//! the full processing pipeline rather than `wrap_text` alone.

use mdtablefix::process_stream;
use rstest::rstest;

#[test]
fn wrap_stream_couples_opening_paren_before_inline_code_in_list() {
    let input = lines_vec![concat!(
        "- `src/cli/mod.rs` (240 lines): defines the `Cli` struct with ",
        "`#[derive(Parser, Serialize, Deserialize, OrthoConfig)]`, its subcommands ",
        "(`Commands` enum), and the `parse_with_localizer_from` function that creates ",
        "a localized clap command and parses arguments."
    )];
    let output = process_stream(&input);
    for line in &output {
        assert!(
            !line.ends_with('('),
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
        assert!(
            !line.ends_with(opener),
            "opening bracket must stay with inline code on line: {line:?}"
        );
    }
}

/// Prose and an inline code span that together fill 78 of the 80 columns, so a
/// following bare bracket reference cannot fit on the line and must move as a
/// unit rather than leaving `[` behind (issue #504).
#[rstest]
#[case("[1]")]
#[case("[12]")]
fn wrap_stream_keeps_bracket_reference_whole(#[case] reference: &str) {
    let input = lines_vec![format!(
        concat!(
            "aaaaa aaaaaa aaaa aa aaaa aamw jkxf ht abm iqy uxqdkre fz ioelg ",
            "**bold**`code` {}"
        ),
        reference,
    )];
    let output = process_stream(&input);
    assert!(
        output.iter().any(|line| line.contains(reference)),
        "expected {reference:?} to stay whole in {output:?}",
    );
    assert!(
        output.iter().all(|line| !line.ends_with('[')),
        "opening bracket must not be stranded at line end: {output:?}",
    );
    assert!(
        output
            .iter()
            .all(|line| !line.trim_start().starts_with(']')),
        "reference tail must not be detached from its opener: {output:?}",
    );
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
