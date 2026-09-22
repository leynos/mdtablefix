//! Inline fixtures for the suppression scan, in both directions.
//!
//! Split from `tests/env_access_suppressions.rs` under the repository's
//! 400-line cap. Every source here is judged as the ordinary file
//! `src/fixture.rs`, through the parent's `scan`, so a rule that depends on
//! the path is exercised on the side that has to hold everywhere. The first
//! test holds each route to a suppression Clippy honours; the second holds
//! the text that merely resembles one, which is what keeps the first honest.

use anyhow::{Result, ensure};

use super::scan;

/// Scenario: suppressions Clippy honours but `allow_attributes` never reports.
/// Invariant: each is an offence. The group form never names the lint, the
/// `cfg_attr` form hides it from any line-start scan, and the function-local
/// form disarms the policy for that helper alone, which is harder to spot in
/// review than a crate-level one. The macro forms are the reason the collector
/// walks token streams: `syn` keeps a `macro_rules!` arm's body opaque, so an
/// attribute written there is never parsed into one, while Clippy sees the
/// expansion and honours it.
///
/// A crate-scoped `expect` is here rather than among the sanctioned forms. One
/// call anywhere in the crate fulfils it, so it reports nothing and never
/// warns, which is `allow` by another name. Only an item-scoped outer `expect`
/// earns its exemption, and the raw spellings are included because Clippy reads
/// `r#allow` and `clippy::r#style` as the plain identifiers while a written
/// comparison would not.
#[rstest::rstest]
#[case::names_the_lint("#![allow(clippy::disallowed_methods)]\n")]
#[case::names_the_group("#![allow(clippy::style)]\n")]
#[case::names_everything("#![allow(clippy::all)]\n")]
#[case::nested_in_cfg_attr("#![cfg_attr(all(), allow(clippy::disallowed_methods))]\n")]
#[case::guard_group("#![allow(clippy::restriction)]\n")]
#[case::spaced_with_a_parenthesis_in_the_reason(
    "#![allow (warnings, reason = \"see the note (below)\")]\n"
)]
#[case::wrapped_across_lines("#![allow(\n    clippy::all,\n    reason = \"wrapped\"\n)]\n")]
#[case::on_an_item("#[allow(warnings, reason = \"x\")]\nfn item() {}\n")]
#[case::inside_a_function("fn outer() {\n    #[allow(clippy::style)]\n    fn inner() {}\n}\n")]
#[case::emitted_by_a_macro(
    concat!(
        "macro_rules! silence {\n    () => {\n        ",
        "#[allow(clippy::disallowed_methods)]\nfn probe() {}\n    };\n}\n"
    )
)]
#[case::crate_scoped_expect("#![expect(clippy::disallowed_methods, reason = \"x\")]\n")]
#[case::crate_scoped_expect_in_cfg_attr(
    "#![cfg_attr(all(), expect(clippy::disallowed_methods, reason = \"x\"))]\n"
)]
#[case::raw_attribute_name("#![r#allow(clippy::disallowed_methods)]\n")]
#[case::raw_lint_name("#![allow(clippy::r#style)]\n")]
#[case::raw_both("#![r#allow(clippy::r#all)]\n")]
#[case::emitted_two_macros_deep(
    concat!(
        "macro_rules! outer {\n    () => {\n        macro_rules! inner {\n            ",
        "() => {\n#![allow(clippy::all)]\n            };\n        }\n    };\n}\n"
    )
)]
// A transcriber that writes the attribute but forwards its path. The arm
// cannot be read for what it applies, and the invocation carries no `#`, so
// neither half is a suppression alone.
#[case::forwards_its_path_over_a_call(
    concat!(
        "macro_rules! forward {\n    ($attr:meta) => {\n        #[$attr]\n        ",
        "pub fn probe() { let _ = std::env::var(\"X\"); }\n    };\n}\n",
        "forward!(allow(clippy::disallowed_methods));\n"
    )
)]
// The same forwarding at inner scope, which applies to everything around it.
#[case::forwards_its_path_at_inner_scope(
    "macro_rules! forward {\n    ($attr:meta) => {\n        #![$attr]\n    };\n}\n"
)]
// Forwarding over an item the caller supplies: the arm names no `env` itself,
// but whatever it is handed comes under the forwarded attribute.
#[case::forwards_its_path_over_a_supplied_item(
    "macro_rules! forward {\n    ($attr:meta, $body:item) => {\n        #[$attr] $body\n    \
     };\n}\n"
)]
// `include!` of a target the scan cannot see: rustc parses it as Rust whatever
// the extension, so an `allow` written there reaches the compiler.
#[case::includes_a_foreign_extension("include!(\"tests/data/probe.rs.txt\");\n")]
// An `include!` whose target is not a literal cannot be judged at all.
#[case::includes_a_computed_path("include!(concat!(env!(\"OUT_DIR\"), \"/probe\"));\n")]
// A computed target holding a `.rs` literal is still one the scan cannot
// resolve; the whole argument has to be a single literal.
#[case::includes_a_computed_rust_path("include!(concat!(env!(\"OUT_DIR\"), \"/probe.rs\"));\n")]
// A bare extension is a name the walk never collects, so the file it reaches is
// never scanned however the inclusion reads.
#[case::includes_a_bare_extension("include!(\".rs\");\n")]
// A `.rs` target is not enough on its own: rustc resolves it against the file
// that writes it, and the walk skips `target` and every dot-prefixed directory.
// Each of these names a real Rust file the compiler reads and the scan does not.
#[case::includes_under_a_dot_directory("include!(\".generated/bypass.rs\");\n")]
#[case::includes_under_target("include!(\"../target/debug/bypass.rs\");\n")]
// A target that climbs out of the tree the walk was handed is reachable by the
// compiler and by no walk rooted there.
#[case::includes_above_the_root("include!(\"../../bypass.rs\");\n")]
#[case::includes_an_absolute_path("include!(\"/tmp/bypass.rs\");\n")]
fn a_suppression_of_a_protected_lint_is_an_offence(#[case] source: &str) -> Result<()> {
    let found = scan(source)?;
    ensure!(found.len() == 1, "expected one offence, found {found:?}");
    Ok(())
}

