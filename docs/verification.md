# Verification ledger

`mdtablefix` uses Verus to prove narrow kernels that the formatter calls in
production. This ledger records every claimed proof result, its executable
function, input domain, external contracts, and result class. A kernel may not
be represented by a separate `proofs/` implementation unless a refinement proof
connects it to the production function.

## Claim ledger

| Claim | Executable function | Input domain | Unverified external contracts | Result class |
| ----- | ------------------- | ------------ | ----------------------------- | ------------ |

No kernel claims have landed yet. Issues #491 and #483 will add the ellipsis and
`ProcessBuffer::finish` claims after their production-used kernels exist.

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

The ledger check invoked by `make lint` rejects a claim whose executable
function name does not occur in `src/`.
