# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

from __future__ import annotations

import os
import sys
import uuid

from PySide6.QtCore import QLockFile, QTimer
from PySide6.QtGui import QIcon
from PySide6.QtWidgets import QApplication, QMenu, QSystemTrayIcon

from yoin.client import HaClient
from yoin.config import (
    apply_autostart,
    hostname,
    load_config,
    save_config,
    config_dir,
)
from yoin.ui import Popup, Settings, asset_path, labels_for, ons_for


class Controller:
    def __init__(self, cfg: dict, icon: QIcon) -> None:
        self.cfg = cfg
        self.known: dict[str, dict] = {}
        self._last_error = ""
        self._busy: set[str] = set()
        self.client = HaClient()
        self.settings = Settings()
        self.popup = Popup()
        self.tray = QSystemTrayIcon(icon)
        self.tray.setToolTip("yoin")
        menu = QMenu()
        menu.addAction("Settings", self.open_settings)
        menu.addSeparator()
        menu.addAction("Quit", QApplication.quit)
        self.tray.setContextMenu(menu)
        self.tray.activated.connect(self._activated)
        self.popup.triggered.connect(self._pressed)
        QApplication.instance().installEventFilter(self.popup)
        self.settings.connect_requested.connect(self._connect)
        self.settings.save_requested.connect(self._save)
        self.client.link.connect(self._link)
        self.client.notify.connect(self._notify)
        self.client.entities_ready.connect(self._entities)
        self.client.entity_state.connect(self._entity_state)
        self.client.failed.connect(self._failed)
        self.client.registered.connect(self._registered)
        self.client.service_done.connect(self._service_done)

    def start(self) -> None:
        self.tray.show()
        if not self.cfg["url"] or not self.cfg["token"]:
            self.open_settings()
            return
        if not self.cfg["device_id"]:
            self.cfg["device_id"] = uuid.uuid4().hex
            save_config(self.cfg)
        self._start_client()

    def open_settings(self) -> None:
        if not self.settings.device.text() and not self.cfg["device_name"]:
            self.cfg["device_name"] = hostname()
        self.settings.fill(self.cfg)
        if self.known:
            self.settings.set_entities(list(self.known.values()), self.cfg["buttons"])
        self.settings.show()
        self.settings.raise_()
        self.settings.activateWindow()

    def _connect(self) -> None:
        self._store(close=False)

    def _save(self) -> None:
        self._store(close=True)

    def _store(self, close: bool) -> None:
        values = self.settings.values()
        if not values["device_name"]:
            values["device_name"] = hostname()
            self.settings.device.setText(values["device_name"])
        if not values["url"] or not values["token"]:
            self.settings.set_status("Address and token are required")
            return
        changed = (
            values["url"] != self.cfg["url"]
            or values["token"] != self.cfg["token"]
            or values["device_name"] != self.cfg["device_name"]
        )
        self.cfg["url"] = values["url"]
        self.cfg["token"] = values["token"]
        self.cfg["device_name"] = values["device_name"]
        self.cfg["buttons"] = values["buttons"]
        self.cfg["autostart"] = values["autostart"]
        if not self.cfg["device_id"]:
            self.cfg["device_id"] = uuid.uuid4().hex
        if changed:
            self.cfg["webhook_id"] = ""
        save_config(self.cfg)
        try:
            apply_autostart(self.cfg["autostart"])
        except OSError as exc:
            self.settings.set_status(f"Could not write autostart: {exc}")
            return
        self._refresh_popup()
        self.settings.set_busy(True)
        self.settings.set_status("Connecting…")
        self._start_client()
        if close:
            self.settings.hide()

    def _start_client(self) -> None:
        self.client.start(
            self.cfg["url"],
            self.cfg["token"],
            self.cfg["webhook_id"],
            self.cfg["device_id"],
            self.cfg["device_name"],
        )

    def _activated(self, reason) -> None:
        if reason != QSystemTrayIcon.ActivationReason.Trigger:
            return
        QTimer.singleShot(0, self._toggle_popup)

    def _toggle_popup(self) -> None:
        mapped = self.popup.is_mapped()
        self._close_popup()
        if mapped:
            return
        self._refresh_popup()
        self._show_popup(self.tray.geometry())

    def _show_popup(self, geo) -> None:
        if self.popup.is_mapped():
            return
        self.popup.setParent(None)
        self.popup.place(geo)
        self.popup.show()
        self.popup.raise_()
        self.popup.activateWindow()
        handle = self.popup.windowHandle()
        if handle is not None:
            handle.setTransientParent(None)
        self.popup.place(geo)

    def _close_popup(self) -> None:
        self.popup.dismiss()
        QTimer.singleShot(0, self.popup.drop_window)

    def _pressed(self, index: int) -> None:
        buttons = self.cfg["buttons"]
        entity = buttons[index] if index < len(buttons) else ""
        # Hide after the mouse release. Hiding a Popup inside the click keeps the grab.
        QTimer.singleShot(0, lambda: self._finish_press(entity))

    def _finish_press(self, entity: str) -> None:
        self._refresh_popup()
        if not entity:
            self.open_settings()
            return
        if entity in self._busy:
            return
        self._busy.add(entity)
        self.client.run(entity)

    def _service_done(self, entity: str, _ok: bool) -> None:
        self._busy.discard(entity)

    def _link(self, up: bool) -> None:
        if not up:
            if self.tray.toolTip() == "yoin":
                self.tray.setToolTip("yoin — disconnected")
            return
        self._last_error = ""
        self.tray.setToolTip("yoin")
        self.settings.set_busy(False)
        self.settings.set_status("Connected")

    def _notify(self, title: str, message: str) -> None:
        self.tray.showMessage(title, message, QSystemTrayIcon.MessageIcon.Information, 8000)

    def _entities(self, entities: list) -> None:
        self.known = {item["entity_id"]: item for item in entities}
        self.settings.set_entities(entities, self.cfg["buttons"])
        self.settings.set_busy(False)
        self._refresh_popup()

    def _entity_state(self, entity_id: str, state: str, name: str) -> None:
        current = self.known.get(entity_id, {"entity_id": entity_id, "domain": entity_id.split(".", 1)[0]})
        current = dict(current)
        current["state"] = state
        current["name"] = name
        self.known[entity_id] = current
        self._refresh_popup()

    def _registered(self, webhook: str) -> None:
        self.cfg["webhook_id"] = webhook
        save_config(self.cfg)

    def _failed(self, text: str) -> None:
        self.settings.set_busy(False)
        self.settings.set_status(text)
        self.tray.setToolTip("yoin — " + text)
        if text == self._last_error:
            return
        self._last_error = text
        self.tray.showMessage("yoin", text, QSystemTrayIcon.MessageIcon.Warning, 8000)

    def _refresh_popup(self) -> None:
        buttons = self.cfg["buttons"]
        self.popup.set_faces(labels_for(buttons, self.known), ons_for(buttons, self.known))


def main() -> int:
    # A Wayland toplevel cannot be placed above the panel. XWayland can.
    if (
        not os.environ.get("QT_QPA_PLATFORM")
        and os.environ.get("WAYLAND_DISPLAY")
        and os.environ.get("DISPLAY")
    ):
        os.environ["QT_QPA_PLATFORM"] = "xcb"
    app = QApplication(sys.argv)
    app.setApplicationName("yoin")
    app.setOrganizationName("yoin")
    app.setDesktopFileName("io.github.yrmuq.yoin")
    app.setQuitOnLastWindowClosed(False)
    icon = QIcon(asset_path("icon.png"))
    app.setWindowIcon(icon)
    config_dir().mkdir(parents=True, exist_ok=True)
    lock = QLockFile(str(config_dir() / "yoin.lock"))
    if not lock.tryLock(200):
        return 0
    controller = Controller(load_config(), icon)
    controller.lock = lock
    controller.start()
    return app.exec()
