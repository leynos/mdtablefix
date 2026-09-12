# Architectural decision record (ADR) 0008: Verify a narrow normalization core

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

Choose Option A. `verus/lib.rs` is the only proof entry point and documents the
`#[path]` convention for future kernels. `make verus` invokes the pinned Verus
release through `rust-prover-tools`, and CI runs the same target on every pull
request. `make verus-selftest` runs a deliberately false proof and requires
Verus to reject it.

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
