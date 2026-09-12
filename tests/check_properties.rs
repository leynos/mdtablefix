//! Properties of the reporting domain: line counting, agreement between
//! byte-equality and the delta, and formatter idempotence.
//!
//! Idempotence is driven through the real binary rather than a reimplemented
//! formatting closure, so the property covers the exact transform the CLI
//! applies, including the flag composition `src/main.rs` builds.
//!
//! The general idempotence suites are `tests/idempotence.rs` and
//! `tests/idempotence_properties.rs`, whose generator samples document
//! *structure*. The idempotence cases here are deliberately narrowed to the
//! document *boundary* — the line-ending style, byte-order mark, and trailing
//! terminator a document was authored with — because that is the dimension
//! those suites do not sample, and the boundary this plan changed.

use std::fs;

use assert_cmd::Command;
use mdtablefix::report::LineDelta;
use proptest::prelude::*;
use rstest::rstest;
use tempfile::tempdir;

/// The eight transform flags the CLI exposes.
const FLAGS: [&str; 8] = [
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
    "--footnotes",
    "--code-emphasis",
    "--headings",
];

/// Documents that drift under exactly one flag each, plus clean and
/// boundary-shaped inputs. `--wrap` drifts the prose, `--renumber` the list,
/// and so on, so every singleton flag observes a change and the idempotence
/// assertion cannot pass vacuously.
const CORPUS: &[(&str, &str)] = &[
    ("ragged_table", "|A|B|\n|---|---|\n|1|2|\n"),
    (
        "clean_table",
        "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n",
    ),
    (
        "prose",
        "one two three four five six seven eight nine ten eleven twelve\n",
    ),
    ("ordered_list", "1. alpha\n1. beta\n1. gamma\n"),
    ("thematic_break", "alpha\n\n***\n\nbeta\n"),
    ("ellipsis", "alpha ... beta\n"),
    ("long_fence", "````sh\necho hi\n````\n"),
    ("footnotes", "alpha[1]\n\n1. beta\n"),
    ("code_emphasis", "alpha *`beta`* gamma\n"),
    ("setext_heading", "Title\n=====\n"),
    ("crlf_table", "|A|B|\r\n|---|---|\r\n|1|2|\r\n"),
    ("bom_table", "\u{FEFF}|A|B|\n|---|---|\n|1|2|\n"),
    ("unterminated_table", "|A|B|\n|---|---|\n|1|2|"),
    ("empty", ""),
];

/// Counts lines the way `similar` tokenizes them: `\n`, `\r\n`, and a lone
/// `\r` each terminate a line, and a non-empty unterminated tail is one line.
fn token_count(text: &str) -> usize {
    let line_feeds = text.matches('\n').count();
    let lone_carriage_returns = text
        .match_indices('\r')
        .filter(|(index, _)| !text[index + 1..].starts_with('\n'))
        .count();
    let unterminated =
        usize::from(!text.is_empty() && !text.ends_with('\n') && !text.ends_with('\r'));
    line_feeds + lone_carriage_returns + unterminated
}

/// Runs the binary over `input` in stdout mode and returns what it printed.
fn format_with_cli(input: &str, flags: &[&str]) -> String {
    let dir = tempdir().expect("temporary directory");
    let path = dir.path().join("input.dat");
    fs::write(&path, input).expect("write input");
    let stdout = Command::cargo_bin("mdtablefix")
        .expect("cargo binary")
        .args(flags)
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(stdout).expect("stdout is UTF-8")
}

/// A pair of texts related by one line-level edit.
///
/// INV-AGREE's domain is pairs that *nearly* agree, so the pair has to be
/// derived from one document rather than drawn twice. Two independently random
/// strings share a line only by accident, and both assertions below then hold
/// for any diff at all: the conservation law follows from the pairing the diff
/// performs, and the byte comparison follows from the strings being distinct.
/// Deriving the second text makes equal lines common, which is what gives the
/// two assertions something to fail on.
fn related_texts() -> impl Strategy<Value = (String, String)> {
    (markdown_document(), 0..=5u8).prop_map(|(document, edit)| {
        let formatted = match edit {
            // Byte-equal, so the agreement assertion is exercised on a clean
            // pair rather than only on a drifting one.
            0 => document.clone(),
            1 => document.replace('\n', "\r\n"),
            2 => document.trim_end_matches('\n').to_string(),
            3 => format!("{document}extra line\n"),
            4 => document.replace("prose words here", "prose"),
            _ => document.replacen('\n', "\r\n", 1),
        };
        (document, formatted)
    })
}

