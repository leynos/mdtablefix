//! Test-only proc-macros used by this repository's test suite through the
//! `test-macros` dev-dependency.
//!
//! `allow_fixture_expansion_lints` contains the `unused_braces` lint that
//! `rstest` fixture expansion triggers when `fn_single_line = true` keeps an
//! expression body wrapped in braces. The generated expectation is attached to
//! the affected fixture to contain that warning. Rust does not report
//! `unfulfilled_lint_expectations` for an expectation introduced by this
//! attribute macro, so an upstream expansion change does not produce a stale
//! expectation diagnostic; the reason records that compiler limitation.
//!
//! `traced_test` wraps `tracing_test::traced_test`, prepending
//! `::tracing::callsite::rebuild_interest_cache();` to the body so the rebuild
//! runs after `tracing-test` installs its subscriber, which prepends its own
//! initialisation to the body it is handed. Without it, a callsite first used
//! before the install caches `Interest::never()` for the process lifetime and
//! stays silent, so a test asserting on its own log lines passes or fails
//! according to which tests ran first.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Item, ItemFn, parse_macro_input, parse_quote};

/// Contains `unused_braces` reported from an `rstest` fixture expansion.
///
/// `rstest` currently emits braces around a single-expression fixture body.
/// With `fn_single_line = true`, the compiler diagnoses those generated braces
/// even though the fixture author cannot remove them. The item-scoped
/// expectation documents that upstream expansion limitation. Rust does not
/// report `unfulfilled_lint_expectations` for this generated expectation if
/// the expansion stops needing it, so its reason records that limitation.
#[proc_macro_attribute]
pub fn allow_fixture_expansion_lints(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let parsed_item = parse_macro_input!(item as Item);

    quote! {
        #[expect(
            unused_braces,
            reason = "rstest fixture expansion retains braces around a single-expression body"
        )]
        #parsed_item
    }
    .into()
}

/// Runs a traced test, healing the callsite interest cache after the
/// subscriber is installed.
///
/// Use this in place of `tracing_test::traced_test`. `tracing-test` installs
/// its global subscriber lazily, from whichever traced test the harness
/// reaches first. `tracing` decides once, when a callsite is first used,
/// whether that callsite can ever be dispatched, and a callsite first used
/// before the install finds no subscriber to ask: it caches
/// `Interest::never()` for the life of the process, because installing a
/// global subscriber does not recompute the cache. The callsite then stays
/// silent, so a test that asserts on its own log lines fails intermittently,
/// on a schedule set by whichever tests the harness happens to run alongside
/// it rather than by the code under test.
///
/// Rebuilding the cache once the subscriber is in place re-enables those
/// callsites. Expansion order places the rebuild after the install: this macro
/// prepends the rebuild to the function body and hands that body to
/// `tracing_test::traced_test`, which prepends its own initialization to
/// whatever body it is given.
#[proc_macro_attribute]
pub fn traced_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut function = parse_macro_input!(item as ItemFn);

    function.block.stmts.insert(
        0,
        parse_quote!(::tracing::callsite::rebuild_interest_cache();),
    );

    quote! {
        #[::tracing_test::traced_test]
        #function
    }
    .into()
}
