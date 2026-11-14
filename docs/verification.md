# Verification ledger

`mdtablefix` uses Verus to prove narrow kernels that the formatter calls in
production. This ledger records every claimed proof result, its executable
function, input domain, external contracts, and result class. A kernel may not
be represented by a separate `proofs/` implementation unless a refinement proof
connects it to the production function.

## Current status

The proofs run under Verus `0.2026.09.06.8dea4a2`, the release pinned in
`tools/verus/VERSION`; `make verus` reports 229 verified items with no errors.
Most of the increase over the base revision's 176 is the fence-normalization
document theorem, its composition lemmas, and its witnesses. Under the previous
release, `0.2025.04.19.1b16620`, the base revision reported 178. The two that
went were the executable constants `COMPRESSED_MARKER` and
`COMPRESSED_MARKER_LEN`, which the new release cannot erase when they sit
outside `verus!` syntax; `compressed` now writes the same two literals, and
`spec_compressed` is unchanged. The new release also requires every verified
executable loop to name a `decreases` clause; the twelve scanner loops in
verified bodies each carry one, a measure such as `chars@.len() - cursor`, so
the proofs show that each of them terminates. Loops inside
`verifier::external_body` functions, such as those in `is_table_delimiter` and
`is_thematic_break`, are trusted and have no termination proof.

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

The region-preservation theorem is one-sided, and that is the honest form. It
is one-sided because a tilde line closes a tilde opener but cannot close the
three-backtick opener the pass writes in its place, so the two runs
legitimately disagree about delimiter identity: what the theorem establishes is
that no line moves between the literal and prose regions -- delimiter spelling
may change, payload regions may not. Lines arrive as pre-parsed `LineFeatures`,
so fence recognition and blockquote parsing stay external contracts throughout.

The document-level form does not take its partition on trust.
`lemma_document_regions_preserved` states
`LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS` over the whole document, and the
chain of preserved spans it composes is the one the pass's own fold builds.
`spec_chain_of` records the spans the driver has completed after each prefix,
appending a cut exactly when a step writes; `lemma_fold_invariant` proves
`INV-DRIVER-FOLD` at every prefix, from the plan alone;
`lemma_chain_preserved_prefix` and `lemma_chain_preserved_push` extend a
preserved chain at its top, because `spec_chain_preserved` peels its last span;
and `lemma_spans_compose_regions` turns the chain into equality of the whole
document's region sequence. `lemma_document_tail_composes` appends the block a
document ended inside, which is the fourth flush path. The driver instantiation
this ledger previously recorded as open is therefore closed.

The only hypothesis the theorem takes about the emitted run is the rendered
spelling: `spec_emission_reads_back` over `spec_agendum_line` at every
position. That remains an external contract, because Verus cannot reason about
a compiled regular expression, but it is consumed rather than restated.
`lemma_released_block_reads_back` and `lemma_released_span_reads_back` derive
each position's emitter fact from the agendum the driver chose, and the
consumers in `verus/plan_spec_emission.rs` extract the three shapes.
`lemma_witness_interior_is_literal` exhibits an interior satisfying every
hypothesis and pins the regions both runs agree on, so the conclusion is not an
empty sequence, and `lemma_witness_conflict_is_rejected` proves the guard fires
on the issue #480 shape. `tests/fence_regions.rs` exercises the same reasoning
over every fixture and four unclosed documents; that sweep is executable
evidence, not the theorem, because a corpus says nothing about a document no
fixture holds.

Non-vacuity is otherwise carried by the witness lemmas beside each obligation,
which state their conclusions as values rather than as equalities between two
unknowns. In `verus/plan_spec_witnesses.rs`, `lemma_witness_agreement_is_live`
shows the reading-agreement hypothesis is satisfiable,
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
transition's depth check from `verus/fence_spec.rs` makes the run fail.

