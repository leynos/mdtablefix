//! Layered configuration for the binary's settings.
//!
//! The command line states a rule set per invocation; this module lets a file
//! or an environment variable state it once. Sources are ordered — compiled-in
//! defaults, discovered files, the `MDTABLEFIX_` environment, then the command
//! line — and merged over [`ConfigFile`], a schema whose every field is
//! optional. The absent case is therefore all-`None`, which leaves the parsed
//! command line to decide every setting and makes a run without configuration
//! behave exactly as it did before this module existed. See
//! `docs/adrs/0014-configuration-layering.md`.
//!
//! Two things are deliberately absent. Nothing here prints or exits: a failure
//! is a [`ConfigError`] for the composition root to report, because only
//! `src/main.rs` and `Cli::parse_validated` touch the process boundary. And
//! nothing here reads the process environment directly: discovery takes a
//! [`SharedEnvSource`] and the merge layer a [`SharedScanEnvSource`], which
//! production supplies from `ortho_config` and tests supply from one `MapEnv`.
//! See `docs/adrs/0012-environment-seam-taxonomy.md`.
//!
//! Declared only from `src/main.rs`, so it belongs to the binary: `src/lib.rs`
//! never names it, and the library gains no reference to the framework.

use std::sync::Arc;

use clap::{ArgMatches, parser::ValueSource};
use ortho_config::{
    AutomaticMode,
    ConfigDiscovery,
    ConfigFilePolicy,
    ConfigPathSelector,
    CsvEnv,
    DiscoveryScope,
    ExplicitMode,
    MergeComposer,
    OrthoError,
    OrthoMergeExt,
    SharedEnvSource,
    SharedScanEnvSource,
    declarative::LayerComposition,
    figment::Figment,
    process_env_source,
    process_scan_env_source,
    sanitize_value,
};

use crate::{
    command::{Cli, FormatOpts},
    select::extensions::{ExtensionFilter, parse_extension},
};

/// The application name discovery derives its file names from.
///
/// The framework stems the dotfile from this, so it yields `.mdtablefix.toml`
/// for the project file and `.mdtablefix.toml` for the user dotfile.
const APP_NAME: &str = "mdtablefix";

/// The environment variable naming an explicit configuration file.
///
/// Its presence is fatal when the path it names cannot be read, which is the
/// documented way to make a broken file visible: an automatic candidate that
/// fails only reports when nothing else loaded.
const SELECTOR_VARIABLE: &str = "MDTABLEFIX_CONFIG_PATH";

/// The variable prefix, spelled the way the provider expects it.
///
/// `CsvEnv` takes the prefix literally rather than deriving it, so the
/// trailing underscore is part of the name here even though the `prefix`
/// attribute spells the bare `MDTABLEFIX`.
const ENV_PREFIX: &str = "MDTABLEFIX_";

/// The `__` separator the provider nests keys with.
///
/// Nothing nests today, because the schema is flat, but declaring the dialect
/// keeps this layer identical to the one the derive would generate.
const ENV_SEPARATOR: &str = "__";

/// The clap identifiers of the arguments configuration is allowed to supply.
///
/// Spelled as clap records them — the field names, not the long flags — because
/// [`ArgMatches::value_source`] is keyed by identifier. The list is the set
/// [`ConfigFile`] mirrors; an argument outside it is an invocation rather than
/// a setting, and never reaches the command-line layer.
const CONFIGURED_ARGUMENTS: [&str; 12] = [
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
/// for the command-line layer in [`command_line_layer`], and otherwise only
/// deserialised.
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
    /// Whether fence delimiters are normalised to three backticks.
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
    /// Carried as strings rather than as [`ExtensionFilter`] because the file
    /// and environment layers supply raw text, which
    /// [`Settings::resolve`] validates with the same parser clap uses for
    /// `--md-exts`. A value the parser rejects is an error rather than a
    /// silently unmatchable extension.
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

/// The settings the driver consumes, after every layer is merged.
///
/// A plain struct with explicit fields: no configuration type appears in its
/// signature, so the code that formats files cannot tell whether a setting came
/// from a file, an environment variable, or the command line — which is the
/// point of merging them before the decision is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The resolved formatting rule set.
    pub opts: FormatOpts,
    /// The resolved extension list for `--git` selection.
    pub md_exts: ExtensionFilter,
    /// Whether `--git` selection was requested.
    pub git: bool,
    /// Whether untracked files should be included.
    pub include_untracked: bool,
    /// Whether conflicted files should be rewritten.
    pub allow_conflicted: bool,
    /// The mode the command line selected.
    pub mode: crate::driver::Mode,
    /// The positional file arguments.
    pub files: Vec<std::path::PathBuf>,
}

