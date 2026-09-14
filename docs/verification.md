# Verification ledger

`mdtablefix` uses Verus to prove narrow kernels that the formatter calls in
production. This ledger records every claimed proof result, its executable
function, input domain, external contracts, and result class. A kernel may not
be represented by a separate `proofs/` implementation unless a refinement proof
connects it to the production function.

## Claim ledger

| Claim | Executable function | Input domain | Unverified external contracts | Result class |
| ----- | ------------------- | ------------ | ----------------------------- | ------------ |

<!-- markdownlint-disable MD013 -->
| LEM-SETEXT-CONSUMES-ONLY-PARAGRAPH | `convert_setext` | Setext candidates and underlines | None | Local correctness |
| LEM-ATX-OUTPUT-IS-NOT-SETEXT-CANDIDATE | `convert_setext` | Emitted ATX headings | None | Cross-pass preservation |
| LEM-CANONICAL-BREAK-REMAINS-STRUCTURAL | `canonical_break` | Canonical seventy-underscore break | None | Cross-pass preservation |
| Scanner refinement surface | `classify_line` | Structural line classifications | None | Local correctness |
<!-- markdownlint-enable MD013 -->

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
- `make verus-mutation` removes the space after the emitted `#` in an isolated
  proof copy and must fail verification. This guards the ATX-output lemma
  against a prefix specification that checks only for the hash marker.

The ledger check invoked by `make lint` rejects a claim whose executable
function name does not occur in `src/`.
