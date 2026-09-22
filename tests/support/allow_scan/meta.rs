//! Judging one parsed attribute.
//!
//! This is the half of the scan that reasons about a `Meta`: which protected
//! lints an attribute suppresses, and how it reads back in a failure message.
//! The other half, in `tokens`, reaches attributes no `Meta` describes.

use syn::{Meta, MetaList, Path, Token, ext::IdentExt, punctuated::Punctuated};

use super::Suppression;

/// Render a lint path with raw identifiers normalized.
///
/// `r#allow` and `allow` are the same identifier to the compiler, as are
/// `clippy::r#style` and `clippy::style`, and Clippy honours the raw spelling.
/// Comparing the written form would let either escape the contract, so every
/// segment is unrawed before it is joined.
pub(super) fn render_path(path: &Path) -> String {
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
fn suppressed_by_cfg_attr(list: &MetaList, inner: bool, sanctioned: bool) -> Vec<String> {
    let Ok(nested) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return Vec::new();
    };
    nested
        .iter()
        .skip(1)
        .flat_map(|meta| suppressed_by(meta, inner, sanctioned))
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
pub(super) fn suppressed_by(meta: &Meta, inner: bool, sanctioned: bool) -> Vec<String> {
    let Ok(list) = meta.require_list() else {
        return Vec::new();
    };
    match render_path(meta.path()).as_str() {
        "allow" => allowed_lints(list),
        "expect" if inner || !sanctioned => allowed_lints(list),
        "cfg_attr" => suppressed_by_cfg_attr(list, inner, sanctioned),
        _ => Vec::new(),
    }
}

/// Render a suppression roughly as written, for a failure message.
pub(super) fn render_attribute(suppression: &Suppression) -> String {
    let bang = if suppression.inner { "!" } else { "" };
    let path = render_path(suppression.meta.path());
    suppression.meta.require_list().map_or_else(
        |_| format!("#{bang}[{path}]"),
        |list| format!("#{bang}[{path}({})]", list.tokens),
    )
}
