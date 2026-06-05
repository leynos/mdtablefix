//! Property-test support for the issue 329 CLI regression.

use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};

/// Generated fenced block used by the combined-flag CLI property test.
#[derive(Debug)]
pub(super) struct GeneratedFenceBlock {
    pub(super) body: String,
    pub(super) body_lines: Vec<String>,
    pub(super) indent: String,
    pub(super) info_suffix: String,
    pub(super) input: String,
}

/// Fenced block region located in formatted CLI output.
pub(super) struct LocatedFenceBlock<'a> {
    pub(super) body_lines: &'a [&'a str],
    pub(super) closing_line: &'a str,
    pub(super) opening_line: &'a str,
}

/// Build a bounded runner for generated fenced block cases.
pub(super) fn fenced_block_runner() -> TestRunner {
    TestRunner::new(Config {
        cases: 96,
        ..Config::default()
    })
}

/// Generate fenced blocks whose body lines do not semantically close the fence.
pub(super) fn fenced_block_strategy() -> impl Strategy<Value = GeneratedFenceBlock> {
    (0usize..=3, fence_marker_strategy(), fence_info_strategy())
        .prop_flat_map(|(indent_width, marker, info)| {
            let marker_char = marker
                .chars()
                .next()
                .expect("fence marker strategy emits a non-empty marker");
            let marker_len = marker.len();
            (
                Just(indent_width),
                Just(marker),
                Just(info),
                fenced_body_strategy(marker_char, marker_len),
            )
        })
        .prop_map(|(indent_width, marker, info, body_lines)| {
            let indent = " ".repeat(indent_width);
            let info_suffix = if info.is_empty() {
                String::new()
            } else {
                format!(" {info}")
            };
            let opening_line = format!("{indent}{marker}{info_suffix}");
            let closing_line = format!("{indent}{marker}");
            let body = body_lines.join("\n");
            let input = format!("{opening_line}\n{body}\n{closing_line}\n");
            GeneratedFenceBlock {
                body,
                body_lines,
                indent,
                info_suffix,
                input,
            }
        })
}

/// Locate the generated fenced body and its surrounding fence lines.
pub(super) fn locate_fenced_block<'a>(
    output_lines: &'a [&str],
    block: &GeneratedFenceBlock,
) -> Result<LocatedFenceBlock<'a>, String> {
    let body_len = block.body_lines.len();
    let body_start = output_lines
        .windows(body_len)
        .position(|window| {
            window
                .iter()
                .zip(&block.body_lines)
                .all(|(output, expected)| *output == expected)
        })
        .ok_or_else(|| format!("missing unchanged fenced body:\n{}", block.body))?;
    let opening_index = body_start
        .checked_sub(1)
        .ok_or_else(|| "missing opening fence before body".to_owned())?;
    let closing_index = body_start + body_len;
    let closing_line = output_lines
        .get(closing_index)
        .ok_or_else(|| "missing closing fence after body".to_owned())?;

    Ok(LocatedFenceBlock {
        body_lines: &output_lines[body_start..closing_index],
        closing_line,
        opening_line: output_lines[opening_index],
    })
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
        printable_ascii_info_strategy(0..24),
    ]
}

fn printable_ascii_line_strategy(len: std::ops::Range<usize>) -> impl Strategy<Value = String> {
    prop::collection::vec(0x20u8..=0x7e, len)
        .prop_map(|bytes| bytes.into_iter().map(char::from).collect())
}

fn printable_ascii_info_strategy(len: std::ops::Range<usize>) -> impl Strategy<Value = String> {
    prop::collection::vec(0x20u8..=0x7e, len).prop_map(|bytes| {
        bytes
            .into_iter()
            .filter(|byte| !matches!(*byte, b'`' | b'~'))
            .map(char::from)
            .collect()
    })
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
    run_len >= marker_len
        && trimmed
            .chars()
            .nth(run_len)
            .is_none_or(|ch| ch.is_ascii_whitespace())
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
