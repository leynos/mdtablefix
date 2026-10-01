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
    fn value_len(&self) -> Option<usize> {
        let text = self.0;
        let mut quote: Option<char> = None;
        let mut depth = 0_usize;
        let mut chars = text.char_indices();
        while let Some((index, c)) = chars.next() {
            match (quote, c) {
                (_, '\\') if quote != Some('\'') => {
                    chars.next();
                }
                (Some(open), _) if c == open => quote = None,
                (None, '"' | '\'') => quote = Some(c),
                (None, '(') => depth += 1,
                (None, ')') => depth = depth.saturating_sub(1),
                (None, _) if c.is_whitespace() && depth == 0 => return Some(index),
                _ => {}
            }
        }
        (quote.is_none() && depth == 0).then_some(text.len())
    }
}
