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
//!
//! Two further routes were closed on 2026-09-14, each measured against Clippy
//! elsewhere in the estate before being closed here. Neither is an attribute
//! where it is written, so neither is judged by a `Meta`.
//!
//! A `macro_rules!` arm may forward the attribute's *path*: `#[$attr]` is
//! written by the arm and completed by the caller, so the arm's attribute does
//! not parse and the invocation carries no `#` for a walk to notice. Invoked as
//! `forward!(allow(clippy::disallowed_methods))`, the expansion silences every
//! call the item contains. And `rustc` parses an `include!` target as Rust
//! whatever its extension, so an `allow` inside a `.rs.txt` fixture silences
//! the calls around the inclusion while an enclosing `expect` stays fulfilled
//! and warns about nothing.
//!
//! Both rules are deliberately narrow, because a contract that reports a false
//! positive gets switched off and then reports nothing at all. A forwarded path
//! is refused at inner scope, which applies to everything around it, and at
//! outer scope only where the arm writes an `env` access itself or forwards a
//! fragment the caller fills with code. `$(#[$meta:meta])*` over an `ident`, an
//! `ident` and a `ty` is the ordinary idiom for carrying doc comments onto a
//! generated setter and is left alone, as are `#[doc = $text]` and
//! `#[derive($traits)]`, whose paths are written out. The walk now descends
//! only into `macro_rules!` transcribers, since an attribute handed to an
//! invocation may be discarded by the macro it reaches; that narrowing is safe
//! only because the forwarded-path rule covers what the transcriber writes.
//!
//! Each rule was proved in both directions through the build, because a rule
//! that reaches nothing and a rule that reaches everything both pass a
//! one-sided proof:
//!
//! ```text
//! forwarded_path declines after reaching its guards
//!   -> the three forwarding cases fail with "expected one offence, found []"
//! refuse every forwarded path, not only a reachable or inner one
//!   -> doc_forwarding_setter fails; the idiom is not an offence
//! refuse any metavariable in the attribute, not only in its path
//!   -> forwards_only_a_doc_argument fails; #[doc = $text] is not an offence
//! accept .txt alongside .rs as an include! target
//!   -> includes_a_foreign_extension fails; the fixture route reopens
//! judge include_str! as source inclusion
//!   -> includes_a_string and includes_bytes fail; embedding is not compiling
//! ```
//!
//! The include! target is judged by its decoded value and by the extension the
//! walk selects on, proved in both directions on 2026-09-15:
//!
//! ```text
//! read the literal as written, trimming quotes off Literal::to_string
//!   -> includes_a_raw_string_path and includes_an_escaped_path fail, and
//!      includes_a_bare_extension with them
//! accept a .rs literal found anywhere in the argument
//!   -> includes_a_computed_rust_path fails, alone
//! report no include! target at all
//!   -> the four refused include cases fail
//! compare the rendered path with ends_with(".rs")
//!   -> includes_a_bare_extension fails, alone
//! ```
//!
//! The two mutations above were re-run against the changed mechanism.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};

#[path = "support/allow_scan.rs"]
mod allow_scan;

use allow_scan::{roots::SANCTIONED_ROOTS, rust_sources, suppressed_lints};

/// The path an inline fixture pretends to be.
///
/// Every fixture below is judged as an ordinary source, not as one of the
/// sanctioned composition roots, so a rule that depends on the path is
/// exercised on the side that has to hold everywhere.
const FIXTURE_PATH: &str = "src/fixture.rs";

/// Judge one inline fixture as if it were an ordinary source file.
fn scan(source: &str) -> Result<Vec<String>> {
    suppressed_lints(Utf8Path::new(FIXTURE_PATH), source)
}

/// Directories that must be represented in the scan.
///
/// These are not a filter. The scan walks the whole repository, so a source
/// added outside them is still read; they are a tripwire, so that a walk which
/// silently returned nothing, or stopped at the first directory, reports a
/// failure rather than a clean repository.
///
/// Fixture sources under `tests/data` keep a `.rs.txt` extension and nothing
/// compiles them, so they are out of scope by construction.
const REQUIRED_ROOTS: [&str; 3] = ["src", "tests", "test-macros/src"];

