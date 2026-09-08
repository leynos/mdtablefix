//! Source scan closing the routes around the policy that Clippy cannot see.
//!
//! The other contracts check the configuration, prove the lint fires, and keep
//! CI running both. None of them sees a source file that switches the lint off
//! for itself. `clippy::allow_attributes`, which is what makes the seam
//! taxonomy's "an `#[expect]` carrying a reason, never an `allow`" rule
//! enforceable, does not fire on an inner attribute, so a crate-level
//! suppression disarms the policy with every gate still green.
//!
//! `expect` is untouched. It is the sanctioned form at a composition root
//! precisely because it warns once the site grows a seam; `allow` is silent
//! forever. A scan that rejected `expect` would push contributors toward the
//! attribute that never warns.
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
//! #![allow(warnings)]                                   1   <- see below
//! ```
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
//! ```
//!
//! The last is the control that keeps this contract honest: that name begins
//! with `clippy::all`, so a substring comparison reports it and a contributor
//! has no way to tell a real finding from a spurious one.

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;

#[path = "support/allow_scan.rs"]
mod allow_scan;

use allow_scan::{rust_sources, suppressed_lints};

/// Directories holding Rust sources the policy governs.
///
/// Fixture sources under `tests/data` keep a `.rs.txt` extension and nothing
/// compiles them, so they are out of scope by construction.
const SOURCE_ROOTS: [&str; 3] = ["src", "tests", "test-macros/src"];

/// The crate root, used as the capability root for the scan.
fn manifest_dir() -> Utf8PathBuf { Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// Scenario: every compiled Rust source is parsed and its attributes examined.
/// Invariant: none allows a protected lint. The inner form is the case that
/// matters, because Clippy's own `allow_attributes` cannot see one, so nothing
/// else in this repository would notice the policy had been switched off.
#[test]
fn no_source_allows_a_policy_lint() -> Result<()> {
    let root = manifest_dir();
    let mut offences = Vec::new();
    let mut scanned = 0_usize;

    for source_root in SOURCE_ROOTS {
        let sources = rust_sources(&root, source_root)?;
        ensure!(
            !sources.is_empty(),
            "{source_root} should contain Rust sources to scan"
        );
        scanned += sources.len();
        for (path, contents) in sources {
            for (lint, attribute) in
                suppressed_lints(&contents).with_context(|| format!("scan {path}"))?
            {
                offences.push(format!("{path} allows {lint} via {attribute}"));
            }
        }
    }

    ensure!(
        scanned > 20,
        "the scan should cover the sources, saw {scanned}"
    );
    ensure!(
        offences.is_empty(),
        "no source may allow a protected lint; use an item-scoped `#[expect(..., reason = \
         \"...\")]` at a composition root instead:\n{}",
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
/// review than a crate-level one.
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
fn a_suppression_of_a_protected_lint_is_an_offence(#[case] source: &str) -> Result<()> {
    let found = suppressed_lints(source)?;
    ensure!(found.len() == 1, "expected one offence, found {found:?}");
    Ok(())
}

/// Scenario: text that resembles a suppression but is not one.
/// Invariant: none is an offence. A doc comment describing the policy, a string
/// literal quoting it, and a lint whose name merely contains a protected one
/// must all pass. This file and its siblings quote the attribute they prohibit,
/// and `clippy::alloc_instead_of_core` begins with `clippy::all`, so a
/// substring comparison would report both.
#[rstest::rstest]
#[case::prose(
    "//! Never write #![allow(clippy::disallowed_methods)] at a crate root.\nfn f() {}\n"
)]
#[case::string_literal("const EXAMPLE: &str = \"#![allow(warnings)]\";\n")]
#[case::longer_name("#[allow(clippy::alloc_instead_of_core)]\nfn f() {}\n")]
#[case::unrelated_lint("#![allow(dead_code, reason = \"shared module\")]\n")]
#[case::expect_form("#![expect(clippy::all, reason = \"x\")]\n")]
fn text_resembling_a_suppression_is_not_an_offence(#[case] source: &str) -> Result<()> {
    let found = suppressed_lints(source)?;
    ensure!(found.is_empty(), "expected no offence, found {found:?}");
    Ok(())
}
