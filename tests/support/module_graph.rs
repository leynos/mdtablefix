//! Which source files a crate root actually reaches.
//!
//! `tests/cli_config_dependency_contract.rs` states the contract; this module
//! resolves it. A textual search for `mod config;` cannot see
//! `#[path = "config.rs"] mod settings;`, and a library source that reaches a
//! binary-only file by either spelling links `ortho_config` into the library
//! just the same. So the walk follows declarations rather than matching text.
//!
//! Two bases are involved, and they differ. An implicit `mod name;` resolves
//! under the declaring file's *module directory*: `src/wrap.rs` owns
//! `src/wrap/`, and `src/wrap/tokenize/mod.rs` owns its own directory. A
//! `#[path = "..."]` resolves under the declaring file's *parent* directory, so
//! `src/select.rs` writes `#[path = "select/conflict_tests.rs"]` while
//! `src/wrap/inline/span_helpers.rs` writes `#[path = "span_helper_props.rs"]`.
//! An inline module appends its own name to both, which is the Rust Reference's
//! rule that inline module components become directories.
//!
//! An `include!` is followed too, against the declaring file's directory.
//! `rustc` compiles the target wherever it lives, so a library source that
//! included a binary-only file would reach it exactly as a `mod` would.
//!
//! Nothing is skipped silently. A declaration that names no existing file, and
//! a construct the walker cannot read, are both errors: a reader that quietly
//! dropped an edge would satisfy the contract by never reaching the file that
//! breaks it.

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use proc_macro2::TokenStream;
use syn::{Expr, ExprLit, Item, Lit, LitStr, Meta, visit::Visit};

/// The files one declaration could name, most specific first.
///
/// More than one candidate arises only for an implicit module, where `name.rs`
/// and `name/mod.rs` are both legal and Cargo rejects a tree holding both.
/// Listing them in precedence order lets the walk take the one that exists
/// without pretending the other was never a candidate.
pub type Candidates = Vec<Utf8PathBuf>;

/// The two directories a declaration in one scope resolves against.
///
/// They start from different directories and are extended identically by an
/// inline module's name, so they are carried together rather than as a pair of
/// arguments at every call.
#[derive(Debug, Clone, Default)]
struct Bases {
    /// Where an unadorned `mod name;` looks.
    implicit: Utf8PathBuf,
    /// Where `#[path]` and `include!` look.
    explicit: Utf8PathBuf,
}

impl Bases {
    /// The bases a whole file's declarations resolve against.
    fn of_file(declaring: &Utf8Path) -> Self {
        let parent = declaring.parent().unwrap_or_else(|| Utf8Path::new(""));
        Self {
            implicit: module_directory(declaring),
            explicit: parent.to_owned(),
        }
    }

    /// The bases inside an inline module called `name`.
    fn within(&self, name: &str) -> Self {
        Self {
            implicit: self.implicit.join(name),
            explicit: self.explicit.join(name),
        }
    }
}

/// The candidate files each declaration in `source` names.
///
/// `declaring` is the file holding the declarations. Split from [`reachable`]
/// so the two resolution rules can be exercised against an inline fixture,
/// without building a directory tree for every spelling worth stating.
pub fn declared_targets(source: &str, declaring: &Utf8Path) -> Result<Vec<Candidates>> {
    let parsed = syn::parse_file(source).with_context(|| format!("parsing {declaring}"))?;
    let mut collector = Declarations::of_file(declaring);
    collector.visit_file(&parsed);
    if let Some(problem) = collector.problem {
        bail!("{declaring}: {problem}");
    }

    Ok(collector.targets)
}

/// Every source reachable from `root`, `root` included, sorted.
///
/// Breadth-first over files rather than a descent over modules, so a file named
/// twice is read once. A declaration whose candidates all miss is fatal: the
/// rules above are the contract's only defence against an unread edge, so one
/// the tree contradicts must surface as a failure rather than as a file quietly
/// absent from the result.
pub fn reachable(root: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let mut pending = vec![root.to_owned()];
    let mut found: Vec<Utf8PathBuf> = Vec::new();

    while let Some(file) = pending.pop() {
        if found.contains(&file) {
            continue;
        }
        let source = std::fs::read_to_string(&file).with_context(|| format!("reading {file}"))?;
        for candidates in declared_targets(&source, &file)? {
            let Some(target) = candidates.iter().find(|path| path.exists()) else {
                bail!(
                    "{file} declares a module at {}, which does not exist",
                    candidates
                        .iter()
                        .map(Utf8PathBuf::as_path)
                        .map(Utf8Path::as_str)
                        .collect::<Vec<_>>()
                        .join(" or ")
                );
            };
            pending.push(target.clone());
        }
        found.push(file);
    }
    found.sort();

    Ok(found)
}

