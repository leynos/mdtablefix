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

The production Setext path calls `is_setext_pair_seq` with the candidate and
underline contexts. That wrapper proves an accepted pair has `ParagraphText` and
`SetextUnderline` classes. `convert_setext_seq` proves the production assembly
for level one or two, a valid retained prefix and arbitrary text, including
`AtxHeading` in canonical context and not `ParagraphText`.
`convert_accepted_setext_seq` is the production acceptance wrapper: it
classifies the actual candidate and underline with their supplied contexts,
checks `valid_setext_prefix`, then calls `convert_setext_seq`. For level one or
two and `prefix.len() + text.len() + 4 <= usize::MAX`, its postcondition says
the result is `Some` exactly when both source roles and prefix admission hold;
the `Some` value is the exact assembled heading and classifies as `AtxHeading`
in every outside-fence context. Since the output is not `ParagraphText`,
`is_setext_text_seq` also rejects it as a later-pass candidate. The native
`String` parsing and content extraction in `detect_setext_heading` are not
refined to scalar sequences; their exact output and classification remain
covered by regressions. `detect_verified_setext_heading` retains its runtime
`is_atx_heading_line` guard as a checked-output safeguard. Executable
regressions cover both levels with plain, indented, quoted and nested-quoted
prefixes. `lemma_setext_prefix_exists` proves non-vacuity for an empty prefix,
three-space indentation, a single quote and a nested quote, and exercises both
heading levels for those prefix forms.

The break pass builds `canonical_break()` from `canonical_break_chars`; the
Verus loop postcondition proves the builder returns exactly seventy underscores.
`lemma_canonical_break_remains_structural` proves that the production
`classify_seq` classifies the exact unprefixed sequence as `ThematicBreak` in
every outside-fence context, regardless of preceding class or prefix-agreement
value. Four consumer lemmas prove that wrapping retains it as a thematic-break
boundary and that Setext candidate, table-input and orphan specifier acceptance
reject it: `lemma_canonical_break_ends_wrapping`,
`lemma_canonical_break_is_not_setext_text`,
`lemma_canonical_break_is_not_table_input`, and
`lemma_canonical_break_is_not_orphan_specifier`. Their executable wrappers
refine those actual consumer decisions. Preservation after adding a quote or
indentation prefix, and preservation inside enclosing containers, remain
tracked by issue #580.

