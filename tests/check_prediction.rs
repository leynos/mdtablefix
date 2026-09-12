//! `--check` must predict what `--in-place` would write.
//!
//! `INV-PREDICTS` is the obligation that gives a reporting mode its value as a
//! gate. A `--check` that says "clean" about a document the writer would
//! rewrite, or "drift" about one it would leave alone, is worse than no gate at
//! all, because a pipeline that trusts it fails in the wrong direction and
//! fails silently.
//!
//! The claim is tested the only way it can be tested honestly: the reporting
//! and writing modes run over byte-identical copies of the same document, and
//! what the reporting mode *said* is compared with what the writing mode
//! actually *did* to its own copy. The writing mode is asked for its bytes
//! rather than for its opinion, so a report cannot agree with a decision that
//! was never carried out.
//!
//! Two runs would still not be enough. `--check` and `--in-place` both consult
//! one shared change decision, so a decision that answered wrongly would move
//! both sides in the same direction and the agreement would hold vacuously.
//! Each case therefore runs the document a third time — printed, with no mode
//! flag — and takes the printer's bytes as what should have happened. Printing
//! renders the shared formatter's output without consulting the change decision
//! at all, so the writer's bytes and the report's answer are each measured
//! against something that cannot be moved by the same fault.
//!
//! `tests/cli_check.rs` holds the fixed cases — the ordering batch, the
//! exit-status matrix, the read-only snapshot — with counts taken from
//! `git diff --numstat`. This file generalises the same claim across the flag
//! cross product and over generated documents, where a hand-written expectation
//! cannot be used; the delta the report line carries is therefore recomputed
//! here from the two byte strings the runs produced.
//!
//! The document generator mirrors `tests/check_properties.rs`, which samples
//! the same shapes for idempotence. Each test binary compiles alone, so the two
//! are re-declared rather than shared, as elsewhere in this directory. The
//! corpus does not mirror it: it adds a fixture per flag, because a flag whose
//! fixture never drifts is untested however many cases name it, and the corpus
//! test asserts each fixture's own flag is what moves it. The corpus itself is
//! a module of its own, [`corpus`].

use std::fs;

use assert_cmd::Command;
use proptest::prelude::*;
use rstest::rstest;
use similar::{ChangeTag, TextDiff};
use tempfile::tempdir;

#[path = "check_prediction/corpus.rs"]
mod corpus;

use corpus::CORPUS;

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

/// What one run of the binary did to its copy of a document.
struct Run {
    /// The exit status, or `-1` if the process was signalled.
    status: i32,
    /// The report line, for `--check`; empty for a clean run.
    stdout: String,
    /// The diagnostic, quoted when an assertion fails.
    stderr: String,
    /// The copy's bytes after the run.
    bytes: Vec<u8>,
    /// The path the run was given, which is the path a report must name.
    path: String,
}

/// Runs the binary with `args` in `mode` over a fresh copy of `input`.
///
/// Each call takes its own copy, so no run can observe another's work: the
/// prediction is compared between separate runs, not between one run and its
/// own aftermath. An empty `mode` is the print mode, which `<path>` becomes the
/// sole argument of.
fn run(mode: &[&str], input: &str, flags: &[&str]) -> Run {
    let directory = tempdir().expect("create temporary directory");
    let file = directory.path().join("input.md");
    fs::write(&file, input).expect("write the document copy");

    let output = Command::cargo_bin("mdtablefix")
        .expect("cargo binary")
        .args(mode)
        .args(flags)
        .arg(&file)
        .output()
        .expect("run mdtablefix");

    Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        bytes: fs::read(&file).expect("read the copy back"),
        path: file.to_string_lossy().into_owned(),
    }
}

/// The line delta of a rewrite, recomputed from the two byte strings.
///
/// The counts come from the bytes the two runs produced rather than from the
/// numbers the reporting mode printed, so the report is compared against an
/// independent reading of the same change instead of against itself.
fn line_delta(before: &str, after: &str) -> (usize, usize) {
    let mut insertions = 0;
    let mut deletions = 0;
    for change in TextDiff::from_lines(before, after).iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => insertions += 1,
            ChangeTag::Delete => deletions += 1,
            ChangeTag::Equal => {}
        }
    }
    (insertions, deletions)
}

/// The flags a sampled bitmask selects.
fn selected_flags(mask: u8) -> Vec<&'static str> {
    FLAGS
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, flag)| *flag)
        .collect()
}