/// Return whether `path` lies under `root`, which is written with `/`.
///
/// Components are compared rather than the rendered string, because the walk
/// joins paths with the platform separator: on Windows every path reads
/// `src\config\mod.rs`, so a `starts_with("src/")` prefix test matched nothing
/// and this file's coverage assertion failed there while passing on Linux.
fn is_under(path: &Utf8Path, root: &str) -> bool {
    let mut components = path.components();
    root.split('/')
        .all(|expected| components.next().map(|actual| actual.as_str()) == Some(expected))
}

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
            sources.iter().any(|(path, _)| is_under(path, required)),
            "the scan should reach {required}, saw {} sources",
            sources.len()
        );
    }

    let mut offences = Vec::new();
    for (path, contents) in &sources {
        for finding in suppressed_lints(path, contents).with_context(|| format!("scan {path}"))? {
            offences.push(format!("{path} {finding}"));
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
///
/// The exemption is pinned to the documented roots. Exempting every
/// item-scoped `expect` honoured the attribute but not the rule ADR-0012
/// states: a contributor could write the same attribute over any function, add
/// a `std::env` read beneath it, and pass both Clippy and this scan. The scan
/// therefore asks which item, in which file, and
/// [`a_sanctioned_expect_elsewhere_is_an_offence`] is the other half.
#[rstest::rstest]
#[case::write_failure_child(0)]
#[case::ambient_variable(1)]
fn a_sanctioned_expect_is_not_an_offence(#[case] index: usize) -> Result<()> {
    let (file, item) = SANCTIONED_ROOTS[index];
    let sanctioned = format!(
        concat!(
            "#[expect(clippy::disallowed_methods, reason = \"composition root\")]\n",
            "fn {item}() -> Option<String> {{ std::env::var(\"HOME\").ok() }}\n",
        ),
        item = item
    );
    let found = suppressed_lints(Utf8Path::new(file), &sanctioned)?;
    ensure!(found.is_empty(), "expected no offence, found {found:?}");
    Ok(())
}

/// Scenario: the sanctioned attribute, written somewhere it was not sanctioned.
/// Invariant: it is an offence. The two cases are the same attribute over the
/// same function body, differing only in the file and the item it sits on,
/// which is exactly the discrimination the rule claims to make. Without both,
/// the rule could be satisfied by a reader that answered the same way whatever
/// it was asked.
///
/// Mutation proof (2026-09-15), each applied alone and reverted: exempting
/// every outer `expect`, as the scan did before, fails both cases with
/// `expected an offence`; refusing every outer `expect` fails
/// [`a_sanctioned_expect_is_not_an_offence`] on both roots.
///
/// A third mutation was applied and is recorded as not discriminating, because
/// a mutation that survives proves nothing and saying so is the point.
/// Comparing the path by its rendered string rather than by its components
/// passes every case on Linux: the walk joins with `/` here, so the two
/// readings agree on every path it can produce. They part only on Windows,
/// where the walk joins with `\` and a `/`-written expectation matches
/// nothing, which is the platform this host cannot exercise. The components
/// reading is kept for the reason the sibling `is_under` reader already uses
/// it, not on the strength of a proof run here.
#[rstest::rstest]
#[case::the_right_item_in_the_wrong_file("src/lib.rs", "write_failure_child")]
#[case::the_wrong_item_in_the_right_file("tests/rewrite_atomic.rs", "some_other_test")]
fn a_sanctioned_expect_elsewhere_is_an_offence(
    #[case] file: &str,
    #[case] item: &str,
) -> Result<()> {
    let source = format!(
        concat!(
            "#[expect(clippy::disallowed_methods, reason = \"composition root\")]\n",
            "fn {item}() -> Option<String> {{ std::env::var(\"HOME\").ok() }}\n",
        ),
        item = item
    );
    let found = suppressed_lints(Utf8Path::new(file), &source)?;
    ensure!(found.len() == 1, "expected an offence, found {found:?}");
    Ok(())
}

// The inline fixture cases live in their own file, per the repository's
// 400-line cap. The path is relative to this file's own directory, `tests/`.
#[path = "env_access_suppressions/fixtures.rs"]
mod fixtures;

/// Scenario: a scanned path is tested against a required root.
///
/// Invariant: the answer depends on the path's components, not on the
/// separator the platform renders them with. The Windows lane on PR #463
/// failed on exactly this: the walk joins with `\` there, so a
/// `starts_with("src/")` prefix test matched none of 285 sources and the
/// coverage assertion reported the scan had missed `src/` entirely.
///
/// `Utf8PathBuf::from(..).join(..)` is used rather than a literal so the
/// separator is the platform's own, which is what the walk produces.
///
/// Mutation proof (2026-09-14), run through the build and reverted: putting
/// `is_under` back to `path.as_str().starts_with(root)` fails
/// `prefix_is_not_a_component`, which is the Linux-visible half of the same
/// defect: `src-generated/lib.rs` is not under `src`.
#[rstest::rstest]
#[case::direct_child(&["src", "lib.rs"], "src", true)]
#[case::nested(&["src", "wrap", "fence.rs"], "src", true)]
#[case::two_deep(&["test-macros", "src", "lib.rs"], "test-macros/src", true)]
#[case::different_root(&["tests", "cli.rs"], "src", false)]
#[case::prefix_is_not_a_component(&["src-generated", "lib.rs"], "src", false)]
#[case::root_deeper_than_path(&["test-macros"], "test-macros/src", false)]
fn a_path_is_under_a_root_by_component(
    #[case] segments: &[&str],
    #[case] root: &str,
    #[case] expected: bool,
) {
    let path = segments
        .iter()
        .fold(Utf8PathBuf::new(), |path, segment| path.join(segment));
    assert_eq!(is_under(&path, root), expected, "{path} under {root}");
}
