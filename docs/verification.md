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
`SetextUnderline` classes. `convert_setext` uses the verified
`setext_atx_marker` builder for the hash run and mandatory separator.
`detect_verified_setext_heading` then checks the complete line with
`is_atx_heading_line` and rejects non-ATX output. The Verus lemma
`lemma_atx_prefix_classifies` proves the marker shape; executable regressions
cover retained indentation and quote prefixes. The break pass builds
`canonical_break()` from `canonical_break_chars`; the Verus loop postcondition
proves the builder returns exactly seventy underscores, and
`lemma_canonical_break_remains_structural` proves that sequence classifies as
`ThematicBreak`.

The production consumer paths also have regression coverage using that exact
canonical break: wrapping, Setext heading detection, table buffering, and
orphan-specifier attachment. Their production decisions call
`wrapping_boundary_seq`, `is_setext_pair_seq`, `is_table_line_seq`, and
`can_be_orphan_specifier_seq`, respectively. `src/classify_fixture_tests.rs`
pins a snapshot for every `tests/data` line. Each entry includes its path, line
number, FNV-1a hash of the original line, and classification, so edits to
fixture content or classification require an explicit oracle update. The
residual block matcher used for link and footnote definitions remains outside
this classifier proof.

## Claim ledger

| Claim                                             | Executable function                             | Input domain                                                            | Unverified external contracts                                  | Result class                                                                                                                 |
| ------------------------------------------------- | ----------------------------------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Structural classification and bounded body offset | `classify_seq`                                  | All `&[char]` lines and `ClassifyCtxKernel` values                      | `is_table_delimiter`, `is_thematic_break`, `ordered_list_item` | Exact `LineClass` and scalar offset (local correctness)                                                                      |
| Setext pair acceptance                            | `is_setext_pair_seq`                            | Candidate and underline with their classifier contexts                  | Same three scanner matcher contracts                           | Acceptance implies `ParagraphText` candidate and `SetextUnderline` underline (local correctness)                             |
| Setext candidate decision                         | `is_setext_text_seq`                            | All lines and classifier contexts                                       | Same three scanner matcher contracts                           | Acceptance implies `ParagraphText` (local correctness)                                                                       |
| Setext underline decision                         | `is_setext_underline_seq`                       | All lines and classifier contexts                                       | Same three scanner matcher contracts                           | Acceptance implies `SetextUnderline` (local correctness)                                                                     |
| ATX marker construction                           | `setext_atx_marker`                             | Levels one and two                                                      | None                                                           | Exactly the level's hash run followed by one space (local correctness)                                                       |
| Emitted Setext ATX check                          | `is_atx_heading_seq`                            | Generated lines and classifier contexts                                 | Same three scanner matcher contracts                           | Uses the production classifier to require `AtxHeading` (local correctness)                                                   |
| Canonical-break line construction                 | `canonical_break_chars`                         | Empty builder state                                                     | None                                                           | Exactly seventy underscore characters (local correctness)                                                                    |
| Canonical-break classification                    | `canonical_break_chars`                         | Canonical line under default classifier context                         | Same three scanner matcher contracts                           | Verus proves the shared classifier returns `ThematicBreak` (local correctness)                                               |
| Setext pair detection                             | `detect_setext_heading`                         | Production candidate/underline pair with indentation and prefix context | Same three scanner matcher contracts                           | Calls the proved pair gate and retains production prefix checks                                                              |
| Setext output construction                        | `convert_setext`                                | Prefix, level one or two, and candidate text                            | Same three scanner matcher contracts                           | Uses the verified marker builder; `detect_verified_setext_heading` checks the completed line (executable prefix regressions) |
| Wrapping break preservation                       | `wrapping_boundary_seq`                         | Exact `canonical_break()` in default wrapper context                    | Same three scanner matcher contracts                           | Returns the boundary used by `classify_block`; exact production-path regression (cross-pass behavioural preservation)        |
| Heading break preservation                        | `is_setext_pair_seq`                            | Exact `canonical_break()` as candidate                                  | Same three scanner matcher contracts                           | Model-level rejection proof; exact production-path regression (cross-pass behavioural preservation)                          |
| Table break preservation                          | `is_table_line_seq`                             | Exact `canonical_break()` while a table is buffered                     | Same three scanner matcher contracts                           | Rejects the gate used by `handle_table_line`; exact production-path regression (cross-pass behavioural preservation)         |
| Orphan-specifier break preservation               | `can_be_orphan_specifier_seq`                   | Exact `canonical_break()` before a fence                                | Same three scanner matcher contracts                           | Rejects the gate used by `preserve_thematic_break`; exact production-path regression (cross-pass behavioural preservation)   |
| Fixture classification compatibility              | `fixture_lines_match_the_classification_oracle` | Every `tests/data` line with carried fence and preceding-line context   | Same three scanner matcher contracts                           | Per-line class and FNV-1a source-line hash are pinned (compatibility regression)                                             |

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
- `lemma_paragraph_exists`, `lemma_delimiter_exists`, and `lemma_break_exists`
  assert concrete examples so the paragraph, delimiter, and thematic-break
  classes are shown to be inhabited.
- `make verus-mutation` removes the mandatory space from the production ATX
  marker builder and checks that its refinement postcondition fails.

The ledger check invoked by `make lint` rejects a claim whose executable
function name does not occur in `src/`.
