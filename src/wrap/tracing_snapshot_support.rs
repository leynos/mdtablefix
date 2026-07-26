//! Stable capture support for tracing-event snapshots.
//!
//! Call this helper inside a `#[traced_test]` function through its injected
//! `logs_assert` closure. Copy the normalized result into an owned buffer
//! before passing it to `insta::assert_snapshot!` after the closure returns.

/// Normalizes captured tracing lines for a single event message.
///
/// Retains only lines containing `message`, removes the volatile prefix before
/// the event level, trims trailing whitespace, and joins the remaining lines.
/// The level, target, message, and structured fields remain intact.
///
/// # Examples
///
/// ```ignore
/// // `tracing-test` prepends a volatile timestamp and span context before the
/// // level; only the level onward is stable enough to snapshot.
/// let lines = [
///     "2026-07-26T00:00:00Z  DEBUG snapshots: mdtablefix::wrap: fragment classified kind=Plain  ",
///     "2026-07-26T00:00:00Z  TRACE snapshots: mdtablefix::wrap: predicate matched",
/// ];
/// let normalized = normalise_event_lines(&lines, "fragment classified");
/// assert_eq!(
///     normalized,
///     "DEBUG snapshots: mdtablefix::wrap: fragment classified kind=Plain",
/// );
/// ```
pub(crate) fn normalise_event_lines(lines: &[&str], message: &str) -> String {
    lines
        .iter()
        .filter(|line| line.contains(message))
        .map(|line| stable_event_start(line).trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

fn stable_event_start(line: &str) -> &str {
    ["TRACE", "DEBUG"]
        .iter()
        .filter_map(|level| {
            line.find(level).filter(|&index| {
                index == 0
                    || line[..index]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace)
            })
        })
        .min()
        .map_or(line, |index| &line[index..])
}

#[cfg(test)]
mod proptests {
    //! Property tests for the normalisation helpers.
    //!
    //! These prove the prefix-stripping and suffix-preserving contract holds
    //! over arbitrary log lines, complementing the fixed event snapshots.

    use proptest::prelude::*;

    use super::{normalise_event_lines, stable_event_start};

    proptest! {
        /// The normalised event start is always a suffix of the input line, and
        /// re-normalising it is a no-op (the level already sits at the front).
        #[test]
        fn stable_event_start_returns_idempotent_suffix(line in "[^\n]*") {
            let start = stable_event_start(&line);
            prop_assert!(line.ends_with(start));
            prop_assert_eq!(stable_event_start(start), start);
        }

        /// Given a synthetic `tracing-test` line — a volatile prefix, the event
        /// level, then stable content — the prefix is stripped while the level
        /// and every following field survive verbatim. The prefix charset omits
        /// the letters in `TRACE`/`DEBUG`, so it cannot masquerade as a level.
        #[test]
        fn stable_event_start_strips_prefix_and_keeps_event(
            prefix in "[0-9TZ:. -]+ ",
            level in prop_oneof![Just("TRACE"), Just("DEBUG")],
            rest in "[^\n]*",
        ) {
            let stable = format!("{level} {rest}");
            let line = format!("{prefix}{stable}");
            prop_assert_eq!(stable_event_start(&line), stable.as_str());
        }

        /// `normalise_event_lines` keeps exactly the lines containing `message`,
        /// leaves no trailing whitespace, and neither invents nor drops lines.
        #[test]
        fn normalise_event_lines_filters_and_trims(
            lines in prop::collection::vec("[^\n]*", 0..8),
            message in "[^\n]{1,4}",
        ) {
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            let out = normalise_event_lines(&refs, &message);
            let matching = refs.iter().filter(|l| l.contains(message.as_str())).count();

            if matching == 0 {
                prop_assert!(out.is_empty());
            } else {
                prop_assert_eq!(out.split('\n').count(), matching);
                for segment in out.split('\n') {
                    prop_assert_eq!(segment, segment.trim_end());
                }
            }
        }
    }
}
