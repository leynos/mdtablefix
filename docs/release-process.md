# Release Process

This project publishes prebuilt binaries for multiple operating systems and architectures.

The GitHub Actions workflow `release.yml` builds and uploads binaries for:

- Linux (x86_64 and aarch64)
- FreeBSD (x86_64 and aarch64)
- macOS (x86_64 and aarch64)
- Windows (x86_64 and aarch64)
- OpenBSD (x86_64 and aarch64)

Each binary is named using the pattern
`mdtablefix-<os>-<arch>` with an `.exe` suffix on Windows.

Binaries are uploaded as soon as they are built so they are available from the
workflow run while other targets build.
