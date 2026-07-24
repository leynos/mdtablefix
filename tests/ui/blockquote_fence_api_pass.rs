//! Compile-pass fixture: the public `BlockquotePrefix`, `FenceTracker`, and
//! `is_fence` API introduced for semantic blockquote handling stays usable by
//! downstream callers, with its documented depth-aware semantics intact.

use mdtablefix::wrap::{BlockquotePrefix, FenceTracker, is_fence};

fn main() {
    // `BlockquotePrefix` borrows the source line, exposing the raw prefix
    // spelling, the nesting depth, and the inner content independently.
    let prefix =
        BlockquotePrefix::parse("> > quoted text").expect("line has a blockquote prefix");
    assert_eq!(prefix.raw_prefix(), "> > ");
    assert_eq!(prefix.depth(), 2);
    assert_eq!(prefix.inner(), "quoted text");
    assert!(BlockquotePrefix::parse("no prefix here").is_none());

    // `FenceTracker` is the depth-aware authority for fenced code-block state.
    let mut tracker = FenceTracker::new();
    assert!(!tracker.in_fence(2));
    assert!(tracker.observe_line("> > ```rust"));
    assert!(tracker.in_fence(2));
    assert!(tracker.in_fence_for_line("> > code"));
    assert!(tracker.observe("```", 2));
    assert!(!tracker.in_fence(2));

    // `is_fence` exposes the structural marker components, spanning the
    // blockquote prefix in the reported indentation.
    assert_eq!(is_fence("> > ```rust"), Some(("> > ", "```", "rust")));
    assert!(is_fence("plain text").is_none());
}
