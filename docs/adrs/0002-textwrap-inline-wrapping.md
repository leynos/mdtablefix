# Architecture Decision Record (ADR) 0002: Delegate inline line fitting to `textwrap`

- Status: Accepted
- Date: 2026-04-22

## Context

The `--wrap` pipeline in `mdtablefix` previously relied on a bespoke
`LineBuffer`-driven loop in `src/wrap/inline.rs` to accumulate tokens into
output lines while preserving inline code spans, Markdown links, and trailing
punctuation. The same module also duplicated prefix-handling logic across
`append_wrapped_with_prefix` and `handle_prefix_line` in
`src/wrap/paragraph.rs`.

Several problems motivated a replacement:

- The `LineBuffer` implementation mixed display-width and byte-length
  calculations in a way that was difficult to audit.
- The whitespace-carry and split-boundary logic was tightly coupled to
  individual token types, making it hard to extend without introducing
  regressions.
- Prefix width was computed twice (once for the first line, once for
  continuation lines) using subtly different code paths, risking drift between
  the two calculations.
- The approach did not use any battle-tested line-breaking library; every edge
  case had to be handled bespoke.

## Decision

The inline line-fitting step now delegates to
`textwrap::wrap_algorithms::wrap_first_fit`, accepting pre-grouped
`InlineFragment` values that implement `textwrap::core::Fragment`.

Key design choices:

1. **Fragment model** — tokens are grouped into `InlineFragment` values before
   wrapping. Each fragment stores its rendered text, precomputed
   display-column width (`UnicodeWidthStr::width`), and a `FragmentKind`
   discriminant (`Whitespace`, `InlineCode`, `Link`, `Plain`). This keeps
   Markdown-aware grouping under repository control while delegating the
   actual line-fitting arithmetic to `textwrap`.

2. **Greedy algorithm** — `wrap_first_fit` was chosen over the optimal-fit
   algorithm because Markdown wrapping must produce deterministic, line-by-line
   output that matches the existing tests. Optimal fit would require look-ahead
   that changes the wrapping of earlier lines based on later content.

3. **Post-processing passes** — two passes normalise the raw fit output:
   `merge_whitespace_only_lines` absorbs whitespace-only separator lines back
   into adjacent content lines, and `rebalance_atomic_tails` moves trailing
   atomic or plain fragments to the following line when the destination line
   can accommodate them within the target width. Both passes are
   width-constrained so they cannot create lines that `wrap_first_fit` would
   have rejected.

4. **Unified prefix helper** — `ParagraphWriter::wrap_with_prefix` computes
   available content width once from the display width of the prefix string,
   then emits first-line and continuation-line prefixes from the same code
   path. `append_wrapped_with_prefix` and `push_wrapped_segment` both delegate
   to this helper.

5. **Public API stability** — `wrap_text`, `Token`, and `tokenize_markdown`
   remain unchanged. The `tokenize_markdown` public API is explicitly out of
   scope for this change because it is used by `src/code_emphasis.rs`,
   `src/footnotes/mod.rs`, `src/footnotes/renumber.rs`, and `src/textproc.rs`.

6. **Dead code removal** — `src/wrap/line_buffer.rs` is deleted because it is
   no longer reachable from the active wrap path after the fragment-based
   implementation is complete.

## Consequences

- Line-fitting arithmetic for inline text is handled by `textwrap`, a
  well-tested dependency, rather than a custom accumulation loop.
- Display-width measurements are centralised through `unicode-width` and passed
  to `textwrap` via the `Fragment` trait, eliminating the earlier mixed
  byte-length / display-width inconsistency.
- The `textwrap` crate (v0.16.2) is added as a dependency. It transitively
  introduces `smawk`, `unicode-linebreak`, and a newer `unicode-width` (v0.2)
  that coexists with the direct `unicode-width` v0.1 dependency.
- The post-processing passes add complexity that did not exist in the
  `LineBuffer` approach. That complexity is warranted because it is narrowly
  scoped, independently testable, and avoids re-implementing the full greedy
  algorithm.
- All existing active wrap tests continue to pass, confirming observable
  behaviour is preserved.