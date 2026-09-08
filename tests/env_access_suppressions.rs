//! Source scan closing the routes around the policy that Clippy cannot see.
//!
//! The other contracts check the configuration, prove the lint fires, and keep
//! CI running both. None of them sees a source file that switches the lint off
//! for itself. `clippy::allow_attributes`, which is what makes the seam
//! taxonomy's "an `#[expect]` carrying a reason, never an `allow`" rule
//! enforceable, does not fire on an inner attribute, so a crate-level
//! suppression disarms the policy with every gate still green.
//!
//! An *item-scoped* `expect` is untouched. It is the sanctioned form at a
//! composition root precisely because it warns once the site grows a seam;
//! `allow` is silent forever, and a scan that rejected `expect` outright would
//! push contributors toward the attribute that never warns.
//!
//! A *crate-scoped* `expect` earns no such exemption. One call anywhere in the
//! crate fulfils it, so it reports nothing and raises no unfulfilled-
//! expectation warning either. Measured with a live `std::env::var` call,
//! `#![expect(clippy::disallowed_methods)]` gave zero diagnostics and zero
//! unfulfilled warnings: silence indistinguishable from `allow`. So `expect` is
//! judged by scope, and the scope that counts is the outermost attribute's,
//! carried down through any `cfg_attr` nesting.
//!
//! # What Clippy actually honours
//!
//! Measured against this repository on 2026-09-08, each with an unannotated
//! `std::env::var("HOME")` in `src/lib.rs`. The count is
//! disallowed-method diagnostics from `cargo clippy --all-targets
//! --all-features`:
//!
//! ```text
//! (nothing)                                             1   <- control
//! #![allow(clippy::disallowed_methods)]                 0
//! #![allow(clippy::style)]                              0   <- the lint's group
//! #![allow(clippy::all)]                                0
//! #![cfg_attr(all(), allow(clippy::disallowed_methods))] 0
//! #![expect(clippy::disallowed_methods)]                0   <- and 0 unfulfilled
//! #![r#allow(clippy::disallowed_methods)]               0   <- raw identifier
//! #![allow(clippy::r#style)]                            0   <- raw identifier
//! #![allow(warnings)]                                   1   <- see below
//! ```
//!
//! The raw spellings are the compiler's own: `r#allow` and `allow` are one
//! identifier, as are `clippy::r#style` and `clippy::style`. Comparing the
//! written form would let either through, so every path segment is unrawed
//! before it is compared.
//!
//! Naming the lint is therefore not enough: Clippy places
//! `disallowed_methods` in `style`, so the group and the wider `clippy::all`
//! each switch it off without ever naming it, and a `cfg_attr` wrapper is
//! honoured while being invisible to both `allow_attributes` and to any scan
//! looking for a line that begins `#![allow(`.
//!
//! `warnings` is the interesting one. It does *not* silence the lint today,
//! because the manifest sets `disallowed_methods = "deny"` and a `warnings`
//! group allow does not override a command-line deny. It is protected anyway:
//! it would silence the lint the moment that level became `warn`, and the
//! level is the other half of a policy a contributor may edit.
//!
//! The guard lints are protected for the same reason one layer down.
//! `#![allow(clippy::restriction)]` silences `clippy::allow_attributes`
//! (measured: one `#[allow] attribute found` error becomes none), which
//! re-opens the item-level `#[allow(clippy::disallowed_methods)]` route.
//!
//! # Why the file is parsed rather than searched
//!
//! An earlier draft matched text at the start of a line. Five shapes defeat
//! that, all of them ordinary: a `cfg_attr` wrapper, a space before the
//! parenthesis, a parenthesis inside a `reason` string, an attribute
//! `rustfmt` has wrapped, and attribute-shaped text inside a string literal or
//! a doc comment. That last one is not hypothetical: the draft's first run
//! reported this repository's own mutation records as violations, because they
//! quote the attribute they exist to prohibit. A contract that reports a false
//! positive gets disabled.
//!
//! Lint names are compared as parsed paths, not as substrings.
//! `clippy::alloc_instead_of_core` begins with `clippy::all`, and a substring
//! test would reject it with no way for a contributor to tell a real finding
//! from a spurious one.
//!
//! # Mutation proof
//!
//! Recorded 2026-09-08. Each applied alone to a real source file, run through
//! the build, and reverted:
//!
//! ```text
//! #![allow(clippy::disallowed_methods)] in src/lib.rs        -> offence
//! #![allow(clippy::style)], naming the group not the lint    -> offence
//! #![cfg_attr(all(), allow(clippy::disallowed_methods))]     -> offence
//! #![allow(clippy::all)] wrapped across several lines        -> offence
//! #[allow(warnings, reason = "...")] on an item              -> offence
//! #[allow(clippy::alloc_instead_of_core)] on an item         -> still passes
//! an allow emitted from a macro_rules! arm                  -> offence
//! the same, nested through a second macro                   -> offence
//! #![expect(clippy::disallowed_methods, reason = "...")]     -> offence
//! #![cfg_attr(all(), expect(clippy::disallowed_methods, ..))] -> offence
//! #![r#allow(clippy::disallowed_methods)]                    -> offence
//! #![allow(clippy::r#style)]                                 -> offence
//! #[expect(clippy::disallowed_methods, reason = "...")]      -> still passes
//!   on a function, which is the sanctioned composition root
//! ```
//!
//! The two macro cases are the reason the collector walks token streams as
//! well as parsed attributes. `syn` keeps a `macro_rules!` arm's body opaque,
//! so an attribute written there never becomes one and never reaches
//! `visit_attribute`. Clippy sees the expansion and honours it: measured at
//! both depths, a macro emitting `#[allow(clippy::disallowed_methods)]` over a
//! live `std::env::var` call produced zero diagnostics.
//!
//! The last is the control that keeps this contract honest: that name begins
//! with `clippy::all`, so a substring comparison reports it and a contributor
//! has no way to tell a real finding from a spurious one.

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;

