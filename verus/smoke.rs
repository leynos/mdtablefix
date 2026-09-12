//! Deliberately failing proof used to prove that the Verus harness is live.
//!
//! `make verus-selftest` and `tests/verus_harness.rs` depend on this proof
//! failing; do not make its assertion provable.

use vstd::prelude::*;

verus! {

proof fn smoke_must_fail() {
    assert(false);
}

} // verus!

fn main() {}
