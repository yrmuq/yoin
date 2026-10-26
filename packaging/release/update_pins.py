# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Refresh the two pins Dependabot cannot see. Exit 2 when nothing changed."""

from __future__ import annotations

import hashlib
import json
import sys
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from pins import apply_build, apply_cache_key, apply_flatpak, python_choice, tool_choice

ROOT = Path(__file__).resolve().parents[2]
_AGENT = {"User-Agent": "yoin-pins", "Accept": "application/vnd.github+json"}


def _get(url: str) -> bytes:
    request = urllib.request.Request(url, headers=_AGENT)
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def _sha(url: str) -> str:
    return hashlib.sha256(_get(url)).hexdigest()


def main() -> int:
    releases = json.loads(
        _get("https://api.github.com/repos/astral-sh/python-build-standalone/releases?per_page=20")
    )
    python = python_choice(releases)
    tool = tool_choice(
        json.loads(_get("https://api.github.com/repos/AppImage/appimagetool/releases/latest"))
    )
    if python is None or tool is None:
        print("could not find both release assets", file=sys.stderr)
        return 1
    python_sha = _sha(python["url"])
    tool_sha = _sha(tool["url"])
    build = ROOT / "packaging/appimage/build.sh"
    flatpak = ROOT / "packaging/flatpak/io.github.yrmuq.yoin.yml"
    workflow = ROOT / ".github/workflows/appimage.yml"
    new_build = apply_build(build.read_text(encoding="utf-8"), python, python_sha, tool, tool_sha)
    new_flatpak = apply_flatpak(flatpak.read_text(encoding="utf-8"), python["url"], python_sha)
    new_workflow = apply_cache_key(workflow.read_text(encoding="utf-8"), python["tag"])
    if (
        new_build == build.read_text(encoding="utf-8")
        and new_flatpak == flatpak.read_text(encoding="utf-8")
        and new_workflow == workflow.read_text(encoding="utf-8")
    ):
        print("pins already current")
        return 2
    build.write_text(new_build, encoding="utf-8")
    flatpak.write_text(new_flatpak, encoding="utf-8")
    workflow.write_text(new_workflow, encoding="utf-8")
    print(f"cpython {python['tag']} appimagetool {tool['version']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
