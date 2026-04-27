//! Orphan fence specifier attachment helpers.

use super::{FENCE_RE, is_null_lang};
use crate::wrap::FenceTracker;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum AttachmentOutcome {
    Attached,
    Preserved,
}

#[derive(Debug, PartialEq, Eq)]
enum NextFence {
    Attachable { blank_count: usize },
    NotAttachable { blank_count: usize },
}

/// Combine an opening fence with a language specifier.
///
/// The fence's indentation is retained whenever present. If the specifier's
/// indentation extends the fence's, the deeper specifier indentation is used.
/// When the fence lacks indentation, the specifier's indentation becomes the fence's.
/// If the indentations differ without one extending the other (e.g., tabs vs spaces),
/// the fence's indentation wins.
///
/// # Examples
///
/// ```rust,ignore
/// use mdtablefix::fences::attach_specifier_to_fence;
/// assert_eq!(attach_specifier_to_fence("```", "rust", "  "), "  ```rust");
/// assert_eq!(attach_specifier_to_fence("  ```", "rust", "    "), "    ```rust");
/// ```
fn attach_specifier_to_fence(fence_line: &str, specifier: &str, spec_indent: &str) -> String {
    let Some(cap) = FENCE_RE.captures(fence_line) else {
        return fence_line.to_owned();
    };
    let fence_indent = cap.get(1).map_or("", |m| m.as_str());
    let fence_marker = cap.get(2).map_or("```", |m| m.as_str());
    let final_indent = if fence_indent.is_empty() || spec_indent.starts_with(fence_indent) {
        spec_indent
    } else {
        fence_indent
    };
    format!("{final_indent}{fence_marker}{specifier}")
}

fn next_attachable_fence<'a, I>(mut lines: std::iter::Peekable<I>) -> NextFence
where
    I: Iterator<Item = &'a String>,
{
    let mut blank_count = 0;
    while let Some(next_line) = lines.peek() {
        if next_line.trim().is_empty() {
            blank_count += 1;
            let _ = lines.next();
            continue;
        }

        let is_attachable = FENCE_RE
            .captures(next_line)
            .is_some_and(|captures| is_null_lang(captures.get(3).map_or("", |m| m.as_str())));
        return if is_attachable {
            NextFence::Attachable { blank_count }
        } else {
            NextFence::NotAttachable { blank_count }
        };
    }

    NextFence::NotAttachable { blank_count }
}

/// Attach an orphan specifier to the next attachable fence.
///
/// The lookahead step is pure: it clones the iterator and reports whether an
/// unlabelled fence follows after only blank lines. This command step then
/// consumes the original iterator, mutates the output buffer, and advances the
/// structural fence tracker only when it actually consumes a fence.
pub(super) fn attach_to_next_fence<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    specifier: &str,
    indent: &str,
    out: &mut Vec<String>,
    specifier_line: &str,
    tracker: &mut FenceTracker,
) -> AttachmentOutcome
where
    I: Iterator<Item = &'a String> + Clone,
{
    match next_attachable_fence(lines.clone()) {
        NextFence::Attachable { blank_count } => {
            for _ in 0..blank_count {
                let _ = lines.next();
            }
            if let Some(fence_line) = lines.next() {
                out.push(attach_specifier_to_fence(fence_line, specifier, indent));
                let _ = tracker.observe(fence_line);
            }
            AttachmentOutcome::Attached
        }
        NextFence::NotAttachable { blank_count } => {
            out.push(specifier_line.to_string());
            for _ in 0..blank_count {
                if let Some(blank_line) = lines.next() {
                    out.push(blank_line.clone());
                }
            }
            AttachmentOutcome::Preserved
        }
    }
}