/// The prediction check, returning whether the formatter changes the document.
///
/// Three runs, because two are not enough. Both the report and the decision to
/// write consult one shared predicate, so a predicate that answered wrongly
/// would corrupt both sides of the comparison in the same direction and the
/// agreement would hold vacuously. The printer is therefore the oracle: it
/// renders the shared formatter's output without asking that predicate
/// anything, and the writer's bytes and the report's answer are both measured
/// against it.
///
/// Returning a [`TestCaseError`] rather than panicking lets the property shrink
/// a counterexample to the smallest document that breaks the prediction; the
/// corpus test turns the same error into a message naming the case.
fn predict(name: &str, input: &str, flags: &[&str]) -> Result<bool, TestCaseError> {
    let printed = run(&[], input, flags);
    let checked = run(&["--check"], input, flags);
    let rewritten = run(&["--in-place"], input, flags);

    prop_assert_eq!(
        printed.status,
        0,
        "{}: printing failed under {:?}: {}",
        name,
        flags,
        printed.stderr
    );
    prop_assert_eq!(
        rewritten.bytes.as_slice(),
        printed.stdout.as_bytes(),
        "{}: --in-place must write exactly the bytes the printer prints, under {:?}",
        name,
        flags
    );
    prop_assert_eq!(
        checked.bytes.as_slice(),
        input.as_bytes(),
        "{}: --check rewrote the document it was assessing",
        name
    );
    prop_assert!(
        checked.status == 0 || checked.status == 1,
        "{}: --check exited {} on a readable document under {:?}: {}",
        name,
        checked.status,
        flags,
        checked.stderr
    );
    prop_assert_eq!(
        rewritten.status,
        0,
        "{}: --in-place failed under {:?}: {}",
        name,
        flags,
        rewritten.stderr
    );

    // What *should* happen is the formatter's own output, not the change
    // decision the two modes share.
    let changed = printed.stdout.as_bytes() != input.as_bytes();
    prop_assert_eq!(
        checked.status,
        i32::from(changed),
        "{}: --check must exit 1 exactly when the formatter changes the bytes, under {:?}",
        name,
        flags
    );

    if changed {
        let (insertions, deletions) = line_delta(input, &printed.stdout);
        prop_assert_eq!(
            checked.stdout.lines().count(),
            1,
            "{}: a drifting document prints one report line under {:?}, printed {:?}",
            name,
            flags,
            checked.stdout
        );
        prop_assert_eq!(
            checked.stdout.trim_end_matches('\n'),
            format!("{} +{insertions} -{deletions}", checked.path),
            "{}: the report line must carry the rewrite's own delta under {:?}",
            name,
            flags
        );
    } else {
        prop_assert!(
            checked.stdout.is_empty(),
            "{}: a clean document must print nothing under {:?}, printed {:?}",
            name,
            flags,
            checked.stdout
        );
    }

    Ok(changed)
}

/// The corpus test's view of [`predict`]: a failure is a panic naming the case.
fn assert_predicts(name: &str, input: &str, flags: &[&str]) -> bool {
    predict(name, input, flags).unwrap_or_else(|error| panic!("{error}"))
}

/// Every singleton flag and the full set must predict the rewrite over the
/// whole corpus, with both answers observed.
///
/// A run that never saw a drifting document proves nothing about drift, and one
/// that never saw a clean document proves nothing about silence, so both counts
/// are asserted rather than left to the corpus's good behaviour. The fixture's
/// own flag is required to drift it, so a flag that became a no-op would fail
/// the case named after it rather than hide behind another fixture's drift.
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
fn check_predicts_in_place_over_the_corpus(#[case] flags: &[&str]) {
    let mut drifted = 0;
    let mut clean = 0;
    for fixture in CORPUS {
        let changed = assert_predicts(fixture.name, fixture.input, flags);
        if changed {
            drifted += 1;
        } else {
            clean += 1;
        }
        if let Some(flag) = fixture.flag
            && flags.contains(&flag)
        {
            assert!(
                changed,
                "{} must drift under {flag}, the flag it was chosen for",
                fixture.name
            );
        }
    }
    assert!(
        drifted > 0,
        "no corpus document drifted under {flags:?}, so the prediction is untested"
    );
    assert!(
        clean > 0,
        "every corpus document drifted under {flags:?}, so the clean case is untested"
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

    /// INV-PREDICTS over generated documents and a sampled flag subset.
    ///
    /// The generator samples the document boundary — ending style,
    /// byte-order mark, and final terminator — which is where a report and a
    /// rewrite are most likely to disagree, because those are the bytes a
    /// body-text comparison would miss.
    #[test]
    fn check_predicts_in_place_on_generated_documents(
        document in markdown_document(),
        mask in any::<u8>(),
    ) {
        let flags = selected_flags(mask);
        if let Err(error) = predict("generated", &document, &flags) {
            prop_assert!(false, "{error}");
        }
    }
}