impl Settings {
    /// Resolves every layer against the process environment.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] when a selected file cannot be read or parsed,
    /// when the environment layer cannot be merged, or when a configuration
    /// source supplies a value the tool cannot use. Absent optional candidates
    /// are not failures: only a selected file or an empty search reports one.
    pub fn resolve(cli: &Cli, matches: &ArgMatches) -> Result<Self, ConfigError> {
        Self::resolve_with_sources(
            cli,
            matches,
            process_env_source(),
            process_scan_env_source(),
        )
    }

    /// Resolves every layer against the supplied environment sources.
    ///
    /// The two sources are separate on purpose: discovery only looks names up,
    /// while the merge layer enumerates a prefix. A test passes one `MapEnv`
    /// twice — as an [`Arc<dyn EnvSource>`](ortho_config::EnvSource) and as an
    /// [`Arc<dyn ScanEnvSource>`] — so a single fixed map drives both halves
    /// and nothing touches the process environment.
    ///
    /// # Errors
    ///
    /// As [`Settings::resolve`].
    pub fn resolve_with_sources(
        cli: &Cli,
        matches: &ArgMatches,
        lookup: SharedEnvSource,
        scan: SharedScanEnvSource,
    ) -> Result<Self, ConfigError> {
        let config = merged_config(cli, matches, lookup, scan)?;

        Ok(Self {
            opts: formatting_options(&config, cli),
            md_exts: extension_filter(&config, cli)?,
            // A typed path always means a file run: a positional argument is the
            // user's own statement of what to act on, so it outranks a
            // configured `git` rather than being silently overridden by it.
            git: config.git.unwrap_or(cli.selects_from_git()) && cli.files.is_empty(),
            include_untracked: config.include_untracked.unwrap_or(cli.includes_untracked()),
            allow_conflicted: config.allow_conflicted.unwrap_or(cli.allows_conflicted()),
            mode: cli.mode(),
            files: cli.files.clone(),
        })
    }
}

/// Everything that can go wrong while resolving configuration.
///
/// Two variants, both with a producer. The framework's own vocabulary
/// distinguishes a file that failed to load from a merge that failed, but both
/// arrive here as an [`OrthoError`]: its variants are opaque enough that
/// re-splitting them would invent distinctions the framework does not promise
/// to keep. The path a file error names is inside the wrapped error's own
/// message, so the diagnostic still says which file to fix.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A configuration source supplied a value that cannot be used.
    #[error("invalid configuration value for {key}: {message}")]
    InvalidValue {
        /// The setting key, as the schema spells it.
        key: String,
        /// Why the value was rejected.
        message: String,
    },
    /// The layered merge rejected the combination of sources.
    #[error("cannot resolve configuration: {0}")]
    Merge(#[from] Arc<OrthoError>),
}

/// Composes the layers and folds them over the schema.
///
/// File layers first, in the order the policy resolves them — system, user,
/// project, each from the least-preferred candidate to the most-preferred — so
/// later always wins. Then the environment, then the command line, but only
/// when the user actually typed one of the configurable arguments: clap records
/// where each value came from, and a declared default is not a statement of
/// intent. See [`command_line_layer`].
fn merged_config(
    cli: &Cli,
    matches: &ArgMatches,
    lookup: SharedEnvSource,
    scan: SharedScanEnvSource,
) -> Result<ConfigFile, ConfigError> {
    let mut errors: Vec<Arc<OrthoError>> = Vec::new();
    let mut composer = MergeComposer::new();

    let policy =
        ConfigFilePolicy::from_builder(ConfigDiscovery::builder(APP_NAME).env_source(lookup))
            .selectors([ConfigPathSelector::env(SELECTOR_VARIABLE)])
            .explicit_mode(ExplicitMode::RequiredExclusive)
            .automatic_mode(AutomaticMode::StackScopes)
            .scope_order([
                DiscoveryScope::System,
                DiscoveryScope::User,
                DiscoveryScope::Project,
            ]);

    let outcome = policy.resolve_layers();
    for layer in outcome.into_layers_and_errors(&mut errors) {
        composer.push_layer(layer);
    }

    let provider = CsvEnv::prefixed(ENV_PREFIX)
        .uppercase(true)
        .split(ENV_SEPARATOR)
        .with_source(scan);
    match Figment::from(provider)
        .extract::<ortho_config::serde_json::Value>()
        .into_ortho_merge()
    {
        Ok(value) => composer.push_environment(value),
        Err(error) => errors.push(error),
    }

    if let Some(value) = command_line_layer(cli, matches)? {
        composer.push_cli(value);
    }

    LayerComposition::new(composer.layers(), errors)
        .into_merge_result(ConfigFile::merge_from_layers)
        .map_err(ConfigError::from)
}

