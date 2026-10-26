# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Labels from languages/*.ini. A line is `id = English original = text shown`."""

from __future__ import annotations

import os
from pathlib import Path

_phrases: dict[int, dict[str, str]] = {}
_name = "English"


def folder() -> Path:
    env = os.environ.get("YOIN_LANGUAGES")
    if env:
        return Path(env)
    return Path(__file__).resolve().parents[1] / "languages"


def languages() -> list[str]:
    """File names in languages/, without .ini. English stays first."""
    root = folder()
    names = []
    if root.is_dir():
        names = sorted(path.stem for path in root.glob("*.ini") if path.is_file())
    if "English" in names:
        names.remove("English")
    names.insert(0, "English")
    return names


def use(name: str) -> None:
    """Load one language file over the English originals. A missing file stays on English."""
    global _phrases, _name
    base = _read(_file("English"))
    chosen = str(name or "").strip() or "English"
    path = _file(chosen)
    if path is None:
        chosen = "English"
        overlay: dict[int, dict[str, str]] = {}
    else:
        overlay = _read(path)
    merged = {}
    for number in set(base) | set(overlay):
        original = base.get(number, {}).get("en") or overlay.get(number, {}).get("en") or ""
        shown = overlay.get(number, {}).get("text") if number in overlay else ""
        merged[number] = {"en": original, "text": shown or original}
    _name = chosen
    _phrases = merged


def current() -> str:
    return _name


def tr(number) -> str:
    """The label shown for this numeric id."""
    number = _number(number)
    row = _phrases.get(number)
    if row is None:
        return "" if number is None else str(number)
    return row["text"] or row["en"] or str(number)


def trf(number, *args) -> str:
    """Like tr, but keeps the English original when a translation drops a {}."""
    number = _number(number)
    row = _phrases.get(number)
    if row is None:
        return "" if number is None else str(number)
    original = row["en"]
    pattern = row["text"] or original
    if pattern.count("{}") != original.count("{}"):
        pattern = original
    try:
        return pattern.format(*args)
    except (IndexError, KeyError, ValueError):
        try:
            return original.format(*args)
        except (IndexError, KeyError, ValueError):
            return original or str(number)


def match_language(ui_languages) -> str:
    """Pick a language file from the system list. Unknown languages stay English."""
    codes: dict[str, str] = {}
    for name in languages():
        code = _code(name)
        if code and code not in codes:
            codes[code] = name
    for item in ui_languages or []:
        token = str(item).replace("-", "_").split(".")[0]
        lang = token.split("_")[0].lower()
        if lang in codes:
            return codes[lang]
    return "English"


def _number(value):
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _file(name: str):
    if not name or name in (".", "..") or "/" in name or "\\" in name:
        return None
    root = folder().resolve()
    path = (root / f"{name}.ini").resolve()
    if path.parent != root or not path.is_file():
        return None
    return path


def _code(name: str) -> str:
    path = _file(name)
    if path is None:
        return ""
    try:
        lines = path.read_text(encoding="utf-8-sig").splitlines()
    except OSError:
        return ""
    for line in lines:
        text = line.strip()
        if text.lower().startswith("; code ="):
            return text.split("=", 1)[1].strip().lower()
    return ""


def _read(path: Path | None) -> dict[int, dict[str, str]]:
    table: dict[int, dict[str, str]] = {}
    if path is None:
        return table
    try:
        lines = path.read_text(encoding="utf-8-sig").splitlines()
    except OSError:
        return table
    for line in lines:
        line = line.strip()
        if not line or line.startswith("#") or line.startswith(";") or line.startswith("["):
            continue
        if line.count("=") < 2:
            continue
        left, rest = line.split("=", 1)
        original, shown = rest.rsplit("=", 1)
        number = left.strip()
        if not number.isdigit():
            continue
        original = original.strip()
        shown = shown.strip()
        table[int(number)] = {"en": original, "text": shown or original}
    return table


use("English")
