# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Plasma notification history. Transient tray balloons are not kept."""

from __future__ import annotations

import ctypes
import os
import shutil


def notification_arguments(title: str, message: str, icon_path: str, timeout_ms: int = 8000) -> list:
    icon = _published_icon(icon_path)
    return ["yoin", 0, icon, title or "yoin", message or "", [], {"desktop-entry": "io.github.yrmuq.yoin"}, int(timeout_ms)]


def announce(title: str, message: str, icon_path: str, timeout_ms: int = 8000) -> bool:
    """Notify with signature susssasa{sv}i. PySide sends int32 and Plasma drops it."""
    if not os.environ.get("DBUS_SESSION_BUS_ADDRESS"):
        return False
    try:
        args = notification_arguments(title, message, icon_path, timeout_ms)
        return _send(args[3], args[4], args[2], args[7])
    except Exception:
        return False


def _send(title: str, message: str, icon: str, timeout_ms: int) -> bool:
    lib = _dbus()
    err = _Error()
    lib.dbus_error_init(ctypes.byref(err))
    conn = lib.dbus_bus_get(0, ctypes.byref(err))
    if not conn or lib.dbus_error_is_set(ctypes.byref(err)):
        lib.dbus_error_free(ctypes.byref(err))
        return False
    msg = lib.dbus_message_new_method_call(
        b"org.freedesktop.Notifications",
        b"/org/freedesktop/Notifications",
        b"org.freedesktop.Notifications",
        b"Notify",
    )
    held: list = []
    try:
        outer = _iter()
        lib.dbus_message_iter_init_append(msg, outer)
        _basic(lib, outer, "s", ctypes.c_char_p(b"yoin"), held)
        _basic(lib, outer, "u", ctypes.c_uint32(0), held)
        _basic(lib, outer, "s", ctypes.c_char_p(icon.encode()), held)
        _basic(lib, outer, "s", ctypes.c_char_p(title.encode()), held)
        _basic(lib, outer, "s", ctypes.c_char_p(message.encode()), held)
        actions = _iter()
        lib.dbus_message_iter_open_container(outer, ord("a"), b"s", actions)
        lib.dbus_message_iter_close_container(outer, actions)
        hints = _iter()
        lib.dbus_message_iter_open_container(outer, ord("a"), b"{sv}", hints)
        _hint(lib, hints, b"desktop-entry", b"s", ctypes.c_char_p(b"io.github.yrmuq.yoin"), held)
        _hint(lib, hints, b"urgency", b"y", ctypes.c_ubyte(1), held)
        lib.dbus_message_iter_close_container(outer, hints)
        _basic(lib, outer, "i", ctypes.c_int32(int(timeout_ms)), held)
        lib.dbus_error_init(ctypes.byref(err))
        reply = lib.dbus_connection_send_with_reply_and_block(conn, msg, 2000, ctypes.byref(err))
        if not reply or lib.dbus_error_is_set(ctypes.byref(err)):
            lib.dbus_error_free(ctypes.byref(err))
            return False
        lib.dbus_message_unref(reply)
        return True
    finally:
        if msg:
            lib.dbus_message_unref(msg)
        lib.dbus_connection_unref(conn)


def _hint(lib, hints, key: bytes, code: bytes, value, held: list) -> None:
    entry = _iter()
    var = _iter()
    lib.dbus_message_iter_open_container(hints, ord("e"), None, entry)
    _basic(lib, entry, "s", ctypes.c_char_p(key), held)
    lib.dbus_message_iter_open_container(entry, ord("v"), code, var)
    _basic(lib, var, code.decode(), value, held)
    lib.dbus_message_iter_close_container(entry, var)
    lib.dbus_message_iter_close_container(hints, entry)


def _basic(lib, iterator, code: str, value, held: list) -> None:
    held.append(value)
    lib.dbus_message_iter_append_basic(iterator, ord(code), ctypes.byref(value))


def _iter():
    return ctypes.create_string_buffer(256)


class _Error(ctypes.Structure):
    _fields_ = [("name", ctypes.c_char_p), ("message", ctypes.c_char_p), ("pad", ctypes.c_void_p * 4)]


_LIB = None


def _dbus():
    global _LIB
    if _LIB is not None:
        return _LIB
    lib = ctypes.CDLL("libdbus-1.so.3")
    lib.dbus_error_init.argtypes = [ctypes.c_void_p]
    lib.dbus_error_free.argtypes = [ctypes.c_void_p]
    lib.dbus_error_is_set.argtypes = [ctypes.c_void_p]
    lib.dbus_error_is_set.restype = ctypes.c_bool
    lib.dbus_bus_get.argtypes = [ctypes.c_int, ctypes.c_void_p]
    lib.dbus_bus_get.restype = ctypes.c_void_p
    lib.dbus_connection_unref.argtypes = [ctypes.c_void_p]
    lib.dbus_message_new_method_call.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]
    lib.dbus_message_new_method_call.restype = ctypes.c_void_p
    lib.dbus_message_unref.argtypes = [ctypes.c_void_p]
    lib.dbus_message_iter_init_append.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    lib.dbus_message_iter_append_basic.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]
    lib.dbus_message_iter_append_basic.restype = ctypes.c_bool
    lib.dbus_message_iter_open_container.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_void_p]
    lib.dbus_message_iter_open_container.restype = ctypes.c_bool
    lib.dbus_message_iter_close_container.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    lib.dbus_message_iter_close_container.restype = ctypes.c_bool
    lib.dbus_connection_send_with_reply_and_block.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]
    lib.dbus_connection_send_with_reply_and_block.restype = ctypes.c_void_p
    _LIB = lib
    return lib


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
