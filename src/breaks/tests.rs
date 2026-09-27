//! Unit tests for thematic-break formatting.

use std::{
    borrow::Cow,
    sync::{Arc, Barrier},
    thread,
};

use super::*;

macro_rules! assert_borrowed_value {
    ($line:expr, $expected:expr $(,)?) => {
        match &$line {
            Cow::Borrowed(value) => assert_eq!(*value, $expected),
            Cow::Owned(value) => panic!("expected borrowed value, got owned {value:?}"),
        }
    };
}

#[test]
fn basic_formatting() {
    let input = vec!["foo", "***", "bar"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "foo");
    assert_borrowed_value!(output[1], THEMATIC_BREAK_LINE.as_str());
    assert_borrowed_value!(output[2], "bar");
}

#[test]
fn ignores_fenced_code() {
    let input = vec!["```", "---", "```"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "```");
    assert_borrowed_value!(output[1], "---");
    assert_borrowed_value!(output[2], "```");
}

#[test]
fn lazylock_initialisation_is_race_safe() {
    const THREADS: usize = 16;

    // Document the application's reliance on the stdlib race-safety guarantee.
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles = (0..THREADS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let input = vec!["---".to_string()];
                barrier.wait();

                let output = format_breaks(&input);
                match &output[0] {
                    Cow::Borrowed(value) => {
                        assert_eq!(*value, THEMATIC_BREAK_LINE.as_str());
                        assert_eq!(value.len(), THEMATIC_BREAK_LEN);
                        assert!(std::ptr::eq(*value, THEMATIC_BREAK_LINE.as_str()));
                        value.as_ptr() as usize
                    }
                    Cow::Owned(value) => {
                        panic!("expected borrowed break line, got owned {value:?}");
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    let pointers = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread must complete"))
        .collect::<Vec<_>>();

    assert!(
        pointers
            .iter()
            .all(|pointer| *pointer == THEMATIC_BREAK_LINE.as_ptr() as usize)
    );
}
