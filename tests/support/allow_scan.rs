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

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use proc_macro2::{TokenStream, TokenTree};
use syn::{AttrStyle, Attribute, ImplItem, Item, Macro, Meta, visit::Visit};

// The paths are relative to this file's own directory, `tests/support/`.
#[path = "allow_scan/discovery.rs"]
mod discovery;
#[path = "allow_scan/inclusion.rs"]
mod inclusion;
#[path = "allow_scan/meta.rs"]
mod meta;
#[path = "allow_scan/roots.rs"]
pub mod roots;
#[path = "allow_scan/tokens.rs"]
mod tokens;

/// The extension a file must carry for the walk to collect it.
///
/// Shared with the `include!` rule in [`tokens`], which has to judge an
/// inclusion target by the same standard the walk selects sources by. Written
/// once so the two cannot drift: a target the walk would not collect is a file
/// the scan never reads, whatever the inclusion looks like.
pub(crate) const SOURCE_EXTENSION: &str = "rs";

pub use discovery::rust_sources;
use inclusion::foreign_inclusion;
use meta::{render_attribute, render_path, suppressed_by};
use roots::is_sanctioned;
use tokens::{attribute_at, could_cover_a_policy_call, forwarded_attribute_at, macro_arms};

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

/// One suppression found in a source, as it was written.
struct Suppression {
    /// Whether it was an inner attribute, for rendering it back.
    inner: bool,
    /// Its parsed contents.
    meta: Meta,
    /// The innermost named item enclosing the attribute, if any.
    ///
    /// An item-scoped `expect` is sanctioned only at a named composition root,
    /// so the judgement needs to know which item it sits on. A crate-root
    /// attribute has none.
    item: Option<String>,
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
    /// The named items enclosing whatever is being visited, outermost first.
    scope: Vec<String>,
    /// Each finding no `Meta` describes, already worded as a report line.
    structural: Vec<String>,
    /// The file being visited, relative to the repository root.
    ///
    /// `rustc` resolves an `include!` against the file that writes it, so the
    /// inclusion rule cannot judge a target without knowing where it was
    /// written.
    path: Utf8PathBuf,
}

/// Return the name an item declares, if it declares one.
///
/// Used to attribute an `expect` to the item it sits on, so the sanctioned
/// composition roots can be named. An `impl` block declares no name of its own;
/// its members are reached through [`impl_item_name`].
fn item_name(item: &Item) -> Option<String> {
    match item {
        Item::Const(inner) => Some(inner.ident.to_string()),
        Item::Enum(inner) => Some(inner.ident.to_string()),
        Item::Fn(inner) => Some(inner.sig.ident.to_string()),
        Item::Macro(inner) => inner.ident.as_ref().map(ToString::to_string),
        Item::Mod(inner) => Some(inner.ident.to_string()),
        Item::Static(inner) => Some(inner.ident.to_string()),
        Item::Struct(inner) => Some(inner.ident.to_string()),
        Item::Trait(inner) => Some(inner.ident.to_string()),
        Item::TraitAlias(inner) => Some(inner.ident.to_string()),
        Item::Type(inner) => Some(inner.ident.to_string()),
        Item::Union(inner) => Some(inner.ident.to_string()),
        _ => None,
    }
}

/// Return the name an `impl` member declares, if it declares one.
fn impl_item_name(item: &ImplItem) -> Option<String> {
    match item {
        ImplItem::Const(inner) => Some(inner.ident.to_string()),
        ImplItem::Fn(inner) => Some(inner.sig.ident.to_string()),
        ImplItem::Type(inner) => Some(inner.ident.to_string()),
        _ => None,
    }
}

impl AttributeCollector {
    /// Visit `walk` with `named` on the scope, if it names anything.
    ///
    /// Shared by the two visitors that introduce a name, which otherwise differ
    /// only in the `syn` function they call and would read as two copies of the
    /// same push, walk and pop.
    fn within(&mut self, named: Option<String>, walk: impl FnOnce(&mut Self)) {
        let pushed = named.is_some();
        if let Some(name) = named {
            self.scope.push(name);
        }
        walk(self);
        if pushed {
            self.scope.pop();
        }
    }

