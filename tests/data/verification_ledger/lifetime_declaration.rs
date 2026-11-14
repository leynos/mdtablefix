// A borrowed lifetime opens with the same character a character literal does,
// and the masker must tell them apart in both directions.
//
// Reading every quote as a character literal leaves the lifetime unbalanced and
// hides this line and, because the literal state persists across lines, every
// declaration after it. Reading every quote followed by an identifier as a
// lifetime does the same to the character literal below.
const SEPARATOR: char = 'x';

fn first(candidate: SetextCandidate<'_>) -> bool { true }

pub fn process_stream_inner() {}

fn second(other: SetextCandidate<'_>) -> bool { true }
