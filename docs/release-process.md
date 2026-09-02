# Release Process

This project publishes prebuilt binaries for multiple operating systems and
architectures. It also publishes `cargo-binstall` archives for the supported
Linux release targets.

The project targets the stable Rust `1.89.0` toolchain, as specified in
`rust-toolchain.toml`.

This Minimum Supported Rust Version (MSRV) is also declared in `Cargo.toml`
(`rust-version = "1.89"`). The `build-test` job in `.github/workflows/ci.yml`
uses this toolchain to guard against regressions.

The GitHub Actions workflow `.github/workflows/release.yml` builds and uploads
binaries for:

- Linux (x86_64 and aarch64)
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

For Linux `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, the
workflow also produces `cargo-binstall` archives named
`mdtablefix-<version>-<target>.tar.gz`. Each archive contains the `mdtablefix`
binary at the archive root, matching the `Cargo.toml`
`[package.metadata.binstall]` configuration.

Binaries are uploaded as soon as they are built, so they are available from the
workflow run while other targets build.

## Workflow details

The `release.yml` workflow defines a matrix of operating system and
architecture combinations. Each entry includes the target triple used by
`cross` and whether the target also needs a `cargo-binstall` archive. During
the build job, `cross` compiles a release binary for every matrix row.

The workflow downloads `cross` 0.2.5 from its official release, verifies the
pinned archive SHA-256, and caches the extracted tools under a versioned
directory. It never compiles `cross` from source. Each release target has a
separate Cargo cache key so concurrent matrix jobs cannot stampede or overwrite
one another's target artefacts. The version probe runs from the runner's
temporary directory so the tagged source tree's `rust-toolchain.toml` cannot
trigger an unrelated toolchain installation before the release build. A local
shell helper owns that rule within the installation step; both cache validation
and post-installation validation use it so their working directories cannot
diverge. Release targets are likewise installed explicitly for the stable
toolchain used by `cross`, rather than inheriting the tagged tree's toolchain.

Each binary is placed in an `artifacts/<os>-<arch>` directory using the naming
pattern `mdtablefix-<os>-<arch>[.exe]`. An SHA-256 checksum is written
alongside each binary for download verification. The Linux `cargo-binstall`
targets also produce `mdtablefix-<version>-<target>.tar.gz` plus a matching
SHA-256 checksum.

Before the build matrix starts, a small job creates the GitHub release if it is
absent. Every successful matrix job uploads its workflow artefact and publishes
its files directly to the GitHub release. Matrix fail-fast is disabled, so one
target's failure cannot cancel another target or suppress assets that were
built successfully. On a retry, an identical existing release asset is reused.
Conflicting content fails the job without deleting or replacing the published
asset.
