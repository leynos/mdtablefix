# Release Process

This project publishes prebuilt binaries for multiple operating systems and
architectures. It also publishes a `cargo-binstall` archive for every release
target.

The project targets the stable Rust `1.89.0` toolchain, as specified in
`rust-toolchain.toml`.

This Minimum Supported Rust Version (MSRV) is also declared in `Cargo.toml`
(`rust-version = "1.89"`). The `build-test` job in `.github/workflows/ci.yml`
uses this toolchain to guard against regressions.

The GitHub Actions workflow `.github/workflows/release.yml` builds and uploads
binaries for:

- Linux (x86_64 and aarch64)
- macOS (x86_64 and aarch64)
- Windows (x86_64, MSVC)
- FreeBSD (x86_64)

Releases start from tags named `v<major>.<minor>.<patch>`. The workflow checks
that the tag's version, without the leading `v`, matches the `Cargo.toml`
`version` field and aborts if they differ.

Maintainers can also dispatch the workflow manually with an existing release
tag. This backfill path runs the current workflow definition but checks out and
builds the tagged source. Use it when a historical release is missing an asset;
it must not be used to rebuild a tag from different source.

Runs for the same release tag are serialized without cancellation, while runs
for different tags remain independent. Third-party actions in the release jobs
use immutable commit pins because those jobs can write release assets.

Each binary is named using the pattern `mdtablefix-<os>-<arch>` with an `.exe`
suffix on Windows.

Every target also produces a `cargo-binstall` archive named
`mdtablefix-<version>-<target>.tar.gz`. Each archive contains the `mdtablefix`
binary (`mdtablefix.exe` on Windows) at the archive root, which is what the
`bin-dir` template in the `Cargo.toml` `[package.metadata.binstall]` section
resolves to. That section carries no per-target overrides, so one `pkg-url`
covers Linux, macOS and Windows alike.

Binaries are uploaded as soon as they are built, so they are available from the
workflow run while other targets build.

## Workflow details

The `release.yml` workflow defines a matrix of operating system and
architecture combinations. Each entry names its runner, its builder and its
target triple. Every entry publishes a `cargo-binstall` archive, and
`tests/release_packaging.rs` fails if that set and the set of targets the
binstall metadata promises ever diverge. Linux and FreeBSD targets build with
`cross` on a GitHub-hosted Ubuntu runner. The macOS and Windows targets build
with Cargo on their own runner images, because `cross` cannot produce Apple or
MSVC binaries from Linux. Every step in the job runs under bash, which the job
pins as a default so Windows runners do not fall back to PowerShell.

The workflow downloads `cross` 0.2.5 from its official release, verifies the
pinned archive SHA-256, and caches the extracted tools under a versioned
directory. It never compiles `cross` from source. Each release target has a
separate Cargo cache key so concurrent matrix jobs cannot stampede or overwrite
one another's target artefacts. The version probe runs from the runner's
temporary directory so the tagged source tree's `rust-toolchain.toml` cannot
trigger an unrelated toolchain installation before the release build. A local
shell helper owns that rule within the installation step; both cache validation
and post-installation validation use it, so their working directories cannot
diverge. The helper compares the first version-output line because `cross`
appends host-Cargo fallback diagnostics when no package metadata is present,
but it preserves the command's status so a failed cached probe is replaced and
a failed post-installation probe stops the job. Release targets are likewise
installed explicitly for the version-pinned Rust 1.89.0 toolchain used by
`cross`, rather than inheriting the tagged tree's toolchain or a moving stable
channel. The Cargo cache key includes that compiler version so a toolchain
change cannot silently reuse incompatible build state.

`scripts/package_release_artifacts.py` stages the assets. It uses only the
Python standard library, because the three runner images do not agree on GNU
coreutils or GNU tar; `sha256sum` is absent on macOS, and `tar` is BSD tar on
macOS and Windows.

Each binary is placed in an `artifacts/<os>-<arch>` directory using the naming
pattern `mdtablefix-<os>-<arch>[.exe]`. An SHA-256 checksum is written
alongside each binary for download verification. The sidecar names the asset
alone, so `sha256sum --check` works in whatever directory the pair is
downloaded to. The `cargo-binstall` targets also produce
`mdtablefix-<version>-<target>.tar.gz` plus a matching SHA-256 checksum.
Publishing an archive for every target keeps the ungated `pkg-url` template
honest: there is no triple it resolves to that has no asset. These archives use
the tagged commit timestamp, normalized ownership and mode, and timestamp-free
gzip metadata. Rebuilding the same tag therefore produces byte-identical
archives instead of changing their compressed headers on every run.

## Guarding the cargo-binstall contract

Two checks stand between a metadata regression and a broken release.

`tests/release_packaging.rs` stages a stand-in binary for every published
target, then runs `scripts/verify_binstall_layout.py` over the result. That
script renders `pkg-url` and `bin-dir` from `Cargo.toml` and fails unless the
staged archive carries exactly that name and exactly that member. It rejects
`bin-dir = "."`, which renders an empty source path and makes `cargo binstall`
refuse the package, and it rejects per-target overrides, which would mean the
single checked contract no longer describes every platform. The same test
asserts that the release matrix builds every advertised target on a runner of
the matching family.

The `binstall-packaging` job in `.github/workflows/ci.yml` is the runner-backed
counterpart. On Ubuntu, macOS and Windows it builds a release target, stages
the assets with the same script, verifies the layout, extracts the archive, and
runs the extracted binary. It proves on every pull request that the crate
compiles for each release platform and that the packaging works there.

The job also builds `x86_64-apple-darwin` on the Apple silicon macOS runner.
That row sets `is-foreign`, which skips the step that runs the extracted
binary, because the runner cannot execute it. It is the only check that the
Intel cross-compile the release publishes still links.

Before the build matrix starts, a small job creates the GitHub release if it is
absent. Every successful matrix job uploads its workflow artefact and publishes
its files directly to the GitHub release. Matrix fail-fast is disabled, so one
target's failure cannot cancel another target or suppress assets that were
built successfully. On a retry, an identical existing release asset is reused.
Conflicting content fails the job without deleting or replacing the published
asset.
