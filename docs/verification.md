# Verification ledger

`mdtablefix` uses Verus to prove narrow kernels that the formatter calls in
production. This ledger records every claimed proof result, its executable
function, input domain, external contracts, and result class. A kernel may not
be represented by a separate `proofs/` implementation unless a refinement proof
connects it to the production function.

## Current status

The pinned Verus release cannot compile `&str` range operations inside
`verus!`. Production `classify_line` therefore converts the line to Unicode
scalars and delegates to the executable `classify_seq` body included by
`verus/lib.rs`. Its postcondition proves the returned class equals
`spec_classify(chars@, ctx@)` for every context and class, and proves the body
offset is in bounds. The context view includes fence state, previous class, and
prefix agreement. Leading tabs and four spaces are proved literal.

The Setext and canonical-break consumer predicates call the same verified
kernel. The production Setext conversion checks its assembled output through
`is_atx_heading_seq` before emitting it, so every emitted replacement has the
ATX class. The proof also establishes that a canonical seventy-underscore line
classifies as a thematic break. The residual block matcher and Rust `String`
assembly remain outside the proof boundary; the output check makes their effect
on the emitted structural class explicit. The break pass also uses the residual
block matcher when carrying prior-line context, so link and footnote
definitions are not treated as paragraph text.

## Claim ledger

| Claim                                             | Executable function       | Input domain                                         | Unverified external contracts                                                | Result class                                                        |
| ------------------------------------------------- | ------------------------- | ---------------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| Structural classification and bounded body offset | `classify_seq`            | All `&[char]` lines and `ClassifyCtxKernel` values   | Table-delimiter grammar, thematic-break grammar, ordered-list marker grammar | Exact `LineClass` and scalar offset                                 |
| Setext text decision                              | `is_setext_text_seq`      | All lines and classifier contexts                    | Same three scanner matcher contracts                                         | Boolean iff `spec_classify` is `ParagraphText`                      |
| Setext underline decision                         | `is_setext_underline_seq` | All lines and classifier contexts                    | Same three scanner matcher contracts                                         | Boolean iff `spec_classify` is `SetextUnderline`                    |
| Emitted Setext ATX check                          | `is_atx_heading_seq`      | All generated lines and classifier contexts          | Same three scanner matcher contracts                                         | Boolean iff `spec_classify` is `AtxHeading`                         |
| Canonical-break decision                          | `is_canonical_break_seq`  | All lines and classifier contexts                    | Same three scanner matcher contracts                                         | Boolean iff `spec_classify` is `ThematicBreak`                      |
| Canonical seventy-underscore sequence             | `is_canonical_break_seq`  | The `canonical_break` sequence under default context | Same three scanner matcher contracts                                         | Thematic-break class; runtime string assembly remains outside Verus |

_Table 1: The verification claim ledger._

## Policy

- `#[verifier::external_body]` wrappers may state contracts for bounded matcher
  recognition, Unicode display width, and line breaking. The classifier relies
  on exact sequence contracts for table delimiters, thematic breaks, and
  ordered-list markers. These contracts are trusted until their bodies are
  verified. They must not assert the property that a proof is meant to
  establish, such as reparsing block preservation or wrapping idempotence.
  Every external contract appears in a claim row before a proof relies on it.
- Specifications use distinct types for byte offsets, Unicode scalar indices,
  and display columns. No proof substitutes `String::len()` for display width.
- Existing property tests remain in place. Verus proofs complement them and do
  not replace them.
- `make verus-mutation` changes the production Setext predicate to accept ATX
  headings and checks that its refinement postcondition fails.

The ledger check invoked by `make lint` rejects a claim whose executable
function name does not occur in `src/`.