The production consumer paths also have regression coverage using that exact
canonical break: wrapping, Setext heading detection, table buffering, and
orphan-specifier attachment. Their production decisions call
`wrapping_boundary_seq`, `is_setext_pair_seq`, `is_table_class`, and
`can_be_orphan_specifier_seq`, respectively. `src/classify_fixture_tests.rs`
compares every `tests/data` line with both an independently stateful regex
reference in `src/classify_reference_tests.rs` and a checked-in snapshot. The
reference derives its fence, bullet and thematic-break expressions from revision
[`5c3b18e`](https://github.com/leynos/mdtablefix/commit/5c3b18e8d0eee078d0b58183c28adadb91e1aeb3),
the parent of the commit that introduced the shared classifier (`01b5a2a`). It
adapts those expressions to the unified classifier contract; the reference
independently scans prefixes and carries its own fence, preceding-class and
prefix state. It shares only the `LineClass` enum with production. The snapshot
records relative path, line number, FNV-1a source-line hash and classification.
It pins reviewed results but is not itself the independent compatibility
evidence.

The differential test names and checks intentional differences from the
historical callers: ATX headings require a valid separator and at most six
hashes; ordered markers use at most nine ASCII digits and a Markdown space;
Markdown whitespace is narrower than Unicode trimming; quote content is
classified by its body rather than as a residual blockquote; malformed
delimiter cells rejected by the delimiter grammar remain table rows or
paragraphs; Setext underlines depend on preceding paragraph and matching-prefix
context; and a tab-prefixed thematic-break spelling is `Literal`, while the
formatter leaves that source line unchanged. These cases have named regressions
in the reference test module. The residual block matcher used for link and
footnote definitions remains outside this classifier proof.

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

Non-vacuity is otherwise carried by the witness lemmas beside each obligation,
which state their conclusions as values rather than as equalities between two
unknowns. In `verus/plan_spec.rs`, `lemma_witness_agreement_is_live` shows the
reading-agreement hypothesis is satisfiable,
`lemma_witness_compressed_block_is_live` pins a compressed block's three
regions one by one, and `lemma_witness_conflict_moves_region` is issue #480 as
a pair of region values. Three further witnesses pin rules the proof leans on
without which it would be silently weaker:
`lemma_witness_marker_family_holds_the_region` fixes what the marker comparison
the fence mutation gate drops actually decides,
`lemma_witness_depth_drop_releases_the_block` fixes the depth check that ends a
block before its line is read, and
`lemma_witness_incompatible_endpoints_forbid_rewrite` fixes the eligibility
rule that refuses a rewrite when only one end of a block may be respelled. Each
was checked to be falsifiable rather than merely true: removing the
transition's depth check from `verus/fence_spec.rs` makes the run fail. These
lemmas live under `verus/` and so appear in the prose above rather than in the
claim table, whose rows must name a symbol that occurs in `src/`.

## Claim ledger

| Claim                                             | Executable function                             | Input domain                                                                                                           | Unverified external contracts                                                                                           | Result class                                                                                                                                            |
| ------------------------------------------------- | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Structural classification and bounded body offset | `classify_seq`                                  | All `&[char]` lines and `ClassifyCtxKernel` values                                                                     | `is_table_delimiter`, `table_cells_are_delimiters`, `is_table_delimiter_cell`, `is_thematic_break`, `ordered_list_item` | Exact `LineClass` and scalar offset (local correctness)                                                                                                 |
| Setext pair acceptance                            | `is_setext_pair_seq`                            | Candidate and underline with their classifier contexts                                                                 | Same five trusted matcher contracts                                                                                     | Acceptance implies `ParagraphText` candidate and `SetextUnderline` underline (local correctness)                                                        |
| Setext candidate decision and later-pass refusal  | `is_setext_text_seq`                            | All lines and classifier contexts, including completed `convert_accepted_setext_seq` output                            | Same five trusted matcher contracts                                                                                     | Acceptance implies `ParagraphText`; therefore the completed `AtxHeading` output is rejected as a candidate (local correctness)                          |
| Setext underline component predicate              | `is_setext_underline_seq`                       | All lines and classifier contexts; used by `is_setext_pair_seq`                                                        | Same five trusted matcher contracts                                                                                     | Acceptance implies `SetextUnderline` (local correctness; component of pair acceptance)                                                                  |
| Prefix separation                                 | `needs_space_after_seq`                         | Any retained Unicode-scalar prefix                                                                                     | None                                                                                                                    | Determines whether the retained prefix needs a separator (local correctness)                                                                            |
| Prefix admission                                  | `valid_setext_prefix`                           | Any retained indentation and quote prefix                                                                              | Same five trusted matcher contracts                                                                                     | Paragraph probe starts after the retained prefix (local correctness)                                                                                    |
| Complete Setext output construction               | `convert_setext_seq`                            | Valid prefix, levels one and two, arbitrary supplied text, and capacity bound                                          | Same five trusted matcher contracts                                                                                     | Exact assembled output is `AtxHeading` and not `ParagraphText` in every outside-fence context (local construction correctness)                          |
| Accepted production Setext conversion             | `convert_accepted_setext_seq`                   | Candidate/underline and their actual contexts, valid prefix, levels one/two, and capacity bound                        | Same five trusted matcher contracts                                                                                     | `Some` iff source roles and prefix admission hold; output is exact assembly and `AtxHeading` in every outside-fence context (local correctness)         |
| Emitted Setext ATX check                          | `is_atx_heading_seq`                            | Completed lines and classifier contexts                                                                                | Same five trusted matcher contracts                                                                                     | Classifies a supplied completed line; runtime guard remains a checked-output safeguard (checked-output guarantee)                                       |
| Canonical-break line construction                 | `canonical_break_chars`                         | Empty builder state                                                                                                    | None                                                                                                                    | Exactly seventy underscore characters (local correctness)                                                                                               |
| Canonical-break classification                    | `classify_seq`                                  | Exact unprefixed 70-underscore line under every outside-fence context and any previous class or prefix-agreement value | Same five trusted matcher contracts                                                                                     | `lemma_canonical_break_remains_structural` proves `ThematicBreak` (local correctness)                                                                   |
| Setext pair detection                             | `detect_setext_heading`                         | Production candidate/underline pair with indentation and prefix context                                                | Same five trusted matcher contracts                                                                                     | Calls the proved pair gate and retains production prefix checks                                                                                         |
| Setext production extraction                      | `detect_setext_heading`                         | Eligible pair, actual underline context, indentation and prefix context                                                | Same five trusted matcher contracts                                                                                     | Passes the actual pair contexts and extracted values to the production acceptance wrapper; exact native parsing/content extraction is regression-tested |
| Setext output assembly adapter                    | `convert_setext`                                | Retained prefix, level one or two, and candidate text                                                                  | Same five trusted matcher contracts                                                                                     | Calls `convert_accepted_setext_seq`, which proves the accepted roles and completed output class                                                         |
| Setext checked-output guarantee                   | `detect_verified_setext_heading`                | Completed output line and classifier context                                                                           | Same five trusted matcher contracts                                                                                     | Runtime guard accepts only output classified `AtxHeading`; it is not a construction proof                                                               |
| Wrapping break preservation                       | `wrapping_boundary_seq`                         | Exact unprefixed canonical line under every outside-fence context                                                      | Same five trusted matcher contracts                                                                                     | Returns the thematic-break boundary, not paragraph content; production wrapper and path regression (cross-pass preservation)                            |
| Heading break preservation                        | `is_setext_pair_seq`                            | Exact unprefixed canonical line as candidate under every outside-fence context                                         | Same five trusted matcher contracts                                                                                     | Thematic-break class fails paragraph-candidate role; wrapper lemma and path regression (cross-pass preservation)                                        |
| Table break preservation                          | `is_table_class`                                | Thematic-break class returned for the exact canonical line                                                             | Same five trusted matcher contracts                                                                                     | Rejects the table-input gate; wrapper lemma and path regression (cross-pass preservation)                                                               |
| Orphan-specifier break preservation               | `can_be_orphan_specifier_seq`                   | Exact unprefixed canonical line under every outside-fence context                                                      | Same five trusted matcher contracts                                                                                     | Returns false for thematic-break class; wrapper lemma and path regression (cross-pass preservation)                                                     |
| Fixture classification compatibility              | `fixture_lines_match_the_classification_oracle` | Every `tests/data` line with independently carried fence, preceding-class and prefix state                             | Same five trusted matcher contracts                                                                                     | Differentially compares an independent regex reference and pins path, class and FNV-1a source-line hash (compatibility evidence and regression)         |
| Fence closing-rule decision                       | `closes_fence`                                  | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Boolean iff `spec_closes`                                                                                                                               |
| Would-be-closer decision                          | `agrees_with_opener`                            | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Boolean iff `spec_agrees_with_opener`                                                                                                                   |
| Interior-delimiter decision                       | `interior_delimiter`                            | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Boolean iff `spec_interior_delimiter`                                                                                                                   |
| Compression-conflict decision                     | `compression_changes_region`                    | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Boolean iff `spec_compression_changes_region`                                                                                                           |
| Compressed delimiter state                        | `compressed`                                    | All opener states                                                                                                      | None                                                                                                                    | Exact state; three backticks at the opening depth                                                                                                       |
| Fence transition and line region                  | `fence_step`                                    | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Exact next state and `Region`                                                                                                                           |
| Whole-document region classification              | `regions`                                       | Every `LineFeatures` slice                                                                                             | Fence recognition and blockquote parsing                                                                                | Exact `Region` sequence, element for element                                                                                                            |
| Region preservation under delimiter compression   | `regions`                                       | A block interior meeting the compression guard                                                                         | Fence recognition and blockquote parsing                                                                                | Equal region sequences under the original and compressed openers                                                                                        |
| Block-opener construction                         | `open_block`                                    | Every `LineFacts` value                                                                                                | Fence recognition and blockquote parsing                                                                                | Exact `Option<Block>`; `None` iff the line has no marker                                                                                                |
| Conflict accumulation                             | `absorb`                                        | Every block and optional line features                                                                                 | Fence recognition and blockquote parsing                                                                                | Exact block; opener fixed, flag a disjunction over the lines                                                                                            |
| Planner routing                                   | `plan_step`                                     | Every planner state and `LineFacts` value                                                                              | Fence recognition and blockquote parsing                                                                                | Exact `Step`; the pass's only source of a release                                                                                                       |
| Delimiter rewrite eligibility                     | `delimiters_eligible`                           | Every release and rewrite-candidate pair                                                                               | Fence recognition and blockquote parsing                                                                                | Exact boolean; opener alone, both ends, or neither                                                                                                      |
| Outer-delimiter position decision                 | `is_outer_delimiter`                            | Every position, block length, release, and flag                                                                        | None                                                                                                                    | Exact boolean; position 0, or the last position of a matched block                                                                                      |
| Per-position emission decision                    | `delimiter_agendum`                             | Every position classification and compression answer                                                                   | None                                                                                                                    | Exact `Agendum`; compressed, normalized, or verbatim                                                                                                    |
| Released-block agenda                             | `block_agenda`                                  | Every block length, release, and answers                                                                               | Fence recognition and blockquote parsing                                                                                | Exact `Agendum` sequence of the block's own length                                                                                                      |
| Document region preservation under compression    | `block_agenda`                                  | Every planner-routed document                                                                                          | Fence recognition, blockquote parsing, and the emission read-back contract                                              | Full `Region`-sequence equality over any chain of preserved spans                                                                                       |
| Segment walked at a document offset               | `regions`                                       | Every span and every offset                                                                                            | Fence recognition and blockquote parsing                                                                                | Equal states and equal region sequences over the span's own lines                                                                                       |
| Released block read at a document offset          | `block_agenda`                                  | Every released block at every offset                                                                                   | Fence recognition, blockquote parsing, and the emission read-back contract                                              | The route's obligation: `ends_at` where a state conclusion exists                                                                                       |
| Line inside an open block                         | `fence_step`                                    | Every opener state and non-fence-shaped line                                                                           | Fence recognition and blockquote parsing                                                                                | The line still leaves the fence open; it cannot route as buffer-or-close                                                                                |

_Table 1: The verification claim ledger._

The table's shorthand “Same five trusted matcher contracts” names the five
trusted matcher bodies `is_table_delimiter`, `table_cells_are_delimiters`,
`is_table_delimiter_cell`, `is_thematic_break` and `ordered_list_item`. Each is
reached through the classifier refinement; their verification is tracked by
issue #567.

## Policy

- `#[verifier::external_body]` wrappers may state contracts for bounded matcher
  recognition, Unicode display width, and line breaking. Five matcher bodies
  across the table-delimiter, thematic-break and ordered-list grammar groups
  remain trusted and are tracked by
  [issue #567](https://github.com/leynos/mdtablefix/issues/567). These
  contracts are trusted until their bodies are verified. They must not assert
  the property that a proof is meant to establish, such as reparsing block
  preservation or wrapping idempotence. Every external contract appears in a
  claim row before a proof relies on it.
- Specifications use distinct types for byte offsets, Unicode scalar indices,
  and display columns. No proof substitutes `String::len()` for display width.
- Existing property tests remain in place. Verus proofs complement them and do
  not replace them.
- `lemma_paragraph_exists`, `lemma_delimiter_exists`, and `lemma_break_exists`
  assert concrete examples so the paragraph, delimiter, and thematic-break
  classes are shown to be inhabited.
- `make verus-mutation` removes the mandatory space from the production ATX
  assembly kernel and checks that the completed output-classification
  postcondition fails. The mutation is meaningful only while that
  production-linked output proof is enabled.
- Preservation of breaks inside all enclosing containers is tracked by
  [issue #580](https://github.com/leynos/mdtablefix/issues/580); the narrow
  canonical-break consumer claims above do not discharge that broader domain.
- `make verus-fence-mutation` drops the marker-character comparison from
  `closes_fence` and checks that the failed obligation names that function's
  contract against `spec_closes`. The mutation leaves a plausible predicate
  behind -- it still tests depth, run length, and trailing whitespace -- so a
  gate that accepted it would show the fence proofs rested on something other
  than the closing rule they claim to establish.
- `make verus-compression-mutation` makes `delimiter_agendum` compress every
  delimiter it is handed, discarding the `compressible` argument that carries
  the block's answer, and requires the run to fail with that kernel's own
  postcondition named. The mutation is a single-line change that still compiles
  and writes a three-backtick delimiter exactly where the correct body writes
  one, so it differs only for the blocks the conflict flag was protecting; the
  mutated run reports exactly one failed obligation and every other proof still
  verifies. `LEM-NO-CONFLICT-ALL-CLEAR` and the compressed-block induction both
  depend on that flag, so a gate that still verified would mean those lemmas
  were proved from something other than the property they claim.

## Remaining external contracts

The rows above record what is proved; this section records what each proof
still takes on trust, so a boundary is a named contract rather than an
unexamined assumption.

- **Fence recognition and blockquote parsing.** Every obligation is stated over
  already-reduced `LineFeatures`, so none depends on the reduction being
  correct. The kernel body itself runs no regex; the reduction happens in
  `src/wrap/fence.rs`, whose entry points are `#[cfg(not(verus_keep_ghost))]`.
- **The emission read-back contract.** `spec_emission_reads_back` states what
  the parser reads off the bytes the emitter writes. It is a claim a
  counterexample could falsify at the frontier, and the property tests over the
  regex and the emitter are its evidence.
- **The driver instantiation is open.** The composition law itself is proved:
  `lemma_spans_compose_regions` turns per-span `ends_at` obligations over a
  chain of cuts into equality of the whole document's region sequence, and
  `lemma_released_span_composes` names which obligation each release route
  discharges. What is not proved is the induction over `spec_drive` that would
  supply those cuts and states from the fold's own agenda. The obstruction is
  structural rather than one of effort: the emitted sequence is not a
  one-line-per-step extension of the fold, because a close step's agenda grows
  by a whole release at once, so the natural recursion has no well-founded
  measure in the agenda index and a well-founded version needs the emitted
  sequence carried in an accumulator. Until that is written, the relation
  between each written line's reading and its source line's reading is
  established per agendum by the contract above and corpus-wide by
  `tests/fence_regions.rs`. **This is the residual gap; it is recorded here
  rather than presented as closed.**
- **The classifier kernel's fence recognizer cannot currently disagree, but is
  not tied to the fence kernel by any proof.**
  `src/classify_kernel_predicates.rs` implements `is_fence_marker` and
  `is_closing_fence` independently of `FENCE_RE` and of `LineFeatures`;
  `is_closing_fence` has no blockquote-depth rule, and `OpenFence` carries only
  a marker and a run length. It cannot disagree with the shared kernel in
  production today because `ClassifyCtxKernel::in_fence` is `#[cfg(test)]`-only
  and `LineClass::FenceMarker` has no production consumer. It is a drift
  hazard, not an active inconsistency.
- **Two fence regexes exist.** `src/wrap/fence.rs` and `src/fences.rs` differ in
  info-string grammar and in trailing-whitespace handling -- the second uses
  Unicode-aware `\s`, while the first's `trailing_blank` is deliberately
  ASCII-only. The second never decides literal-ness: it supplies delimiter
  spelling and attachment eligibility, and its result is gated through the
  planner and `compression_changes_region`. No reachable case was found where
  it alone changes which lines are literal, but the asymmetry deserves a
  targeted test before the claim is relied on.

The ledger check invoked by `make lint` rejects a claim whose executable
function name does not occur in `src/`.
