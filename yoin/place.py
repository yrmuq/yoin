# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Ask KWin to place a window. Wayland clients cannot do this themselves."""

from __future__ import annotations

import os


def place_script(title: str, x: int, y: int, width: int, height: int) -> str:
    safe = title.replace("\\", "\\\\").replace('"', '\\"')
    x, y, width, height = int(x), int(y), int(width), int(height)
    return (
        "(function () {\n"
        "    function place(win) {\n"
        "        if (!win || String(win.caption).indexOf(\"%s\") < 0)\n"
        "            return;\n"
        "        var rect = Qt.rect(%d, %d, %d, %d);\n"
        "        win.frameGeometry = rect;\n"
        "    }\n"
        "    var list = workspace.windowList ? workspace.windowList() : workspace.clientList();\n"
        "    for (var i = 0; i < list.length; ++i)\n"
        "        place(list[i]);\n"
        "})();\n" % (safe, x, y, width, height)
    )


def move_kwin(title: str, x: int, y: int, width: int, height: int) -> bool:
    from PySide6.QtDBus import QDBusConnection, QDBusMessage

    cache = os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache")
    folder = os.path.join(cache, "yoin")
    os.makedirs(folder, exist_ok=True)
    path = os.path.join(folder, "place.js")
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(place_script(title, x, y, width, height))
    bus = QDBusConnection.sessionBus()
    script_id = _load(bus, path)
    if script_id is None:
        return False
    for object_path in (f"/Scripting/Script{script_id}", f"/{script_id}"):
        if _ok(bus, object_path, "run"):
            _ok(bus, object_path, "stop")
            return True
    return False


def _load(bus, path: str) -> int | None:
    from PySide6.QtDBus import QDBusMessage

    for args in ([path, "yoinplace"], [path]):
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
