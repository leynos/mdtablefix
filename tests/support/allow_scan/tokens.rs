//! Reading attributes out of token streams, where `syn` sees none.
//!
//! A `macro_rules!` transcriber is an opaque token stream to `syn`, yet Clippy
//! expands it and honours whatever attribute it writes. One further shape is
//! not a complete attribute where it is written at all, and is refused
//! structurally rather than by its `Meta`: an attribute whose path the caller
//! supplies. The other structural refusal, an `include!` of a file the scan
//! cannot read, is in `inclusion` beside this module, because it is judged by
//! the walk's rule for reachable paths rather than by reading tokens.

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use syn::Meta;

/// Fragment specifiers whose value can carry an environment access.
///
/// A caller supplying one of these supplies code, so an attribute forwarded
/// over it can cover a call the arm never mentions. An `ident`, a `ty`, a
/// `lifetime` or a `literal` cannot carry a call, which is what keeps the
/// doc-forwarding idiom out of the findings.
const CODE_FRAGMENTS: [&str; 5] = ["item", "block", "stmt", "expr", "tt"];

/// Return each arm of a `macro_rules!` body as its pattern and transcriber.
///
/// Only a transcriber is expanded, so only a transcriber can suppress
/// anything. An arm's pattern is not output, and the arguments of an ordinary
/// macro invocation may be discarded by the macro they are handed to: walking
/// either reports an attribute that never reaches the compiler, and a contract
/// that reports a false positive gets switched off.
///
/// The pattern comes back with the transcriber because the fragment specifiers
/// declared there decide whether a forwarded attribute could bear on the
/// policy. An arm is `(pattern) => {transcriber};`.
pub(super) fn macro_arms(tokens: TokenStream) -> Vec<(TokenStream, TokenStream)> {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut arms = Vec::new();
    for (index, tree) in trees.iter().enumerate() {
        if !matches!(tree, TokenTree::Punct(punct) if punct.as_char() == '=') {
            continue;
        }
        if !matches!(trees.get(index + 1), Some(TokenTree::Punct(punct)) if punct.as_char() == '>')
        {
            continue;
        }
        let Some(TokenTree::Group(transcriber)) = trees.get(index + 2) else {
            continue;
        };
        let pattern = match index.checked_sub(1).and_then(|before| trees.get(before)) {
            Some(TokenTree::Group(group)) => group.stream(),
            _ => TokenStream::new(),
        };
        arms.push((pattern, transcriber.stream()));
    }
    arms
}

/// Return whether `tokens` name the `env` module at any depth.
///
/// A call an arm writes sits inside the item's block, one group down, so the
/// search recurses.
fn mentions_env(tokens: &TokenStream) -> bool {
    tokens.clone().into_iter().any(|tree| match tree {
        TokenTree::Ident(ident) => ident == "env",
        TokenTree::Group(group) => mentions_env(&group.stream()),
        TokenTree::Punct(_) | TokenTree::Literal(_) => false,
    })
}

/// Return whether an arm could put a forwarded attribute over a policy call.
///
/// Either the arm writes the access itself, so `env` appears among its tokens,
/// or it forwards a fragment the caller fills with code.
/// `$(#[$meta:meta])* $name:ident, $field:ident, $ty:ty` does neither: it is
/// the ordinary way to carry doc comments onto a generated setter, and
/// reporting it would be the false positive that gets a contract switched off.
pub(super) fn could_cover_a_policy_call(pattern: &TokenStream, transcriber: &TokenStream) -> bool {
    if mentions_env(transcriber) {
        return true;
    }
    let trees: Vec<TokenTree> = pattern.clone().into_iter().collect();
    (0..trees.len()).any(|index| declares_code_fragment(&trees, index))
}