/// Scenario: text that resembles a suppression but is not one.
/// Invariant: none is an offence. A doc comment describing the policy, a string
/// literal quoting it, and a lint whose name merely contains a protected one
/// must all pass. This file and its siblings quote the attribute they prohibit,
/// and `clippy::alloc_instead_of_core` begins with `clippy::all`, so a
/// substring comparison would report both.
///
/// An item-scoped `expect` was among these cases and is not any longer: it is
/// sanctioned at a named composition root and an offence anywhere else, so it
/// cannot be judged without a path. [`a_sanctioned_expect_is_not_an_offence`]
/// and [`a_sanctioned_expect_elsewhere_is_an_offence`] hold the two halves,
/// each naming the file and item it is judged against.
#[rstest::rstest]
#[case::prose(
    "//! Never write #![allow(clippy::disallowed_methods)] at a crate root.\nfn f() {}\n"
)]
#[case::string_literal("const EXAMPLE: &str = \"#![allow(warnings)]\";\n")]
#[case::longer_name("#[allow(clippy::alloc_instead_of_core)]\nfn f() {}\n")]
#[case::unrelated_lint("#![allow(dead_code, reason = \"shared module\")]\n")]
// The ordinary idiom for carrying doc comments onto a generated setter. It
// forwards the path, but over an `ident`, an `ident` and a `ty`, none of which
// can carry a call.
#[case::doc_forwarding_setter(
    concat!(
        "macro_rules! option_setter {\n    ($(#[$meta:meta])* $name:ident, $field:ident, ",
        "$ty:ty) => {\n        $(#[$meta])*\n        pub fn $name(mut self, value: $ty) ",
        "-> Self { self.$field = Some(value); self }\n    };\n}\n"
    )
)]
// Attributes whose own path is written out, forwarding only an argument.
#[case::forwards_only_a_doc_argument(
    "macro_rules! documented { ($text:expr) => { #[doc = $text] pub fn f() {} }; }\n"
)]
#[case::forwards_only_a_derive_argument(
    "macro_rules! derived { ($traits:path) => { #[derive($traits)] pub struct H; }; }\n"
)]
// A transcriber emitting an allow of a lint the policy does not protect.
#[case::generated_allow_of_an_unprotected_lint(
    "macro_rules! generated { () => { #[allow(dead_code, reason = \"generated\")] fn f() {} }; }\n"
)]
// Embedding bytes is not compiling source, whatever the extension.
#[case::includes_a_string("const S: &str = include_str!(\"data/table.dat\");\n")]
#[case::includes_bytes("const B: &[u8] = include_bytes!(\"data/table.dat\");\n")]
// An `include!` of a literal `.rs` path names a file the scan reads itself,
// whichever way the literal is spelled: the rule judges what it means.
#[case::includes_rust_source("include!(\"generated.rs\");\n")]
#[case::includes_a_raw_string_path("include!(r\"generated.rs\");\n")]
#[case::includes_an_escaped_path("include!(\"generated\\x2Ers\");\n")]
// A target in a directory the walk descends into is read in its own right, so
// the depth of the path is not what decides.
#[case::includes_a_nested_rust_source("include!(\"sub/generated.rs\");\n")]
// Only directories are judged by the walk's rule. The walk collects a file by
// its extension alone, so a dot-prefixed file name is read like any other.
#[case::includes_a_dot_prefixed_file("include!(\".hidden.rs\");\n")]
// An attribute handed to a macro that may discard it is not a suppression.
#[case::attribute_handed_to_an_invocation(
    "assert_shape!(#[allow(clippy::disallowed_methods)] fn f() {});\n"
)]
fn text_resembling_a_suppression_is_not_an_offence(#[case] source: &str) -> Result<()> {
    let found = scan(source)?;
    ensure!(found.is_empty(), "expected no offence, found {found:?}");
    Ok(())
}
