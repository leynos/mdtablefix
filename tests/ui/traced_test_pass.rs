//! Compile-time smoke test for `traced_test`.
//!
//! The body logs after the injected `rebuild_interest_cache`, so the callsite
//! the rebuild exists to heal is part of the expansion this file compiles. A
//! pass fixture is what the repository's other proc-macro case uses: trybuild
//! compiles the file against the parent's dependencies, so the wrapper's two
//! generated paths — `::tracing::callsite` and `#[::tracing_test::traced_test]`
//! — are checked to resolve.

#[test_macros::traced_test]
fn traced_test_expands() {
    ::tracing::debug!("a callsite the rebuilt cache can reach");
}

fn main() {
    traced_test_expands();
}
