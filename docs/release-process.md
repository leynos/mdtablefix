# Release Process

This project publishes prebuilt binaries for multiple operating systems and
architectures.

The GitHub Actions workflow `.github/workflows/release.yml` builds and uploads
binaries for:

- Linux (x86_64 and aarch64)
- FreeBSD (x86_64 and aarch64)
- macOS (x86_64 and aarch64)
- Windows (x86_64 and aarch64)
- OpenBSD (x86_64 and aarch64)

Each binary is named using the pattern `mdtablefix-<os>-<arch>` with an `.exe`
suffix on Windows.

Binaries are uploaded as soon as they are built, so they are available from the
workflow run while other targets build.

## Workflow details

The `release.yml` workflow defines a matrix of operating system and architecture
combinations. Each entry includes the target triple used by `cross` and a
filename extension for Windows. During the build job, `cross` compiles a release
binary for every matrix row.

`cross` is installed from a specific git tag to avoid unexpected behavior from
its main branch. Each binary is placed in an `artifacts/<os>-<arch>` directory
using the naming pattern `mdtablefix-<os>-<arch>[.exe]`. A SHA-256 checksum is
written alongside each binary for download verification.

After every build completes, the artifact is uploaded so that the GitHub Actions
interface provides it immediately. Once the matrix has finished, the `release`
job downloads all artifacts and uploads them to the GitHub release using
`gh release upload`.
