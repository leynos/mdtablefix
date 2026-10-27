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

pub(crate) use self::schema::ConfigFile;
use self::schema::{APP_NAME, CONFIGURED_ARGUMENTS, ENV_PREFIX, ENV_SEPARATOR, SELECTOR_VARIABLE};
use crate::{
    command::{Cli, FormatOpts},
    select::extensions::{ExtensionFilter, parse_extension},
};

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
    errors.extend(promoted_discovery_errors(&outcome));
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

/// Promotes a discovered file's failure that the framework would otherwise hide.
///
/// The framework classifies an automatic candidate as *optional*, because a
/// location that holds no file is the normal case. Two different outcomes land
/// in that bucket, though: a candidate that is absent, which the loader reports
/// as no error at all, and a candidate that exists but cannot be loaded, which
/// is an error. The framework withholds the second kind whenever *any* scope
/// loaded, so a malformed project file beside a valid user file would leave the
/// run to proceed on the user settings alone.
///
/// That is the one silence this module refuses. Formatting with a rule set the
/// user did not ask for is the destructive surprise
/// `docs/adrs/0014-configuration-layering.md` exists to prevent, and the
/// adoption's stated behaviour is that a malformed file is a diagnostic with
/// exit status 2. Promoting the withheld errors restores it without touching
/// the absent case: a missing file still contributes nothing.
///
/// The errors are promoted only when a layer loaded. With no layer the
/// framework already returns them itself, so promoting unconditionally would
/// report each one twice.
fn promoted_discovery_errors(outcome: &ortho_config::FileLayerOutcome) -> Vec<Arc<OrthoError>> {
    if outcome.layers_are_empty() {
        return Vec::new();
    }

    outcome.ignorable_errors().to_vec()
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
#[path = "config_error_tests.rs"]
mod error_tests;
#[path = "config_schema.rs"]
mod schema;
#[cfg(test)]
#[path = "config_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
