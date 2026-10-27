# Configuration layering

## Status

Accepted.

## Date

2026-10-27.

## Context

`mdtablefix` has no configuration file. Every run states its own rule set, so a
repository that wants one set of formatting rules repeats them in its Makefile,
in its continuous-integration workflow, in an editor hook, and in every hand
written invocation. This repository does exactly that: the `fmt` and
`check-fmt` recipes pass
`--git --include-untracked --wrap --renumber --breaks --ellipsis --fences` on
every call, and a contributor who forgets one flag gets a different result from
the same tree.

[`ortho_config` v0.10.0](https://github.com/leynos/ortho-config) is a released
layered-configuration framework: it discovers configuration files in
conventional locations, reads a prefixed environment, orders the sources, and
merges them over a typed schema. Adopting it gives `mdtablefix` a configuration
file and an environment override without inventing a bespoke precedence ladder,
a bespoke discovery walk, and a bespoke error vocabulary.

Two constraints shape the adoption.

The first is behaviour preservation. For every argument vector the tool accepts
today, a run in which no configuration file is discoverable and no
`MDTABLEFIX_` variable is set must write the same standard output, the same
standard error, and the same exit status as before. That rules out routing
process arguments through the framework's generated parser, which renders every
boolean switch with a `<BOOL>` value placeholder and accepts `--wrap=false`,
where this tool renders a bare switch and rejects `--wrap=80`. The generated
parser also adds a `--config-path` option; it is hidden from the help text by
default, so it would be accepted without being documented — a worse outcome
than either showing it or not having it.

The second is [ADR 0012](0012-environment-seam-taxonomy.md), which prohibits
this repository's code from reading the process environment and names the
injection shapes that replace it. A configuration system that reads the
environment must therefore do so through an injected source, not through a call
this repository's own source makes.

This record follows the shape of
[ADR 0012](0012-environment-seam-taxonomy.md) — a bare title, `Status`, `Date`,
`Context`, `Decision`, `Consequences`, and `References`, in `docs/adrs/` with a
four-digit prefix. `docs/documentation-style-guide.md` instead prescribes
`adr-NNN-short-description.md` in `docs/` and an
`# Architectural decision record (ADR) NNN:` title. Every record in
`docs/adrs/` deviates from that template in the same way, so following the
local form is the consistent choice; changing the convention is a
repository-wide decision and is not made here.

## Decision

### Configure settings, not invocations

Two kinds of thing arrive on the command line. A *setting* is a property of how
the tool formats, or of which files it selects. An *invocation* is what to do
and to which files. Configuration supplies settings and never invocations.

The schema therefore holds:

- the eight transformation switches — `wrap`, `renumber`, `breaks`, `ellipsis`,
  `fences`, `footnotes`, `code_emphasis`, and `headings`;
- `md_exts`, the extension list a Git selection keeps; and
- `git`, `include_untracked`, and `allow_conflicted`, the Git selection's own
  switches.

It does not hold the mode — `in_place`, `check`, `diff`, or `list_files` — and
it does not hold the positional file arguments. A file that silently switched a
bare `mdtablefix notes.md` into an in-place rewrite would be a destructive
surprise caused by a file the user forgot existed. The paths are the user's
statement of what to act on, which a file cannot make on their behalf.

`git` is the one selection setting that can be configured, because "format the
tracked Markdown of this repository" is a property of the repository rather
than of one invocation. It decides the input source only when the user names no
files: a typed path always means a file run, since a positional argument
outranks a setting. The other Git settings then select nothing, but they are
still validated, because a configuration file that names an extension the tool
cannot parse is wrong whether or not this particular run would have consulted
it, and a validation that depended on the resolved mode would surface a typo on
some invocations and not others.

### Discover `.mdtablefix.toml` through scopes

The file name is derived rather than overridden. Discovery is built with
`app_name = "mdtablefix"`, which leaves the framework's `config.toml` default
as the nested name and derives the dotfile `.mdtablefix.toml`; the project file
name defaults to the dotfile's. The candidates, in preference order, are:

1. the path named by `MDTABLEFIX_CONFIG_PATH`, when the variable is set to a
   non-empty value;
2. `$XDG_CONFIG_HOME/mdtablefix/config.toml`, then
   `$XDG_CONFIG_HOME/.mdtablefix.toml`;
3. for each directory in `XDG_CONFIG_DIRS` — or `/etc/xdg` when the variable is
   unset or names no usable directory — `mdtablefix/config.toml`, then
   `.mdtablefix.toml`;
4. the same pair under `%APPDATA%`, then under `%LOCALAPPDATA%`;
5. `$HOME/.config/mdtablefix/config.toml`, then
   `$HOME/.config/.mdtablefix.toml`, then `$HOME/.mdtablefix.toml`; and
6. `.mdtablefix.toml` in the working directory.

The selector is a policy selector rather than a discovery-level variable, and
the policy runs it with `ExplicitMode::RequiredExclusive`: a user who names a
file has asked for that file, so a path that is missing or unreadable is an
error naming the path rather than a silent fall back to discovery.

Automatic discovery runs in `AutomaticMode::StackScopes` with a scope order of
`system`, `user`, `project`. The alternative, `AutomaticMode::FirstWins`, is
the framework's default and cannot express this design: it walks the candidate
list flat, stopping at the first file that loads, and every user-level location
precedes the project file in that list. A user with a `~/.mdtablefix.toml`
would then never see the project file at all — the reverse of the precedence a
project file exists to provide.

Scope stacking reverses the candidate walk within a scope, so the
least-preferred location is applied first and the most-preferred last. The
location the flat scan would have chosen still wins inside its scope, and scope
order gives system below user below project. Three consequences follow, and
each is accepted deliberately:

- Several files can contribute at once, so a system-wide file can provide a base
  that a project file refines key by key.
- A candidate that exists but cannot be loaded is fatal, even when another scope
  loaded. The framework classifies an automatic candidate as *optional*,
  because a location that holds no file is the normal case, and it withholds an
  optional failure whenever any layer loaded — which would let a malformed
  project file fall back silently to the user file beneath it. The integration
  refuses that silence. Formatting under a rule set the user did not ask for,
  taken from a file that is not the one they are editing, is the destructive
  surprise this record exists to prevent. A candidate that is merely *absent*
  still contributes nothing and is still not an error, so the promotion is
  precise. A file named by `MDTABLEFIX_CONFIG_PATH` was already fatal either
  way.
- The project scope is the working directory and nothing else. There is no walk
  upward and no stop at a Git root, so a run from a subdirectory does not see
  the repository's file. An upward walk was rejected because it makes the same
  command behave differently depending on where in a tree it is typed.

The promotion is a small function in `src/config.rs`, not a framework setting.
It asks `FileLayerOutcome::layers_are_empty()` first and, only when a layer did
load, reports the outcome's `ignorable_errors()`. Promoting unconditionally
would report each error twice in the all-empty case, because the framework
already returns them itself when nothing loaded. Both accessors are public
upstream for exactly this kind of consumer check.

The loader also honours an `extends` key, resolving a relative parent against
the declaring file and applying parents before children. This is inherited
rather than designed, and it is recorded so documentation describes what the
tool actually does.

### Read TOML and nothing else

The dependency is a caret requirement with default features, which supplies
`serde_json` and `toml`. The `json5` and `yaml` features stay off, so those
candidate spellings are never generated, and a selected file with a `.json5` or
`.yaml` extension fails with a message saying the feature is disabled. A
selected file with any other unrecognized extension is parsed as TOML, which is
the format the documented `.toml` name implies.

The reason for one format is that a configuration file a user cannot guess the
syntax of is worse than no configuration file. TOML is what the framework's
default build reads, what `Cargo.toml` and `rustfmt.toml` have accustomed this
repository's contributors to, and what a hand-edited rule list needs: no
significant indentation, and comments. Enabling another format later is a
feature-flag change plus a documented extension, not a schema change.

### Spell the environment in the framework's own dialect

The prefix is `MDTABLEFIX`, which the framework normalizes to `MDTABLEFIX_`.
Matching is case-insensitive, `__` separates nested keys, and the provider is
exactly `CsvEnv::prefixed("MDTABLEFIX_").uppercase(true).split("__")`, which is
what the derive generates for that prefix. Declaring the same dialect in the
hand-composed layer is what keeps the two in step if a future change ever moves
to the generated loader.

Values follow the provider's documented rules, and the surprising ones are
worth stating because they are not a matter of taste:

- a value containing a comma and not beginning with `[`, `{`, `"`, or `'` splits
  into a list, so `MDTABLEFIX_MD_EXTS=md,mdc` is two extensions;
- a lone `true` or `false`, compared case-insensitively, becomes a boolean, and
  any other scalar that parses as JSON becomes that value;
- a value beginning with `[` is a JSON array, which is the way to write a
  single-element list: a bare `md` is the string `md`, not a list of one, and a
  trailing comma yields an empty element that validation then rejects; and
- the `__` separator has nothing to nest today, because the schema is flat.

No legacy variable or file name is honoured, because there was no earlier
configuration system to be compatible with.

### Order the layers defaults, files, environment, command line

Precedence is compiled-in defaults, then discovered files, then the
environment, then the command line. File layers run system, then user, then
project, and within each scope from the least-preferred candidate to the
most-preferred, so later always wins. The composed list is handed to the
derive's `merge_from_layers`, which folds it left to right.

The layers are composed by hand rather than through the generated loader, for
the reason given in the context: the generated parser would change argv
behaviour. The schema, the generated merge, and the framework's error
vocabulary are still used, so the integration is a composition root rather than
a parallel implementation.

Precedence for a list-valued setting is whole-value replacement, not
accumulation: a higher layer that supplies `md_exts` replaces the lower layer's
list rather than appending to it. This is the framework's merge for a value
that is neither a `Vec` nor a map — `merge_strategy` may not even be written on
such a field, where it is a compile error — and it is the behaviour the
precedence rule requires. A user who sets an extension list means that list.

A schema whose every field is optional, merged from no layers at all, still
deserializes: the framework normalizes the empty accumulator to an empty object
before deserialization, so the absent case is all-`None` rather than an error.
That is what makes the no-configuration run identical to the run before this
change.

### Judge a flag by whether it was typed

The `--git` requirement on `--include-untracked`, `--allow-conflicted`,
`--list-files`, and `--md-exts` is judged on whether the user typed the flag,
never on whether a configuration source supplied the corresponding setting. It
is enforced where it is today, in `Cli::parse_validated` and before any
configuration is consulted, so a configuration file can neither satisfy the
requirement nor provoke it. That ordering is why the no-configuration behaviour
is preserved by construction rather than by test alone.

The distinction also decides precedence: clap records where each value came
from, and only `ValueSource::CommandLine` counts as typed. `DefaultValue` is
treated as absent, so a compiled-in default never outranks a configured value.
`--md-exts` is the case that makes this visible: its default is
`md,mdc,markdown`, and treating it as typed would both override a configuration
file and demand `--git` from a user who typed nothing.

The framework spells the same rule `cli_default_as_absent`. This repository's
path does not need the attribute, because its command-line layer is built from
the parser's own record of provenance and its schema fields carry no clap
default that could be mistaken for a typed value.

### Ignore keys the schema does not name

A configuration key the schema does not declare is ignored. The framework's
types carry no `deny_unknown_fields`, serde's default is to skip what it is not
asked for, and the guide treats an unrecognized table the same way it treats
any other unknown key.

The reason is compatibility across versions: a configuration file is checked
into a repository and read by whatever version of the tool a contributor
happens to have, so a file that a newer release can read must not be rejected
by an older one that does not yet know one of its keys. The cost is that a typo
in a key is silent. A known key with a value of the wrong type is still an
error, because deserialization is typed.

### Inject the environment at the composition root

Discovery reads five environment variables — the selector, `XDG_CONFIG_HOME`,
`XDG_CONFIG_DIRS`, `APPDATA`, `LOCALAPPDATA` — and the home directory lookup
reads `HOME` and `USERPROFILE`. The merge layer enumerates the `MDTABLEFIX_`
prefix. Both are injected:

- discovery takes a `SharedEnvSource` on the builder, and the merge layer takes
  a `SharedScanEnvSource` through `CsvEnv::with_source`;
- production supplies the framework's `process_env_source()` and
  `process_scan_env_source()`, named once, in the composition root; and
- tests supply one `MapEnv`, whose `Arc` coerces to both shared types, so a test
  drives discovery lookups and merge enumeration from one fixed map without
  touching the process environment.

The two traits are separate on purpose. The lookup trait deliberately cannot
enumerate, so a discovery caller cannot read an unrelated secret; scanning a
prefix is a distinct capability that the merge layer opts into. Injected
sources also return no platform home fallback, so a test's candidate list does
not vary with the host.

This satisfies ADR 0012 without an exception. That record's enforcement is
lexical — a lint on this repository's own call sites — and this design makes no
such call: the reads happen inside the dependency, behind an explicitly
supplied seam, which is the shared-environment-trait shape that record
prescribes. Binary-level tests control the child's environment with
`Command::env` and `Command::env_remove`, which ADR 0012 already sanctions;
in-process tests inject `MapEnv` and mutate nothing.

### Keep the library free of the framework

One package builds both the library and the binary, and Cargo has no
binary-only dependency table, so the manifest cannot keep `ortho_config` and
`serde` away from the library. The boundary is therefore drawn in the module
graph: the configuration module is declared from `src/main.rs` beside the other
binary modules and is not reachable from `src/lib.rs`, and `src/lib.rs` is
unchanged by the adoption. A test asserts the contract by reading the manifest
and the library's module sources, so a future import into the library fails the
suite rather than passing unnoticed.

### Do not adopt subcommands, profiles, or localization

The binary has no subcommands, so the schema is flat and there are no
per-subcommand namespaces. Profiles and `--profile` are not adopted: they add a
second axis of precedence and a reserved key for a tool whose settings fit in
one table. Help text and parse failures stay in English. No `cargo-mdtablefix`
entry point is added, because the binary is not named that and nobody invokes
it through Cargo. No `--config-path` option is added either: explicit selection
is `MDTABLEFIX_CONFIG_PATH` alone, which keeps `--help` byte-identical.

## Consequences

- With no file, no `MDTABLEFIX_CONFIG_PATH`, and no `MDTABLEFIX_*` variable, the
  resolved settings equal the compiled-in defaults and the run is unchanged,
  including the pinned `--help` snapshot.
- A `.mdtablefix.toml` in a working directory changes the behaviour of every
  invocation there, including `make check-fmt` and editor hooks, which is the
  point; this repository adds no such file, so its own gates keep pinning every
  rule they mean.
- Behaviour becomes a function of the working directory, and only of the working
  directory: a run from a subdirectory does not read the parent's file.
- A malformed automatic candidate fails the run even beside another scope that
  loaded, so a typo in a project file is reported instead of silently falling
  back to the user file. An absent candidate is still not an error.
- Configuration failures exit with status 2 and write to standard error, so the
  exit-status and stream contracts are unchanged in kind.
- Because scopes stack, the resolved outcome reports which scopes contributed a
  layer; under a flat scan that report would always be empty.
- A typo in a configuration key is silently ignored, which is the price of a
  file that survives a version skew; the users' guide lists the accepted keys
  exhaustively to make the intended spelling findable.
- The dependency graph grows. The framework brings `figment` and its TOML
  provider, `serde_json`, `toml`, `uncased`, `clap-dispatch`, `camino`, `dirs`
  (or `xdg` on Unix, or `directories` elsewhere), and `ortho_l10n`; several of
  those, such as `camino`, `cap-std`, `tracing`, `thiserror`, and `clap`, this
  workspace already depends on, so only the remainder is new. The declared
  minimum supported Rust version does not move: the framework's floor is 1.89.0
  and this workspace already declares `rust-version = "1.89"`.

## References

- [`ortho_config` v0.10.0 user's guide](../ortho-config-users-guide.md), the
  vendored reference for discovery, the environment provider, and merging.
- [Provenance of the vendored guides](../ortho-config-v0-10-0-provenance.md),
  the tag, commit, and checksums the guides were imported from.
- [Migrating `ortho_config` to v0.10.0](../ortho-config-v0-10-0-migration-guide.md),
  the v0.9.0-to-v0.10.0 delta.
- [Environment seam taxonomy](0012-environment-seam-taxonomy.md), the policy
  that requires the environment source to be injected.
- [Git file selection](0010-git-file-selection.md), the selection whose
  extension list and switches the schema configures.
- [Issue #666](https://github.com/leynos/mdtablefix/issues/666), which requested
  this adoption and the behaviour-preservation constraint.
- [`ortho_config` issue #627](https://github.com/leynos/ortho-config/issues/627),
  why the dependency carries default features.
