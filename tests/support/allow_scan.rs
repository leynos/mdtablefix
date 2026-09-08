//! Readers behind the environment-access suppression scan.
//!
//! `tests/env_access_suppressions.rs` states the contract; this module
//! holds the walk and the parsing it needs. The two are separated so
//! neither file outgrows the repository's 400-line limit, and so the
//! readers can be exercised against inline fixtures as well as against the
//! repository's own sources.
//!
//! Every reader returns a `Result`. None panics, so a source that does not
//! parse surfaces as a test failure naming the file rather than as a panic
//! in a helper.

use std::collections::VecDeque;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, DirEntry},
};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::{
    AttrStyle,
    Attribute,
    Macro,
    Meta,
    MetaList,
    Path,
    Token,
    ext::IdentExt,
    punctuated::Punctuated,
    visit::Visit,
};

/// Lint names whose suppression disarms the environment-access policy.
///
/// Each is either a policy lint, a group containing one, or a blanket that
/// covers them. See the module documentation for the measurement behind each.
const PROTECTED_LINTS: [&str; 7] = [
    // The policy itself, and the groups that contain it.
    "clippy::disallowed_methods",
    "clippy::style",
    "clippy::all",
    "warnings",
    // The guard that keeps a suppression to an `expect`, and its group.
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
    "clippy::restriction",
];

/// What one directory entry contributes to the walk.
enum Found {
    /// A subdirectory, with the capability to read it and its path.
    Directory(Dir, Utf8PathBuf),
    /// A Rust source, with its path and contents.
    Source(Utf8PathBuf, String),
    /// Anything else, which the scan does not govern.
    Ignored,
}

/// Directory names the walk does not descend into.
///
/// `target` holds build output, and a dot-prefixed name holds tool state such
/// as `.git`. Neither is a place a contributor writes a source Cargo compiles.
/// Everything else is walked, so a build script, bench, example or second
/// binary is seen wherever it is added.
fn is_walkable(name: &str) -> bool { !name.starts_with('.') && name != "target" }

/// Render a walk prefix for a message, naming the root rather than showing an
/// empty string.
fn shown(prefix: &Utf8Path) -> &str {
    if prefix.as_str().is_empty() {
        "the repository root"
    } else {
        prefix.as_str()
    }
}

/// Classify one directory entry, reading it if it is a Rust source.
///
/// Split out from [`rust_sources`] so the walk reads as a walk: the name, type,
/// open and read steps are four more fallible operations that otherwise sit
/// between the loop and the one decision it makes.
fn classify_entry(current: &Dir, prefix: &Utf8Path, entry: &DirEntry) -> Result<Found> {
    let name = entry
        .file_name()
        .with_context(|| format!("read a file name in {}", shown(prefix)))?;
    let path = prefix.join(&name);
    if entry
        .file_type()
        .with_context(|| format!("stat {path}"))?
        .is_dir()
    {
        if !is_walkable(&name) {
            return Ok(Found::Ignored);
        }
        let child = current
            .open_dir(&name)
            .with_context(|| format!("open {path}"))?;
        return Ok(Found::Directory(child, path));
    }
    if path.extension() != Some("rs") {
        return Ok(Found::Ignored);
    }
    let contents = current
        .read_to_string(&name)
        .with_context(|| format!("read {path}"))?;
    Ok(Found::Source(path, contents))
}

/// Read every `.rs` file under `relative`, breadth first, with its contents.
///
/// Each directory is opened through its parent's capability rather than by
/// absolute path, so the walk cannot leave the tree it was handed. Pass `"."`
/// for `relative` to cover a whole repository; the paths then come back
/// relative to its root.
pub fn rust_sources(root: &Utf8Path, relative: &str) -> Result<Vec<(Utf8PathBuf, String)>> {
    let directory = Dir::open_ambient_dir(root.join(relative), ambient_authority())
        .with_context(|| format!("open {relative}"))?;
    let base = if relative == "." {
        Utf8PathBuf::new()
    } else {
        Utf8PathBuf::from(relative)
    };
    let mut pending = VecDeque::from([(directory, base)]);
    let mut sources = Vec::new();

    while let Some((current, prefix)) = pending.pop_front() {
        let entries = current
            .entries()
            .with_context(|| format!("read {}", shown(&prefix)))?;
        for candidate in entries {
            let entry =
                candidate.with_context(|| format!("read an entry of {}", shown(&prefix)))?;
            match classify_entry(&current, &prefix, &entry)? {
                Found::Directory(child, path) => pending.push_back((child, path)),
                Found::Source(path, contents) => sources.push((path, contents)),
                Found::Ignored => {}
            }
        }
    }
    Ok(sources)
}

/// One suppression found in a source, as it was written.
struct Suppression {
    /// Whether it was an inner attribute, for rendering it back.
    inner: bool,
    /// Its parsed contents.
    meta: Meta,
}

/// Collect every attribute in a parsed file, wherever it sits.
///
/// A visitor is used rather than a walk over top-level items so that
/// attributes on nested items, on items declared inside a function body, and
/// on expressions are all reached. A suppression hidden in a private helper
/// disarms the policy for that helper just as effectively as one at the crate
/// root.
///
/// Macro token streams are walked as well. `syn` keeps a `macro_rules!` arm's
/// body as an opaque `TokenStream`, so an attribute written there is never
/// parsed into one and never reaches `visit_attribute`; Clippy, which sees the
/// expansion, honours it. Every group is descended into, so a suppression
/// nested through more than one macro is still found.
#[derive(Default)]
struct AttributeCollector {
    found: Vec<Suppression>,
}

