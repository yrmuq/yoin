# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""X.Y.Z rules. Patch releases are the automatic ones."""

from __future__ import annotations

import re

_VERSION = re.compile(r"(\d+)\.(\d+)\.(\d+)")
_ASSIGN = re.compile(r'^__version__ = "\d+\.\d+\.\d+"$', re.M)


def parse_version(text: str) -> tuple[int, int, int]:
    match = _VERSION.fullmatch(text.strip())
    if match is None:
        raise ValueError(f"not a version: {text}")
    return int(match.group(1)), int(match.group(2)), int(match.group(3))


def format_version(parts: tuple[int, int, int]) -> str:
    return f"{parts[0]}.{parts[1]}.{parts[2]}"


def bump_patch(text: str) -> str:
    major, minor, patch = parse_version(text)
    return format_version((major, minor, patch + 1))


def bump_minor(text: str) -> str:
    major, minor, _patch = parse_version(text)
    return format_version((major, minor + 1, 0))


def is_prerelease(text: str) -> bool:
    return parse_version(text)[2] != 0


def replace_version(source: str, version: str) -> str:
    parse_version(version)
    updated, count = _ASSIGN.subn(f'__version__ = "{version}"', source, count=1)
    if count != 1:
        raise ValueError("version assignment missing")
    return updated


def main(argv: list[str]) -> int:
    import pathlib

    command = argv[1]
    if command == "patch":
        print(bump_patch(argv[2]))
        return 0
    if command == "minor":
        print(bump_minor(argv[2]))
        return 0
    if command == "write":
        path = pathlib.Path(argv[3])
        path.write_text(replace_version(path.read_text(encoding="utf-8"), argv[2]), encoding="utf-8")
        return 0
    raise SystemExit(f"unknown command {command}")


if __name__ == "__main__":
    import sys

    raise SystemExit(main(sys.argv))
