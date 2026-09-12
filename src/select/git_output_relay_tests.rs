//! Tests for `relayable`: Git's own text, scrubbed before it is shown.
//!
//! Git's diagnostic is the one part of this tool's output this repository does
//! not author, so it is the one part that has to be made safe rather than
//! trusted: one line, no control characters, and a bound on how much of it a
//! user is asked to read.

use rstest::rstest;

use super::{RELAYED_LIMIT, relayable};

/// `relayable`: Git's text is scrubbed into one line before it is shown.
#[rstest]
#[case(b"", "")]
#[case(b"fatal: bad revision\n", "fatal: bad revision")]
// A message that is several lines is shown as one, so a repository cannot
// forge additional lines of this tool's stderr.
#[case(b"fatal: bad\nusage: git ls-files\n", "fatal: bad usage: git ls-files")]
#[case(b"a\r\nb\rc", "a b c")]
// A run at either end disappears rather than becoming a gap.
#[case(b"\n\nfatal\n\n", "fatal")]
// An escape sequence loses its escape character, so the rest is inert text
// rather than a terminal instruction a path in the repository chose.
#[case(b"\x1b[31mfatal\x1b[0m", "[31mfatal [0m")]
#[case(b"a\x07b", "a b")]
// Bytes that are not UTF-8 become the replacement character: this is a
// diagnostic, not a path, and nothing acts on it.
#[case(b"bad \xff byte", "bad \u{fffd} byte")]
fn relayed_diagnostics_are_scrubbed_into_one_line(#[case] input: &[u8], #[case] expected: &str) {
    assert_eq!(relayable(input), expected);
}

/// A diagnostic long enough to bury the message it supports is cut, and the
/// cut is visible rather than silent.
///
/// The cap falls at the limit rather than one side of it: a run of exactly
/// [`RELAYED_LIMIT`] characters is relayed whole, so the shortening of a
/// longer one is never mistaken for git having said that much.
#[test]
fn a_diagnostic_of_exactly_the_limit_is_relayed_whole() {
    let at_limit = "x".repeat(RELAYED_LIMIT);

    assert_eq!(relayable(at_limit.as_bytes()), at_limit);
}

/// The character past the limit is what costs the last one its place, and the
/// ellipsis is what says so.
#[test]
fn a_diagnostic_past_the_limit_is_cut() {
    let flood = "x".repeat(4096);

    let relayed = relayable(flood.as_bytes());

    assert_eq!(relayed.chars().count(), RELAYED_LIMIT + 1, "{relayed:?}");
    assert!(relayed.ends_with('…'), "{relayed:?}");
    assert!(
        relayed.starts_with(&"x".repeat(RELAYED_LIMIT)),
        "the cap must keep a prefix, not drop the message: {relayed:?}"
    );
}
