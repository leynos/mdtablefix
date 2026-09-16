//! Judging an `include!` target, which the scan has to read as a second source.
//!
//! `rustc` parses an included file as Rust whatever its extension, and
//! resolves the target against the file that writes it. Neither fact is
//! visible to the attribute walk: an `allow` written in the included file is a
//! complete, ordinary attribute, in a file the scan may never open. The
//! inclusion itself is therefore the finding, judged here rather than beside
//! the token readers, because what it needs is the walk's own rule for which
//! paths are reachable, not a reading of tokens.

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use proc_macro2::TokenStream;
use syn::LitStr;

use super::{SOURCE_EXTENSION, discovery::is_walkable};

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
///
/// A `.rs` extension is not enough on its own. `rustc` resolves an inclusion
/// relative to the file that writes it, and the walk skips `target` and every
/// dot-prefixed directory, so `include!(".generated/bypass.rs")` names a real
/// Rust file that the scan never reads. The target is therefore resolved
/// against `including` and each directory it passes through is judged by
/// [`is_walkable`], the same function the walk decides with, so the two cannot
/// drift. A target that climbs out of the tree the walk was given is refused
/// for the same reason: nothing rooted there reaches it.
///
/// Only the directories are judged. The walk collects a file by its extension
/// alone, so a dot-prefixed *file* name such as `.hidden.rs` is read like any
/// other and is not a finding.
pub(super) fn foreign_inclusion(tokens: &TokenStream, including: &Utf8Path) -> Option<String> {
    let Ok(target) = syn::parse2::<LitStr>(tokens.clone()) else {
        return Some(format!(
            "include!({tokens}) names a target the scan cannot resolve; name a literal `.rs` \
             path, which is scanned in its own right"
        ));
    };
    let path = target.value();
    if Utf8Path::new(&path).extension() != Some(SOURCE_EXTENSION) {
        return Some(format!(
            "include!(\"{path}\") compiles a file the scan cannot see as Rust; name a `.rs` path, \
             which is scanned in its own right"
        ));
    }
    let Some(resolved) = resolve_against(including, Utf8Path::new(&path)) else {
        return Some(format!(
            "include!(\"{path}\") from {including} resolves outside the tree the scan walks; name \
             a `.rs` path inside it, which is scanned in its own right"
        ));
    };
    let skipped = resolved
        .components()
        .rev()
        .skip(1)
        .find_map(|component| match component {
            Utf8Component::Normal(name) if !is_walkable(name) => Some(name.to_owned()),
            _ => None,
        })?;
    Some(format!(
        "include!(\"{path}\") from {including} resolves to {resolved}, under `{skipped}`, which \
         the walk skips; name a `.rs` path the walk collects, which is scanned in its own right"
    ))
}

/// Resolve `target` against the directory holding `including`, lexically.
///
/// Lexical rather than filesystem resolution, because the scan has to judge a
/// target that need not exist yet, and because a walk that followed links or
/// canonicalised paths could be led outside the tree it was handed.
///
/// `None` means the target climbs above the walk's root, or names an absolute
/// path: either way no walk rooted there reaches it.
fn resolve_against(including: &Utf8Path, target: &Utf8Path) -> Option<Utf8PathBuf> {
    let base = including.parent().unwrap_or_else(|| Utf8Path::new(""));
    let mut parts: Vec<&str> = Vec::new();
    for component in base.components().chain(target.components()) {
        match component {
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                parts.pop()?;
            }
            Utf8Component::Normal(name) => parts.push(name),
            Utf8Component::RootDir | Utf8Component::Prefix(_) => return None,
        }
    }
    Some(parts.iter().collect())
}
