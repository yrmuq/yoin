# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Plasma notification history. Transient tray balloons are not kept."""

from __future__ import annotations

import os
import shutil


def notification_arguments(title: str, message: str, icon_path: str, timeout_ms: int = 8000) -> list:
    icon = _published_icon(icon_path)
    # Hints stay empty. A typed hint dict is what made PySide drop the call,
    # and an empty one still lands in the notification history.
    return ["yoin", 0, icon, title or "yoin", message or "", [], {}, int(timeout_ms)]


def announce(title: str, message: str, icon_path: str, timeout_ms: int = 8000) -> bool:
    try:
        from PySide6.QtDBus import QDBusConnection, QDBusInterface, QDBusMessage
    except ImportError:
        return False
    args = notification_arguments(title, message, icon_path, timeout_ms)
    bus = QDBusConnection.sessionBus()
    if not bus.isConnected():
        return False
    iface = QDBusInterface(
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
        bus,
    )
    if not iface.isValid():
        return False
    reply = iface.call("Notify", *args)
    if reply.type() == QDBusMessage.MessageType.ErrorMessage:
        return False
    values = reply.arguments()
    return bool(values) and isinstance(values[0], int)


def _published_icon(icon_path: str) -> str:
    """Copy the icon where plasmashell can read it. An AppImage mount is not."""
    if not icon_path or not os.path.isfile(icon_path):
        return ""
    source = os.path.abspath(icon_path)
    cache = os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache")
    folder = os.path.join(cache, "yoin")
    try:
        os.makedirs(folder, exist_ok=True)
        dest = os.path.join(folder, "notify.png")
        if not os.path.isfile(dest) or os.path.getmtime(source) > os.path.getmtime(dest):
            tmp = dest + ".tmp"
            shutil.copyfile(source, tmp)
            os.replace(tmp, dest)
        return dest
    except OSError:
        return source
