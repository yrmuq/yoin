# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

from __future__ import annotations

import json
import os
import socket
import sys
from pathlib import Path

AUTOSTART_NAME = "io.github.yrmuq.yoin.desktop"


def config_dir() -> Path:
    base = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    return base / "yoin"


def config_path() -> Path:
    return config_dir() / "config.json"


def autostart_path() -> Path:
    # Plasma reads the real home directory. Flatpak's XDG_CONFIG_HOME
    # is the sandbox and is not scanned at login.
    return Path.home() / ".config" / "autostart" / AUTOSTART_NAME


def hostname() -> str:
    name = socket.gethostname().split(".")[0].strip()
    return name or "yoin"


def defaults() -> dict:
    return {
        "url": "",
        "token": "",
        "device_name": "",
        "device_id": "",
        "webhook_id": "",
        "buttons": ["", ""],
        "autostart": False,
    }


def _buttons(value) -> list[str]:
    if not isinstance(value, list):
        value = []
    out = [item if isinstance(item, str) else "" for item in value[:2]]
    while len(out) < 2:
        out.append("")
    return out


def normalize(data: dict) -> dict:
    cfg = defaults()
    if not isinstance(data, dict):
        return cfg
    for key in ("url", "token", "device_name", "device_id", "webhook_id"):
        value = data.get(key)
        if isinstance(value, str):
            cfg[key] = value.strip()
    cfg["buttons"] = _buttons(data.get("buttons"))
    cfg["autostart"] = data.get("autostart") is True
    return cfg


def load_config(path: Path | None = None) -> dict:
    path = path or config_path()
    if not path.is_file():
        return defaults()
    try:
        raw = path.read_text(encoding="utf-8")
        data = json.loads(raw)
    except json.JSONDecodeError:
        broken = path.with_name("config.json.bak")
        path.replace(broken)
        return defaults()
    except OSError:
        return defaults()
    return normalize(data)


def save_config(cfg: dict, path: Path | None = None) -> None:
    path = path or config_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    os.chmod(path.parent, 0o700)
    payload = json.dumps(normalize(cfg), indent=2) + "\n"
    tmp = path.with_suffix(".json.tmp")
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as handle:
        handle.write(payload)
    os.replace(tmp, path)
    os.chmod(path, 0o600)


def format_exec(args: list[str]) -> str:
    out = []
    for arg in args:
        if any(ch in arg for ch in ' \t"'):
            out.append('"' + arg.replace('"', '\\"') + '"')
        else:
            out.append(arg)
    return " ".join(out)


def launch_args() -> list[str]:
    if os.environ.get("FLATPAK_ID"):
        return ["flatpak", "run", "io.github.yrmuq.yoin"]
    appimage = os.environ.get("APPIMAGE")
    if appimage:
        return [appimage]
    return [sys.executable, "-m", "yoin"]


def autostart_text(exec_line: str) -> str:
    return (
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Name=yoin\n"
        "Comment=Home Assistant buttons in the KDE tray\n"
        f"Exec={exec_line}\n"
        "Icon=yoin\n"
        "Terminal=false\n"
        "Categories=Utility;\n"
        "X-KDE-autostart-after=panel\n"
        "X-GNOME-Autostart-enabled=true\n"
    )


def apply_autostart(enabled: bool, path: Path | None = None) -> None:
    path = path or autostart_path()
    if not enabled:
        if path.is_file():
            path.unlink()
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    text = autostart_text(format_exec(launch_args()))
    path.write_text(text, encoding="utf-8")
    os.chmod(path, 0o644)
