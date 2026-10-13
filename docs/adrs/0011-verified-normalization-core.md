# Architectural decision record (ADR) 0011: Verify a narrow normalization core

## Status

Accepted.

## Date

2026-09-12.

## Context and problem statement

`mdtablefix` composes several string-processing passes. Property tests exercise
their observable behaviour, but cannot establish the unbounded preservation and
fixed-point obligations tracked by the formal-verification issue set. A proof
harness must make those obligations executable in continuous integration
without creating a second formatter that can drift from production.

Verus cannot annotate the whole string-processing pipeline without pulling I/O,
regex engines, Unicode-width libraries, and line-breaking implementations into
one proof boundary. Those dependencies are not the narrow normalization logic
the formatter needs to establish first.

## Decision drivers

- A claimed theorem must apply to the executable function the formatter calls.
- Continuous integration must reject both unproved obligations and a harness
  that accidentally stops checking proofs.
- External dependencies must have explicit, limited contracts rather than
  silently supplying the result being proved.
- Specifications must keep byte offsets, Unicode scalar indices, and display
  columns distinct.

## Options considered

### Option A: Verify a narrow production-used core

Include a production kernel from `verus/lib.rs` with `#[path]`, or connect an
adapted proof boundary to that function with a refinement proof. Keep I/O and
library boundaries outside the kernel, with their assumptions stated in the
verification ledger.

### Option B: Maintain a separate `proofs/` re-implementation

Model the normalizer separately from the formatter and prove properties of the
model.

### Option C: Annotate the whole string pipeline

Place Verus annotations throughout parsing, regex recognition, Unicode width,
line breaking, and formatter orchestration.

| Topic                      | Option A | Option B             | Option C       |
| -------------------------- | -------- | -------------------- | -------------- |
| Applies to formatter code  | yes      | only with refinement | yes            |
| Proof boundary             | narrow   | separate model       | whole pipeline |
| Drift risk                 | low      | high                 | low            |
| Initial proof cost         | focused  | focused              | high           |
| External contracts visible | ledger   | model-specific       | widespread     |

_Table 1: Comparison of verification-boundary options._

## Decision outcome

Choose Option A. `verus/lib.rs` is the only production proof entry point and
documents the `#[path]` convention for future kernels. `make verus` invokes the
pinned Verus release through `rust-prover-tools`, and CI runs the same target
on every pull request. `make verus-selftest` runs a deliberately false proof
and requires Verus to reject it.

The verification ledger records the executable function, its input domain,
every unverified external contract, and whether each result is local
correctness, cross-pass preservation, or byte-level idempotence.

An `#[verifier::external_body]` wrapper may describe regex recognition, Unicode
display width, or line breaking. It must not assert the property under proof.
Specifications model byte offsets, Unicode scalar indices, and display columns
as distinct types, and never use `String::len()` as display width.

## Consequences

- The first kernels for ellipsis normalization and `ProcessBuffer::finish` can
  land incrementally without duplicating formatter logic.
- Each claim receives a reviewable scope and a recorded list of trusted
  boundary contracts.
- Property tests remain the primary broad-input regression defence; proofs add
  unbounded guarantees for their stated kernels.
- Proof work must structure production code around a small, pure kernel when a
  directly included function is not Verus-compatible.

## Known risks and limitations

- `#[path]` inclusion needs deliberate maintenance when a production module
  changes language features Verus does not support.
- An external contract can still be too strong. The ledger and code review must
  reject a contract that restates the theorem under proof.
- The harness contains no kernel theorem until issues #491 and #483 add their
  production-used code, so it currently proves toolchain resolution and
  non-vacuity rather than formatter semantics.

## Addendum (2026-09-24)

The final bullet of "Known risks and limitations" above is retired. It said the
harness contained no kernel theorem until issues #491 and #483 added their
production-used code, and that it therefore proved toolchain resolution and
non-vacuity rather than formatter semantics. A production-linked kernel theorem
has since landed, so the claim no longer holds.

