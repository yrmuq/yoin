# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""KWin keeps a small script loaded and pins the button window to the panel."""

from __future__ import annotations

import os
from pathlib import Path

_seq = 0


def popup_origin(tray: tuple[int, int, int, int], cursor: tuple[int, int], area: tuple[int, int, int, int], size: tuple[int, int]) -> tuple[int, int]:
    """Top-left of the button window. Tray wins, then a cursor sitting on a screen edge."""
    tx, ty, tw, th = tray
    cx, cy = cursor
    ax, ay, aw, ah = area
    width, height = size
    if tw > 1 and th > 1 and not _contains(tx + tw / 2, ty + th / 2, area):
        x = tx + tw // 2 - width // 2
        y = ty - height - 8
        if y < ay:
            y = ty + th + 8
    else:
        edge = _edge(cx, cy, area)
        if edge == "bottom":
            x = cx - width // 2
            y = cy - height - 8
        elif edge == "top":
            x = cx - width // 2
            y = cy + 8
        elif edge == "left":
            x = cx + 8
            y = cy - height // 2
        elif edge == "right":
            x = cx - width - 8
            y = cy - height // 2
        else:
            x = ax + aw - width - 8
            y = ay + ah - height - 8
    x = max(ax, min(int(x), ax + aw - width))
    y = max(ay, min(int(y), ay + ah - height))
    return x, y


def _contains(x: float, y: float, area: tuple[int, int, int, int]) -> bool:
    ax, ay, aw, ah = area
    return ax <= x < ax + aw and ay <= y < ay + ah


def _edge(x: int, y: int, area: tuple[int, int, int, int], margin: int = 64) -> str:
    ax, ay, aw, ah = area
    if y >= ay + ah - margin:
        return "bottom"
    if y <= ay + margin:
        return "top"
    if x <= ax + margin:
        return "left"
    if x >= ax + aw - margin:
        return "right"
    return ""


def placer_script() -> str:
    return Path(__file__).with_name("placer.js").read_text(encoding="utf-8")


def install_placer() -> bool:
    """Load the panel placer and leave it running. A one-shot script misses the window."""
    from PySide6.QtDBus import QDBusConnection, QDBusMessage

    global _seq
    cache = os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache")
    folder = os.path.join(cache, "yoin")
    os.makedirs(folder, exist_ok=True)
    id_path = os.path.join(folder, "placer.id")
    _stop_saved(id_path)
    path = os.path.join(folder, "placer.js")
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(placer_script())
    bus = QDBusConnection.sessionBus()
    _seq += 1
    script_id = _load(bus, path, f"yoinplace{_seq}")
    if script_id is None:
        return False
    for object_path in (f"/Scripting/Script{script_id}", f"/{script_id}"):
        if _ok(bus, object_path, "run"):
            with open(id_path, "w", encoding="utf-8") as handle:
                handle.write(str(script_id))
            return True
    return False


def _stop_saved(id_path: str) -> None:
    if not os.path.isfile(id_path):
        return
    try:
        script_id = int(Path(id_path).read_text(encoding="utf-8").strip())
    except ValueError:
        return
    _stop_id(script_id)


def _stop_id(script_id: int) -> None:
    from PySide6.QtDBus import QDBusConnection

    bus = QDBusConnection.sessionBus()
    for object_path in (f"/Scripting/Script{script_id}", f"/{script_id}"):
        if _ok(bus, object_path, "stop"):
            return


def _load(bus, path: str, plugin: str) -> int | None:
    from PySide6.QtDBus import QDBusMessage

    for args in ([path, plugin], [path]):
        reply = _call(bus, "/Scripting", "org.kde.kwin.Scripting", "loadScript", args)
        if reply.type() == QDBusMessage.MessageType.ErrorMessage:
            continue
        values = reply.arguments()
        if values and isinstance(values[0], int):
            return int(values[0])
    return None


def _ok(bus, path: str, method: str) -> bool:
    from PySide6.QtDBus import QDBusMessage

    for interface in ("org.kde.kwin.Script", "org.kde.kwin.Scripting"):
        reply = _call(bus, path, interface, method, [])
        if reply.type() != QDBusMessage.MessageType.ErrorMessage:
            return True
    return False


def _call(bus, path: str, interface: str, method: str, args: list):
    from PySide6.QtDBus import QDBusMessage

    message = QDBusMessage.createMethodCall("org.kde.KWin", path, interface, method)
    if args:
        message.setArguments(args)
    return bus.call(message)