/// Return whether a fragment specifier at `index` names something that can
/// carry code.
///
/// A specifier is written `$name:kind`, so the colon is what marks one and the
/// identifier after it is the kind. An `ident`, a `ty`, a `lifetime` or a
/// `literal` cannot carry a call; the kinds that can are in [`CODE_FRAGMENTS`].
fn declares_code_fragment(trees: &[TokenTree], index: usize) -> bool {
    if !matches!(trees.get(index), Some(TokenTree::Punct(punct)) if punct.as_char() == ':') {
        return false;
    }
    matches!(
        trees.get(index + 1),
        Some(TokenTree::Ident(ident)) if CODE_FRAGMENTS.contains(&ident.to_string().as_str())
    )
}

/// Return a finding if an attribute in a transcriber forwards its own path.
///
/// `#[$attr]` is written by the arm and completed by the caller, so the arm
/// cannot be read for what it applies and the invocation carries no `#` for a
/// walk to notice. Neither half is a suppression alone, and together they are:
/// invoked as `forward!(allow(clippy::disallowed_methods), ...)`, the expansion
/// silences every call the item contains.
///
/// Two things keep the rule narrow. Only a forwarded *path* is refused, so
/// `#[doc = $text]` and `#[derive($traits)]` report nothing however much of
/// their argument is forwarded. And an outer forwarded path is refused only
/// where the arm could put it over a policy call. An inner one is refused
/// wherever it appears: `#![$attr]` applies to everything enclosing it, so
/// there is no call it could fail to cover.
fn forwarded_path(group: &Group, inner: bool, reachable: bool) -> Option<String> {
    let stream = group.stream();
    let first = stream.clone().into_iter().next()?;
    if !matches!(&first, TokenTree::Punct(punct) if punct.as_char() == '$') {
        return None;
    }
    if !inner && !reachable {
        return None;
    }
    let bang = if inner { "!" } else { "" };
    Some(format!(
        "#{bang}[{stream}] forwards its own path, which the caller can complete with `allow`; \
         write the attribute out, or take the item rather than the attribute"
    ))
}

/// An attribute recovered from a token stream, before it is attributed to an
/// item.
///
/// The token walk cannot see the enclosing item; the visitor supplies that when
/// the shape becomes a [`Suppression`].
pub(super) struct AttributeShape {
    pub(super) inner: bool,
    pub(super) meta: Meta,
}

/// Return the attribute beginning at `index`, with the index just past it.
///
/// An attribute is `#`, optionally `!`, then a bracketed group. The group's
/// contents are parsed as a `Meta`, so a recovered attribute is judged by the
/// same function as a real one rather than by a second, weaker test. `None`
/// covers everything else, including a group that does not parse: a macro body
/// may hold token sequences that are not Rust until they are expanded.
pub(super) fn attribute_at(trees: &[TokenTree], index: usize) -> Option<(AttributeShape, usize)> {
    let (inner, group, after) = attribute_shape_at(trees, index)?;
    let meta = syn::parse2::<Meta>(group.stream()).ok()?;
    Some((AttributeShape { inner, meta }, after))
}

/// Return the scope, bracketed group and end of the attribute at `index`.
///
/// An attribute is `#`, optionally `!`, then a bracketed group. Both readers
/// below need exactly that much of it and differ only in what they do with the
/// group: one parses it as a `Meta`, the other judges it as tokens, because
/// `$attr` is not a `Meta` and that is why the route existed. Reading the shape
/// once keeps them from drifting into two spellings of the same rule.
fn attribute_shape_at(trees: &[TokenTree], index: usize) -> Option<(bool, &Group, usize)> {
    let TokenTree::Punct(hash) = trees.get(index)? else {
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
    let TokenTree::Group(group) = trees.get(next)? else {
        return None;
    };
    if group.delimiter() != Delimiter::Bracket {
        return None;
    }
    Some((inner, group, next + 1))
}

/// Return a forwarded-path finding for the attribute beginning at `index`.
///
/// Shares [`attribute_at`]'s reading of what an attribute looks like, and
/// differs only in judging the group rather than parsing it: `$attr` is not a
/// `Meta`, which is exactly why the route existed.
pub(super) fn forwarded_attribute_at(
    trees: &[TokenTree],
    index: usize,
    reachable: bool,
) -> Option<String> {
    let (inner, group, _) = attribute_shape_at(trees, index)?;
    forwarded_path(group, inner, reachable)
}
