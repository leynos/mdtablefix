//! Reading attributes out of token streams, where `syn` sees none.
//!
//! A `macro_rules!` transcriber is an opaque token stream to `syn`, yet Clippy
//! expands it and honours whatever attribute it writes. Two further shapes are
//! not complete attributes where they are written at all, and are refused
//! structurally rather than by their `Meta`: an attribute whose path the caller
//! supplies, and an `include!` of a file the scan cannot read.

use camino::Utf8Path;
use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use syn::{LitStr, Meta};

use super::SOURCE_EXTENSION;

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
    trees.iter().enumerate().any(|(index, tree)| {
        matches!(tree, TokenTree::Punct(punct) if punct.as_char() == ':')
            && matches!(
                trees.get(index + 1),
                Some(TokenTree::Ident(ident))
                    if CODE_FRAGMENTS.contains(&ident.to_string().as_str())
            )
    })
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

/// Return a finding if an `include!` names a target that is not Rust source.
///
/// `rustc` parses an included file as Rust whatever its extension, so
/// `include!("fixture.rs.txt")` compiles that fixture's contents into this
/// crate. An `allow` written there suppresses the policy for the calls around
/// the inclusion, and an enclosing `expect` stays fulfilled, so nothing warns.
/// The scan cannot read the target, which need not exist when the scan runs, so
/// the inclusion itself is the finding.
///
/// A literal `.rs` path is not a finding: such a file is scanned in its own
/// right. `include_str!` and `include_bytes!` embed bytes rather than compiling
/// source and never reach here.
///
/// The target is parsed as one [`LitStr`] and judged by its *value*, not by how
/// it was written. `r"support.rs"` and `"support\x2Ers"` name the same file as
/// `"support.rs"`, and rendering the literal back to text would report two of
/// the three as targets the scan cannot see. Parsing the whole argument as a
/// single literal is also what keeps a computed target refused, including one
/// that holds a `.rs` literal somewhere inside.
///
/// The extension is compared the way the walk selects sources, against
/// [`SOURCE_EXTENSION`], rather than by a suffix test on the rendered path. A
/// suffix test is case-sensitive in a way the path reader is not, and it
/// accepts `include!(".rs")`, a bare extension that the walk never collects, so
/// the file it reaches would go unread.
pub(super) fn foreign_inclusion(tokens: &TokenStream) -> Option<String> {
    let Ok(target) = syn::parse2::<LitStr>(tokens.clone()) else {
        return Some(format!(
            "include!({tokens}) names a target the scan cannot resolve; name a literal `.rs` \
             path, which is scanned in its own right"
        ));
    };
    let path = target.value();
    if Utf8Path::new(&path).extension() == Some(SOURCE_EXTENSION) {
        return None;
    }
    Some(format!(
        "include!(\"{path}\") compiles a file the scan cannot see as Rust; name a `.rs` path, \
         which is scanned in its own right"
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
    Some((AttributeShape { inner, meta }, next + 1))
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
    forwarded_path(group, inner, reachable)
}
