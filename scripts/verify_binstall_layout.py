#!/usr/bin/env python3
"""Check staged release assets against the crate's cargo-binstall metadata.

``cargo binstall`` derives the asset URL from ``pkg-url`` and then looks inside
the downloaded archive at the path ``bin-dir`` renders to. Those two templates
live in ``Cargo.toml``; the archive is built by the release workflow. Nothing
connects them at build time, so this script renders both templates for a target
and asserts that the staged directory really contains what they promise.

It is the shared checker behind the release-packaging contract test and the
runner-backed packaging dry run in continuous integration.

Example:
    $ python3 scripts/verify_binstall_layout.py \\
        --artifact-dir artifacts/linux-x86_64 \\
        --target x86_64-unknown-linux-gnu
    mdtablefix-0.6.0-x86_64-unknown-linux-gnu.tar.gz -> mdtablefix
"""

from __future__ import annotations

import argparse
import hashlib
import re
import tarfile
import tomllib
import typing as typ
from pathlib import Path
from urllib.parse import urlsplit

# Binstall writes placeholders with padding, as in `{ binary-ext }`.
PLACEHOLDER = re.compile(r"\{\s*([A-Za-z0-9_-]+)\s*\}")


class LayoutError(RuntimeError):
    """Raised when the staged assets contradict the binstall metadata."""


def render(template: str, values: dict[str, str]) -> str:
    """Substitute binstall placeholders in ``template``.

    Examples:
        >>> render("{ name }-{ version }", {"name": "tool", "version": "1.0"})
        'tool-1.0'
    """

    def substitute(match: re.Match[str]) -> str:
        key = match.group(1)
        if key not in values:
            raise LayoutError(f"unsupported binstall placeholder: {match.group(0)}")
        return values[key]

    return PLACEHOLDER.sub(substitute, template)


def load_metadata(manifest: Path) -> tuple[dict[str, typ.Any], dict[str, typ.Any]]:
    """Return the ``[package]`` and ``[package.metadata.binstall]`` tables."""
    manifest_data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    package = manifest_data["package"]
    binstall = package.get("metadata", {}).get("binstall")
    if not binstall:
        raise LayoutError(f"{manifest} declares no [package.metadata.binstall]")
    if binstall.get("bin-dir") == ".":
        raise LayoutError("bin-dir '.' renders an empty source path")
    if binstall.get("overrides"):
        # Overrides are legitimate in general, but this crate publishes one
        # archive shape for every target, so an override would mean the single
        # rendered contract checked here no longer describes every platform.
        raise LayoutError("unexpected per-target binstall overrides")
    return package, binstall


def template_values(package: dict[str, typ.Any], target: str) -> dict[str, str]:
    """Build the placeholder table binstall would use for ``target``."""
    name = package["name"]
    return {
        "name": name,
        "bin": name,
        "version": package["version"],
        "repo": package["repository"].rstrip("/"),
        "target": target,
        "binary-ext": ".exe" if "windows" in target else "",
        "archive-format": "tgz",
        "archive-suffix": ".tar.gz",
        "binary-name": name,
    }


def digest_of(path: Path) -> str:
    """Return the lowercase hexadecimal SHA-256 digest of ``path``."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_checksum(asset: Path) -> None:
    """Fail unless ``asset`` has a sidecar naming it with a matching digest."""
    sidecar = asset.with_name(f"{asset.name}.sha256")
    if not sidecar.is_file():
        raise LayoutError(f"missing checksum sidecar: {sidecar.name}")
    fields = sidecar.read_text(encoding="utf-8").split()
    if len(fields) != 2 or fields[1] != asset.name:
        raise LayoutError(f"{sidecar.name} should name {asset.name} alone")
    if fields[0] != digest_of(asset):
        raise LayoutError(f"{sidecar.name} does not match {asset.name}")


def archive_member_names(archive: Path) -> list[str]:
    """Return the member names inside the gzip tarball at ``archive``."""
    with tarfile.open(archive, "r:gz") as opened:
        return opened.getnames()


def verify(artifact_dir: Path, manifest: Path, target: str) -> tuple[str, str]:
    """Verify one target's staged assets and return the archive and bin paths."""
    package, binstall = load_metadata(manifest)
    values = template_values(package, target)

    if binstall.get("pkg-fmt") != "tgz":
        raise LayoutError('this checker only understands pkg-fmt = "tgz"')

    url = render(binstall["pkg-url"], values)
    archive_name = Path(urlsplit(url).path).name
    archive = artifact_dir / archive_name
    if not archive.is_file():
        raise LayoutError(f"pkg-url names {archive_name}, which was not staged")
    verify_checksum(archive)

    bin_path = render(binstall["bin-dir"], values)
    members = archive_member_names(archive)
    if members != [bin_path]:
        raise LayoutError(
            f"{archive_name} holds {members}, but bin-dir resolves to {bin_path!r}"
        )
    return archive_name, bin_path


def parse_arguments(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse the command-line arguments for the verification entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--target", required=True, help="Rust target triple")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    """Verify the staged assets described by the command line."""
    arguments = parse_arguments(argv)
    try:
        archive_name, bin_path = verify(
            arguments.artifact_dir, arguments.manifest, arguments.target
        )
    except LayoutError as error:
        raise SystemExit(f"binstall layout check failed: {error}") from error
    print(f"{archive_name} -> {bin_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
