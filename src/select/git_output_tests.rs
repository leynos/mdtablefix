//! Tests for the NUL framing of a `git ls-files` listing.
//!
//! The parser is a pure function of the bytes Git writes, so it is tested as
//! one: the property is that splitting is a faithful inverse of Git's framing,
//! including the parts a hand-written case list tends to miss — an empty
//! listing, a path that is not UTF-8, and a stream that stops mid-path.
//!
//! What needs a real program lives next door: the invocation, its failure
//! mapping, and its tracing in `git_ls_files_git_tests`, and the scrubbing of
//! Git's own diagnostic in `git_output_relay_tests`.

use std::cell::Cell;

use camino::Utf8PathBuf;
use proptest::{collection::vec, prelude::*, test_runner::TestRunner};
use rstest::rstest;

use super::split_nul_delimited;

/// Printable ASCII, the common case for a repository path.
fn ascii_text() -> impl Strategy<Value = Vec<u8>> { vec(0x20u8..=0x7e, 1..=40) }

/// Arbitrary bytes that are not NUL and so can be a path in Git's framing.
fn raw_bytes() -> impl Strategy<Value = Vec<u8>> {
    vec(
        any::<u8>().prop_filter("a path holds no NUL byte", |byte| *byte != 0),
        1..=40,
    )
}

/// A path-shaped byte string that is sometimes not valid UTF-8.
fn segment() -> impl Strategy<Value = Vec<u8>> { prop_oneof![3 => ascii_text(), 1 => raw_bytes()] }

/// INV-NUL-SPLIT: splitting is a faithful inverse of Git's framing.
#[test]
fn splitting_is_a_faithful_inverse_of_nul_framing() {
    let mut runner = TestRunner::default();
    let saw_non_utf8 = Cell::new(false);
    let saw_empty_input = Cell::new(false);

    runner
        .run(&vec(segment(), 0..=20), |segments| {
            let mut framed = Vec::new();
            for segment in &segments {
                framed.extend_from_slice(segment);
                framed.push(0);
            }

            let listing = split_nul_delimited(&framed);
            let textual: Vec<&str> = segments
                .iter()
                .filter_map(|segment| std::str::from_utf8(segment).ok())
                .collect();

            prop_assert_eq!(listing.skipped_non_utf8, segments.len() - textual.len());
            prop_assert_eq!(
                listing
                    .paths
                    .iter()
                    .map(|path| path.as_str())
                    .collect::<Vec<_>>(),
                textual
            );

            if listing.skipped_non_utf8 > 0 {
                saw_non_utf8.set(true);
            }
            if segments.is_empty() {
                saw_empty_input.set(true);
                prop_assert!(
                    listing.paths.is_empty(),
                    "empty input must yield an empty list, not a list holding one empty path"
                );
            }
            Ok(())
        })
        .expect("splitting is a faithful inverse of NUL framing");

    assert!(
        saw_non_utf8.get(),
        "the generator must reach the drop-and-count path, or that path is untested"
    );
    assert!(
        saw_empty_input.get(),
        "the empty-input case must be generated and asserted, not assumed"
    );
}

#[rstest]
#[case(b"", &[], 0)]
#[case(b"\0", &[], 0)]
#[case(b"a.md\0", &["a.md"], 0)]
#[case(b"a.md\0b.md\0", &["a.md", "b.md"], 0)]
// Git always terminates the last path, but a truncated stream must not invent
// one either way: the final entry is returned, and no empty path is.
#[case(b"a.md", &["a.md"], 0)]
#[case(b"a.md\0\xff\xfe.md\0b.md\0", &["a.md", "b.md"], 1)]
fn splitting_cases(#[case] input: &[u8], #[case] expected: &[&str], #[case] skipped: usize) {
    let listing = split_nul_delimited(input);
    assert_eq!(
        listing.paths,
        expected
            .iter()
            .copied()
            .map(Utf8PathBuf::from)
            .collect::<Vec<_>>()
    );
    assert_eq!(listing.skipped_non_utf8, skipped);
}
