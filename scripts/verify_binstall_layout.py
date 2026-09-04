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
    """Raised when the staged assets contradict the binstall metadata.

    The message names the asset and the template it failed to satisfy, so a
    failing release job reports which of the two sides drifted.
    """


def render(template: str, values: dict[str, str]) -> str:
    """Substitute binstall placeholders in a template string.

    Parameters
    ----------
    template : str
        A ``pkg-url`` or ``bin-dir`` template, whose placeholders may carry
        padding, as in ``{ binary-ext }``.
    values : dict of str to str
        The substitutions to apply, keyed by placeholder name.

    Returns
    -------
    str
        The template with every placeholder replaced.

    Raises
    ------
    LayoutError
        If the template names a placeholder absent from ``values``.

    Examples
    --------
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
    """Return the package and binstall tables from a Cargo manifest.

    Parameters
    ----------
    manifest : Path
        The ``Cargo.toml`` to read.

    Returns
    -------
    tuple of dict
        The ``[package]`` table and the ``[package.metadata.binstall]`` table.

    Raises
    ------
    LayoutError
        If the manifest declares no binstall metadata, if ``bin-dir`` is
        ``"."``, or if the metadata carries per-target overrides.
    OSError
        If the manifest cannot be read.
    """
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
    """Build the placeholder table binstall would use for a target.

    Parameters
    ----------
    package : dict
        The manifest's ``[package]`` table.
    target : str
        The Rust target triple the templates are rendered for.

    Returns
    -------
    dict of str to str
        Every placeholder this checker understands, including the Windows-only
        ``binary-ext``.

    Raises
    ------
    KeyError
        If the package table omits a name, version or repository.
    """
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
    """Return the SHA-256 digest of a file.

    Parameters
    ----------
    path : Path
        The file to read. Release assets are small enough to read whole.

    Returns
    -------
    str
        The digest as lowercase hexadecimal.

    Raises
    ------
    OSError
        If the file cannot be read.
    """
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_checksum(asset: Path) -> None:
    """Check that an asset's checksum sidecar is present and correct.

    Parameters
    ----------
    asset : Path
        The staged asset whose ``.sha256`` sidecar is checked.

    Returns
    -------
    None
        Nothing. The check reports failure by raising.

    Raises
    ------
    LayoutError
        If the sidecar is missing, does not name the asset alone, or records a
        digest that does not match the asset.
    OSError
        If the asset or the sidecar cannot be read.
    """
    sidecar = asset.with_name(f"{asset.name}.sha256")
    if not sidecar.is_file():
        raise LayoutError(f"missing checksum sidecar: {sidecar.name}")
    fields = sidecar.read_text(encoding="utf-8").split()
    if len(fields) != 2 or fields[1] != asset.name:
        raise LayoutError(f"{sidecar.name} should name {asset.name} alone")
    if fields[0] != digest_of(asset):
        raise LayoutError(f"{sidecar.name} does not match {asset.name}")


def archive_member_names(archive: Path) -> list[str]:
    """Return the member names inside a gzip tarball.

    Parameters
    ----------
    archive : Path
        The ``.tar.gz`` to inspect.

    Returns
    -------
    list of str
        The member names, in archive order.

    Raises
    ------
    tarfile.TarError
        If the archive cannot be opened or read.
    """
    with tarfile.open(archive, "r:gz") as opened:
        return opened.getnames()


def verify(artifact_dir: Path, manifest: Path, target: str) -> tuple[str, str]:
    """Check one target's staged assets against the binstall templates.

    Parameters
    ----------
    artifact_dir : Path
        The staging directory holding the archive and its sidecar.
    manifest : Path
        The ``Cargo.toml`` supplying ``pkg-url``, ``bin-dir`` and ``pkg-fmt``.
    target : str
        The Rust target triple the templates are rendered for.

    Returns
    -------
    tuple of str
        The archive name and the archive-relative binary path, both as
        rendered from the manifest.

    Raises
    ------
    LayoutError
        If the format is not ``tgz``, the archive named by ``pkg-url`` is
        absent, its checksum sidecar is wrong, or its members are not exactly
        the single path ``bin-dir`` renders to.
    """
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
    """Parse the command line for the verification entry point.

    Parameters
    ----------
    argv : list of str, optional
        Arguments to parse. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    argparse.Namespace
        The parsed arguments.

    Raises
    ------
    SystemExit
        If the arguments are invalid, as raised by :mod:`argparse`.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--target", required=True, help="Rust target triple")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    """Check the staged assets described by the command line.

    Parameters
    ----------
    argv : list of str, optional
        Arguments to parse. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        Zero on success, having printed the archive name and the binary path
        it contains.

    Raises
    ------
    SystemExit
        If the staged assets contradict the manifest's binstall metadata.
    """
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
