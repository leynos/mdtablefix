//! Regression guards for issue `#261`: fenced and indented shell blocks
//! must remain byte-identical when `wrap_text` processes surrounding
//! Markdown.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

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
