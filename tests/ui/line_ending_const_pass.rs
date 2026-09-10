//! Compile-pass fixture: `LineEnding::as_str` is a `const fn`, so downstream
//! callers can resolve the emitted terminators at compile time rather than
//! paying for a run-time call.

use mdtablefix::LineEnding;

// Both variants are evaluated in const contexts, which is what makes the
// `const fn` part of the signature load-bearing.
const LF: &str = LineEnding::Lf.as_str();
const CRLF: &str = LineEnding::Crlf.as_str();

const TERMINATORS: [&str; 2] = [LF, CRLF];

fn main() {
    assert_eq!(LF, "\n");
    assert_eq!(CRLF, "\r\n");
    assert_eq!(TERMINATORS, ["\n", "\r\n"]);
}
