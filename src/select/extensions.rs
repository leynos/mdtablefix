//! Parsing and matching of `--md-exts` values.
//!
//! Depends on `camino` and `thiserror` alone: no filesystem, no subprocess, no
//! capability. The type is named for what it does rather than for Markdown,
//! because `--md-exts` accepts any extension.

use std::collections::BTreeSet;

use camino::Utf8Path;

/// The set `--md-exts` means when the user does not give one.
const DEFAULT_EXTENSIONS: [&str; 3] = ["md", "mdc", "markdown"];

/// A case-insensitive set of file extensions, stored without dots.
///
/// Named for what it does rather than for Markdown: `--md-exts` accepts any
/// extension, so a `MarkdownExtensions` type would be a false promise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionFilter(BTreeSet<String>);

impl Default for ExtensionFilter {
    /// Returns `md`, `mdc`, and `markdown`.
    fn default() -> Self { DEFAULT_EXTENSIONS.into_iter().map(str::to_owned).collect() }
}

impl ExtensionFilter {
    /// Reports whether `path` ends in one of these extensions.
    ///
    /// Only the last extension is consulted, and case is folded, so
    /// `docs/guide.MD` matches `md`. A leading dot with no second dot is part
    /// of the file name rather than an extension — [`camino::Utf8Path`]'s own
    /// reading, and the one that keeps `.md` from matching every dotfile.
    #[must_use]
    pub fn matches(&self, path: &Utf8Path) -> bool {
        path.extension()
            .is_some_and(|extension| self.0.contains(&extension.to_ascii_lowercase()))
    }

    /// Iterates the extensions shortest-first, then byte-wise.
    ///
    /// Shortest-first is what renders the default set as `md, mdc, markdown`;
    /// a byte-wise sort alone would render the same set as
    /// `markdown, md, mdc`. Either way the order is a total one independent of
    /// how the user spelled the flag, so a `--help` rendering or a diagnostic
    /// does not change with the order of the same values.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        let mut extensions: Vec<&str> = self.0.iter().map(String::as_str).collect();
        extensions.sort_unstable_by_key(|extension| (extension.len(), *extension));
        extensions.into_iter()
    }
}

/// Builds a filter from values [`parse_extension`] has already accepted.
///
/// The values are folded again, so collecting `["md", "MD"]` yields one
/// extension, and a caller need not keep a `Vec` on the way to a set.
impl FromIterator<String> for ExtensionFilter {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        Self(
            iter.into_iter()
                .map(|value| value.to_ascii_lowercase())
                .collect(),
        )
    }
}

/// Renders as `md, mdc, markdown`, for `--help` and diagnostics.
impl std::fmt::Display for ExtensionFilter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, extension) in self.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            formatter.write_str(extension)?;
        }

        Ok(())
    }
}

/// Parses one extension, for use as a clap `value_parser`.
///
/// Strips one optional leading dot, trims surrounding whitespace, and folds
/// ASCII case. A trailing dot is kept: `mdc.` is a suffix a repository may
/// genuinely use, and rejecting it would be this parser inventing a rule Git
/// does not have.
///
/// # Errors
///
/// Returns [`ExtensionSpecError`] for an empty or dot-only value, or one
/// containing a path separator or a NUL byte.
pub fn parse_extension(value: &str) -> Result<String, ExtensionSpecError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ExtensionSpecError::Empty);
    }
    if trimmed == "." {
        return Err(ExtensionSpecError::DotOnly {
            value: value.to_owned(),
        });
    }

    let without_dot = trimmed.strip_prefix('.').unwrap_or(trimmed);
    if let Some(kind) = invalid_character(without_dot) {
        return Err(ExtensionSpecError::InvalidCharacter {
            value: value.to_owned(),
            kind,
        });
    }

    Ok(without_dot.to_ascii_lowercase())
}

/// The first reason `value` cannot be an extension, if any.
///
/// A path separator is reported ahead of a NUL byte for `md/\0`, where either
/// would do: the separator is the one a user is more likely to have meant.
fn invalid_character(value: &str) -> Option<InvalidCharacterKind> {
    let separator = value
        .find(['/', '\\'])
        .map(|_| InvalidCharacterKind::PathSeparator);
    let nul = value.find('\0').map(|_| InvalidCharacterKind::Nul);

    separator.or(nul)
}

/// The reason an extension value was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ExtensionSpecError {
    #[error("extension is empty")]
    Empty,
    #[error("extension {value:?} is only a dot")]
    DotOnly { value: String },
    #[error("extension {value:?} contains {kind}")]
    InvalidCharacter {
        value: String,
        kind: InvalidCharacterKind,
    },
}

/// Why a value is not usable as an extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InvalidCharacterKind {
    /// A `/` or `\`, which would make one extension name two path components.
    #[error("a path separator")]
    PathSeparator,
    /// A NUL byte, which cannot survive an `OsStr` round trip on all platforms.
    #[error("a NUL byte")]
    Nul,
}

#[cfg(test)]
#[path = "extensions_tests.rs"]
mod tests;
