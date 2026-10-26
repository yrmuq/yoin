# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Ask KWin to place a window. Wayland clients cannot do this themselves."""

from __future__ import annotations

import os

_seq = 0
_current: int | None = None


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


def place_script(title: str, x: int, y: int, width: int, height: int) -> str:
    safe = title.replace("\\", "\\\\").replace('"', '\\"')
    x, y, width, height = int(x), int(y), int(width), int(height)
    anchor_x = x + width // 2
    anchor_y = y + height + 8
    return (
        "(function () {\n"
        "    var fallbackX = %d;\n"
        "    var fallbackY = %d;\n"
        "    var winW = %d;\n"
        "    var winH = %d;\n"
        "    var locked = null;\n"
        "    function spot() {\n"
        "        if (locked)\n"
        "            return locked;\n"
        "        var cx = fallbackX;\n"
        "        var cy = fallbackY;\n"
        "        if (workspace.cursorPos) {\n"
        "            cx = workspace.cursorPos.x;\n"
        "            cy = workspace.cursorPos.y;\n"
        "        }\n"
        "        locked = { x: cx, y: cy };\n"
        "        return locked;\n"
        "    }\n"
        "    function yoinPlaceOnce() {\n"
        "        var list = workspace.windowList ? workspace.windowList() : workspace.clientList();\n"
        "        var cursor = spot();\n"
        "        var found = false;\n"
        "        for (var i = 0; i < list.length; ++i) {\n"
        "            var win = list[i];\n"
        "            if (!win || String(win.caption).indexOf(\"%s\") < 0)\n"
        "                continue;\n"
        "            found = true;\n"
        "            var area = workspace.clientArea(KWin.PlacementArea, win);\n"
        "            var x = cursor.x - Math.round(winW / 2);\n"
        "            var y = cursor.y - winH - 12;\n"
        "            if (cursor.y < area.y + 64)\n"
        "                y = cursor.y + 16;\n"
        "            x = Math.max(area.x, Math.min(x, area.x + area.width - winW));\n"
        "            y = Math.max(area.y, Math.min(y, area.y + area.height - winH));\n"
        "            win.frameGeometry = Qt.rect(x, y, winW, winH);\n"
        "        }\n"
        "        return found;\n"
        "    }\n"
        "    var tries = 0;\n"
        "    function tick() {\n"
        "        tries += 1;\n"
        "        var found = false;\n"
        "        try { found = yoinPlaceOnce(); } catch (e) { found = false; }\n"
        "        if (tries > 25)\n"
        "            yoinPlaceTimer.stop();\n"
        "    }\n"
        "    yoinPlaceTimer = new QTimer();\n"
        "    yoinPlaceTimer.interval = 40;\n"
        "    yoinPlaceTimer.timeout.connect(tick);\n"
        "    yoinPlaceTimer.start();\n"
        "    tick();\n"
        "})();\n" % (anchor_x, anchor_y, width, height, safe)
    )


def move_kwin(title: str, x: int, y: int, width: int, height: int) -> bool:
    from PySide6.QtDBus import QDBusConnection, QDBusMessage

    global _current
    if _current is not None:
        _stop_id(_current)
        _current = None
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
            _current = script_id
            _stop_later(script_id)
            return True
    return False


def _stop_later(script_id: int) -> None:
    from PySide6.QtCore import QTimer
    from PySide6.QtWidgets import QApplication

    def finish(done: int = script_id) -> None:
        global _current
        if _current == done:
            _current = None
        _stop_id(done)

    if QApplication.instance() is None:
        return
    QTimer.singleShot(1500, finish)


def _stop_id(script_id: int) -> None:
    from PySide6.QtDBus import QDBusConnection

    bus = QDBusConnection.sessionBus()
    for object_path in (f"/Scripting/Script{script_id}", f"/{script_id}"):
        if _ok(bus, object_path, "stop"):
            return


def _load(bus, path: str) -> int | None:
    from PySide6.QtDBus import QDBusMessage

    global _seq
    _seq += 1
    plugin = f"yoinplace{_seq}"
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
