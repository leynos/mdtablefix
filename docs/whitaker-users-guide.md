# Whitaker User's Guide

Vendored from `leynos/whitaker` at commit `5949193d` (blob `65ccb3c7`,
2026-11-13), where it is `docs/users-guide.md`. Three deliberate deviations
keep the copy usable here: repository-relative links are rewritten to absolute
URLs into the Whitaker repository; the ancestor-context example nests its
helper in the annotated test, because the lint derives test context from HIR
ancestry alone; and the Markdown is formatted by this repository's pipeline.
Everything else follows that upstream revision.

Whitaker is a collection of opinionated Dylint lints for Rust. This guide
explains how to integrate the lints into a project and configure them.

For contributors who want to develop new lints or work on Whitaker itself, see
the
[Developer's Guide](https://github.com/leynos/whitaker/blob/main/docs/developers-guide.md).

## Upgrading library consumers

Version 0.3.0 includes source-breaking changes to public Rust APIs, covering
spelling, installer interfaces, crate re-exports, the `whitaker_suite` entry
point, and the fallible `suggest_decomposition` query. Existing library
consumers must update imports, field access, and call sites as described in the
[Whitaker 0.3.0 migration guide](https://github.com/leynos/whitaker/blob/main/docs/v0-3-0-migration-guide.md).
The changes affect Rust source only: the installer's CLI flags and SARIF 2.1.0
JSON keys retain their existing spellings, so existing command invocations and
serialized data require no migration.

## UI test harness and offline Cargo

The `whitaker_lint_core::testing::ui` harness builds the lint library with
`--frozen`. For those Cargo invocations the harness resolves dependencies from
local data only: it makes no network access and will not update `Cargo.lock`.
Every dependency the lint crate needs must already be in the local Cargo cache,
and `Cargo.lock` must agree with `Cargo.toml`. Run `cargo fetch --locked`
first, or use the repository's `make fetch` target, which also prepares the
pinned Dylint driver.

That guarantee covers the harness's own metadata and lint-library build
commands, not every Cargo process a test may start. A wholly offline test run
additionally needs an offline environment — `make test` sets
`CARGO_NET_OFFLINE=true` — and a Dylint driver that has already been prepared,
because a cold driver builds itself and that build needs the network. Offline
runs also still contend for Cargo's shared package-cache lock with any other
Cargo process on the machine; no Cargo flag removes that wait.

A cold cache or a stale lockfile now produces a Cargo resolution failure
instead of a download. This is deliberate: a UI test that silently fetches a
crate measures the network rather than the lint, and an offline run is
reproducible where a fetching one is not.

## Quick Setup

### Prerequisites

Install `cargo-dylint` and `dylint-link`:

```sh
cargo install cargo-dylint dylint-link
```

The published `whitaker-installer` crate requires Rust 1.88 or newer. This is
the first stable compiler that supports the Rust 2024 edition used by the
installer.

Published x86_64 GNU/Linux installer, dependency, and lint artefacts target the
Ubuntu 22.04 `GLIBC_2.35` baseline and do not require a newer glibc version. On
an incompatible system, build Whitaker and its dependencies from source instead
of using the published Linux artefacts.

### Standalone installation (recommended)

The simplest way to use Whitaker is via the standalone installer, which handles
setup automatically:

```sh
cargo install whitaker-installer
whitaker-installer
whitaker --all
```

This:

1. Installs `cargo-dylint` and `dylint-link` if not present
   The installer first attempts to download pre-built dependency binaries from
   Whitaker's GitHub Releases page for the current platform. If the release
   asset is absent (HTTP 404 or 410), the installer skips `cargo binstall` and
   falls back directly to building from source with `cargo install`, pinned to
   the version recorded in the dependency manifest. If the release asset is
   present but the download fails for another reason, the installer falls back
   to `cargo binstall` when available and then to `cargo install`. When the
   source-build path is taken, the installer reports that it is falling back to
   Cargo, and on success it prints
   `Installed <tool> from source with cargo install.`. After installation,
   `cargo-dylint` is verified by running `cargo dylint --version`.
   `dylint-link` is never executed: it is a linker wrapper that forwards its
   arguments to the underlying linker, so it has no reliable self-reporting
   subcommand. A release artefact is trusted once its checksum, extraction, and
   executable permissions have been established, and a Cargo-managed copy is
   checked by resolving it on `PATH` and comparing the version Cargo recorded.
2. Installs the verified Clippy snapshot that Dylint's driver build reads (see
   [The installed Dylint bootstrap](#the-installed-dylint-bootstrap))
3. Clones the Whitaker repository to a platform-specific data directory
4. Builds or downloads the lint libraries
5. Creates `whitaker` and `whitaker-ls` wrapper scripts. Both call
   `whitaker-installer run`, which runs `cargo dylint` with the installed
   libraries, Clippy snapshot, and driver directory; `whitaker-ls` lists the
   installed Whitaker suite libraries
6. Ensures the pinned Rust toolchain and components are installed via rustup

After installation, run `whitaker --all` in any Rust project to lint it. Use
`whitaker-ls` to list the installed Whitaker suite libraries.

The installer's own release archives are also usable by `cargo binstall`:

```sh
cargo binstall whitaker-installer
```

Every published archive stores its files as regular entries, which is the only
kind cargo-binstall extracts. Archives built before that change stored the
binary as a GNU sparse entry, which binstall skips without an error, so it
found no binary and compiled the installer from source instead
([#461](https://github.com/leynos/whitaker/issues/461)). From the next
installer release, `cargo binstall` installs the prebuilt binary, and each
release verifies that by installing it with source builds disabled.

The source-build fallback can fail when the *active* Rust toolchain is older
than the installer's pinned dependencies require. `cargo-dylint` 6.3.0's shipped
`Cargo.lock` pins `cargo-util` 0.2.32, which declares `rust-version = "1.97"`,
so a host on Rust 1.96 or older cannot build it from source. The installer
recognizes that refusal and adds the commands that resolve it: install a newer
toolchain with `rustup toolchain install stable`, then rerun with
`cargo +stable install --locked --version 6.3.0 <tool>`. Alternatively, install
prebuilt copies of both `cargo-dylint` and `dylint-link` manually and rerun with
`--skip-deps`, which skips the availability check for every tool rather than
only the one whose build failed.

On Windows, the installer's `PATH` check honours `PATHEXT` and falls back to
the usual executable suffixes when `PATHEXT` is unset, so a normal
Cargo-installed executable such as `dylint-link.exe` in
`%USERPROFILE%\.cargo\bin` is located correctly and then matched against the
version Cargo recorded for it, without needing a separate wrapper or manual
environment-variable workaround.

**Options:**

- `--cranelift` — Tell the installer to add the
  `rustc-codegen-cranelift` component via `rustup component add`. The
  `rustc-codegen-cranelift` component is not included in the standard nightly
  toolchain, so enable `--cranelift` when a project or CI pipeline requires the
  Cranelift back-end and would otherwise need an explicit
  `rustc-codegen-cranelift` component-add step before running the installer.
- `--skip-deps` — Skip `cargo-dylint`/`dylint-link` installation check
- `--skip-wrapper` — Skip wrapper script generation and print the
  `DYLINT_LIBRARY_PATH`, `DYLINT_CLIPPY_REPO`, and `DYLINT_DRIVER_PATH` exports
  instead. Without the wrappers nothing serializes the first driver build, so
  let one `cargo dylint` run finish before starting others
- `--clippy-snapshot-bundle FILE` — Install this Clippy snapshot bundle, with
  its `.sha256` sidecar beside it, instead of downloading the published one. It
  is verified exactly as a downloaded bundle is
- `--no-update` — Don't update existing repository clone
- `--suite-version REF` (alias `--suite-ref`) — Build the lint suite from a
  given tag, branch or commit rather than from the default branch tip. See
  [Pinning the lint suite](#pinning-the-lint-suite)
- `--no-source-fallback` — Fail rather than build from source when a published
  artefact is absent. See [Refusing a source build](#refusing-a-source-build)
- `-v`, `--verbose` — Include debug diagnostics, including bounded reasons why
  a `PATH` candidate was skipped. The candidate path is not printed.
- `-q`, `--quiet` — Suppress progress output and show only errors.

Installer diagnostics are written to standard error without ANSI formatting.
The default verbosity shows warnings and errors; `-v` enables debug events, and
`-q` limits tracing output to errors.

The installer scans `PATH` entries in order and uses the first regular
executable candidate for `dylint-link`. It skips missing candidates and entries
that cannot be represented as UTF-8 or opened, then continues searching later
entries.

### The installed Dylint bootstrap

Running a Dylint lint library needs a driver binary built against the nightly
compiler, and building it needs Clippy source at the revision matching that
nightly. Dylint finds that revision in Clippy's Git history. Whitaker no longer
lets it clone public Clippy for this: an upstream change to Clippy's history
(the `master` to `main` rename, #526) broke every first run in every consumer.
The installer instead installs a published, checksummed snapshot of the Clippy
history the suite was verified against, and the driver is built from it on
first use.

Everything lives beside the managed Whitaker clone, in the platform data
directory (`$XDG_DATA_HOME`, usually `~/.local/share`, on Linux):

```text
whitaker-bootstrap/
  bootstrap.json                         what this installation provisioned
  clippy/<commit>/                       the verified Clippy snapshot
  drivers/dylint-<version>/clippy-<commit>/
    <toolchain>/dylint-driver            built on the first run
```

The installer's generated `whitaker` and `whitaker-ls` wrapper scripts contain
no paths. They find `whitaker-installer` beside themselves, or else on `PATH`,
and call `whitaker-installer run`, which:

- checks that the installation is complete, that the snapshot is still the
  pinned one (with `main` as its only branch), and that
  `cargo dylint --version` reports the Dylint release the bootstrap was built
  for;
- sets `DYLINT_LIBRARY_PATH`, `DYLINT_CLIPPY_REPO`, and `DYLINT_DRIVER_PATH`,
  overwriting any values already in the environment, and passes every other
  variable (including `RUSTC_WRAPPER` and `RUSTFLAGS`) through unchanged;
- builds the driver on the first run while other runs wait, so concurrent
  first runs build it once, then lets later runs share it. Any run that left a
  newly built driver, including a rare rebuild after Dylint finds a cached
  driver unusable, ends with
  `whitaker: built the Dylint driver on first use; later runs reuse it`, which
  explains a slow run. A warm run whose driver Dylint rebuilt also re-verifies
  the snapshot before it succeeds;
- exits with `cargo dylint`'s own status, so lint failures still fail CI.

Paths are worked out from the current data directory each time, so moving the
data directory (or pointing `XDG_DATA_HOME` elsewhere) keeps an installation
working. A source-built suite whose libraries live outside the data directory
records their absolute path.

Every snapshot failure names its remedy and says that Whitaker never falls back
to public Clippy:

- *is not installed* or *is invalid*: re-run `whitaker-installer`; for an
  invalid generation, remove the named directory first, because the installer
  never modifies an installed snapshot.
- *was not found* during installation: that Whitaker release does not publish
  the snapshot it pins. Use a release that does, or pass
  `--clippy-snapshot-bundle`.
- *failed SHA-256 verification*: the download is corrupt or was tampered with.
  Delete it and download it again.
- *cargo-dylint reports version …*: the `cargo-dylint` on `PATH` is not the
  release this installation was bootstrapped for. Re-run `whitaker-installer`.
- *I/O failed* or *git … failed*: check the permissions and free space at the
  named path, or that `git` is installed and on `PATH`, then re-run
  `whitaker-installer`.
- *invalid Clippy snapshot manifest*: the installer's embedded manifest is
  damaged. Reinstall `whitaker-installer`.

`whitaker-installer --dry-run` prints the snapshot and driver directories,
which is useful for caching them in CI.

#### Running the lints directly

`whitaker --all` is shorthand for `whitaker-installer run -- --all`. Call the
installer directly where the wrappers are not on `PATH`, for example in a CI
step:

```sh
whitaker-installer run -- --all
```

Everything after `--` is passed to `cargo dylint` unchanged, the three
bootstrap variables are supplied for that one run, and the command exits with
`cargo dylint`'s status. Plain `cargo dylint` without them does not use the
bootstrap: Dylint would try to clone public Clippy to build its driver, which
is exactly what the bootstrap replaces.

An installation made with `--skip-wrapper` prints the three variables as
`export` (bash and zsh), `set -gx` (fish), and `$env:` (PowerShell) lines.
Apply them in the shell or CI step before running `cargo dylint` directly.
Nothing serializes the first driver build in that case, so let one
`cargo dylint` run finish, building the driver, before starting others in
parallel; later runs reuse it.

### Root CLI

The root `whitaker` command provides `install` and transitional `ls` commands,
including `whitaker ls --json`; other arguments, such as `--all`, are forwarded
to `cargo dylint`. From a checkout, install it with `cargo install --path .`.
Crates.io packages and binstall archives are pending. When both `whitaker`
binaries are on `PATH`, the first wins; `install` writes no compatibility
scripts, so use `whitaker ls` rather than `whitaker-ls`. Consumers upgrading
from 0.2.x should read
[the migration note](https://github.com/leynos/whitaker/blob/main/docs/v0-3-0-migration-guide.md#root-whitaker-package)
for the move of the shared library APIs to `whitaker_lint_core`.

Forwarding runs through the installed bootstrap, so `whitaker --all` uses the
same verified Clippy snapshot, driver directory, and serialized first build as
`whitaker-installer run -- --all`. A first run therefore works without Dylint's
upstream Clippy clone, and a diagnostic from the bootstrap reaches the
terminal. When no bootstrap has been recorded — a fresh checkout, or a manually
configured `DYLINT_LIBRARY_PATH` — forwarding starts `cargo dylint` directly,
which is the behaviour the legacy wrapper had before installation.

The root `whitaker` CLI also remembers each successful install's staging path
by Rust toolchain. This lets `whitaker --all` find libraries installed with a
custom `--target-dir`, including when run from another project. An explicitly
set `DYLINT_LIBRARY_PATH` continues to override the recorded paths.

`-q` and `-v` control tracing diagnostics for every command, including
forwarded invocations. `-q` shows only error-level tracing events, while direct
bootstrap notices such as the cold driver build are still written to standard
error.

### Adding Whitaker to a project

Add the following to the workspace `Cargo.toml`:

```toml
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "whitaker_suite" }
]
```

Then run the lints through the installed bootstrap, which supplies the Clippy
snapshot and driver directory that Dylint needs (see
[Running the lints directly](#running-the-lints-directly)):

```sh
whitaker --all
```

### Pinning the lint suite

The installer builds the suite from a Whitaker checkout. Without a pin it uses
whatever is at the default branch tip, so a change to that branch alters lint
results with no commit in the consuming repository: a gate can turn red on a
branch that has not been touched. `--suite-version` names the reference to
build from instead, so a suite change arrives as a reviewed bump.

```sh
whitaker-installer --suite-version v0.2.7
```

It accepts a tag, a branch or a commit, and the installer hands it to `git` to
resolve.

Two consequences are worth knowing before pinning.

**A pinned suite is built from source.** Prebuilt lint libraries are published
only for the branch tip, under the `rolling` tag, so a pin can never be served
from them; the installer does not attempt the download at all when a pin is
set, and compiles the suite locally instead. Pinning trades install time for
reproducibility, and that is the whole of the trade: a pinned lane is slower on
a cold cache and does not change what it produces.

The prebuilt path would not help a pin even if artefacts existed for every tag.
A lint library must be built with the exact toolchain that will load it, and
artefacts are named for the toolchain they were built with, so a pin held
against a newer `rust-toolchain.toml` misses and builds from source anyway. The
guarantee a pin buys is reproducibility, not speed.

With `--no-update`, an already-local `--suite-version` reference resolves
without network access. A later unpinned update reattaches a detached managed
clone to its default branch, then fetches `origin`, refreshes the remote's
default branch and hard-resets the clone to it. The managed clone is a build
input, so the reset discards local edits to tracked files and overwrites
untracked files that the suite now tracks, which a cache restored from an older
run can carry; do not keep work in it.

**A pin cannot be applied from inside a Whitaker checkout.** Checking out a
reference there would move the working tree, so the installer refuses and says
so. Run it from another directory, where it manages its own clone, or check out
the reference directly.

Pinning the installer and pinning the suite are separate decisions. The
installer's version selects the tool; `--suite-version` selects the lints it
builds.

### Version pinning through Cargo metadata

When Dylint is driven directly rather than through the installer, pin to a
specific release tag or commit:

```toml
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "whitaker_suite", tag = "v0.1.0" }
]
```

Or pin to a specific commit:

```toml
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "whitaker_suite", rev = "abc123def456" }
]
```

### Rolling release downloads

Whitaker publishes a `rolling` pre-release tag that is continuously updated and
overwritten on every push to `main`. It is intended for early adopters who want
the latest available build outputs before the next stable release is cut.

Rolling releases are best-effort builds. If some matrix legs fail, Whitaker
still publishes the artefacts that were built successfully. For example, a
target-specific `cargo-dylint` archive may be missing from one rolling release
even though other target archives were updated successfully. Do not assume that
every supported target is present in every rolling release.

This best-effort policy applies to ordinary pushes and unforced manual runs. A
forced manual dependency-binary rebuild is a recovery operation: it republishes
only after both five-target build matrices and all required lint, manifest,
dependency, and checksum assets pass validation. If that check fails, the
existing `rolling` release remains unchanged.

Stable releases differ from `rolling`: a stable tag is expected to contain the
complete artefact set for the release. For production installs, pin to a stable
release tag rather than consuming `rolling`.

The rolling lint archive holds `libwhitaker_suite` and the standalone libraries
of the nine standard lints. It holds no experimental lint, so
`string_continuation_style` is neither in the archive nor registered by the
published suite. Installing it needs `--experimental`, which builds from
source. (An earlier rolling release shipped it as a standalone library beside a
suite that already registered it, and the lint run failed with a
duplicate-specification error; see issue #565.)

Scripts or CI pipelines that consume rolling-release archives should verify
that the required target archive exists before proceeding. Treat missing
archives as an expected condition for rolling releases rather than assuming the
artefact set is complete.

### Refusing a source build

The installer prefers published binaries and falls back to building from source
when one is missing: a prebuilt lint library becomes a local compilation, and a
Dylint tool archive becomes `cargo install`. Both succeed, which is the
difficulty. A run that took either reports success while having built something
nobody pinned, and the only sign is a line in the log.

`--no-source-fallback`, or `WHITAKER_NO_SOURCE_FALLBACK` in the environment for
callers that cannot easily add a flag, turns each fallback into an error naming
the artefact and why it could not be fetched:

```sh
whitaker-installer --no-source-fallback
```

Any value other than an empty string, `0` or `false` enables the rule. The
comparison ignores case and surrounding whitespace, so `FALSE` and ` false `
also turn it off. Exporting the variable at all expresses an intention, and
reading an unrecognized value as off would quietly remove a protection; a value
that is not valid text enables the rule for the same reason.

The flag is rejected alongside `--build-only`, `--experimental` and
`--suite-version`. Each of those requires a source build, so combining them is
a contradiction rather than a preference, and the installer says so instead of
silently preferring one.

Each installation that selects a suite source prints the path it took on
standard output. A dry run selects none, so it prints nothing:

```text
whitaker-installer: suite-source=prebuilt
```

The value is `prebuilt` or `source`. It reports where the suite came from and
adds to the existing checks rather than replacing them: `install-whitaker`
still scans the installer's standard error for the fallback notice and fails an
unapproved source build on what it finds there. Reading only the marker would
let a source build the marker missed pass as `prebuilt`.

The streams are separate on purpose. The marker is the only thing on standard
output, so it can be parsed; the fallback notice and every other diagnostic go
to standard error.

The rolling release is the reason this exists. It is republished on every push
to `main`, and a consumer whose install starts during a republish can miss an
archive that is present moments later. In CI a source build is a defect rather
than a degraded mode, so fail and retry rather than proceeding.

### Selecting individual lints

To load specific lints instead of the full suite, specify each lint explicitly:

```toml
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "crates/module_max_lines" },
  { git = "https://github.com/leynos/whitaker", pattern = "crates/no_expect_outside_tests" }
]
```

### Standard vs Experimental Lints

Whitaker lints are divided into two categories:

- **Standard lints** are stable, well-tested, and included in the default suite.
  They are recommended for general use and have predictable behaviour.
- **Experimental lints** are newer or more aggressive checks that may produce
  false positives or undergo breaking changes between releases. They must be
  explicitly enabled.

The default `whitaker_suite` pattern includes only standard lints. Whitaker
currently ships two experimental lints, `rstest_helper_should_be_fixture` and
`string_continuation_style`, which are available only when experimental lints
are enabled.

### Enabling experimental lints

#### Via standalone installer

```sh
whitaker-installer --experimental
```

This enables experimental suite features when building `whitaker_suite`. To
build experimental lints as individual libraries, combine it with
`--individual-lints`. Explicit `--lint` requests for experimental lints also
require `--experimental`; without that opt-in the installer rejects the request
before building anything.

## Lint Configuration

Configure lint behaviour in `dylint.toml` at the workspace root:

```toml
# Diagnostic language (default: en-GB)
locale = "cy"

# Module size threshold (default: 400)
[module_max_lines]
max_lines = 500

# Conditional branch limit (default: 2)
[conditional_max_n_branches]
max_branches = 3

# Custom test attributes
[no_expect_outside_tests]
additional_test_attributes = ["my_framework::test", "wasm_bindgen_test"]

# Additional test markers for `test_must_not_have_example`
[test_must_not_have_example]
additional_test_attributes = ["actix_rt::test", "my_framework::test"]

# Allow panics in main
[no_unwrap_or_else_panic]
allow_in_main = true

# Experimental rstest fixture extraction lint
[rstest_helper_should_be_fixture]
min_calls = 2
min_distinct_tests = 2
require_identical_fixture_arg_names = false
provider_param_attributes = ["case", "values", "files", "future", "context"]
use_source_callee_fallback = false

# Source-line joins in string literals
[string_continuation_style]
applicability = "manual"
excluded_crates = ["my_cli_app"]
excluded_paths = ["my_app::legacy_text"]
```

### Planned Whitaker CLI configuration

The planned unified CLI uses `whitaker.toml` (or `.whitaker.toml`) for TOML
settings. The configuration foundation is available to the CLI integration; the
shipped `whitaker-installer` does not yet read these files. A selected
`--config` file takes precedence over `WHITAKER_CONFIG` for choosing the file.
Automatic discovery searches system, user, and project locations. If both
project filenames exist, `.whitaker.toml` overrides `whitaker.toml`. Missing
optional files are ignored. An explicitly selected missing or unreadable file
is an error; malformed files are errors whether selected or discovered. TOML
`extends` layers are supported.

Within a selected file chain, later layers override earlier layers. Effective
settings follow defaults, files, legacy environment values, `WHITAKER_*`
environment values, then explicit CLI values. For example:

```toml
[lint]
select = ["DEFAULT"]
ignore = []
experimental = false

[diagnostics]
locale = "en-GB"

[output]
colour = "auto"
progress = "plain"
```

Use `WHITAKER_LINT_SELECT` and `WHITAKER_LINT_IGNORE` for comma-separated
selectors, `WHITAKER_DIAGNOSTICS_LOCALE` for language, and
`WHITAKER_OUTPUT_COLOUR` or `WHITAKER_OUTPUT_PROGRESS` for presentation.
Boolean values use lower-case `true` or `false`; integer values are
non-negative, and floating-point values must be finite. Lists are
comma-separated: surrounding whitespace is trimmed and empty items are ignored.

Out-of-range tuning values keep the lints' existing normalization policy rather
than failing the load. A finite non-negative bumpy-road threshold is kept, and
any other value falls back to `2.5`; the smoothing window must be non-zero and
odd, so other values fall back to `3`; `min_bump_lines` is raised to at least
`1`; bumpy-road weights that are not finite and non-negative fall back to their
defaults; `max_branches` is raised to at least `1`; and `min_calls` and
`min_distinct_tests` are raised to at least `2`. Provider attribute names are
trimmed, lose a leading `rstest::`, and are deduplicated. Non-numeric values
and unknown keys remain errors. The existing `WHITAKER_NO_SOURCE_FALLBACK`
control forbids source builds for any value other than an empty string, `0`, or
`false` (ignoring surrounding whitespace and case); disabled values leave any
file setting unchanged. Non-Unicode values also forbid fallback, as in the
installer.

Command-specific settings use `[cmds.<command>]` and the corresponding
`WHITAKER_CMDS_<COMMAND>_*` environment prefix. The allowed presentation values
are `auto`, `always`, and `never` for colour, and `auto`, `plain`, and `off`
for progress.

For one release, when no canonical file is selected or found, the foundation
falls back to `dylint.toml` and maps its root `locale` key to
`diagnostics.locale`. An explicitly selected `dylint.toml` uses the same bridge.
`DYLINT_LOCALE` remains a lower-priority environment fallback. Both legacy
paths return a notice naming the replacement for the application to display.
The existing installer continues to use its own configuration and
source-fallback behaviour.

## Localized Diagnostics

Whitaker supports multiple languages for diagnostic messages. Set the locale
via the `DYLINT_LOCALE` environment variable or in `dylint.toml`:

```toml
locale = "cy"
```

Available locales:

- `en-GB` (default) - English
- `cy` - Welsh (Cymraeg)
- `gd` - Scottish Gaelic (Gàidhlig)

______________________________________________________________________

## Available Lints

### `bumpy_road_function`

#### Purpose <!-- bumpy_road_function -->

Detects functions with multiple distinct clusters of nested conditional
complexity.

#### Scope and behaviour <!-- bumpy_road_function -->

Flags a function when peak detection finds two or more separated complexity
regions above the configured threshold. Detection smooths the local complexity
signal with the configured `window` and only considers peaks spanning at least
`min_bump_lines`.

The defaults are a weighted smoothed score of at least `2.5`, a minimum of `2`
contiguous qualifying lines per bump and a centred moving-average window of
`3`. Depth contributes `1.0`, each predicate branch `0.5`, and match-arm
control flow `0.5`. Overlapping contributions add. Smoothing contracts at
function boundaries and can suppress short peaks or merge nearby regions; its
effect is not a fixed percentage reduction.

Direct `let` conditions contribute no predicate segment, including those in
`if let` and `while let`; their bodies can still contribute depth. A
`let ... else` declaration does not itself create a scored `if` region. In
`relative_to_manifest`, the loop's inner `if` reaches at most `1.5` on its
predicate line and `2.0` in its body, below the default `2.5` threshold even
before smoothing.

**CodeScene difference:** the current lint is a weighted heuristic, not a
CodeScene-equivalent structural detector. Two shallow nested regions can
produce no warning even when CodeScene reports Bumpy Road. The
`relative_to_manifest` reproduction and the proposed alignment strategy are in
[RFC 0013](https://github.com/leynos/whitaker/blob/main/docs/rfcs/rfc-0013-bumpy-road-semantic-alignment.md).
Semantic alignment is a project goal, recorded in
[ADR 0011](https://github.com/leynos/whitaker/blob/main/docs/adrs/adr-0011-bumpy-road-reference-semantics.md),
rather than a claim about the current implementation.

#### Configuration <!-- bumpy_road_function -->

```toml
[bumpy_road_function]
threshold = 2.5  # Raise to 3.0 or higher to reduce false positives
window = 3
min_bump_lines = 2
```

Lowering `threshold`, choosing `window = 1` to disable smoothing, or setting
`min_bump_lines = 1` can expose shorter or weaker peaks, but can also increase
false positives. These settings do not change predicate exclusions or establish
CodeScene parity. There is currently no structural strategy switch; the RFC's
proposed `strategy` key is not supported. Keep the defaults when reproducible
existing behaviour matters, and use CodeScene separately when its diagnostic is
the required authority.

#### What is allowed <!-- bumpy_road_function -->

- A single complexity peak in a function.
- Simple predicates that remain below the configured threshold.

#### What is denied <!-- bumpy_road_function -->

- Two or more separated complexity peaks above the configured threshold.

#### How to fix <!-- bumpy_road_function -->

Split complex regions into helper functions and simplify branch-heavy
predicates.

______________________________________________________________________

### `conditional_max_n_branches`

Limits the complexity of conditional predicates by enforcing a maximum number
of boolean branches.

**Configuration:**

```toml
[conditional_max_n_branches]
max_branches = 2
```

The default threshold is 2 branches. A predicate like `a && b && c` has three
branches and would trigger the lint.

**How to fix:** Extract complex conditions into helper functions:

```rust
// Before: Too many branches
if condition_a && condition_b && condition_c {
    // action
}

// After: Extract to helper function
fn should_proceed() -> bool {
    condition_a && condition_b && condition_c
}

if should_proceed() {
    // action
}
```

______________________________________________________________________

### `function_attrs_follow_docs`

<!-- markdownlint-disable-next-line MD024 -->
#### Purpose

Ensures doc comments appear before other outer attributes on functions,
methods, and trait methods.

<!-- markdownlint-disable-next-line MD024 -->
#### Scope and behaviour

When attributes are generated or reordered by a procedural macro (for example,
`rstest` or `derive`), the lint recovers the original source span from the
macro expansion chain. Attributes whose spans cannot be traced back to any
user-written source location (macro-only glue) are silently excluded from the
ordering check, so the lint never fires on compiler- or macro-generated code
that the developer cannot edit.

<!-- markdownlint-disable-next-line MD024 -->
#### Configuration

`function_attrs_follow_docs` has no configuration knobs.

<!-- markdownlint-disable-next-line MD024 -->
#### What is allowed

- Doc comments that appear before every other outer attribute on the same
  function, method, or trait method.
- Macro-generated attributes whose spans are excluded because they are
  macro-only.
- Inner attributes, which are outside the lint's scope.

<!-- markdownlint-disable-next-line MD024 -->
#### What is denied

- Outer attributes that appear before a doc comment on the same function,
  method, or trait method.
- Macro-expanded attributes that recover to a user-editable source span and
  sort before the doc comment.

<!-- markdownlint-disable-next-line MD024 -->
#### How to fix

Move doc comments so they appear before other outer attributes:

```rust
// Wrong
#[inline]
/// This function does something.
fn example() {}

// Correct
/// This function does something.
#[inline]
fn example() {}
```

With `rstest`, place the doc comment before all attributes, including the test
annotation:

```rust
// Wrong
#[rstest]
#[case(1, 2, 3)]
/// Verifies addition.
fn adds(#[case] a: i32, #[case] b: i32, #[case] expected: i32) {
    assert_eq!(a + b, expected);
}

// Correct
/// Verifies addition.
#[rstest]
#[case(1, 2, 3)]
fn adds(#[case] a: i32, #[case] b: i32, #[case] expected: i32) {
    assert_eq!(a + b, expected);
}
```

______________________________________________________________________

### `module_max_lines`

Warns when modules exceed a configurable line count threshold.

**Configuration:**

```toml
[module_max_lines]
max_lines = 400
```

**How to fix:** Split large modules into smaller, focused submodules.

______________________________________________________________________

### `module_must_have_inner_docs`

Enforces that every module begins with an inner documentation comment (`//!`).

**How to fix:**

```rust
mod my_module {
    //! Explain the module's purpose here.
    pub fn value() {}
}
```

______________________________________________________________________

### `no_expect_outside_tests`

<!-- markdownlint-disable-next-line MD024 -->
#### Purpose

Detect test attributes correctly so `no_expect_outside_tests` can allow
`.expect()` in recognized test-only code while still flagging production use.

<!-- markdownlint-disable-next-line MD024 -->
#### Scope and behaviour

Whitaker recognizes `#[test]`, prelude-qualified `#[test]` forms,
`#[tokio::test]`, `#[async_std::test]`, `#[gpui::test]`, `#[rstest]`,
`#[rstest::rstest]`, `#[rstest_parametrize]`, `#[rstest::rstest_parametrize]`,
`#[case]`, and `#[rstest::case]` by default. The `additional_test_attributes`
setting extends that matching list with project-specific markers, so the lint
treats those annotated functions as tests too.

Code that is compiled only under `#[cfg(test)]` is test-only as well. A helper
or fixture in a `#[cfg(test)] mod tests;` companion module, whether the module
is inline or file-backed, may call `.expect()`, and so may nested modules, free
functions, inherent and trait methods, and impl blocks that carry
`#[cfg(test)]`. A module that is merely named `tests` but is not gated on
`cfg(test)` is ordinary code, and `.expect()` in it is still reported. A
`cfg(not(test))` item is production code, and `any(test, ..)` counts only when
every alternative requires `cfg(test)`.

<!-- markdownlint-disable-next-line MD024 -->
#### Configuration

```toml
[no_expect_outside_tests]
additional_test_attributes = ["my_framework::test", "wasm_bindgen_test"]
```

Set `additional_test_attributes` to an array of attribute paths written as
strings. Each entry should match the path Whitaker sees on the test function,
for example `my_framework::test` or `wasm_bindgen_test`.

<!-- markdownlint-disable-next-line MD024 -->
#### Ancestor context propagation

`additional_test_attributes` now apply during ancestor context detection as
well as direct annotation matching. If a parent function is annotated with a
configured custom test attribute, Whitaker treats nested code within that
function as test context too, so `.expect()` remains allowed throughout that
ancestry chain.

```rust
// dylint.toml
// [no_expect_outside_tests]
// additional_test_attributes = ["my_framework::test"]

#[my_framework::test]
async fn my_test() {
    // Nesting the helper makes the annotated test an ancestor of its
    // `.expect()` call. A sibling helper has no such ancestry and is still
    // reported.
    fn helper() {
        let v: Option<u32> = Some(1);
        let _ = v.expect("value present"); // allowed — nested within the test
    }
    helper();
}
```

<!-- markdownlint-disable-next-line MD024 -->
#### What is allowed

- Default markers such as `#[test]`, `#[::test]`,
  `#[::std::prelude::v1::test]`, `#[tokio::test]`, `#[async_std::test]`,
  `#[gpui::test]`, `#[rstest]`, `#[rstest::rstest]`, `#[rstest_parametrize]`,
  `#[rstest::rstest_parametrize]`, `#[case]`, and `#[rstest::case]`
- Project-specific markers listed in `additional_test_attributes`, such as
  `#[wasm_bindgen_test]`

<!-- markdownlint-disable-next-line MD024 -->
#### What is denied

Functions using `.expect()` will still be flagged when their test attribute is
not in Whitaker's default list and is not listed in
`additional_test_attributes`.

<!-- markdownlint-disable-next-line MD024 -->
#### How to fix

- Add the missing test marker to `additional_test_attributes` if the function is
  genuinely part of a supported test framework
- Change the attribute usage to a recognized form such as `#[test]`,
  `#[::test]`, `#[::std::prelude::v1::test]`, `#[tokio::test]`,
  `#[async_std::test]`, `#[gpui::test]`, `#[rstest]`, `#[rstest::rstest]`,
  `#[rstest_parametrize]`, `#[rstest::rstest_parametrize]`, `#[case]`, or
  `#[rstest::case]` where appropriate
- If the function is not test-only code, replace `.expect()` with explicit error
  handling such as `?` or `map_err`

______________________________________________________________________

### `rstest_helper_should_be_fixture`

<!-- markdownlint-disable-next-line MD024 -->
#### Purpose

Bootstraps the experimental lint that will recommend converting repeated helper
calls inside `#[rstest]` tests into injected `#[fixture]` parameters.

<!-- markdownlint-disable-next-line MD024 -->
#### Scope and behaviour

This lint is experimental. The current implementation registers the lint, loads
configuration defaults, and passively collects local helper calls inside strict
`#[rstest]` tests, fingerprinting fixture-local, literal, `const`, and `static`
arguments for later aggregation. The lint remains diagnostic-silent: threshold
evaluation and actionable diagnostics are tracked by 8.2.3, while UI pass/fail
coverage is tracked by 8.2.4.

<!-- markdownlint-disable-next-line MD024 -->
#### Configuration

```toml
[rstest_helper_should_be_fixture]
min_calls = 2
min_distinct_tests = 2
require_identical_fixture_arg_names = false
provider_param_attributes = ["case", "values", "files", "future", "context"]
use_source_callee_fallback = false
```

`provider_param_attributes` lists `rstest` parameter attributes that should be
treated as data providers rather than fixture-local bindings. Entries may be
written either as bare names such as `case` or qualified names such as
`rstest::case`; Whitaker normalizes them to the shared detection policy.

<!-- markdownlint-disable-next-line MD024 -->
#### What is allowed

- Single-use helper calls inside `#[rstest]` tests
- Helper calls whose totals stay below `min_calls` or `min_distinct_tests`
- Parameter-provider uses covered by `provider_param_attributes`, such as
  `case`, `values`, `files`, `future`, and `context`

<!-- markdownlint-disable-next-line MD024 -->
#### What is denied

When diagnostic phases are implemented, `rstest_helper_should_be_fixture` will
deny repeated non-provider helper invocations across `#[rstest]` tests when
they meet or exceed both `min_calls` and `min_distinct_tests`. The
`require_identical_fixture_arg_names` setting controls whether candidate
fixture arguments must use the same names, and `use_source_callee_fallback`
controls whether source-callsite recovery may be used for macro-expanded callee
locations.

<!-- markdownlint-disable-next-line MD024 -->
#### How to fix

- Replace repeated helper calls with a shared `#[fixture]` parameter.
- If `require_identical_fixture_arg_names` is enabled, rename helper arguments
  so repeated calls use the same fixture argument names.
- Prefer provider attributes listed in `provider_param_attributes` for
  parameterized data inputs rather than modelling them as fixture-local helper
  calls.
- Tune `min_calls`, `min_distinct_tests`, and `use_source_callee_fallback` when
  repository conventions need stricter or looser matching.

______________________________________________________________________

### `test_must_not_have_example`

Warns when test function documentation includes example headings (for example
`# Examples`) or fenced code blocks.

**Configuration:**

```toml
[test_must_not_have_example]
additional_test_attributes = ["actix_rt::test", "my_framework::test"]
```

Use `additional_test_attributes` for frameworks not covered by default test
markers such as `#[test]`, `#[tokio::test]`, `#[async_std::test]`,
`#[gpui::test]`, and `#[rstest]`.

**How to fix:** Keep test docs focused on intent and assertions, and move
example/tutorial snippets into user-facing documentation.

```rust
// Before
#[test]
/// # Examples
/// ```rust
/// assert_eq!(sum(2, 2), 4);
/// ```
fn sums_values() { /* ... */ }

// After
#[test]
/// Verifies summation handles two positive integers.
fn sums_values() { /* ... */ }
```

______________________________________________________________________

### `no_std_fs_operations`

Enforces capability-based filesystem access by forbidding direct use of
`std::fs` operations.

**Configuration:**

```toml
[no_std_fs_operations]
excluded_crates = ["my_cli_entrypoint", "my_test_utilities"]
excluded_paths = ["my_app::legacy_io", "my_app::bin::migrate"]
```

The `excluded_crates` option allows specified crates to use `std::fs`
operations without triggering diagnostics. This is useful for:

- CLI entry points where ambient filesystem access is the intended boundary
- Test support utilities that manage fixtures with ambient access
- Build scripts or code generators that require direct filesystem operations

The `excluded_paths` option narrows an exclusion to individual modules rather
than a whole crate. Each entry is a fully qualified path anchored at the crate
identifier, and it matches on segment boundaries: `my_app::legacy_io` exempts
that module and everything nested beneath it (for example
`my_app::legacy_io::reader`), but never a sibling such as
`my_app::legacy_io_utils`. Reach for this when only a bounded corner of a crate
needs ambient filesystem access, while the rest stays under the capability
policy.

> **Note:** For both options, use Rust identifiers (underscores), not Cargo
> package names (hyphens). For example, use `my_cli_app` rather than
> `my-cli-app`, and `my_app::legacy_io` rather than `my-app::legacy_io`.
>
> **Note:** In-source `#[allow]` and `#[expect]` attributes are **not**
> honoured by this lint
> ([issue #270](https://github.com/leynos/whitaker/issues/270)). Annotating an
> item with `#[allow(no_std_fs_operations)]` leaves the diagnostic in place,
> and using `#[expect]` additionally reports an unfulfilled lint expectation
> for the same site. The lint emits through
> `LintContext::emit_span_lint`, which resolves the lint level at the
> visitor's current lint node rather than at the node that owns the offending
> code. Scope a suppression through `excluded_paths` instead; for a
> `which`-style PATH resolver, a single entry such as `my_app::paths` confines
> the exemption to the module that probes the system PATH.
>
> **Note:** A malformed `excluded_paths` entry is discarded rather than
> repaired, and a warning naming the rejected entry is logged under the
> `no_std_fs_operations` target. Malformed means an empty string, a bare
> `::`, or an entry with leading, trailing, or repeated separators. Rejection
> matters most for a trailing separator: `my_app::` would otherwise collapse
> to the crate-root prefix `my_app` and disable the lint across the whole
> crate, which is the opposite of the narrow, module-scoped exemption the
> entry was meant to express.

**How to fix:** Replace `std::fs` with `cap_std`:

```rust
// Before
use std::fs;
fn read_config() -> std::io::Result<String> {
    fs::read_to_string("config.toml")
}

// After
use cap_std::fs::Dir;
use camino::Utf8Path;
fn read_config(config_dir: &Dir, path: &Utf8Path) -> std::io::Result<String> {
    config_dir.read_to_string(path)
}
```

______________________________________________________________________

### `no_unwrap_or_else_panic`

Denies panicking `unwrap_or_else` fallbacks on `Option`/`Result`, including
tests. Doctest runs remain exempt.

**Configuration:**

```toml
[no_unwrap_or_else_panic]
allow_in_main = true
```

**What is allowed:**

- Panicking `unwrap_or_else` fallbacks inside doctests
- Panicking `unwrap_or_else` fallbacks inside `main` when
  `allow_in_main = true`
- `unwrap_or_else(|| panic!("value was {:?}", value))` inside test code when
  the closure interpolates a runtime value into the panic message
- Non-panicking `unwrap_or_else` fallbacks

**What is denied:**

- `unwrap_or_else(|| panic!(..))` outside tests, subject to the standard
  denial
- `unwrap_or_else(|| panic!(..))` in tests unless the `panic!` message
  interpolates runtime values
- `unwrap_or_else(|| panic!("static message"))` in tests; use
  `.expect("static message")` instead
- `unwrap_or_else(|| value.unwrap())`

**How to fix:** Propagate errors with `?` or use `.expect()` with a clear
message if a panic is truly intended. In tests, replace
`unwrap_or_else(|| panic!("msg"))` with `.expect("msg")` for clarity and
brevity, unless the closure needs to interpolate runtime state for a more
useful diagnostic. The rule denies only closures whose `panic!` message does
not meet the interpolated-only test exception. When the closure contains a
static string literal in tests, prefer `.expect("static message")`; only
interpolated-only `panic!` fallbacks are permitted there.

______________________________________________________________________

### `string_continuation_style`

This lint is experimental. It runs only when you opt in with `--experimental`.

Flags string literals that hide a fragment boundary inside a source-line join:

```rust
let message = "first fragment \
               second fragment";
```

The trailing backslash removes the newline and the following indentation, so
the reader sees two lines and the program sees one string. The lint suggests a
`concat!()` expression, which keeps the same value and makes the boundary
explicit:

```rust
let message = concat!("first fragment ", "second fragment");
```

The lint runs on rustc's expanded syntax tree, which is why it can see through
formatting macros to the literal the user actually typed.

**Configuration:**

```toml
[string_continuation_style]
applicability = "manual"
excluded_crates = ["my_cli_app"]
excluded_paths = ["my_app::legacy_text"]
```

| Key               | Default    | Meaning                                                                         |
| ----------------- | ---------- | ------------------------------------------------------------------------------- |
| `applicability`   | `"manual"` | `"manual"` offers the rewrite as a hint; `"machine"` lets `cargo fix` apply it. |
| `excluded_crates` | `[]`       | Crate names whose literals are never diagnosed.                                 |
| `excluded_paths`  | `[]`       | Module paths, and everything nested beneath them, never diagnosed.              |

`excluded_crates` and `excluded_paths` take Rust identifiers with underscores,
not Cargo package names with hyphens. An application whose output is
deliberately formatted as wrapped source text is the usual reason to exclude a
crate.

**What is diagnosed:**

- A cooked string literal containing one or more source-line joins, where the
  literal is not the argument of a macro with a matcher that could depend on
  its shape
- A join inside a format string whose arguments are all positional or
  explicitly named, so no implicit capture is lost, and which was passed
  through one of the compiler's built-in formatting macros. A format string
  reached through a lookalike or a facade macro of the same name is not
  diagnosed, even when every argument is positional or explicitly named
- Several joins in one literal produce one diagnostic and one whole-literal
  rewrite, rather than a cascade

**What is not diagnosed:**

- Raw strings, which do not interpret continuation escapes at all
- Byte strings and C strings, where `concat!()` would change the type
- Format strings containing any implicit capture (`{name}`), because
  `concat!()` would turn the format string into macro output and disable the
  capture
- A joined literal that ends or begins with layout whitespace, since the
  whitespace expresses source layout rather than a fragment boundary
- Format strings already generated by `concat!()`, `include_str!()` or another
  eager macro, since there is no source literal to rewrite
- Files outside the workspace root and generated files beneath the target
  directory, which the user cannot annotate

**Crate-wide opt-out:** if a crate defines its own `println!`, `format!` or
other name in the lint's known-macro set — by `macro_rules!`, `macro`, a `use`
that binds such a name, a glob import, or `#[macro_use] extern crate` — the
lint switches itself off for that crate. A bare name check is only sound when
the name cannot be shadowed. The alternative is diagnosing literals passed to a
user macro whose matcher the rewrite could break.

A `use` counts by the name it *binds*, not by the path it names:
`use helpers::println as shout` binds `shout` and leaves `println!` untouched.

Glob imports count because they are checked without name resolution: the lint
cannot tell whether `use some_crate::*` brings in a `concat`. If it does, the
suggested `concat!(...)` is ambiguous between the imported macro and the
built-in one, and rustc rejects the rewrite with `E0659` rather than applying
it. Treating a glob import as disqualifying is the conservative reading — it
can silence the lint for a crate that merely re-exports a prelude, and since
that silence is visible here, it is preferable to emitting a suggestion that
breaks the build.

Only a glob written in the crate counts. The compiler prepends
`use std::prelude::rust_20xx::*;` to every crate, and counting that one would
switch the lint off everywhere; it is recognized by its missing source
location, not by its path.

Diagnostics are localized like the rest of the suite. To trace why a literal
was or was not diagnosed, run with:

```sh
RUST_LOG=string_continuation_style=debug cargo dylint --all
```

## Clone Detection: AST Feature Extraction

Whitaker's experimental clone detector runs in two passes. Pass A is a token
scan over the workspace; Pass B lifts each candidate span into an abstract
syntax tree (AST) substrate that later scoring will consume. This release ships
the Pass B substrate only — it does not yet report clones (see below).

**What the substrate does.** Given a source file and a candidate byte range,
`lower_span` validates a non-empty, UTF-8-aligned `ByteSpan`, parses the
supplied source, and lowers the smallest syntax subtree that covers the span
into a parser-independent `NormalizedTree`. Working from the smallest covering
subtree keeps the representation tied to the candidate rather than the whole
file. Lowering stops when the selected subtree exceeds 256 levels of depth or
10,000 normalized nodes. Non-trivia token leaves count towards the node budget.
The operation returns `AstError::TreeTooDeep` or `AstError::TreeTooLarge` when
it exceeds either limit, so callers can handle oversized candidates without
accepting a partial tree.

**Feature vectors.** From a `NormalizedTree`, extraction derives a
deterministic set of features:

- exact counts of each syntax kind;
- dyadic fixed-point depth weighting (`2^63 >> depth`), so weights halve with
  depth and collapse to zero beyond depth 63;
- production bigrams and trigrams (parent-to-child and
  grandparent-to-parent-to-child kind sequences); and
- an opaque hexadecimal canonical hash of the normalized subtree.

**Schema-versioned hashes.** Every canonical hash mixes in
`PARSER_SCHEMA_VERSION`, which is tied to the pinned parser snapshot. A
parser-schema change therefore changes every hash by design, intentionally
invalidating persisted snapshots and caches so stale AST fingerprints are never
reused across incompatible parser versions.

**Parser feature.** `whitaker_clones_core` enables its exact-pinned
`ra_ap_syntax` parser adapter through the default `parser` feature. The feature
is on by default; building the crate with it disabled makes `lower_span` return
`AstError::ParserUnavailable` instead of lowering anything.

**Not yet emitted.** Type-3 clone scoring and SARIF Run 1 emission are deferred
to roadmap item 7.3.2. This release builds and exposes the AST substrate only;
it does not score clones or emit AST-based SARIF results.

Contributors maintaining the pinned parser should follow the
[`ra_ap_syntax` re-pinning runbook](https://github.com/leynos/whitaker/blob/main/docs/developers-guide.md#ra_ap_syntax-re-pinning-runbook)
in the Developer's Guide.

## Agent skills

Whitaker ships an agent-facing skill under
[`skills/addressing-whitaker-findings/`](https://github.com/leynos/whitaker/blob/main/skills/addressing-whitaker-findings/SKILL.md).
It distils the remediation patterns from adopting the suite across more than
forty repositories: what each lint expects, which fixes hold up under review,
when an exclusion is legitimate, and the traps that cost earlier adopters
rework. The skill is written for an agent rather than a human reader, and it
covers the lints described in [Available Lints](#available-lints).

The skill lives in the repository rather than in a separate package, so it
tracks the suite as the lints change.

A skill is a directory holding a `SKILL.md` whose YAML frontmatter is an Agent
Skills manifest. The manifest `name` is the discovery name a strict loader
uses, so a manifest that omits it leaves the skill undiscoverable. Every
shipped skill directory matches its manifest `name`, so one identifier names
the skill both on disk and at the point of discovery.

Making the skill available means copying its directory into the skills
directory that an agent tool reads. For Claude Code that directory is
`~/.claude/skills/`; other tools that consume Agent Skills use their own:

```sh
git clone --depth 1 https://github.com/leynos/whitaker.git /tmp/whitaker
mkdir -p ~/.claude/skills/
cp -R /tmp/whitaker/skills/addressing-whitaker-findings ~/.claude/skills/
```

The repository gates every shipped manifest, so a malformed one cannot reach a
copy taken from it: `make lint` runs `skill-frontmatter-lint`,
`skill-manifest-validate`, and `skill-metadata-check` over each manifest. The
[Developer's Guide](https://github.com/leynos/whitaker/blob/main/docs/developers-guide.md#skill-manifest-checks)
records how those targets behave and when to run them alone.