#[path = "support/allow_scan.rs"]
mod allow_scan;

use allow_scan::{rust_sources, suppressed_lints};

/// Directories that must be represented in the scan.
///
/// These are not a filter. The scan walks the whole repository, so a source
/// added outside them is still read; they are a tripwire, so that a walk which
/// silently returned nothing, or stopped at the first directory, reports a
/// failure rather than a clean repository.
///
/// Fixture sources under `tests/data` keep a `.rs.txt` extension and nothing
/// compiles them, so they are out of scope by construction.
const REQUIRED_ROOTS: [&str; 3] = ["src/", "tests/", "test-macros/src/"];

/// The crate root, used as the capability root for the scan.
fn manifest_dir() -> Utf8PathBuf { Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// Scenario: every Rust source in the repository is parsed and its attributes
/// examined.
/// Invariant: none allows a protected lint. The inner form is the case that
/// matters, because Clippy's own `allow_attributes` cannot see one, so nothing
/// else in this repository would notice the policy had been switched off.
///
/// The walk starts at the repository root rather than at a list of source
/// directories. A list is a filter, and a filter is another thing the
/// mechanism cannot see past: a build script, a bench, an example or a second
/// binary declared outside it would be compiled, would carry any suppression
/// it liked, and would leave this test green. Mutation: `benches/probe.rs`
/// holding `#![allow(clippy::disallowed_methods)]`, with the bench declared in
/// `Cargo.toml`, fails here with `benches/probe.rs allows
/// clippy::disallowed_methods`; under the previous root list it passed.
#[test]
fn no_source_allows_a_policy_lint() -> Result<()> {
    let root = manifest_dir();
    let sources = rust_sources(&root, ".")?;
    for required in REQUIRED_ROOTS {
        ensure!(
            sources
                .iter()
                .any(|(path, _)| path.as_str().starts_with(required)),
            "the scan should reach {required}, saw {} sources",
            sources.len()
        );
    }

    let mut offences = Vec::new();
    for (path, contents) in &sources {
        for (lint, attribute) in
            suppressed_lints(contents).with_context(|| format!("scan {path}"))?
        {
            offences.push(format!("{path} allows {lint} via {attribute}"));
        }
    }

    ensure!(
        offences.is_empty(),
        concat!(
            "no source may allow a protected lint; use an item-scoped ",
            "`#[expect(..., reason = \"...\")]` at a composition root instead:\n{}"
        ),
        offences.join("\n")
    );
    Ok(())
}

/// Scenario: the sanctioned form at a composition root.
/// Invariant: `expect` is not an offence. Rejecting it would push contributors
/// toward `allow`, which is the attribute that never warns once the site is
/// migrated.
#[test]
fn a_sanctioned_expect_is_not_an_offence() -> Result<()> {
    let sanctioned = concat!(
        "#[expect(clippy::disallowed_methods, reason = \"composition root\")]\n",
        "fn read() -> Option<String> { std::env::var(\"HOME\").ok() }\n",
    );
    ensure!(suppressed_lints(sanctioned)?.is_empty());
    Ok(())
}

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
fn a_suppression_of_a_protected_lint_is_an_offence(#[case] source: &str) -> Result<()> {
    let found = suppressed_lints(source)?;
    ensure!(found.len() == 1, "expected one offence, found {found:?}");
    Ok(())
}

/// Scenario: text that resembles a suppression but is not one.
/// Invariant: none is an offence. A doc comment describing the policy, a string
/// literal quoting it, an item-scoped `expect`, and a lint whose name merely
/// contains a protected one must all pass. This file and its siblings quote the attribute they
/// prohibit, and `clippy::alloc_instead_of_core` begins with `clippy::all`, so a
/// substring comparison would report both.
#[rstest::rstest]
#[case::prose(
    "//! Never write #![allow(clippy::disallowed_methods)] at a crate root.\nfn f() {}\n"
)]
#[case::string_literal("const EXAMPLE: &str = \"#![allow(warnings)]\";\n")]
#[case::longer_name("#[allow(clippy::alloc_instead_of_core)]\nfn f() {}\n")]
#[case::unrelated_lint("#![allow(dead_code, reason = \"shared module\")]\n")]
#[case::item_scoped_expect(
    "#[expect(clippy::disallowed_methods, reason = \"composition root\")]\nfn f() {}\n"
)]
fn text_resembling_a_suppression_is_not_an_offence(#[case] source: &str) -> Result<()> {
    let found = suppressed_lints(source)?;
    ensure!(found.is_empty(), "expected no offence, found {found:?}");
    Ok(())
}
