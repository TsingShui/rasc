#!/usr/bin/env python3
"""Synchronize the workspace package versions for a manually requested release."""

import re
import sys
import tomllib
from pathlib import Path


VERSION_RE = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\Z")
MANIFESTS = (
    (Path("Cargo.toml"), "rasc"),
    (Path("crates/dexdec/Cargo.toml"), "dexdec"),
    (Path("crates/rusty-dex/Cargo.toml"), "rusty-dex"),
)


def version_parts(version):
    match = VERSION_RE.fullmatch(version)
    if not match:
        raise ValueError("version must be a stable X.Y.Z version without a v prefix")
    return tuple(int(part) for part in match.groups())


def replace_package_version(text, version, path):
    section = re.search(r"(?ms)^\[package\][ \t]*\r?\n.*?(?=^\[|\Z)", text)
    if not section:
        raise ValueError(f"{path}: missing [package] section")
    updated, count = re.subn(
        r'(?m)^([ \t]*version[ \t]*=[ \t]*")[^"\r\n]*(")',
        lambda match: match.group(1) + version + match.group(2),
        section.group(),
    )
    if count != 1:
        raise ValueError(f"{path}: expected exactly one package version")
    return text[: section.start()] + updated + text[section.end() :]


def prepare(version):
    requested = version_parts(version)
    contents = {path: path.read_text(encoding="utf-8") for path, _ in MANIFESTS}
    manifests = {path: tomllib.loads(text) for path, text in contents.items()}
    current = manifests[MANIFESTS[0][0]]["package"]["version"]
    if requested < version_parts(current):
        raise ValueError(f"version {version} is older than the current version {current}")

    for path, name in MANIFESTS:
        package = manifests[path]["package"]
        if package.get("name") != name or package.get("version") != current:
            raise ValueError(f"{path}: expected package {name} at version {current}")
    dexdec = Path("crates/dexdec/Cargo.toml")
    dependency = manifests[dexdec]["dependencies"]["rusty-dex"]
    if dependency.get("version") != current:
        raise ValueError(f"{dexdec}: rusty-dex dependency is not at version {current}")

    updated = {
        path: replace_package_version(contents[path], version, path)
        for path, _ in MANIFESTS
    }
    updated[dexdec], count = re.subn(
        r'(?m)^([ \t]*rusty-dex[ \t]*=[ \t]*\{[^}\r\n]*\bversion[ \t]*=[ \t]*")[^"\r\n]*(")',
        lambda match: match.group(1) + version + match.group(2),
        updated[dexdec],
    )
    if count != 1:
        raise ValueError(f"{dexdec}: expected one inline rusty-dex version")

    # Validate all inputs before writing anything; the lockfile is updated by Cargo.
    for path, text in updated.items():
        if text != contents[path]:
            path.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: prepare_version.py X.Y.Z")
    try:
        prepare(sys.argv[1])
    except (KeyError, OSError, TypeError, ValueError) as error:
        sys.exit(f"cannot prepare release: {error}")
