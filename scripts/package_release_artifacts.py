#!/usr/bin/env python3
"""Stage the release assets for one target triple.

The release workflow runs this on Linux, macOS and Windows runners, so the
packaging cannot rely on GNU coreutils or GNU tar. Everything here uses the
standard library, and the archive is written with fixed member metadata and a
timestamp-free gzip header so that rebuilding a tag yields byte-identical
archives.

Two kinds of asset are produced in the staging directory:

``<package>-<os>-<arch>[.exe]``
    The bare binary, for people who download a single file.

``<package>-<version>-<target>.tar.gz``
    A ``cargo-binstall`` archive holding the binary at the archive root under
    its plain name, which is what the ``bin-dir`` template in
    ``[package.metadata.binstall]`` resolves to.

Each asset gets a ``.sha256`` sidecar in ``sha256sum`` format, naming the asset
alone so that ``sha256sum --check`` works in the directory holding the
download.

Example:
    $ python3 scripts/package_release_artifacts.py \\
        --binary target/x86_64-unknown-linux-gnu/release/mdtablefix \\
        --artifact-dir artifacts/linux-x86_64 \\
        --version 0.6.0 --target x86_64-unknown-linux-gnu \\
        --os linux --arch x86_64
    artifacts/linux-x86_64/mdtablefix-linux-x86_64
    artifacts/linux-x86_64/mdtablefix-linux-x86_64.sha256
    artifacts/linux-x86_64/mdtablefix-0.6.0-x86_64-unknown-linux-gnu.tar.gz
    artifacts/linux-x86_64/mdtablefix-0.6.0-x86_64-unknown-linux-gnu.tar.gz.sha256
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import os
import shutil
import tarfile
import tomllib
from pathlib import Path

DEFAULT_PACKAGE_NAME = "mdtablefix"
# Release binaries are executable for everybody and writable only by the owner,
# so the archive member mode cannot inherit a runner's umask.
BINARY_MODE = 0o755
CHUNK_SIZE = 1024 * 1024


def binary_extension(target: str) -> str:
    """Return the executable suffix Rust appends for ``target``.

    Examples:
        >>> binary_extension("x86_64-unknown-linux-gnu")
        ''
        >>> binary_extension("x86_64-pc-windows-msvc")
        '.exe'
    """
    return ".exe" if "windows" in target else ""


def digest_of(path: Path) -> str:
    """Return the lowercase hexadecimal SHA-256 digest of ``path``."""
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(CHUNK_SIZE), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def write_checksum(path: Path) -> Path:
    """Write ``<path>.sha256`` beside ``path`` and return the sidecar path.

    The sidecar names the asset without any directory component, so it stays
    valid wherever the pair is downloaded to.

    Examples:
        >>> import tempfile
        >>> with tempfile.TemporaryDirectory() as directory:
        ...     asset = Path(directory) / "asset.bin"
        ...     _ = asset.write_bytes(b"")
        ...     sidecar = write_checksum(asset)
        ...     sidecar.read_text(encoding="utf-8").split()[1]
        'asset.bin'
    """
    sidecar = path.with_name(f"{path.name}.sha256")
    sidecar.write_text(f"{digest_of(path)}  {path.name}\n", encoding="utf-8")
    return sidecar


def write_binstall_archive(
    binary: Path, archive_path: Path, member_name: str, source_date_epoch: int
) -> Path:
    """Write a deterministic single-member gzip tarball and return its path.

    Determinism comes from three places: fixed member metadata, an
    alphabetically trivial single-entry layout, and a gzip header that carries
    neither a timestamp nor the original file name.

    Examples:
        >>> import tempfile
        >>> with tempfile.TemporaryDirectory() as directory:
        ...     root = Path(directory)
        ...     binary = root / "tool"
        ...     _ = binary.write_bytes(b"binary")
        ...     archive = write_binstall_archive(
        ...         binary, root / "tool.tar.gz", "tool", 1_700_000_000
        ...     )
        ...     with tarfile.open(archive) as opened:
        ...         opened.getnames()
        ['tool']
    """
    payload = binary.read_bytes()
    info = tarfile.TarInfo(member_name)
    info.size = len(payload)
    info.mtime = source_date_epoch
    info.mode = BINARY_MODE
    info.type = tarfile.REGTYPE
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""

    with archive_path.open("wb") as raw:
        # ``filename=""`` and ``mtime=0`` keep the gzip header free of the
        # run's own metadata, which would otherwise differ on every rebuild.
        with gzip.GzipFile(
            filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=9
        ) as compressed:
            with tarfile.open(
                fileobj=compressed, mode="w", format=tarfile.GNU_FORMAT
            ) as archive:
                archive.addfile(info, io.BytesIO(payload))
    return archive_path


def stage_binary(binary: Path, artifact_dir: Path, asset_name: str) -> Path:
    """Copy ``binary`` into ``artifact_dir`` under ``asset_name``."""
    destination = artifact_dir / asset_name
    shutil.copyfile(binary, destination)
    destination.chmod(BINARY_MODE)
    return destination


def package(
    *,
    binary: Path,
    artifact_dir: Path,
    package_name: str,
    version: str,
    target: str,
    operating_system: str,
    arch: str,
    should_archive: bool,
    source_date_epoch: int,
) -> list[Path]:
    """Stage every asset for one target and return the written paths."""
    extension = binary_extension(target)
    artifact_dir.mkdir(parents=True, exist_ok=True)

    staged = stage_binary(
        binary, artifact_dir, f"{package_name}-{operating_system}-{arch}{extension}"
    )
    written = [staged, write_checksum(staged)]

    if should_archive:
        archive = write_binstall_archive(
            binary,
            artifact_dir / f"{package_name}-{version}-{target}.tar.gz",
            f"{package_name}{extension}",
            source_date_epoch,
        )
        written.extend([archive, write_checksum(archive)])
    return written


def manifest_version(manifest: Path) -> str:
    """Return the ``package.version`` declared by ``manifest``."""
    return tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]["version"]


def default_source_date_epoch() -> int:
    """Return ``SOURCE_DATE_EPOCH`` from the environment, or zero when unset."""
    raw = os.environ.get("SOURCE_DATE_EPOCH", "").strip()
    return int(raw) if raw else 0


def parse_arguments(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse the command-line arguments for the packaging entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--binary", type=Path, required=True, help="Built binary")
    parser.add_argument(
        "--artifact-dir", type=Path, required=True, help="Staging directory"
    )
    parser.add_argument("--package-name", default=DEFAULT_PACKAGE_NAME)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("Cargo.toml"),
        help="Manifest supplying the version when --version is omitted",
    )
    parser.add_argument(
        "--version",
        default=None,
        help="Release version without the leading 'v' (default: the manifest's)",
    )
    parser.add_argument("--target", required=True, help="Rust target triple")
    parser.add_argument("--os", dest="operating_system", required=True)
    parser.add_argument("--arch", required=True)
    parser.add_argument(
        "--archive",
        dest="should_archive",
        default=None,
        choices=("true", "false"),
        help="Whether to write the cargo-binstall archive (default: true)",
    )
    parser.add_argument(
        "--source-date-epoch",
        type=int,
        default=None,
        help="Archive member timestamp (default: $SOURCE_DATE_EPOCH, else 0)",
    )
    arguments = parser.parse_args(argv)
    arguments.should_archive = arguments.should_archive != "false"
    if arguments.version is None:
        arguments.version = manifest_version(arguments.manifest)
    if arguments.source_date_epoch is None:
        arguments.source_date_epoch = default_source_date_epoch()
    return arguments


def main(argv: list[str] | None = None) -> int:
    """Stage the assets described by the command line and list them."""
    arguments = parse_arguments(argv)
    if not arguments.binary.is_file():
        raise SystemExit(f"binary not found: {arguments.binary}")
    for path in package(
        binary=arguments.binary,
        artifact_dir=arguments.artifact_dir,
        package_name=arguments.package_name,
        version=arguments.version,
        target=arguments.target,
        operating_system=arguments.operating_system,
        arch=arguments.arch,
        should_archive=arguments.should_archive,
        source_date_epoch=arguments.source_date_epoch,
    ):
        print(path.as_posix())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