Five further witnesses have the driver fold itself as their subject, one per
route a document can take through it. Until they were added no witness exercised
`spec_drive` at all, so nothing in that block would have failed had the fold
wired its rules together wrongly. They pin the whole plan — `agenda`,
`unclosed`, and `unclosed_lines` — as values rather than as an equation between
two unknowns. Four live in `verus/plan_spec_witnesses_fold.rs`:
`lemma_witness_matched_block_plan_is_live` for a matched close, whose agenda is
`Compressed` throughout because a block with no interior line never raises its
conflict flag; `lemma_witness_unmatched_block_plan_is_live` for the fourth
flush path, which no fixture under `tests/data/` reaches because every fixture
closes its fences; `lemma_witness_conflict_fallback_plan_is_live` for the
[issue #480](https://github.com/leynos/mdtablefix/issues/480) shape driven to
its fallback, whose observable effect is `Normalized` at the opener and
`Verbatim` for the lines after it; and
`lemma_witness_empty_document_plan_is_live` for the fold's base case. The fifth,
`lemma_witness_matched_normalized_plan_is_live`, lives in its own
`verus/plan_spec_witnesses_fold_normalized.rs` — split out to keep both files
under the 400-line ceiling — because the route it pins, a *matched* block
released `Normalized`, is one the other four cannot reach. Its document is a
four-backtick `null` opener, an interior three-backtick pair and a
four-backtick closer, and its agenda is `Normalized`, `Verbatim`, `Verbatim`,
`Normalized` with no unclosed block. These lemmas live under `verus/` and so
appear in the prose rather than in the claim table, whose executable-function
column must name a symbol that occurs in `src/`.

Two further witnesses guard the emission contract itself, in
`verus/plan_spec_witnesses.rs`.
`lemma_witness_normalized_contract_refuses_a_wrong_respelling` exhibits a
`null`-tagged opener against a repainted emission that the contract rejects, and
`lemma_witness_null_specifier_reads_back` exhibits the same opener against the
emission production actually writes, which it accepts. Together they show the
relation is neither vacuous nor true of every pair — the property a contract
weakened to `true` silently loses. Both are load-bearing: the second is what
fails when the blank-tag disjunct is deleted, as
`make verus-blank-tag-mutation` demonstrates.

## Claim ledger

| Claim                                                  | Executable function                             | Input domain                                                                                                           | Unverified external contracts                                                                                           | Result class                                                                                                                                                |
| ------------------------------------------------------ | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Structural classification and bounded body offset      | `classify_seq`                                  | All `&[char]` lines and `ClassifyCtxKernel` values                                                                     | `is_table_delimiter`, `table_cells_are_delimiters`, `is_table_delimiter_cell`, `is_thematic_break`, `ordered_list_item` | Exact `LineClass` and scalar offset (local correctness)                                                                                                     |
| Setext pair acceptance                                 | `is_setext_pair_seq`                            | Candidate and underline with their classifier contexts                                                                 | Same five trusted matcher contracts                                                                                     | Acceptance implies `ParagraphText` candidate and `SetextUnderline` underline (local correctness)                                                            |
| Setext candidate decision and later-pass refusal       | `is_setext_text_seq`                            | All lines and classifier contexts, including completed `convert_accepted_setext_seq` output                            | Same five trusted matcher contracts                                                                                     | Acceptance implies `ParagraphText`; therefore the completed `AtxHeading` output is rejected as a candidate (local correctness)                              |
| Setext underline component predicate                   | `is_setext_underline_seq`                       | All lines and classifier contexts; used by `is_setext_pair_seq`                                                        | Same five trusted matcher contracts                                                                                     | Acceptance implies `SetextUnderline` (local correctness; component of pair acceptance)                                                                      |
| Prefix separation                                      | `needs_space_after_seq`                         | Any retained Unicode-scalar prefix                                                                                     | None                                                                                                                    | Determines whether the retained prefix needs a separator (local correctness)                                                                                |
| Prefix admission                                       | `valid_setext_prefix`                           | Any retained indentation and quote prefix                                                                              | Same five trusted matcher contracts                                                                                     | Paragraph probe starts after the retained prefix (local correctness)                                                                                        |
| Complete Setext output construction                    | `convert_setext_seq`                            | Valid prefix, levels one and two, arbitrary supplied text, and capacity bound                                          | Same five trusted matcher contracts                                                                                     | Exact assembled output is `AtxHeading` and not `ParagraphText` in every outside-fence context (local construction correctness)                              |
| Accepted production Setext conversion                  | `convert_accepted_setext_seq`                   | Candidate/underline and their actual contexts, valid prefix, levels one/two, and capacity bound                        | Same five trusted matcher contracts                                                                                     | `Some` iff source roles and prefix admission hold; output is exact assembly and `AtxHeading` in every outside-fence context (local correctness)             |
| Emitted Setext ATX check                               | `is_atx_heading_seq`                            | Completed lines and classifier contexts                                                                                | Same five trusted matcher contracts                                                                                     | Classifies a supplied completed line; runtime guard remains a checked-output safeguard (checked-output guarantee)                                           |
| Canonical-break line construction                      | `canonical_break_chars`                         | Empty builder state                                                                                                    | None                                                                                                                    | Exactly seventy underscore characters (local correctness)                                                                                                   |
| Canonical-break classification                         | `classify_seq`                                  | Exact unprefixed 70-underscore line under every outside-fence context and any previous class or prefix-agreement value | Same five trusted matcher contracts                                                                                     | `lemma_canonical_break_remains_structural` proves `ThematicBreak` (local correctness)                                                                       |
| Setext pair detection                                  | `detect_setext_heading`                         | Production candidate/underline pair with indentation and prefix context                                                | Same five trusted matcher contracts                                                                                     | Calls the proved pair gate and retains production prefix checks                                                                                             |
| Setext production extraction                           | `detect_setext_heading`                         | Eligible pair, actual underline context, indentation and prefix context                                                | Same five trusted matcher contracts                                                                                     | Passes the actual pair contexts and extracted values to the production acceptance wrapper; exact native parsing/content extraction is regression-tested     |
| Setext output assembly adapter                         | `convert_setext`                                | Retained prefix, level one or two, and candidate text                                                                  | Same five trusted matcher contracts                                                                                     | Calls `convert_accepted_setext_seq`, which proves the accepted roles and completed output class                                                             |
| Setext checked-output guarantee                        | `detect_verified_setext_heading`                | Completed output line and classifier context                                                                           | Same five trusted matcher contracts                                                                                     | Runtime guard accepts only output classified `AtxHeading`; it is not a construction proof                                                                   |
| Wrapping break preservation                            | `wrapping_boundary_seq`                         | Exact unprefixed canonical line under every outside-fence context                                                      | Same five trusted matcher contracts                                                                                     | Returns the thematic-break boundary, not paragraph content; production wrapper and path regression (cross-pass preservation)                                |
| Heading break preservation                             | `is_setext_pair_seq`                            | Exact unprefixed canonical line as candidate under every outside-fence context                                         | Same five trusted matcher contracts                                                                                     | Thematic-break class fails paragraph-candidate role; wrapper lemma and path regression (cross-pass preservation)                                            |
| Table break preservation                               | `is_table_class`                                | Thematic-break class returned for the exact canonical line                                                             | Same five trusted matcher contracts                                                                                     | Rejects the table-input gate; wrapper lemma and path regression (cross-pass preservation)                                                                   |
| Orphan-specifier break preservation                    | `can_be_orphan_specifier_seq`                   | Exact unprefixed canonical line under every outside-fence context                                                      | Same five trusted matcher contracts                                                                                     | Returns false for thematic-break class; wrapper lemma and path regression (cross-pass preservation)                                                         |
| Fixture classification compatibility                   | `fixture_lines_match_the_classification_oracle` | Every `tests/data` line with independently carried fence, preceding-class and prefix state                             | Same five trusted matcher contracts                                                                                     | Differentially compares an independent regex reference and pins path, class and FNV-1a source-line hash (compatibility evidence and regression)             |
| Fence closing-rule decision                            | `closes_fence`                                  | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Boolean iff `spec_closes`                                                                                                                                   |
| Would-be-closer decision                               | `agrees_with_opener`                            | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Boolean iff `spec_agrees_with_opener`                                                                                                                       |
| Interior-delimiter decision                            | `interior_delimiter`                            | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Boolean iff `spec_interior_delimiter`                                                                                                                       |
| Compression-conflict decision                          | `compression_changes_region`                    | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Boolean iff `spec_compression_changes_region`                                                                                                               |
| Compressed delimiter state                             | `compressed`                                    | All opener states                                                                                                      | None                                                                                                                    | Exact state; three backticks at the opening depth                                                                                                           |
| Fence transition and line region                       | `fence_step`                                    | All opener states and parsed line features                                                                             | Fence recognition and blockquote parsing                                                                                | Exact next state and `Region`                                                                                                                               |
| Whole-document region classification                   | `regions`                                       | Every `LineFeatures` slice                                                                                             | Fence recognition and blockquote parsing                                                                                | Exact `Region` sequence, element for element                                                                                                                |
| Region preservation under delimiter compression        | `regions`                                       | A block interior meeting the compression guard                                                                         | Fence recognition and blockquote parsing                                                                                | Equal region sequences under the original and compressed openers                                                                                            |
| Block-opener construction                              | `open_block`                                    | Every `LineFacts` value                                                                                                | Fence recognition and blockquote parsing                                                                                | Exact `Option<Block>`; `None` iff the line has no marker                                                                                                    |
| Conflict accumulation                                  | `absorb`                                        | Every block and optional line features                                                                                 | Fence recognition and blockquote parsing                                                                                | Exact block; opener fixed, flag a disjunction over the lines                                                                                                |
| Planner routing                                        | `plan_step`                                     | Every planner state and `LineFacts` value                                                                              | Fence recognition and blockquote parsing                                                                                | Exact `Step`; the pass's only source of a release                                                                                                           |
| Delimiter rewrite eligibility                          | `delimiters_eligible`                           | Every release and rewrite-candidate pair                                                                               | Fence recognition and blockquote parsing                                                                                | Exact boolean; opener alone, both ends, or neither                                                                                                          |
| Outer-delimiter position decision                      | `is_outer_delimiter`                            | Every position, block length, release, and flag                                                                        | None                                                                                                                    | Exact boolean; position 0, or the last position of a matched block                                                                                          |
| Per-position emission decision                         | `delimiter_agendum`                             | Every position classification and compression answer                                                                   | None                                                                                                                    | Exact `Agendum`; compressed, normalized, or verbatim                                                                                                        |
| Released-block agenda                                  | `block_agenda`                                  | Every block length, release, and answers                                                                               | Fence recognition and blockquote parsing                                                                                | Exact `Agendum` sequence of the block's own length                                                                                                          |
| Whole-document region preservation under normalization | `compress_fences`                               | Every source sequence and matching rewritability sequence whose rendering parses back to the plan's agenda             | Fence recognition, blockquote parsing, and the rendered-spelling contract at every position                             | `LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS`: equal `Region` sequences for the source and emitted runs over the whole document (document-level preservation) |
| Emission read-back consumption                         | `rewrite_marker`                                | Every released block, its planner agenda, and an emitted sequence meeting the contract at every position               | Fence recognition, blockquote parsing, and the rendered-spelling contract (consumed rather than proved)                 | Three emission shapes derived from the agendum: verbatim agreement, delimiter read-back, and the compressed three-backtick shape (local correctness)        |
| Emission contract inhabited and discriminating         | `rewrite_marker`                                | A `null`-tagged opener against both the emitter's actual respelling and a repainted one                                | Fence recognition and blockquote parsing                                                                                | The contract accepts the real normalization and rejects the wrong respelling (non-vacuity)                                                                  |
| Matched-block normalization route                      | `drive_step`                                    | A four-backtick `null` opener, a three-backtick interior pair, and a four-backtick closer, all rewritable              | Fence recognition and blockquote parsing                                                                                | Agenda exactly `Normalized`, `Verbatim`, `Verbatim`, `Normalized`, with no unclosed block (non-vacuity for the fold)                                        |
| Rendered delimiter spelling                            | `rewrite_marker`                                | Generated fence lines: both markers, runs of three to ten, indent up to eight columns, and bounded language strings    | The rendering/parsing boundary itself: the regex that reads the written bytes back                                      | Compress writes three backticks keeping indent and non-`null` language; preserve keeps the original run; both idempotent (executable evidence only)         |
| Segment walked at a document offset                    | `regions`                                       | Every span and every offset                                                                                            | Fence recognition and blockquote parsing                                                                                | Equal states and equal region sequences over the span's own lines                                                                                           |
| Released block read at a document offset               | `block_agenda`                                  | Every offset, for a released block whose per-line facts are supplied as hypotheses                                     | Fence recognition, blockquote parsing, and the emission read-back contract                                              | The route's obligation: `ends_at` where a state conclusion exists                                                                                           |
| Line inside an open block                              | `fence_step`                                    | Every opener state and non-fence-shaped line                                                                           | Fence recognition and blockquote parsing                                                                                | The line still leaves the fence open; it cannot route as buffer-or-close                                                                                    |

*Table 1: The verification claim ledger.*

The table's shorthand “Same five trusted matcher contracts” names the five
trusted matcher bodies `is_table_delimiter`, `table_cells_are_delimiters`,
`is_table_delimiter_cell`, `is_thematic_break` and `ordered_list_item`. Each is
reached through the classifier refinement; their verification is tracked by
issue #567.

## Executable evidence

Not every check of the pass is a proof, and the distinction is load-bearing
here: a corpus sweep cannot establish a property over a document no fixture
holds, and a property test cannot establish one over unbounded input. These are
the executable checks the claims above lean on, with the claim each supports.
They are evidence, not proof, and none should be cited as a theorem.

- **Corpus region sweep.** `tests/fence_regions.rs` runs
  `regions(compress_fences(lines)) == regions(lines)` at every index for every
  fixture under `tests/data/`, plus four hand-written documents that reach
  `flush_unclosed` because no fixture ends inside a fence. It also runs the
  full flag set through the binary and requires every classifier-literal line
  to survive byte for byte. It supports the document theorem's conclusion over
  the corpus it reads and says nothing about the rest.
- **Delimiter spelling properties.** `src/fences_properties.rs` generates fence
  lines over both markers, runs of three to ten, indentation up to eight
  columns, and a language drawn from empty, the `null` variants and a bounded
  non-`null` word. It pins that `rewrite_marker` under `Strategy::Compress`
  writes exactly three backticks while keeping indentation and any non-`null`
  language, that `Strategy::Preserve` keeps the original marker run, and that
  both are idempotent. This is the rendered-spelling half of the emission
  contract, which the proof consumes as a hypothesis.
- **Reference classification.** `src/classify_fixture_tests.rs` compares every
  fixture line against an independent regex reference carrying its own state
  and against a reviewed snapshot; the fixture-compatibility row cites it.
- **Pass-level regressions.** `tests/fences.rs` pins the issue #480
  reproduction, the `null`-specifier blank-tag cases and the orphan-specifier
  cases — production behaviour the specifications model rather than prove.
- **Idempotence sweeps.** `tests/idempotence_properties.rs` and
  `src/fences_properties.rs::rewriting_is_idempotent` cover stability under
  repeated passes. Idempotence is tracked by
  [ADR 0006](adrs/0006-single-pass-idempotence.md) and is not a claim in this
  ledger's table.

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
- `make verus-emission-mutation` replaces the `Normalized` arm of
  `spec_emission_reads_back` with `true` and requires the run to fail in the
  consumer lemma and in the witness that pins what the arm rejects. A contract
  that is `true` rejects nothing, so a gate that still verified would mean the
  document theorem never read the clause it claims to consume.
- `make verus-blank-tag-mutation` deletes the `spec_blank_tag_raised` disjunct
  from `spec_delimiter_reads_back`. Every theorem takes read-back as a
  hypothesis, so narrowing it cannot break a theorem body; what fails is
  `lemma_witness_null_specifier_reads_back`, the evidence that a real
  normalization satisfies the hypothesis at all. That is what makes the witness
  load-bearing: the gate also fails, as a mutation that verified, if the
  witness is ever removed.
- The two contract gates share `assert_failures_attributed` from
  `scripts/verus-mutation-mirror.sh`: every reported error must be a failed
  postcondition located in a named module, and the summary line must show the
  verifier ran to completion. An unrelated obligation, a parse error, or a
  runner failure therefore cannot pass as a detection. Both gates are exercised
  in five shapes by `tests/support/verus_mutation_checks.rs`, where only the
  intended rejection may succeed.

## Remaining external contracts

The rows above record what is proved; this section records what each proof
still takes on trust, so a boundary is a named contract rather than an
unexamined assumption.

- **Fence recognition and blockquote parsing.** Every obligation is stated over
  already-reduced `LineFeatures`, so none depends on the reduction being
  correct. The kernel body itself runs no regex; the reduction happens in
  `src/wrap/fence.rs`, whose entry points are `#[cfg(not(verus_keep_ghost))]`.
- **The rendered-spelling contract is consumed but not proved.**
  `spec_emission_reads_back` states what the parser reads off the bytes the
  emitter writes, and it is the one hypothesis the document theorem takes about
  the emitted run. It is **consumed**: `spec_agendum_line` constructs the
  contract's `SpecLine` from the agendum the driver chose,
  `lemma_released_block_reads_back` and `lemma_released_span_reads_back` derive
  each position's emitter fact from it, and the consumers in
  `verus/plan_spec_emission.rs` extract the three shapes. It is still not
  **proved**: Verus cannot reason about a compiled regular expression, so the
  claim that the written bytes parse back as the agendum's spelling rests on
  the production regex and on the property tests in `src/fences_properties.rs`
  (`compress_writes_three_backticks_and_preserves_indent_and_language`,
  `preserve_retains_the_original_marker_run`).

  The contract's weaker half is `spec_delimiter_reads_back`: family, run and
  depth preserved, with only a raised blank tag admitted. A `null` language
  specifier is non-blank, so dropping it — which `src/fences.rs::is_null_lang`
  does, and which
  `tests/fences.rs::preserves_null_language_during_interior_conflict` pins —
  makes the emitted line blank-tagged where the source was not, and
  `spec_closes` reads exactly that flag. That is why the disjunction exists
  rather than plain agreement. `lemma_witness_null_specifier_reads_back`
  exhibits a real normalization that satisfies it, and
  `lemma_witness_normalized_contract_refuses_a_wrong_respelling` exhibits one
  the contract rejects, so the relation is neither vacuous nor satisfied by
  every pair. `lemma_blank_tag_flip_needs_the_guard` is a concrete example of
  the flip — it evaluates `spec_masked` and two region values on a
  `null`-tagged line and its untagged emission — not a proof that the guard is
  necessary. The production behaviour is pinned by
  `tests/fences.rs::dropped_null_specifier_raises_the_blank_tag`.

  Both halves now have negative controls. `make verus-emission-mutation`
  replaces the `Normalized` arm's body with `true`; the block theorem can then
  no longer derive its delimiter fact, and the witness above fails with it.
  `make verus-blank-tag-mutation` deletes the `spec_blank_tag_raised` disjunct;
  every theorem takes read-back as a hypothesis, so no theorem body breaks, and
  what fails is the witness that the hypothesis is satisfiable by a real
  normalization. A contract whose weakening left the tree verifying would be a
  comment rather than a specification, which is why both gates exist.
- **The classifier kernel's fence recognizer cannot currently disagree, but is
  not tied to the fence kernel by any proof.**
  `src/classify_kernel_predicates.rs:120` declares `is_fence_marker`, and the
  same module's `is_closing_fence` implements the closing rule independently of
  `FENCE_RE` and of `LineFeatures`; `is_closing_fence` has no blockquote-depth
  rule, and `OpenFence` carries only a marker and a run length. It cannot
  disagree with the shared kernel in production today because
  `ClassifyCtxKernel::in_fence` is `#[cfg(test)]`-only and
  `LineClass::FenceMarker` has no production consumer. It is a drift hazard,
  not an active inconsistency, and it is tracked by issue #651.
- **Two fence regexes exist.** The recognizers read a delimiter differently.
  `src/fences.rs:30` uses a bounded info-string grammar and a Unicode-aware
  `\s` tail:

  ```text
  ^(\s*)(`{3,}|~{3,})([A-Za-z0-9_+.,-]*)\s*$
  ```

  `src/wrap/fence.rs:39` instead accepts any info string without a line
  terminator:

  ```text
  ^(\s*)(`{3,}|~{3,})([^\r\n]*)$
  ```

  while the kernel's `trailing_blank` is deliberately ASCII-only. The second
  never decides literal-ness: it supplies delimiter spelling and attachment
  eligibility, and its result is gated through the planner and
  `compression_changes_region`. No reachable case was found where it alone
  changes which lines are literal, but the asymmetry deserves a targeted test
  before the claim is relied on.

The ledger check invoked by `make lint` rejects a claim whose executable
function name does not occur in `src/`.
