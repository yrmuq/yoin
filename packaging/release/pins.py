# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Pick the CPython and appimagetool pins written into the build."""

from __future__ import annotations

import re

_PY_ASSET = re.compile(
    r"^cpython-3\.12\.\d+\+\d{8}-x86_64-unknown-linux-gnu-install_only_stripped\.tar\.gz$"
)
_SHA = re.compile(r"^[0-9a-f]{64}$")


def python_url(tag: str, name: str) -> str:
    if not tag.isdigit() or len(tag) != 8 or _PY_ASSET.fullmatch(name) is None:
        raise ValueError("bad cpython asset")
    return (
        "https://github.com/astral-sh/python-build-standalone/releases/download/"
        + tag
        + "/"
        + name.replace("+", "%2B")
    )


def python_choice(releases: list) -> dict | None:
    best: dict | None = None
    for release in releases:
        if not isinstance(release, dict) or release.get("draft") or release.get("prerelease"):
            continue
        tag = str(release.get("tag_name") or "")
        if not tag.isdigit() or len(tag) != 8:
            continue
        assets = release.get("assets")
        if not isinstance(assets, list):
            continue
        for asset in assets:
            if not isinstance(asset, dict):
                continue
            name = str(asset.get("name") or "")
            if _PY_ASSET.fullmatch(name) is None:
                continue
            if best is None or tag > best["tag"]:
                best = {"tag": tag, "name": name, "url": python_url(tag, name)}
    return best


def local_python_tar(name: str) -> str:
    if _PY_ASSET.fullmatch(name) is None:
        raise ValueError("bad cpython asset")
    return re.sub(r"\+\d{8}-x86_64-unknown-linux-gnu", "", name)


def tool_choice(release: dict) -> dict | None:
    if not isinstance(release, dict) or release.get("draft") or release.get("prerelease"):
        return None
    tag = str(release.get("tag_name") or "").removeprefix("v")
    if re.fullmatch(r"\d+\.\d+\.\d+", tag) is None:
        return None
    assets = release.get("assets")
    if not isinstance(assets, list):
        return None
    for asset in assets:
        if isinstance(asset, dict) and asset.get("name") == "appimagetool-x86_64.AppImage":
            return {
                "version": tag,
                "url": "https://github.com/AppImage/appimagetool/releases/download/"
                + tag
                + "/appimagetool-x86_64.AppImage",
            }
    return None


def _assign(text: str, key: str, value: str) -> str:
    updated, count = re.subn(
        rf'^{re.escape(key)}="[^"]*"$',
        f'{key}="{value}"',
        text,
        count=1,
        flags=re.M,
    )
    if count != 1:
        raise ValueError(f"missing {key}")
    return updated


def apply_build(text: str, python: dict, python_sha: str, tool: dict, tool_sha: str) -> str:
    if _SHA.fullmatch(python_sha) is None or _SHA.fullmatch(tool_sha) is None:
        raise ValueError("bad sha256")
    text = _assign(text, "PY_URL", python["url"])
    text = _assign(text, "PY_SHA", python_sha)
    text = _assign(text, "PY_TAR", "$CACHE/" + local_python_tar(python["name"]))
    text = _assign(text, "TOOL_URL", tool["url"])
    return _assign(text, "TOOL_SHA", tool_sha)


def apply_flatpak(text: str, url: str, sha: str) -> str:
    if _SHA.fullmatch(sha) is None or not url.startswith(
        "https://github.com/astral-sh/python-build-standalone/releases/download/"
    ):
        raise ValueError("bad flatpak pin")
    text, count = re.subn(
        r"^        url: https://github.com/astral-sh/python-build-standalone/\S+$",
        f"        url: {url}",
        text,
        count=1,
        flags=re.M,
    )
    if count != 1:
        raise ValueError("missing flatpak url")
    text, count = re.subn(
        r"^        sha256: [0-9a-f]{64}$",
        f"        sha256: {sha}",
        text,
        count=1,
        flags=re.M,
    )
    if count != 1:
        raise ValueError("missing flatpak sha")
    return text


def apply_cache_key(text: str, tag: str) -> str:
    if not tag.isdigit() or len(tag) != 8:
        raise ValueError("bad cache tag")
    updated, count = re.subn(
        r"^          key: yoin-appimage-deps-\d{8}$",
        f"          key: yoin-appimage-deps-{tag}",
        text,
        count=1,
        flags=re.M,
    )
    if count != 1:
        raise ValueError("missing cache key")
    return updated
