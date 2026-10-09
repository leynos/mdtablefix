//! Skips the `NAME=value` assignments that precede the executable of a Make
//! recipe command.
//!
//! Split from `policy_reader.rs` so that file keeps to judging what the
//! readers return; this one only scans shell text.

/// A recipe command whose leading `NAME=value` assignments are being skipped.
///
/// A recipe composes `RUSTFLAGS` in front of Cargo, as
/// `RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings" $(CARGO) clippy`, and
/// the executable is still the first word that runs. A value may hold quoted
/// spaces and a `$(...)` reference, so it is scanned rather than split on
/// whitespace. Only assignments are skipped: `echo`, `:` and every other word
/// stop the scan, so a command that merely mentions Cargo stays rejected.
pub struct LeadingAssignments<'a>(pub &'a str);

impl<'a> LeadingAssignments<'a> {
    /// Return the command without its leading assignments, or an empty string
    /// when an assignment never closes.
    pub fn skipped(self) -> &'a str {
        let mut rest = Self(self.0.trim_start());
        while let Some(name_len) = rest.name_len() {
            let value = Self(rest.0.get(name_len + 1..).unwrap_or_default());
            let Some(value_len) = value.value_len() else {
                return "";
            };
            rest = Self(value.0.get(value_len..).unwrap_or_default().trim_start());
        }
        rest.0
    }

    /// Return the length of a leading shell variable name that is followed by
    /// `=`.
    fn name_len(&self) -> Option<usize> {
        let text = self.0;
        let name_len = text
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(text.len());
        let starts_like_a_name = text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
        (starts_like_a_name && text.get(name_len..)?.starts_with('=')).then_some(name_len)
    }

    /// Return the length of the assignment value at the start of the text, or
    /// `None` when a quote or `$(` is still open at the end of the input.
    ///
    /// Quotes and parentheses nest: inside `$(...)` a quote opens its own string, so the `"` that
    /// starts `"$(printf " %s")"`'s inner argument does not close the outer one.
    fn value_len(&self) -> Option<usize> {
        let text = self.0;
        let mut open: Vec<Open> = Vec::new();
        let mut chars = text.char_indices().peekable();
        while let Some((index, c)) = chars.next() {
            if open.is_empty() && c.is_whitespace() {
                return Some(index);
            }
            let next = chars.peek().map(|&(_, following)| following);
            if step(&mut open, c, next) {
                chars.next();
            }
        }
        open.is_empty().then_some(text.len())
    }
}

/// A construct still open while a value is scanned.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Open {
    Single,
    Double,
    Paren,
}

/// Apply one character to the stack of open constructs. Returns whether the next character was
/// consumed too (an escaped character, or the `(` of `$(`).
fn step(open: &mut Vec<Open>, c: char, next: Option<char>) -> bool {
    match open.last().copied() {
        Some(Open::Single) => {
            if c == '\'' {
                open.pop();
            }
            false
        }
        _ if c == '\\' => true,
        Some(Open::Double) => in_double(open, c, next),
        _ => outside_quotes(open, c),
    }
}

/// Handle a character inside double quotes: the closing quote, or the start of `$(...)`.
fn in_double(open: &mut Vec<Open>, c: char, next: Option<char>) -> bool {
    match c {
        '"' => {
            open.pop();
            false
        }
        '$' if next == Some('(') => {
            open.push(Open::Paren);
            true
        }
        _ => false,
    }
}

/// Handle a character outside quotes: an opening quote or parenthesis, or the closing parenthesis.
fn outside_quotes(open: &mut Vec<Open>, c: char) -> bool {
    match c {
        ')' if open.last() == Some(&Open::Paren) => {
            open.pop();
        }
        '"' => open.push(Open::Double),
        '\'' => open.push(Open::Single),
        '(' => open.push(Open::Paren),
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    //! Generated checks of the assignment scanner: valid assignment prefixes are
    //! removed whole whatever their quoting, and an assignment that never
    //! closes leaves nothing to run.

    use proptest::prelude::*;

    use super::LeadingAssignments;

    /// A shell variable name.
    fn name() -> impl Strategy<Value = String> { "[A-Za-z_][A-Za-z0-9_]{0,6}" }

    /// An assignment value in any of the spellings a recipe uses: bare, double
    /// quoted with spaces and an escaped quote followed by a space, single quoted with a backslash,
    /// a `$(...)` reference holding spaces and a nested reference, or a double-quoted `$(...)`
    /// whose own arguments are quoted.
    fn value() -> impl Strategy<Value = String> {
        prop_oneof![
            "[a-z0-9./-]{0,8}".prop_map(|word| word),
            "[a-z0-9 :+-]{0,10}".prop_map(|words| format!("\"{words}\\\" tail\"")),
            "[a-z0-9 \\\\-]{0,8}".prop_map(|words| format!("'{words}'")),
            "[a-z]{1,5}".prop_map(|word| format!("$({word} $(inner arg) tail)")),
            "[a-z]{1,5}".prop_map(|word| format!("\"$({word} \" %s\" \"$X\")\"")),
            "[a-z]{1,5}".prop_map(|word| format!("\"$({word} ')' \"nested ( quote\")\"")),
        ]
    }

    /// An assignment `NAME=value`.
    fn assignment() -> impl Strategy<Value = String> {
        (name(), value()).prop_map(|(name, value)| format!("{name}={value}"))
    }

    /// The command that follows the prefix; its first word is never an assignment.
    fn command() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("cargo clippy --all-targets".to_owned()),
            Just("$(CARGO) clippy".to_owned()),
            Just("echo A=b".to_owned()),
            Just(": cargo".to_owned()),
        ]
    }

    proptest! {
        #[test]
        fn a_prefix_of_assignments_is_removed_whole(
            assignments in proptest::collection::vec(assignment(), 0..4),
            command in command(),
            gap in "[ \t]{1,3}",
        ) {
            let prefix = assignments.join(&gap);
            let text = if assignments.is_empty() {
                command.clone()
            } else {
                format!("{prefix}{gap}{command}")
            };

            prop_assert_eq!(LeadingAssignments(&text).skipped(), command);
        }

        #[test]
        fn a_command_with_no_prefix_is_returned_trimmed_at_the_front(
            command in command(),
            lead in "[ \t]{0,3}",
        ) {
            let text = format!("{lead}{command}");

            prop_assert_eq!(LeadingAssignments(&text).skipped(), command);
        }

        #[test]
        fn a_word_that_does_not_start_like_a_name_is_not_an_assignment(
            digit in "[0-9]",
            rest in "[A-Z0-9_]{0,4}",
            value in "[a-z]{1,4}",
        ) {
            let text = format!("{digit}{rest}={value} cargo clippy");

            prop_assert_eq!(LeadingAssignments(&text).skipped(), text.as_str());
        }

        #[test]
        fn an_assignment_that_never_closes_leaves_nothing_to_run(
            name in name(),
            open in prop_oneof![Just("\""), Just("'"), Just("$(")],
            tail in "[a-z ]{0,8}",
        ) {
            let text = format!("{name}={open}{tail} cargo clippy");

            prop_assert_eq!(LeadingAssignments(&text).skipped(), "");
        }
    }
}
