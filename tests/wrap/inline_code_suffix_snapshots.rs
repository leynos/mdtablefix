//! Snapshot tests for inflectional-suffix atomicity during paragraph wrapping.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

fn wrap_lines(input: &str, width: usize) -> String {
    let lines: Vec<String> = input.lines().map(str::to_owned).collect();
    wrap_text(&lines, width).join("\n")
}

fn assert_wrap_snapshot(name: &str, input: &str, width: usize) {
    insta::with_settings!({
        snapshot_path => "../snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(name, wrap_lines(input, width));
    });
}

#[rstest]
#[case(
    "inline_code_plural_suffix_stays_atomic",
    "The list of `VarGuard`s is validated at startup by the configuration loader.",
    40
)]
#[case(
    "inline_code_possessive_suffix_stays_atomic",
    "Each `class`'s constructor is called once during initialisation.",
    40
)]
#[case(
    "inline_code_ed_suffix_stays_atomic",
    "Values are `fetch`ed lazily on first access by the runtime.",
    40
)]
#[case(
    "inline_code_ing_suffix_stays_atomic",
    "The scheduler keeps `run`ning until the queue is drained.",
    40
)]
#[case(
    "inline_code_hyphen_compound_stays_atomic",
    "Use a `code`-style identifier when naming configuration keys.",
    40
)]
#[case(
    "inline_code_leading_hyphen_stays_atomic",
    "The pre-`LLMPort` interface defines the contract for model invocations.",
    40
)]
#[case(
    "inline_code_leading_hyphen_with_paren_stays_atomic",
    "Use the (pre-`LLMPort`) interface for testing stubs.",
    40
)]
fn inline_code_suffix_wrap_snapshots(
    #[case] name: &str,
    #[case] input: &str,
    #[case] width: usize,
) {
    assert_wrap_snapshot(name, input, width);
}
