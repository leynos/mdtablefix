//! The layered-configuration schema and the constants that describe it.
//!
//! Split from `src/config.rs` so the resolution logic and the schema it folds
//! over stay separately readable, and so neither file grows past the line
//! limit. Nothing here decides anything: the type is a serde target, and the
//! constants are the spellings the framework and the tests share.
//!
//! Declared only from `src/main.rs`'s `config` module, so it belongs to the
//! binary: `src/lib.rs` never names it, and the library gains no reference to
//! the framework.

/// The application name discovery derives its file names from.
///
/// The framework stems the dotfile from this, so it yields `.mdtablefix.toml`
/// for the project file and `.mdtablefix.toml` for the user dotfile.
pub(crate) const APP_NAME: &str = "mdtablefix";

/// The environment variable naming an explicit configuration file.
///
/// Its presence is fatal when the path it names cannot be read, which is the
/// documented way to make a broken file visible: an automatic candidate that
/// fails only reports when nothing else loaded, unless the caller promotes it.
pub(crate) const SELECTOR_VARIABLE: &str = "MDTABLEFIX_CONFIG_PATH";

/// The variable prefix, spelled the way the provider expects it.
///
/// `CsvEnv` takes the prefix literally rather than deriving it, so the
/// trailing underscore is part of the name here even though the `prefix`
/// attribute spells the bare `MDTABLEFIX`.
pub(crate) const ENV_PREFIX: &str = "MDTABLEFIX_";

/// The `__` separator the provider nests keys with.
///
/// Nothing nests today, because the schema is flat, but declaring the dialect
/// keeps this layer identical to the one the derive would generate.
pub(crate) const ENV_SEPARATOR: &str = "__";

/// The clap identifiers of the arguments configuration is allowed to supply.
///
/// Spelled as clap records them — the field names, not the long flags — because
/// [`clap::ArgMatches::value_source`] is keyed by identifier. The list is the
/// set [`ConfigFile`] mirrors; an argument outside it is an invocation rather
/// than a setting, and never reaches the command-line layer.
pub(crate) const CONFIGURED_ARGUMENTS: [&str; 12] = [
    "wrap",
    "renumber",
    "breaks",
    "ellipsis",
    "fences",
    "footnotes",
    "code_emphasis",
    "headings",
    "md_exts",
    "git",
    "include_untracked",
    "allow_conflicted",
];

/// The configuration schema, as the derive and serde see it.
///
/// Every field is optional because absence is meaningful: it says the layer
/// had no opinion, which lets a lower layer's value survive. The generated
/// `merge_from_layers` folds the composed layers left to right, so a later
/// layer's `Some` replaces an earlier one's whole value — including for
/// `md_exts`, where replacement rather than accumulation is what a user who
/// names an extension list means.
///
/// The type is never constructed from configuration directly; it is built once
/// for the command-line layer, and otherwise only deserialized.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize, ortho_config::OrthoConfig)]
#[ortho_config(prefix = "MDTABLEFIX")]
#[ortho_config(discovery(app_name = "mdtablefix"))]
pub struct ConfigFile {
    /// Whether paragraphs and list items are wrapped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    /// Whether ordered list items are renumbered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renumber: Option<bool>,
    /// Whether thematic breaks are rewritten as underscores.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breaks: Option<bool>,
    /// Whether `...` becomes the ellipsis character.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ellipsis: Option<bool>,
    /// Whether fence delimiters are normalized to three backticks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fences: Option<bool>,
    /// Whether bare numeric references become footnote links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnotes: Option<bool>,
    /// Whether emphasis markers beside inline code are repaired.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_emphasis: Option<bool>,
    /// Whether Setext headings become hash-prefixed headings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headings: Option<bool>,
    /// The extensions a Git selection keeps.
    ///
    /// Carried as strings rather than as an extension filter because the file
    /// and environment layers supply raw text, which the resolver validates
    /// with the same parser clap uses for `--md-exts`. A value the parser
    /// rejects is an error rather than a silently unmatchable extension.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md_exts: Option<Vec<String>>,
    /// Whether the input is the files Git tracks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<bool>,
    /// Whether untracked, unignored files are selected too.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_untracked: Option<bool>,
    /// Whether files carrying conflict markers may be rewritten.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_conflicted: Option<bool>,
}
