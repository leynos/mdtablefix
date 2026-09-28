# Architecture Decision Record (ADR) 0002: Delegate line fitting to `textwrap`

- Status: Accepted
- Date: 2026-04-22

## Context

The previous wrapping engine in `src/wrap/line_buffer.rs` implemented a bespoke
`LineBuffer` struct that accumulated tokens, tracked a split-point cursor, and
flushed completed lines one at a time. This approach had three compounding
problems:

- Width measurement was byte-based in early versions, producing incorrect splits
  for non-ASCII characters such as CJK glyphs and emoji.
- The split-with-carry logic required carefully coordinated state between
  `push_span`, `split_with_span`, and `flush_trailing_whitespace`, making the
  code difficult to reason about and extend.
- Each fragment addition triggered a full re-evaluation of the buffer, risking
  quadratic behaviour on long paragraphs.

## Decision

Replace `LineBuffer` with `textwrap::wrap_algorithms::wrap_first_fit` and a
fragment model built on the `textwrap::core::Fragment` trait. Each token group
becomes an `InlineFragment` that carries pre-computed display width (via
`unicode-width`) and a `FragmentKind` tag. `wrap_first_fit` performs greedy
line fitting over the fragment slice; post-processing in
`src/wrap/inline/postprocess.rs` normalizes whitespace-only lines and
rebalances atomic tails. Prefix handling is centralized in
`ParagraphWriter::wrap_with_prefix`, which computes available width once and
prepends the correct prefix to every wrapped output line.

The greedy first-fit algorithm is chosen over `textwrap`'s optimal-fit
algorithm because the optimal algorithm may produce non-local changes to
earlier lines when a later fragment is added, which conflicts with the
incremental buffer model and produces surprising diffs.

## Consequences

Positive:

- Line fitting is delegated to a well-tested upstream crate; the bespoke split
  logic and `LineBuffer` state machine are removed entirely.
- Display widths are computed by `unicode-width` according to Unicode Standard
  Annex `#11`, giving correct column counts for non-ASCII text.
- `InlineFragment::kind` centralizes token classification, so post-processing
  predicates (`is_whitespace`, `is_atomic`, `is_plain`) do not repeat
  classification logic.

Negative:

- Greedy first-fit produces wider first lines than optimal-fit would in some
  cases, though this difference is not visible in standard Markdown prose.
- The project now depends on `textwrap 0.16` in addition to `unicode-width`.

## Alternatives considered

- **Optimal-fit algorithm** (`textwrap::wrap_algorithms::wrap_optimal_fit`):
  rejected because it requires the complete fragment list upfront and may
  redistribute earlier lines when later fragments are added, which conflicts
  with the streaming model.
- **Patching `LineBuffer` for Unicode correctness**: rejected because the
  split-point cursor and carry semantics remained inherently fragile; the
  maintenance burden outweighed the risk of introducing a new dependency.

## Addendum (2026-09-27)

[Issue 561](https://github.com/leynos/mdtablefix/issues/561) showed that the
fragment model this ADR adopted let `textwrap` break between any two fragments,
including two that touch in the source. Markdown renders a soft line break as a
space, so such a break added a space to the rendered text:
`([Python Packaging][4])` became `( Python Packaging)`. From
[pull request 271](https://github.com/leynos/mdtablefix/pull/271) onwards,
attached constructs had been protected one coupling at a time (opening
punctuation before code or inline links, hyphen prefixes, digit bracket
references), and every uncoupled shape stayed breakable.

The rule is now structural. After grouping, `join_touching_fragments` merges
each run of fragments with no whitespace at its seams, so a break can fall only
at source whitespace. `render_line` trims every trailing space from a line the
wrapper broke, since two or more would render as a hard break; only the final
line keeps the source's trailing spaces. A run with no whitespace is never
split, even when that leaves a line wider than the target, which is the same
trade-off this ADR already accepts for atomic code spans and links. The
per-construct couplings stay, because `FragmentKind` still drives tail
rebalancing.
