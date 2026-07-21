//! Compile-time regression tests for dependency integration.

#[test]
fn html5ever_rcdom_parser_stack_compiles() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/html5ever_rcdom_pass.rs");
}

#[test]
fn tracing_instrument_attributes_compile() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/tracing_instrument_pass.rs");
}

#[test]
fn allow_fixture_expansion_lints_proc_macro_compiles() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/allow_fixture_expansion_lints_pass.rs");
}

#[test]
fn blockquote_and_fence_public_api_compiles() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/blockquote_fence_api_pass.rs");
}
