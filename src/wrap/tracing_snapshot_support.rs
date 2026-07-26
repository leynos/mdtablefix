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