    /// Record any attribute-shaped token sequence in a transcriber.
    ///
    /// Every group is descended into, so a suppression nested through more than
    /// one macro is still found. `reachable` says whether the arm could put a
    /// forwarded attribute over a policy call, which decides the outer half of
    /// the forwarded-path rule.
    fn collect_from_tokens(&mut self, tokens: TokenStream, reachable: bool) {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        let mut index = 0;
        while index < trees.len() {
            if let Some(finding) = forwarded_attribute_at(&trees, index, reachable) {
                self.structural.push(finding);
                index += 1;
                continue;
            }
            if let Some((shape, after)) = attribute_at(&trees, index) {
                self.found.push(Suppression {
                    inner: shape.inner,
                    meta: shape.meta,
                    item: self.scope.last().cloned(),
                });
                index = after;
                continue;
            }
            if let TokenTree::Group(group) = &trees[index] {
                self.collect_from_tokens(group.stream(), reachable);
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
            item: self.scope.last().cloned(),
        });
    }

    fn visit_item(&mut self, item: &'ast Item) {
        self.within(item_name(item), |collector| {
            syn::visit::visit_item(collector, item);
        });
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        self.within(impl_item_name(item), |collector| {
            syn::visit::visit_impl_item(collector, item);
        });
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        match render_path(&mac.path).rsplit("::").next() {
            Some("macro_rules") => {
                for (pattern, transcriber) in macro_arms(mac.tokens.clone()) {
                    let reachable = could_cover_a_policy_call(&pattern, &transcriber);
                    self.collect_from_tokens(transcriber, reachable);
                }
            }
            Some("include") => {
                if let Some(finding) = foreign_inclusion(&mac.tokens, &self.path) {
                    self.structural.push(finding);
                }
            }
            _ => {}
        }
        syn::visit::visit_macro(self, mac);
    }
}

/// Return every protected lint suppressed in one source file, with the
/// attribute that suppressed it.
///
/// `path` is the file's path relative to the repository root. It is needed
/// because an item-scoped `expect` is sanctioned at a named composition root
/// and nowhere else, so the same attribute is an offence one file over. Inline
/// fixtures pass the path they are pretending to be.
pub fn suppressed_lints(path: &Utf8Path, contents: &str) -> Result<Vec<String>> {
    let parsed = syn::parse_file(contents).context("parse the source as Rust")?;
    let mut collector = AttributeCollector {
        path: path.to_owned(),
        ..AttributeCollector::default()
    };
    collector.visit_file(&parsed);

    let mut found = collector.structural.clone();
    for suppression in &collector.found {
        let sanctioned = is_sanctioned(path, suppression.item.as_deref());
        for lint in suppressed_by(&suppression.meta, suppression.inner, sanctioned) {
            if PROTECTED_LINTS.contains(&lint.as_str()) {
                found.push(describe(suppression, &lint));
            }
        }
    }
    Ok(found)
}

/// Word one finding as the report line a contributor reads.
///
/// An `expect` outside a sanctioned root gets its own wording, because the
/// remedy differs: an `allow` has to become an `expect`, whereas an `expect` in
/// the wrong place has to become a seam, or the site has to be argued into
/// [`roots::SANCTIONED_ROOTS`] alongside the ADR paragraph that justifies it.
fn describe(suppression: &Suppression, lint: &str) -> String {
    let rendered = render_attribute(suppression);
    if render_path(suppression.meta.path()) == "expect" && !suppression.inner {
        let item = suppression.item.as_deref().unwrap_or("the crate root");
        return format!(
            "expects {lint} at {item} via {rendered}, which is not a documented composition root"
        );
    }
    format!("allows {lint} via {rendered}")
}
