# Architecture Decision Record (ADR) 0013: Keep unspaced inline syntax seams atomic

- Status: Accepted
- Date: 2026-11-13

## Context

`mdtablefix --wrap` reflows a paragraph and the next pass reads the result
back. A line break between two tokens is content, not an invisible boundary:
when the next pass joins the lines it inserts a space where the break was. Any
seam the formatter must not widen therefore has to stay inside one atomic span
for the whole wrap, or the document never settles.

Inline span grouping already protected several seams. Opening punctuation was
coupled forward to a following inline code span or link, footnote references
were coupled to their openers, and trailing punctuation was absorbed into a
preceding code span, link, or reference. One class was protected by a narrower
rule still: a token ending in a hyphen and containing at least one alphabetic
character — `pre-`, `LLM-`, `(API-` — was coupled forward by the
`ends_with_hyphen_prefix` predicate so compounds such as `` pre-`LLMPort` ``
stayed whole.

That predicate answered the wrong question. The seam that must not break is any
unspaced run, not the hyphenated subset of it. `` **bold**`code` `` has no
hyphen and split anyway, and the resulting space was written into the document
on the join. The enumerate-the-shapes approach also meant every newly reported
seam needed a new predicate and new test coverage.

## Decision

Couple an unspaced token directly before an inline code span to that span.
`initial_token_span` in `src/wrap/inline/mod.rs` applies the rule: when the
current token is not whitespace and the next token is an inline code token, the
pair is grouped as one `SpanKind::Code` span and trailing punctuation is
absorbed into it. The rule replaces `ends_with_hyphen_prefix`, which is deleted
along with its predicate-tracing entry and its unit tests.

Whitespace remains a break opportunity. The rule tests the token itself, not
its neighbourhood, so separately spaced inline code spans still wrap
independently — `--wrap` can break between `.toml`, `.json`, and `.yaml`.

The rule covers the seam that broke, which is the one this layer had reached;
it is deliberately not a general "never break adjacent inline syntax" policy.
Other adjacent-construct seams remain out of scope until a reported case
reaches them.

## Consequences

- Unspaced runs before an inline code span survive a wrap unchanged, so
  `` **bold**`code` `` keeps its fixed point. A regression fixture records this
  under the mask-71 real-CLI case.
- The predicate set shrinks: one fewer exported predicate, one fewer tracing
  row in the instrumentation table, and one fewer unit-test module.
- The rule is stated in terms of token class rather than token spelling, so a
  future seam does not need a bespoke predicate to be protected.
- Longer runs stay atomic even when a break inside them would be legal
  Markdown — a break there would insert a space, so there is no legal break to
  preserve.
- Grouping an unspaced run can exceed the wrap width where the pieces alone
  would have fitted; the existing long-token fallback applies, as it does to
  every other atomic fragment.

## Alternatives considered

- **Keep `ends_with_hyphen_prefix` and add a second predicate for**
  `**bold**`. Rejected: it protects the reported instance and leaves the next
  unspaced run to break the same way, which is the recurrence the class invites.
- **Detect emphasis markers specifically.** Rejected: `**bold**` is one
  spelling of "not whitespace, not a code span". Testing the class rather than
  the spelling costs nothing and closes the whole family.
- **Couple all adjacent inline syntax tokens, including links.** Rejected for
  this decision: it widens scope past the reported seam without a case that
  needs it. The code-span rule is the narrow change that fixes the class
  observed.