/// Return the attribute beginning at `index`, with the index just past it.
///
/// An attribute is `#`, optionally `!`, then a bracketed group. The group's
/// contents are parsed as a `Meta`, so a recovered attribute is judged by the
/// same function as a real one rather than by a second, weaker test. `None`
/// covers everything else, including a group that does not parse: a macro body
/// may hold token sequences that are not Rust until they are expanded.
fn attribute_at(trees: &[TokenTree], index: usize) -> Option<(Suppression, usize)> {
    let Some(TokenTree::Punct(hash)) = trees.get(index) else {
        return None;
    };
    if hash.as_char() != '#' {
        return None;
    }
    let mut next = index + 1;
    let inner = matches!(trees.get(next), Some(TokenTree::Punct(bang)) if bang.as_char() == '!');
    if inner {
        next += 1;
    }
    let Some(TokenTree::Group(group)) = trees.get(next) else {
        return None;
    };
    if group.delimiter() != Delimiter::Bracket {
        return None;
    }
    let meta = syn::parse2::<Meta>(group.stream()).ok()?;
    Some((Suppression { inner, meta }, next + 1))
}

impl AttributeCollector {
    /// Record any attribute-shaped token sequence in `tokens`.
    ///
    /// Every group is descended into, so a suppression nested through more than
    /// one macro is still found.
    fn collect_from_tokens(&mut self, tokens: TokenStream) {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        let mut index = 0;
        while index < trees.len() {
            if let Some((suppression, after)) = attribute_at(&trees, index) {
                self.found.push(suppression);
                index = after;
                continue;
            }
            if let TokenTree::Group(group) = &trees[index] {
                self.collect_from_tokens(group.stream());
            }
            index += 1;
        }
    }
}

impl<'ast> Visit<'ast> for AttributeCollector {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.found.push(Suppression {
            inner: matches!(attribute.style, AttrStyle::Inner(_)),
            meta: attribute.meta.clone(),
        });
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.collect_from_tokens(mac.tokens.clone());
        syn::visit::visit_macro(self, mac);
    }
}

/// Render a lint path with raw identifiers normalized.
///
/// `r#allow` and `allow` are the same identifier to the compiler, as are
/// `clippy::r#style` and `clippy::style`, and Clippy honours the raw spelling.
/// Comparing the written form would let either escape the contract, so every
/// segment is unrawed before it is joined.
fn render_path(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Return the lint names an `allow` list suppresses.
///
/// A key-value argument such as `reason = "..."` is not a lint name and is
/// skipped, as is a nested list.
fn allowed_lints(list: &MetaList) -> Vec<String> {
    let Ok(nested) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return Vec::new();
    };
    nested
        .iter()
        .filter_map(|meta| match meta {
            Meta::Path(path) => Some(render_path(path)),
            Meta::List(_) | Meta::NameValue(_) => None,
        })
        .collect()
}

/// Return the lint names nested inside a `cfg_attr`.
///
/// The first element is the condition and is skipped. The condition is never
/// evaluated: a suppression that applies under some configuration is still a
/// suppression, and deciding which configurations are reachable is not this
/// contract's job.
///
/// `inner` is the scope of the *outermost* attribute and is carried down
/// unchanged, because that is what decides the scope a nested suppression
/// actually takes effect at: `#![cfg_attr(all(), expect(..))]` expects at crate
/// scope however deeply the `expect` is wrapped.
fn suppressed_by_cfg_attr(list: &MetaList, inner: bool) -> Vec<String> {
    let Ok(nested) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return Vec::new();
    };
    nested
        .iter()
        .skip(1)
        .flat_map(|meta| suppressed_by(meta, inner))
        .collect()
}

/// Return the lint names one attribute's contents suppress, following
/// `cfg_attr`.
///
/// `expect` is judged by scope rather than waved through. An item-scoped outer
/// `#[expect(..., reason = "...")]` is the sanctioned form: it covers one site
/// and warns once that site grows a seam. An *inner* `#![expect(..)]` is not,
/// because a single call anywhere in the crate fulfils it, so it reports
/// nothing and never warns. Measured: with a live `std::env::var` call,
/// `#![expect(clippy::disallowed_methods)]` produced no diagnostic and no
/// unfulfilled-expectation warning, which is silence indistinguishable from
/// `allow`.
///
/// Taking a `Meta` rather than an `Attribute` is what lets an attribute
/// recovered from a macro token stream be judged by exactly this function,
/// rather than by a second and weaker test written for tokens.
fn suppressed_by(meta: &Meta, inner: bool) -> Vec<String> {
    let Ok(list) = meta.require_list() else {
        return Vec::new();
    };
    match render_path(meta.path()).as_str() {
        "allow" => allowed_lints(list),
        "expect" if inner => allowed_lints(list),
        "cfg_attr" => suppressed_by_cfg_attr(list, inner),
        _ => Vec::new(),
    }
}

/// Render a suppression roughly as written, for a failure message.
fn render_attribute(suppression: &Suppression) -> String {
    let bang = if suppression.inner { "!" } else { "" };
    let path = render_path(suppression.meta.path());
    suppression.meta.require_list().map_or_else(
        |_| format!("#{bang}[{path}]"),
        |list| format!("#{bang}[{path}({})]", list.tokens),
    )
}

/// Return every protected lint suppressed in one source file, with the
/// attribute that suppressed it.
pub fn suppressed_lints(contents: &str) -> Result<Vec<(String, String)>> {
    let parsed = syn::parse_file(contents).context("parse the source as Rust")?;
    let mut collector = AttributeCollector::default();
    collector.visit_file(&parsed);

    let mut found = Vec::new();
    for suppression in &collector.found {
        for lint in suppressed_by(&suppression.meta, suppression.inner) {
            if PROTECTED_LINTS.contains(&lint.as_str()) {
                found.push((lint, render_attribute(suppression)));
            }
        }
    }
    Ok(found)
}