`verus/lib.rs` includes `src/classify_kernel.rs` by `#[path]`, so the
specification is written against the same source the binary compiles.
`classify_seq` proves its result against `spec_classify`, the structural
precedence the kernel must implement, and its postcondition also bounds the
returned body offset within the input sequence.

The consumer decisions are verified separately over that same kernel.
`is_setext_pair_seq` composes the paragraph-text and underline checks used by
the production Setext detector. `convert_setext_seq` proves construction and
classification of the complete line for a valid retained prefix, level one or
two, arbitrary supplied text, and the scalar capacity bound
`prefix.len() + text.len() + 4 <= usize::MAX`. Its output is `AtxHeading` in
every outside-fence context and is not `ParagraphText` in canonical context.
`convert_accepted_setext_seq` is the production conversion wrapper: it checks
the actual candidate and underline classes under their supplied contexts, and
prefix admission, then returns `Some` exactly when those conditions hold. Its
result is the exact assembled line with the same outside-fence classification
guarantee. `lemma_setext_prefix_exists` witnesses empty, three-space, single
quote and nested-quote prefixes and applies construction at both levels. Native
`String` parsing and candidate-text extraction are not refined to scalar
sequences; regressions check the exact retained content and output
classification. `detect_verified_setext_heading` keeps a runtime classifier
check as a checked-output safeguard. The break pass builds its shared
`canonical_break()` value from `canonical_break_chars`, whose loop contract
fixes its length and contents. `lemma_canonical_break_remains_structural`
proves that the exact unprefixed seventy-underscore sequence is `ThematicBreak`
under every outside-fence context, for any preceding class or prefix-agreement
value. The four consumer lemmas `lemma_canonical_break_ends_wrapping`,
`lemma_canonical_break_is_not_setext_text`,
`lemma_canonical_break_is_not_table_input`, and
`lemma_canonical_break_is_not_orphan_specifier` cover the corresponding
wrapping, heading, table and orphan-specifier decisions. Concrete witness
lemmas assert paragraph text, a table delimiter and the canonical break.

Fixture compatibility has two distinct records. The checked-in snapshot pins
each `tests/data` line's class with its relative path, line number and source
hash. An independent regex reference in `src/classify_reference_tests.rs`
derives its expressions from revision `5c3b18e`, the parent of the
shared-classifier introduction `01b5a2a`, then adapts them to the unified
contract. It carries its own prefix, fence and preceding-class state and shares
only `LineClass` with production. Named regressions document deliberate
differences from historical callers, including ATX separators and levels,
ordered-list digit and whitespace limits, Markdown whitespace, quoted-body
roles, malformed table delimiters, contextual Setext underlines and
tab-prefixed thematic-break spellings.

The Setext and break consumer lemmas establish class-based rejection in the
specified outside-fence contexts. Prefix and enclosing-container break
preservation is outside this domain and tracked by issue #580. Separate
executable regressions send the exact canonical break through wrapping, heading
detection, table buffering, and orphan attachment; the ledger distinguishes
these cross-pass checks from local kernel correctness. `make verus-mutation`
removes the mandatory separator from the production `convert_setext_seq` kernel
and requires Verus to reject its complete output-classification postcondition.
The mutation guards full assembly, rather than only the separator's character
and length.

Five matcher bodies across three grammar groups remain trusted:
`is_table_delimiter`, `table_cells_are_delimiters`, `is_table_delimiter_cell`,
`is_thematic_break` and `ordered_list_item`. They are listed as unverified
external contracts in the claim ledger in `docs/verification.md` and tracked by
[issue #567](https://github.com/leynos/mdtablefix/issues/567);
none is claimed as verified. Broader preservation of breaks inside enclosing
containers is tracked separately by
[issue #580](https://github.com/leynos/mdtablefix/issues/580).
