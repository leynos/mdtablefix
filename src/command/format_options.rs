//! The formatting switches the command line exposes, and their into-options
//! mapping.
//!
//! Kept beside `src/command.rs` rather than inside it because the eight
//! switches plus their optional-`=<BOOL>` attributes are long enough to take
//! that file past the cap `AGENTS.md` sets. Nothing here parses an argument or
//! touches a document: the struct is a clap `Args` group flattened into `Cli`
//! and the conversion turns it into the library's `Options`, so the split is
//! by line count rather than by responsibility.
//!
//! Declared only from `src/command.rs`, and therefore reachable only from the
//! binary: `src/lib.rs` never names it. `SWITCH_VALUE` and
//! `HIDE_SWITCH_DEFAULT` are `pub(super)` because `Cli`'s own `--git`,
//! `--include-untracked` and `--allow-conflicted` switches declare the same
//! optional-value grammar and must render identically.

use mdtablefix::Options;

/// The value name every switch with an optional `=<BOOL>` renders in `--help`.
///
/// Named rather than repeated, because it is the reader's cue that
/// `--wrap=false` is accepted at all: a bare `bool` field would render
/// `--wrap` alone, and the help would then advertise a switch that cannot be
/// turned off.
pub(super) const SWITCH_VALUE: &str = "BOOL";

/// Hides the `[default: false]` that an optional-value switch would print.
///
/// The switch still declares `ArgAction::SetTrue`, which supplies `false` as a
/// default value when the field declares none, so the resolved default is
/// unchanged; only the rendering is suppressed. `--help` would otherwise claim
/// `--wrap` defaults to false, which stops being the whole truth once a
/// configuration file or an `MDTABLEFIX_*` variable can set it true — the help
/// describes the command line, and on the command line the flag is absent.
///
/// A plain `bool` field needs none of this: it takes no value, so clap prints
/// neither a default nor a possible-value list. Both appear here only because
/// `num_args = 0..=1` makes the argument take one.
pub(super) const HIDE_SWITCH_DEFAULT: bool = true;

/// Independent content transformations requested by the command-line caller.
///
/// These flags are intentionally represented separately so the processing
/// pipeline can apply its stable normalization order to any selected subset.
///
/// Each takes an optional `=<BOOL>` value: `--wrap` alone still means `true`,
/// `--wrap=false` is a valid override of a configured `true`, and `--wrap=80`
/// is still rejected. The equals sign is required for the value, so
/// `--wrap false` leaves `false` a positional argument exactly as it did
/// before, and every argv that worked before the change is accepted with the
/// same meaning. `tests/cli_config_binary.rs` pins all four spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI exposes independent flags via separate switches"
)]
pub struct FormatOpts {
    /// Wrap paragraphs and list items to 80 columns
    #[arg(
        long = "wrap",
        value_name = SWITCH_VALUE,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        hide_default_value = HIDE_SWITCH_DEFAULT,
    )]
    pub wrap: bool,
    /// Renumber ordered list items
    #[arg(
        long = "renumber",
        value_name = SWITCH_VALUE,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        hide_default_value = HIDE_SWITCH_DEFAULT,
    )]
    pub renumber: bool,
    /// Reformat thematic breaks as underscores
    #[arg(
        long = "breaks",
        value_name = SWITCH_VALUE,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        hide_default_value = HIDE_SWITCH_DEFAULT,
    )]
    pub breaks: bool,
    /// Replace "..." with the ellipsis character
    #[arg(
        long = "ellipsis",
        value_name = SWITCH_VALUE,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        hide_default_value = HIDE_SWITCH_DEFAULT,
    )]
    pub ellipsis: bool,
    /// Normalize fence delimiters to three backticks
    #[arg(
        long = "fences",
        value_name = SWITCH_VALUE,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        hide_default_value = HIDE_SWITCH_DEFAULT,
    )]
    pub fences: bool,
    /// Convert bare numeric references and the final numbered list to
    /// Markdown footnote links
    #[arg(
        long = "footnotes",
        value_name = SWITCH_VALUE,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        hide_default_value = HIDE_SWITCH_DEFAULT,
    )]
    pub footnotes: bool,
    /// Fix emphasis markers adjacent to inline code
    #[arg(
        long = "code-emphasis",
        value_name = SWITCH_VALUE,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        hide_default_value = HIDE_SWITCH_DEFAULT,
    )]
    pub code_emphasis: bool,
    /// Convert Setext-style headings to hash-prefixed headings
    #[arg(
        long = "headings",
        value_name = SWITCH_VALUE,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        hide_default_value = HIDE_SWITCH_DEFAULT,
    )]
    pub headings: bool,
}

impl From<FormatOpts> for Options {
    fn from(opts: FormatOpts) -> Self {
        Self {
            wrap: opts.wrap,
            ellipsis: opts.ellipsis,
            fences: opts.fences,
            footnotes: opts.footnotes,
            renumber: opts.renumber,
            code_emphasis: opts.code_emphasis,
            headings: opts.headings,
        }
    }
}
