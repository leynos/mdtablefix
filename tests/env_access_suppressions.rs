//! Contract coverage for the one route around the policy that Clippy misses.
//!
//! `clippy::allow_attributes` rejects `#[allow(clippy::disallowed_methods)]` on
//! an item, which is what makes the seam taxonomy's "an `#[expect]` carrying a
//! reason, never an `allow`" rule enforceable rather than merely stated. It
//! does not fire on an *inner* attribute. So this,
//!
//! ```text
//! #![allow(clippy::disallowed_methods, reason = "...")]
//! ```
//!
//! at the top of a module silences the ban for everything in it and passes
//! `make lint`. Measured on 2026-09-08: with that line at the top of
//! `src/lib.rs` and a `std::env::var` call below it, Clippy reported no
//! disallowed-method diagnostic at all.
//!
//! Nothing else in this repository would notice. `clippy.toml` still lists the
//! six methods, both manifests still deny the lint, the `lint` recipe still
//! runs Clippy over both packages, and CI still runs the recipe. Every contract
//! passes and the policy is off.
//!
//! This file closes that by reading the sources directly. It is a text scan,
//! which is normally the wrong shape for a contract here, but an attribute *is*
//! source text: there is no execution to model, and the thing being asserted is
//! exactly "this text does not appear in an allow attribute".
//!
//! A blanket suppression is treated the same way, because `warnings` and
//! `clippy::all` disable the policy lints just as effectively as naming them.
//!
//! Mutation proof (2026-09-08). Each mutation was applied alone and reverted:
//!
//! ```text
//! add an inner allow of clippy::disallowed_methods to src/lib.rs
//!   -> src/lib.rs allows clippy::disallowed_methods, which switches the
//!      environment-access policy off for that module
//! add an item allow of `warnings` to renumber_lists in src/lists.rs
//!   -> src/lists.rs allows warnings, ...
//! wrap the same inner allow across four lines, as rustfmt would
//!   -> src/lib.rs allows clippy::disallowed_methods, ...
//! ```
//!
//! `tests/support/lint_policy.rs` carries `#![allow(dead_code, ...)]` and
//! passes, as it should: that module is shared by five test binaries and each
//! uses a different subset of its readers. The rule is about the lints that
//! carry the policy, not about suppression in general.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, DirEntry},
};

/// The lint names no source may allow.
///
/// The first three are the policy itself. The last two are blanket
/// suppressions that would switch all three off without naming any of them.
const PROTECTED_LINTS: [&str; 5] = [
    "clippy::disallowed_methods",
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
    "warnings",
    "clippy::all",
];

/// The directories holding compiled Rust sources.
///
/// Fixture sources under `tests/data` are stored with a `.rs.txt` extension and
/// nothing compiles them, so they are out of scope by construction.
const SOURCE_ROOTS: [&str; 3] = ["src", "tests", "test-macros/src"];

/// The crate root, used as the capability root for the scan.
fn manifest_dir() -> Utf8PathBuf { Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// Return one directory entry's path relative to the scan root, and whether it
/// is itself a directory.
///
/// Split out from [`rust_sources`] so the walk reads as a walk: the name and
/// type lookups are two more fallible steps that otherwise sit between the loop
/// and the one decision it makes.
fn entry_path(entry: &DirEntry, relative: &Utf8Path) -> Result<(Utf8PathBuf, bool)> {
    let name = entry
        .file_name()
        .with_context(|| format!("read a file name in {relative}"))?;
    let path = relative.join(&name);
    let is_directory = entry
        .file_type()
        .with_context(|| format!("stat {path}"))?
        .is_dir();
    Ok((path, is_directory))
}

/// Collect every `.rs` file under `relative`, depth first.
fn rust_sources(root: &Utf8Path, relative: &Utf8Path, found: &mut Vec<Utf8PathBuf>) -> Result<()> {
    let directory = Dir::open_ambient_dir(root.join(relative), ambient_authority())
        .with_context(|| format!("open {relative}"))?;
    for entry in directory
        .entries()
        .with_context(|| format!("read {relative}"))?
    {
        let entry = entry.with_context(|| format!("read an entry of {relative}"))?;
        let (path, is_directory) = entry_path(&entry, relative)?;
        if is_directory {
            rust_sources(root, &path, found)?;
        } else if path.extension() == Some("rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Return the lint names inside every `allow` attribute in `source`.
///
/// Both `#[allow(..)]` and `#![allow(..)]` are read, since the inner form is
/// the one Clippy's own `allow_attributes` lint does not police.
///
/// An attribute is recognized only where a line begins with one, which is where
/// `rustfmt` puts every attribute in this repository. Searching the whole text
/// instead would match prose: this very file, and the mutation record in
/// `tests/env_access_policy.rs`, both quote the attribute they exist to
/// prohibit, and a scan that could not tell the difference reported them as
/// violations on its first run.
///
/// The contents run to the matching parenthesis, so an attribute that `rustfmt`
/// has wrapped across several lines, or one carrying a nested
/// `reason = "..."`, is read whole rather than truncated.
fn allowed_lints(source: &str) -> Vec<&str> {
    let mut found = Vec::new();
    for (offset, line) in line_offsets(source) {
        let trimmed = line.trim_start();
        let Some(opening) = ["#![allow(", "#[allow("]
            .into_iter()
            .find(|opening| trimmed.starts_with(opening))
        else {
            continue;
        };
        let contents = offset + (line.len() - trimmed.len()) + opening.len();
        found.push(until_closing_parenthesis(&source[contents..]));
    }
    found
}

/// Return each line of `source` with its byte offset.
fn line_offsets(source: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut offset = 0;
    source.lines().map(move |line| {
        let start = offset;
        // `lines` strips the terminator, which is one byte for `\n` and two for
        // `\r\n`; either way the next line starts after it.
        offset += line.len() + 1;
        (start, line)
    })
}

/// Return the prefix of `text` up to the parenthesis closing the one already
/// opened, or all of it if the source is truncated.
fn until_closing_parenthesis(text: &str) -> &str {
    let mut depth = 1_usize;
    for (index, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &text[..index];
                }
            }
            _ => {}
        }
    }
    text
}

/// Scenario: every compiled Rust source is read for its `allow` attributes.
/// Invariant: none allows a policy lint or a blanket suppression that would
/// cover one. Clippy cannot enforce this itself, because `allow_attributes`
/// does not fire on an inner attribute, so a module-level allow is the one way
/// left to switch the environment-access policy off without any other contract
/// in this repository noticing.
#[test]
fn no_source_allows_a_policy_lint() -> Result<()> {
    let root = manifest_dir();
    let mut sources = Vec::new();
    for source_root in SOURCE_ROOTS {
        rust_sources(&root, Utf8Path::new(source_root), &mut sources)?;
    }
    ensure!(
        !sources.is_empty(),
        "the scan should find Rust sources under {SOURCE_ROOTS:?}"
    );

    let directory = Dir::open_ambient_dir(&root, ambient_authority())
        .with_context(|| format!("open the crate root {root}"))?;
    for path in sources {
        let source = directory
            .read_to_string(path.as_str())
            .with_context(|| format!("read {path}"))?;
        for allowed in allowed_lints(&source) {
            for lint in PROTECTED_LINTS {
                ensure!(
                    !allowed.split(',').any(|name| name.trim() == lint),
                    "{path} allows {lint}, which switches the environment-access policy off for \
                     that module; use an item-scoped `#[expect(..., reason = \"...\")]` at a \
                     composition root instead"
                );
            }
        }
    }
    Ok(())
}
