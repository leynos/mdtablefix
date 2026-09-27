//! Unit tests for ordered list renumbering.
//!
//! These tests cover the parent module's parsing helpers, state
//! transitions, and public renumbering behaviour.

use super::*;

#[test]
fn parse_numbered_parts() {
    let line = "  12. item";
    assert_eq!(parse_numbered(line), Some((2, "  ", " ", "item")));
}

#[test]
fn parse_numbered_with_tab() {
    let line = "	1.	foo";
    assert_eq!(parse_numbered(line), Some((4, "	", "	", "foo")));
}

#[test]
fn simple_renumber() {
    let input = vec!["1. a", "3. b"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let expected = vec!["1. a", "2. b"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn nested_renumber() {
    let input = vec!["1. a", "    1. sub", "    3. sub2", "2. b"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let expected = vec!["1. a", "    1. sub", "    2. sub2", "2. b"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn list_state_end_lists_at_zero_clears_indent_stack_and_counters() {
    let mut state = ListState::default();
    let _ = state.next_number(0);
    let _ = state.next_number(0);
    let _ = state.next_number(4);
    assert!(!state.indent_stack.is_empty());
    assert!(!state.counters.is_empty());

    state.end_lists_at(0);

    assert!(state.indent_stack.is_empty());
    assert!(state.counters.is_empty());
}

#[test]
fn list_state_next_number_increments_and_prunes_deeper_indents() {
    let mut state = ListState::default();
    assert_eq!(state.next_number(0), 1);
    assert_eq!(state.next_number(0), 2);
    // A deeper indent starts its own counter at 1.
    assert_eq!(state.next_number(4), 1);
    assert_eq!(state.next_number(4), 2);
    // Returning to the original indent prunes the deeper one and continues
    // counting from where the outer level left off.
    assert_eq!(state.next_number(0), 3);
    assert!(!state.counters.contains_key(&4));
}

mod proptest_tests {
    //! Property tests for ordered list state invariants.
    //!
    //! These generated cases exercise the same `ListState` state machine
    //! used by `renumber_lists` across varied indent sequences.

    use proptest::prelude::*;

    use super::ListState;

    /// One step of a generated list history: an item at a marker column with
    /// its content offset, or a block at a column.
    #[derive(Clone, Debug)]
    enum Step {
        Item(usize, usize),
        Block(usize),
    }

    /// Generates an item or a block step over small columns.
    fn step() -> impl Strategy<Value = Step> {
        prop_oneof![
            (0usize..=8, 2usize..=5).prop_map(|(indent, offset)| Step::Item(indent, offset)),
            (0usize..=10).prop_map(Step::Block),
        ]
    }

    proptest! {
        #[test]
        fn list_state_next_number_always_starts_at_1_for_new_indent(
            indents in proptest::collection::vec(0usize..=8, 1..=20),
        ) {
            let mut state = ListState::default();
            for &indent in &indents {
                // Capture absence before the call: `next_number` may
                // prune deeper counters, but the counter for `indent`
                // itself is only removed by an earlier shallower call.
                let was_absent = !state.counters.contains_key(&indent);
                let returned = state.next_number(indent);
                if was_absent {
                    prop_assert_eq!(
                        returned,
                        1,
                        "indent {} first appeared (or re-emerged after pruning) but returned {}",
                        indent,
                        returned,
                    );
                }
            }
        }

        #[test]
        fn list_state_prunes_deeper_counters_when_returning_to_outer_indent(
            outer_count in 1usize..=6,
            deeper_count in 1usize..=6,
        ) {
            let mut state = ListState::default();
            for expected in 1..=outer_count {
                prop_assert_eq!(state.next_number(0), expected);
            }
            for expected in 1..=deeper_count {
                prop_assert_eq!(state.next_number(4), expected);
            }

            prop_assert_eq!(state.next_number(0), outer_count + 1);
            prop_assert!(!state.counters.contains_key(&4));
            prop_assert_eq!(state.counters.get(&0), Some(&(outer_count + 2)));
        }

        /// `end_lists_at` ends exactly the innermost lists whose items
        /// cannot contain the block: what remains is a prefix of the old
        /// stack whose top item contains the column, and every kept level's
        /// counter is unchanged.
        #[test]
        fn end_lists_at_keeps_the_prefix_that_contains_the_block(
            steps in proptest::collection::vec(step(), 1..=24),
        ) {
            let mut state = ListState::default();
            for step in steps {
                match step {
                    Step::Item(indent, offset) => {
                        let _ = state.next_number(indent);
                        state.record_content_column(indent, indent + offset);
                    }
                    Step::Block(column) => {
                        let before = state.indent_stack.clone();
                        let counters = state.counters.clone();
                        state.end_lists_at(column);
                        prop_assert!(before.starts_with(&state.indent_stack));
                        if let Some(top) = state.indent_stack.last() {
                            prop_assert!(state.content_columns[top] <= column);
                        }
                        for depth in &state.indent_stack {
                            prop_assert_eq!(state.counters.get(depth), counters.get(depth));
                        }
                        prop_assert!(state.indent_stack.windows(2).all(|w| w[0] < w[1]));
                    }
                }
            }
        }
    }
}
