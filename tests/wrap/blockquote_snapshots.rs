//! Snapshot tests for semantic blockquote wrapping.
//!
//! These record the full wrapped output for representative blockquote
//! structures — nested quotes, compound list-in-quote prefixes, depth-aware
//! fence exit, and mixed space/tab prefixes — so prefix reconstruction and line
//! boundaries are reviewed as snapshot changes rather than inferred from
//! individual predicates. They complement the example-based assertions in
//! `blockquotes.rs`.

use mdtablefix::{process::WRAP_COLS, wrap::wrap_text};

fn assert_blockquote_snapshot(name: &str, input: &[String]) {
    insta::with_settings!({
        snapshot_path => "../snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(name, wrap_text(input, WRAP_COLS).join("\n"));
    });
}

#[test]
fn blockquote_snapshot_nested_prose() {
    let input = lines_vec![concat!(
        "> > This nested quote contains enough text to require wrapping so that we ",
        "can verify multi-level handling across the configured wrap width."
    )];
    assert_blockquote_snapshot("blockquote_nested_prose", &input);
}

#[test]
fn blockquote_snapshot_bullet_in_quote() {
    let input = lines_vec![concat!(
        "> - This list item contains enough prose to wrap onto multiple lines while ",
        "retaining its compound blockquote-plus-bullet prefix."
    )];
    assert_blockquote_snapshot("blockquote_bullet_in_quote", &input);
}

#[test]
fn blockquote_snapshot_ordered_list_in_nested_quote() {
    let input = lines_vec![concat!(
        "> > 1. This ordered item contains enough prose to wrap while preserving both ",
        "quote levels on every continuation line."
    )];
    assert_blockquote_snapshot("blockquote_ordered_list_in_nested_quote", &input);
}

#[test]
fn blockquote_snapshot_depth_decrease_ends_fence() {
    let input = lines_vec![
        "> > ```rust",
        "> > let quoted = true;",
        concat!(
            "> This depth-one prose follows the implicitly closed nested fence and is ",
            "long enough to wrap across the configured width."
        ),
    ];
    assert_blockquote_snapshot("blockquote_depth_decrease_ends_fence", &input);
}

#[test]
fn blockquote_snapshot_mixed_space_tab_prefix() {
    let input = lines_vec![
        "> \t> \tThis blockquote mixes spaces and tabs in the prefix and is long enough to wrap \
         while preserving that exact prefix spelling.",
    ];
    assert_blockquote_snapshot("blockquote_mixed_space_tab_prefix", &input);
}
