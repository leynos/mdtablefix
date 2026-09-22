//! Drives the main-publisher rules against fixtures directly.
//!
//! Split from `pull_request_cases.rs` under the repository's 400-line cap.
//! Each case varies one clause of an otherwise complying publisher and
//! asserts the rule names that clause and nothing else.

use anyhow::{Result, ensure};
use rstest::rstest;

use super::{pull_request_cases::parse, rules};

/// Scenario: push triggers that name other branches, tags, or none.
///
/// Invariant: only a push restricted to `main` is the publisher.
#[rstest]
#[case::main_only("on:\n  push:\n    branches: [main]\njobs: {}\n", true)]
#[case::unfiltered("on:\n  push:\njobs: {}\n", false)]
#[case::scalar("on: push\njobs: {}\n", false)]
#[case::tags("on:\n  push:\n    tags: ['v*']\njobs: {}\n", false)]
#[case::another_branch("on:\n  push:\n    branches: [develop]\njobs: {}\n", false)]
#[case::main_and_another("on:\n  push:\n    branches: [main, develop]\njobs: {}\n", false)]
fn only_a_push_restricted_to_main_is_the_publisher(
    #[case] source: &str,
    #[case] expected: bool,
) -> Result<()> {
    ensure!(
        rules::publishes_from_main(&parse(source)?) == expected,
        "expected {expected} for {source:?}"
    );
    Ok(())
}

/// A publisher, with the three pieces the cases vary left as placeholders.
const PUBLISHER: &str = r#"
on:
  push:
    branches: [main]
  workflow_dispatch:
@CONCURRENCY@
jobs:
  coverage:
    steps:
      - uses: leynos/shared-actions/.github/actions/generate-coverage@abc
        with:
          with-ratchet: 'true'
@EXTRA_STEP@
      - env:
          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}
        if: "@UPLOAD_IF@"
        uses: leynos/shared-actions/.github/actions/upload-codescene-coverage@abc
"#;

/// Renders [`PUBLISHER`] with the pieces a case varies.
fn publisher(concurrency: &str, upload_if: &str, extra_step: &str) -> String {
    PUBLISHER
        .replace("@CONCURRENCY@", concurrency)
        .replace("@UPLOAD_IF@", upload_if)
        .replace("@EXTRA_STEP@", extra_step)
}

/// The publisher's concurrency block as this repository writes it.
const NEVER_CANCEL: &str = "concurrency:\n  group: pub\n  cancel-in-progress: false";
/// The upload condition as this repository writes it.
const GUARD: &str = "${{ env.CS_ACCESS_TOKEN != '' && github.ref == 'refs/heads/main' }}";

/// Scenario: a publisher is varied one clause at a time.
///
/// Invariant: each variation is reported by the clause it breaks, and the
/// unvaried publisher has no findings. The appended disjunction is the
/// mutation a substring check passes. The prepended one is the case only the
/// refusal of `||` catches: `&&` binds tighter, so the ref check survives as
/// an exact conjunct of the split while the leading disjunct uploads a
/// dispatch from any branch on its own.
#[rstest]
#[case::complies(NEVER_CANCEL, GUARD, "", None)]
#[case::disjunction_appended(
    NEVER_CANCEL,
    "${{ env.CS_ACCESS_TOKEN != '' && github.ref == 'refs/heads/main' || github.event_name == \
     'workflow_dispatch' }}",
    "",
    Some("not guarded")
)]
#[case::disjunction_prepended(
    NEVER_CANCEL,
    "${{ github.event_name == 'workflow_dispatch' || env.CS_ACCESS_TOKEN != '' && github.ref == \
     'refs/heads/main' }}",
    "",
    Some("not guarded")
)]
#[case::no_ref_guard(
    NEVER_CANCEL,
    "${{ env.CS_ACCESS_TOKEN != '' }}",
    "",
    Some("not guarded")
)]
#[case::cancels(
    "concurrency:\n  group: pub\n  cancel-in-progress: true",
    GUARD,
    "",
    Some("cancel")
)]
#[case::cancels_by_expression(
    "concurrency:\n  group: pub\n  cancel-in-progress: ${{ true }}",
    GUARD,
    "",
    Some("cancel")
)]
#[case::no_group("", GUARD, "", Some("no concurrency group"))]
#[case::token_elsewhere(
    NEVER_CANCEL,
    GUARD,
    "      - run: echo ${{ secrets.CS_ACCESS_TOKEN }}",
    Some("other than the upload")
)]
fn the_publisher_rule_names_the_clause_broken(
    #[case] concurrency: &str,
    #[case] upload_if: &str,
    #[case] extra_step: &str,
    #[case] expected: Option<&str>,
) -> Result<()> {
    let source = publisher(concurrency, upload_if, extra_step);
    let findings = rules::publisher_findings(&parse(&source)?);
    match expected {
        None => ensure!(findings.is_empty(), "unexpected findings: {findings:?}"),
        Some(clause) => ensure!(
            findings.len() == 1 && findings[0].contains(clause),
            "expected one finding naming {clause:?}, saw {findings:?}"
        ),
    }
    Ok(())
}