/// Independent random pairs widen the domain; related pairs make the two
/// assertions falsifiable. Both shapes are needed, so both are sampled.
fn text_pairs() -> impl Strategy<Value = (String, String)> {
    prop_oneof![
        1 => (any::<String>(), any::<String>()),
        3 => related_texts(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// LEM-COUNT's conservation law, with INV-AGREE folded in.
    #[test]
    fn count_conserves_tokens_and_agrees_with_byte_equality(
        (original, formatted) in text_pairs(),
    ) {
        let delta = LineDelta::between(&original, &formatted);
        let before = token_count(&original).cast_signed();
        let after = token_count(&formatted).cast_signed();
        prop_assert_eq!(
            after,
            before + delta.insertions().cast_signed() - delta.deletions().cast_signed(),
            "tokens are conserved: after = before + insertions - deletions"
        );
        prop_assert_eq!(
            delta.has_changes(),
            original != formatted,
            "the delta is non-zero exactly when the bytes differ"
        );
    }
}

/// Expected counts captured with
/// `git diff --numstat --no-index <before> <after>`.
#[rstest]
#[case::pure_insertion(
    include_str!("data/numstat/pure_insertion.before.dat"),
    include_str!("data/numstat/pure_insertion.after.dat"),
    1,
    0
)]
#[case::pure_deletion(
    include_str!("data/numstat/pure_deletion.before.dat"),
    include_str!("data/numstat/pure_deletion.after.dat"),
    0,
    1
)]
#[case::table_replacement(
    include_str!("data/numstat/table_replacement.before.dat"),
    include_str!("data/numstat/table_replacement.after.dat"),
    3,
    3
)]
#[case::long_file_single_line(
    include_str!("data/numstat/long_file_single_line.before.dat"),
    include_str!("data/numstat/long_file_single_line.after.dat"),
    1,
    1
)]
fn count_matches_git_numstat(
    #[case] before: &str,
    #[case] after: &str,
    #[case] insertions: usize,
    #[case] deletions: usize,
) {
    let delta = LineDelta::between(before, after);
    assert_eq!(
        (delta.insertions(), delta.deletions()),
        (insertions, deletions)
    );
}

#[test]
fn count_handles_empty_and_identical_texts() {
    let inserted = LineDelta::between("", "alpha\n");
    assert_eq!((inserted.insertions(), inserted.deletions()), (1, 0));
    let deleted = LineDelta::between("alpha\n", "");
    assert_eq!((deleted.insertions(), deleted.deletions()), (0, 1));
    let identical = LineDelta::between("alpha\n", "alpha\n");
    assert_eq!((identical.insertions(), identical.deletions()), (0, 0));
    assert!(!identical.has_changes());
}

#[rstest]
#[case::same_bytes("alpha\nbeta\n", "alpha\nbeta\n", false)]
#[case::line_endings_only("alpha\nbeta\n", "alpha\r\nbeta\r\n", true)]
#[case::trailing_newline_only("alpha\nbeta\n", "alpha\nbeta", true)]
fn agree_reports_drift_exactly_when_bytes_differ(
    #[case] original: &str,
    #[case] formatted: &str,
    #[case] expected: bool,
) {
    assert_eq!(
        LineDelta::between(original, formatted).has_changes(),
        expected
    );
}

/// Every singleton flag and the full set must reach a fixed point, and each
/// must observe at least one drifting document in `CORPUS`.
///
/// `CORPUS` is boundary-heavy on purpose: the CRLF, byte-order-marked, and
/// unterminated documents are the ones the general suite does not generate,
/// and the ones whose second pass is most likely to differ from their first.
#[rstest]
#[case(&[])]
#[case(&["--wrap"])]
#[case(&["--renumber"])]
#[case(&["--breaks"])]
#[case(&["--ellipsis"])]
#[case(&["--fences"])]
#[case(&["--footnotes"])]
#[case(&["--code-emphasis"])]
#[case(&["--headings"])]
#[case(&[
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
    "--footnotes",
    "--code-emphasis",
    "--headings",
])]
fn formatting_is_idempotent(#[case] flags: &[&str]) {
    let mut drifted = 0;
    for (name, input) in CORPUS {
        let once = format_with_cli(input, flags);
        let twice = format_with_cli(&once, flags);
        assert_eq!(twice, once, "{name} must be a fixed point under {flags:?}");
        if once != *input {
            drifted += 1;
        }
    }
    assert!(
        drifted > 0,
        "no corpus document drifted under {flags:?}, so the case is vacuous"
    );
}

/// A line-level generator mixing tables, prose, lists, and fences.
fn markdown_lines() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("|A|B|"),
        Just("|---|---|"),
        Just("|1|2|"),
        Just(""),
        Just("prose words here"),
        Just("1. item"),
        Just("***"),
        Just("```sh"),
        Just("echo hi"),
        Just("```"),
        Just("---"),
        Just("Title"),
    ]
}

/// A whole document with a sampled ending style, optional byte-order mark,
/// and optional final terminator.
fn markdown_document() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(markdown_lines(), 0..12),
        prop_oneof![Just("\n"), Just("\r\n")],
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(lines, ending, bom, terminated)| {
            let mut text = String::new();
            if bom {
                text.push('\u{FEFF}');
            }
            for (index, line) in lines.iter().enumerate() {
                text.push_str(line);
                if index + 1 < lines.len() || terminated {
                    text.push_str(ending);
                }
            }
            text
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// INV-IDEMPOTENT over generated documents and a sampled flag subset.
    ///
    /// The generator samples the document boundary — ending style,
    /// byte-order mark, and final terminator — over trivial content, which is
    /// the complement of `tests/idempotence_properties.rs` rather than a
    /// repetition of it.
    #[test]
    fn generated_documents_reach_a_fixed_point(
        document in markdown_document(),
        mask in any::<u8>(),
    ) {
        let flags: Vec<&str> = FLAGS
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, flag)| *flag)
            .collect();
        let once = format_with_cli(&document, &flags);
        let twice = format_with_cli(&once, &flags);
        prop_assert_eq!(twice, once, "document must be a fixed point under {:?}", flags);
    }
}
