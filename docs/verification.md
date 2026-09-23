# Verification ledger

`mdtablefix` uses Verus to prove narrow kernels that the formatter calls in
production. This ledger records every claimed proof result, its executable
function, input domain, external contracts, and result class. A kernel may not
be represented by a separate `proofs/` implementation unless a refinement proof
connects it to the production function.

## Current status

Issue #485 has no production-linked proof claims yet. The current
`verus/lib.rs` exercises a structural model, but it does not refine the runtime
scanner or consumer functions and is therefore deliberately excluded from the
claim ledger.

The pinned Verus release cannot compile `&str` range operations inside
`verus!`. The production scanner now delegates to a character-sequence kernel
that `verus/lib.rs` compiles, but `classify_seq` still has a trivial
postcondition, its scanner predicates use `#[verifier::external_body]`, and
`spec_classify` is not connected to the executable result. The missing
refinement and consumer obligations are tracked in [#512][issue-512].

## Claim ledger

| Claim | Executable function | Input domain | Unverified external contracts | Result class |
| ----- | ------------------- | ------------ | ----------------------------- | ------------ |

_Table 1: The verification claim ledger._

## Policy

- `#[verifier::external_body]` wrappers may state contracts for regex
  recognition, Unicode display width, and line breaking. They must not assert
  the property that a proof is meant to establish, such as reparsing block
  preservation or wrapping idempotence. Every external contract appears in a
  claim row before a proof relies on it.
- Specifications use distinct types for byte offsets, Unicode scalar indices,
  and display columns. No proof substitutes `String::len()` for display width.
- Existing property tests remain in place. Verus proofs complement them and do
  not replace them.
- `make verus-mutation` currently mutates the exploratory model only. It does
  not satisfy the production-linked mutation obligation from #485. That work is
  part of [#512][issue-512].

The ledger check invoked by `make lint` rejects a claim whose executable
function name does not occur in `src/`.

[issue-512]: https://github.com/leynos/mdtablefix/issues/512
