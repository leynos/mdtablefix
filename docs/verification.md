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
block matcher when carrying prior-line context, so standalone link and footnote
definitions are not treated as paragraph text. A link-shaped line following
paragraph text at the same blockquote depth remains paragraph continuation.

`src/wrap/fence/kernel.rs` is the pure fence transition kernel, included by
`verus/lib.rs` through `#[path]` so the proofs constrain the body the formatter
runs. Recognition is excluded from the proof build: the kernel's regex-facing
entry points are `#[cfg(not(verus_keep_ghost))]`, exactly as the classifier's
are, so a line arrives already reduced to pre-parsed features. The kernel's
whole-document `regions` is proved to equal the seeded-prefix specification
element for element, and `compress_fences` consults the same
`compression_changes_region` predicate the proofs are stated over, so the batch
classification and the rewrite decision cannot hold different notions of what a
delimiter is.

The region-preservation theorem is one-sided, and that is the honest form: a
tilde line closes a tilde opener but cannot close the three-backtick opener the
pass writes in its place, so the two runs legitimately disagree about delimiter
identity. What the theorem establishes is that no line moves between the
literal and prose regions -- delimiter spelling may change, payload regions may
not. `lemma_witness_interior_is_literal` exhibits an interior satisfying every
hypothesis and pins the regions both runs agree on, so the conclusion is not an
empty sequence, and `lemma_witness_conflict_is_rejected` proves the guard fires
on the issue #480 shape. `tests/fence_regions.rs` replays the same reasoning
over every fixture.

## Claim ledger

| Claim                                             | Executable function          | Input domain                                         | Unverified external contracts                                                | Result class                                                        |
| ------------------------------------------------- | ---------------------------- | ---------------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| Structural classification and bounded body offset | `classify_seq`               | All `&[char]` lines and `ClassifyCtxKernel` values   | Table-delimiter grammar, thematic-break grammar, ordered-list marker grammar | Exact `LineClass` and scalar offset                                 |
| Setext text decision                              | `is_setext_text_seq`         | All lines and classifier contexts                    | Same three scanner matcher contracts                                         | Boolean iff `spec_classify` is `ParagraphText`                      |
| Setext underline decision                         | `is_setext_underline_seq`    | All lines and classifier contexts                    | Same three scanner matcher contracts                                         | Boolean iff `spec_classify` is `SetextUnderline`                    |
| Emitted Setext ATX check                          | `is_atx_heading_seq`         | All generated lines and classifier contexts          | Same three scanner matcher contracts                                         | Boolean iff `spec_classify` is `AtxHeading`                         |
| Canonical-break decision                          | `is_canonical_break_seq`     | All lines and classifier contexts                    | Same three scanner matcher contracts                                         | Boolean iff `spec_classify` is `ThematicBreak`                      |
| Canonical seventy-underscore sequence             | `is_canonical_break_seq`     | The `canonical_break` sequence under default context | Same three scanner matcher contracts                                         | Thematic-break class; runtime string assembly remains outside Verus |
| Fence closing-rule decision                       | `closes_fence`               | All opener states and parsed line features           | Fence recognition and blockquote parsing                                     | Boolean iff `spec_closes`                                           |
| Would-be-closer decision                          | `agrees_with_opener`         | All opener states and parsed line features           | Fence recognition and blockquote parsing                                     | Boolean iff `spec_agrees_with_opener`                               |
| Interior-delimiter decision                       | `interior_delimiter`         | All opener states and parsed line features           | Fence recognition and blockquote parsing                                     | Boolean iff `spec_interior_delimiter`                               |
| Compression-conflict decision                     | `compression_changes_region` | All opener states and parsed line features           | Fence recognition and blockquote parsing                                     | Boolean iff `spec_compression_changes_region`                       |
| Compressed delimiter state                        | `compressed`                 | All opener states                                    | None                                                                         | Exact state; three backticks at the opening depth                   |
| Fence transition and line region                  | `fence_step`                 | All opener states and parsed line features           | Fence recognition and blockquote parsing                                     | Exact next state and `Region`                                       |
| Whole-document region classification              | `regions`                    | Every `LineFeatures` slice                           | Fence recognition and blockquote parsing                                     | Exact `Region` sequence, element for element                        |
| Region preservation under delimiter compression   | `regions`                    | A block interior meeting the compression guard       | Fence recognition and blockquote parsing                                     | Equal region sequences under the original and compressed openers    |

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
- `make verus-fence-mutation` drops the marker-character comparison from
  `closes_fence` and checks that the failed obligation names that function's
  contract against `spec_closes`. The mutation leaves a plausible predicate
  behind -- it still tests depth, run length, and trailing whitespace -- so a
  gate that accepted it would show the fence proofs rested on something other
  than the closing rule they claim to establish.

The ledger check invoked by `make lint` rejects a claim whose executable
function name does not occur in `src/`.