/// The upload step's token, as [`PUBLISHER`] writes it.
const UPLOAD_TOKEN: &str =
    "      - env:\n          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}\n";
/// A scope-wide declaration of the token, indented for the workflow root.
const WIDE_TOKEN: &str = "env:\n  CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}\n";

/// A job calling a reusable workflow, awaiting the mapping a case gives it.
const REUSABLE: &str = "  forward:\n    uses: ./.github/workflows/elsewhere.yml\n";

/// Scenario: the token is moved off the upload step, or declared more widely.
///
/// Invariant: each placement is named. Moving the token to the coverage step
/// satisfies "some step holds it" while the upload's own guard goes false and
/// publishing silently stops, and a workflow- or job-level `env` hands it to
/// every step, so neither may pass as the upload holding it.
#[rstest]
#[case::moved_to_coverage(
    |source: String| source
        .replace(UPLOAD_TOKEN, "      - env: {}\n")
        .replace("        with:\n          with-ratchet", "        env:\n          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}\n        with:\n          with-ratchet"),
    &["does not receive", "other than the upload"][..],
)]
#[case::workflow_env(|source: String| source.replace("jobs:\n", &format!("{WIDE_TOKEN}jobs:\n")), &["for every job"][..])]
#[case::job_env(
    |source: String| source.replace("    steps:\n", &format!("    {}    steps:\n", WIDE_TOKEN.replace("\n  ", "\n      "))),
    &["for every step"][..],
)]
#[case::forwarded_as_an_input(
    |source: String| source.replace("jobs:\n", &format!("jobs:\n{REUSABLE}    with:\n      token: ${{{{ secrets.CS_ACCESS_TOKEN }}}}\n")),
    &["to a reusable workflow"][..],
)]
#[case::forwarded_by_name(
    |source: String| source.replace("jobs:\n", &format!("jobs:\n{REUSABLE}    secrets:\n      CS_ACCESS_TOKEN: ${{{{ secrets.CS_ACCESS_TOKEN }}}}\n")),
    &["to a reusable workflow"][..],
)]
#[case::forwarded_by_inheritance(
    |source: String| source.replace("jobs:\n", &format!("jobs:\n{REUSABLE}    secrets: inherit\n")),
    &["to a reusable workflow"][..],
)]
#[case::computed_elsewhere(
    |source: String| source.replace("        with:\n          with-ratchet", "        env:\n          T: ${{ secrets['CS_ACCESS_TOKEN'] }}\n        with:\n          with-ratchet"),
    &["computed name"][..],
)]
fn the_token_sits_on_the_upload_alone(
    #[case] vary: fn(String) -> String,
    #[case] expected: &[&str],
) -> Result<()> {
    let source = vary(publisher(NEVER_CANCEL, GUARD, ""));
    let findings = rules::publisher_findings(&parse(&source)?);
    ensure!(
        findings.len() == expected.len()
            && expected
                .iter()
                .all(|clause| findings.iter().any(|f| f.contains(clause))),
        "expected findings naming {expected:?}, saw {findings:?}"
    );
    Ok(())
}

