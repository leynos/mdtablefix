//! Proc-macros for test fixtures that suppress lints triggered by macro expansion.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Item, ItemFn, parse_macro_input, parse_quote};

/// Allows `unused_braces` lint for fixture functions.
///
/// This attribute is used on rstest fixture functions that expand to single-expression
/// bodies. When combined with `fn_single_line = true` in rustfmt.toml, the generated
/// code triggers the `unused_braces` lint. This attribute suppresses that lint
/// specifically for fixture expansions.
#[proc_macro_attribute]
pub fn allow_fixture_expansion_lints(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let parsed_item = parse_macro_input!(item as Item);

    quote! {
        #[allow(
            unused_braces,
            reason = "fixture macro expansion triggers unused-braces on expression bodies"
        )]
        #[cfg_attr(
            clippy,
            expect(
                clippy::allow_attributes,
                reason = "needed to allow unused_braces for fixture macro expansion"
            )
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