/// The directory a declaring file's implicit `mod name;` declarations use.
///
/// `src/wrap.rs` and `src/wrap/mod.rs` both own `src/wrap/`, and the crate
/// roots `lib.rs` and `main.rs` own their own directory, so the file name alone
/// decides: a file owns a directory of its own stem unless it is a `mod.rs` or
/// a crate root.
fn module_directory(declaring: &Utf8Path) -> Utf8PathBuf {
    let parent = declaring.parent().unwrap_or_else(|| Utf8Path::new(""));
    let name = declaring.file_name().unwrap_or_default();
    if matches!(name, "mod.rs" | "lib.rs" | "main.rs") {
        return parent.to_owned();
    }

    parent.join(declaring.file_stem().unwrap_or_default())
}

/// Collects the file each declaration in one file names.
///
/// A visitor rather than a walk over top-level items, because an inline module
/// nests declarations and the bases have to follow. The first construct the
/// collector cannot read is kept in `problem` and reported once: a `Visit`
/// method returns nothing, so it cannot fail in place.
#[derive(Default)]
struct Declarations {
    /// The files found so far.
    targets: Vec<Candidates>,
    /// The bases of the scope currently being visited.
    bases: Bases,
    /// The first unreadable construct, if any.
    problem: Option<String>,
}

impl Declarations {
    /// A collector that starts from `declaring`.
    fn of_file(declaring: &Utf8Path) -> Self {
        Self {
            bases: Bases::of_file(declaring),
            ..Self::default()
        }
    }

    /// Record `problem` unless one is already recorded.
    ///
    /// The first is kept rather than the last so the report names the construct
    /// nearest the top of the file, which is the one a reader meets first.
    fn fail(&mut self, problem: impl Into<String>) {
        if self.problem.is_none() {
            self.problem = Some(problem.into());
        }
    }

    /// Visit `items` in the current scope.
    fn visit_items(&mut self, items: &[Item]) {
        for item in items {
            self.visit_item(item);
        }
    }
}

impl<'ast> Visit<'ast> for Declarations {
    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        let name = module.ident.to_string();
        let explicit = match path_attribute(module) {
            Ok(explicit) => explicit,
            Err(error) => return self.fail(error.to_string()),
        };

        if let Some(written) = explicit {
            let target = self.bases.explicit.join(written);
            self.targets.push(vec![target.clone()]);
            if let Some((_, body)) = &module.content {
                // A `#[path]` on an inline module rebases its subtree to the
                // named file, so the nested declarations resolve from there.
                // Restoring afterwards keeps a sibling declaration in the outer
                // scope resolving where it did.
                let outer = std::mem::take(&mut self.bases);
                self.bases = Bases::of_file(&target);
                self.visit_items(body);
                self.bases = outer;
            }
            return;
        }

        let Some((_, body)) = &module.content else {
            let directory = self.bases.implicit.join(&name);
            let sibling = self.bases.implicit.join(format!("{name}.rs"));
            self.targets.push(vec![sibling, directory.join("mod.rs")]);
            return;
        };

        let outer = self.bases.clone();
        self.bases = outer.within(&name);
        self.visit_items(body);
        self.bases = outer;
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac.path.is_ident("include") {
            match inclusion_target(&mac.tokens) {
                Ok(target) => {
                    let resolved = self.bases.explicit.join(target);
                    self.targets.push(vec![resolved]);
                }
                Err(problem) => self.fail(problem.to_string()),
            }
        }
        syn::visit::visit_macro(self, mac);
    }
}

/// The literal path an `include!` names.
///
/// Only the literal form is accepted, for the same reason a `#[path]` must be
/// literal: a computed target is unreachable to the walker, and guessing would
/// defeat the purpose of following declarations rather than text.
fn inclusion_target(tokens: &TokenStream) -> Result<String> {
    let literal = syn::parse2::<LitStr>(tokens.clone())
        .with_context(|| format!("`include!({tokens})` is not a literal path"))?;

    Ok(literal.value())
}

/// The `#[path = "..."]` value on `module`, if it carries one.
fn path_attribute(module: &syn::ItemMod) -> Result<Option<String>> {
    for attribute in &module.attrs {
        if !attribute.path().is_ident("path") {
            continue;
        }
        let Meta::NameValue(named) = &attribute.meta else {
            bail!(
                "a `#[path]` on `{}` is not a name-value attribute",
                module.ident
            );
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(written),
            ..
        }) = &named.value
        else {
            bail!("a `#[path]` on `{}` is not a string literal", module.ident);
        };
        return Ok(Some(written.value()));
    }

    Ok(None)
}
