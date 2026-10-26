# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Plasma notification history. Transient tray balloons are not kept."""

from __future__ import annotations

import os
from pathlib import Path


def notification_arguments(title: str, message: str, icon_path: str, timeout_ms: int = 8000) -> list:
    icon = os.path.abspath(icon_path) if icon_path else ""
    if not icon or not os.path.isfile(icon):
        icon = ""
    image = Path(icon).as_uri() if icon else ""
    hints = {
        "desktop-entry": "io.github.yrmuq.yoin",
        "image-path": image,
        "image_path": image,
    }
    return ["yoin", 0, icon, title, message, [], hints, int(timeout_ms)]


def announce(title: str, message: str, icon_path: str, timeout_ms: int = 8000) -> bool:
    from PySide6.QtCore import QVariant
    from PySide6.QtDBus import QDBusConnection, QDBusMessage, QDBusVariant

    args = notification_arguments(title, message, icon_path, timeout_ms)
    hints = {
        key: QDBusVariant(QVariant(value))
        for key, value in args[6].items()
        if value
    }
    message_bus = QDBusMessage.createMethodCall(
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
        "Notify",
    )
    message_bus.setArguments(args[:6] + [hints, args[7]])
    reply = QDBusConnection.sessionBus().call(message_bus)
    return reply.type() != QDBusMessage.MessageType.ErrorMessage
