# Verification claim ledger

This ledger records bounded statements that have a local formal proof. It does
not replace executable tests or claim whole-pipeline correctness.

| Kernel                       | Input domain                              | External contracts | Result class      |
| ---------------------------- | ----------------------------------------- | ------------------ | ----------------- |
| `ProcessBuffer` chunk ledger | Arbitrary finite sequences of line chunks | None               | Local correctness |

## ProcessBuffer chunk ledger

`verus/process_buffer.rs` proves that emitted chunks followed by pending chunks
equal the processed source prefix. It proves that `finish` empties pending
state and emits the source sequence in order. The proof abstracts table reflow
and ellipsis rewriting as a chunk-preserving transform; it does not establish
byte-level output equality or any cross-pass property.

Run `make verus VERUS_Z3_PATH=/path/to/z3`. The target installs the
checksum-pinned Verus release with `rust-prover-tools`, uses its required Rust
1.98.0 toolchain, and requires a Z3 4.16.0 executable supplied by the
verification environment. Issue #479 owns general verifier provisioning.