/// The command line's contribution, or `None` when nothing was typed.
///
/// Judged on provenance, never on value: [`ValueSource::CommandLine`] is the
/// only source that counts as a statement, so a compiled-in default such as
/// `--md-exts md,mdc,markdown` never outranks a configured value and never
/// counts as typing the flag. Returning `None` rather than an all-`None` schema
/// keeps the absent case free of any layer at all.
fn command_line_layer(
    cli: &Cli,
    matches: &ArgMatches,
) -> Result<Option<ortho_config::serde_json::Value>, ConfigError> {
    if !CONFIGURED_ARGUMENTS.iter().any(|id| is_typed(matches, id)) {
        return Ok(None);
    }

    let layer = ConfigFile {
        wrap: typed_bool(matches, "wrap", cli.opts.wrap),
        renumber: typed_bool(matches, "renumber", cli.opts.renumber),
        breaks: typed_bool(matches, "breaks", cli.opts.breaks),
        ellipsis: typed_bool(matches, "ellipsis", cli.opts.ellipsis),
        fences: typed_bool(matches, "fences", cli.opts.fences),
        footnotes: typed_bool(matches, "footnotes", cli.opts.footnotes),
        code_emphasis: typed_bool(matches, "code_emphasis", cli.opts.code_emphasis),
        headings: typed_bool(matches, "headings", cli.opts.headings),
        md_exts: is_typed(matches, "md_exts")
            .then(|| cli.extensions().iter().map(str::to_owned).collect()),
        git: typed_bool(matches, "git", cli.selects_from_git()),
        include_untracked: typed_bool(matches, "include_untracked", cli.includes_untracked()),
        allow_conflicted: typed_bool(matches, "allow_conflicted", cli.allows_conflicted()),
    };

    // `sanitize_value` drops the `None`s, so the layer carries exactly the
    // settings the user stated and leaves every other key to the layers below.
    sanitize_value(&layer).map(Some).map_err(ConfigError::from)
}

/// Whether `id` was supplied on the command line rather than defaulted.
fn is_typed(matches: &ArgMatches, id: &str) -> bool {
    matches.value_source(id) == Some(ValueSource::CommandLine)
}

/// The parsed value of a flag, but only when the user typed it.
fn typed_bool(matches: &ArgMatches, id: &str, value: bool) -> Option<bool> {
    is_typed(matches, id).then_some(value)
}

/// Maps the eight transformation switches onto their resolved rule set.
///
/// A configured value wins over the parsed command line, which is the parsed
/// default whenever the user typed nothing, so the no-configuration case
/// reproduces the command line exactly.
fn formatting_options(config: &ConfigFile, cli: &Cli) -> FormatOpts {
    FormatOpts {
        wrap: config.wrap.unwrap_or(cli.opts.wrap),
        renumber: config.renumber.unwrap_or(cli.opts.renumber),
        breaks: config.breaks.unwrap_or(cli.opts.breaks),
        ellipsis: config.ellipsis.unwrap_or(cli.opts.ellipsis),
        fences: config.fences.unwrap_or(cli.opts.fences),
        footnotes: config.footnotes.unwrap_or(cli.opts.footnotes),
        code_emphasis: config.code_emphasis.unwrap_or(cli.opts.code_emphasis),
        headings: config.headings.unwrap_or(cli.opts.headings),
    }
}

/// The extension filter configuration supplied, or the command line's.
///
/// Configuration values did not pass through clap's `value_parser`, so they are
/// parsed here with the same function. A rejected value names the key rather
/// than silently narrowing the selection.
fn extension_filter(config: &ConfigFile, cli: &Cli) -> Result<ExtensionFilter, ConfigError> {
    let Some(values) = config.md_exts.as_deref() else {
        return Ok(cli.extensions());
    };

    values
        .iter()
        .map(|value| {
            parse_extension(value).map_err(|error| ConfigError::InvalidValue {
                key: String::from("md_exts"),
                message: error.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