/// Scenario: a publisher gains a second upload, through the CLI, with its
/// command split across a shell continuation.
///
/// Invariant: it is read as an upload, so its missing guard is reported. The
/// shell joins `cs-coverage \` and `upload` into one command; a contiguous
/// text search would not, and the unguarded upload would pass unjudged.
#[test]
fn a_continued_cli_upload_is_still_an_upload() -> Result<()> {
    let extra = "      - run: |\n          cs-coverage \\\n            upload --format lcov\n";
    let findings = rules::publisher_findings(&parse(&publisher(NEVER_CANCEL, GUARD, extra))?);
    ensure!(
        findings.iter().any(|f| f.contains("not guarded")),
        "the continued upload was not judged: {findings:?}"
    );
    Ok(())
}

/// Scenario: a publisher that runs the upload action in `check` mode.
///
/// Invariant: that is not an upload, so the omission is reported.
#[test]
fn check_mode_is_not_an_upload() -> Result<()> {
    let source = publisher(NEVER_CANCEL, GUARD, "").replace(
        "upload-codescene-coverage@abc\n",
        "upload-codescene-coverage@abc\n        with:\n          mode: check\n",
    );
    let findings = rules::publisher_findings(&parse(&source)?);
    ensure!(
        findings.iter().any(|f| f.contains("uploads nothing")),
        "check mode read as an upload: {findings:?}"
    );
    Ok(())
}

/// Scenario: conditions whose operators sit inside quoted literals.
///
/// Invariant: a `||` inside a string is not a disjunction, and an `&&`
/// inside one does not split a conjunct.
#[rstest]
#[case::quoted_or("${{ github.ref == 'refs/heads/main' && env.X != 'a||b' }}", Some(2))]
#[case::quoted_and("${{ github.ref == 'refs/heads/main' && env.X != 'a&&b' }}", Some(2))]
#[case::bare_or("github.ref == 'refs/heads/main' || true", None)]
fn quoted_operators_are_not_operators(#[case] condition: &str, #[case] expected: Option<usize>) {
    assert_eq!(
        rules::conjuncts(condition).map(|parts| parts.len()),
        expected,
        "for {condition}"
    );
}

/// A publisher wired as this repository wires it: the upload reads what the
/// coverage step writes and passes the token its step was given.
const WIRED: &str = r"
on:
  push:
    branches: [main]
jobs:
  coverage:
    steps:
      - uses: leynos/shared-actions/.github/actions/generate-coverage@abc
        with:
          output-path: lcov.info
          format: lcov
      - env:
          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}
        uses: leynos/shared-actions/.github/actions/upload-codescene-coverage@abc
        with:
          path: lcov.info
          format: lcov
          access-token: ${{ env.CS_ACCESS_TOKEN }}
";

/// Scenario: the upload is rewired away from what was measured, or from the
/// token.
///
/// Invariant: each variation is named, and the wired publisher has none.
/// Every other publisher clause passes all four variations, because each
/// judges one step at a time.
#[rstest]
#[case::wired("", "", None)]
#[case::other_path(
    "path: lcov.info",
    "path: other.info",
    Some("which no coverage step writes")
)]
#[case::other_format(
    "          format: lcov\n          access",
    "          format: cobertura\n          access",
    Some("which no coverage step writes")
)]
#[case::no_token(
    "          access-token: ${{ env.CS_ACCESS_TOKEN }}\n",
    "",
    Some("not the token it was given")
)]
#[case::other_token(
    "${{ env.CS_ACCESS_TOKEN }}",
    "${{ env.OTHER }}",
    Some("not the token it was given")
)]
fn the_upload_sends_what_was_measured(
    #[case] from: &str,
    #[case] to: &str,
    #[case] expected: Option<&str>,
) -> Result<()> {
    let source = if from.is_empty() {
        WIRED.to_owned()
    } else {
        WIRED.replacen(from, to, 1)
    };
    ensure!(
        source != WIRED || from.is_empty(),
        "the case changed nothing"
    );
    let findings = rules::wiring_findings(&parse(&source)?);
    match expected {
        None => ensure!(findings.is_empty(), "unexpected findings: {findings:?}"),
        Some(clause) => ensure!(
            findings.len() == 1 && findings[0].contains(clause),
            "expected one finding naming {clause:?}, saw {findings:?}"
        ),
    }
    Ok(())
}
