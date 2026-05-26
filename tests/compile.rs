//! Compile-time regression tests for dependency integration.

#[test]
fn html5ever_rcdom_parser_stack_compiles() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/html5ever_rcdom_pass.rs");
}
