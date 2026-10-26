# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

from __future__ import annotations

import os
import sys
import uuid

from PySide6.QtCore import QFileSystemWatcher, QLocale, QLockFile, Qt, QTimer
from PySide6.QtGui import QGuiApplication, QIcon, QPalette
from PySide6.QtWidgets import QApplication, QMenu, QSystemTrayIcon

from yoin.client import HaClient
from yoin.config import (
    apply_autostart,
    hostname,
    load_config,
    save_config,
    config_dir,
)
from yoin.i18n import match_language, tr, trf, use
from yoin.notify import announce
from yoin.place import install_placer
from yoin.theme import dialog_stylesheet, prefers_dark, resolved_theme
from yoin.ui import Popup, Settings, asset_path, labels_for, ons_for


class Controller:
    def __init__(self, cfg: dict, icon: QIcon) -> None:
        self.cfg = cfg
        self.known: dict[str, dict] = {}
        self._last_error = ""
        self._busy: set[str] = set()
        self._place_warned = False
        self.client = HaClient()
        self.settings = Settings()
        self.popup = Popup()
        self.tray = QSystemTrayIcon(icon)
        self.tray.setToolTip("yoin")
        menu = QMenu()
        self._settings_action = menu.addAction(tr(1), self.open_settings)
        menu.addSeparator()
        self._quit_action = menu.addAction(tr(2), QApplication.quit)
        self.menu = menu
        self.tray.setContextMenu(menu)
        self.tray.activated.connect(self._activated)
        self.popup.triggered.connect(self._pressed)
        QApplication.instance().installEventFilter(self.popup)
        self._placer_ok = install_placer()
        self.settings.connect_requested.connect(self._connect)
        self.settings.save_requested.connect(self._save)
        self.settings.language_changed.connect(self._language)
        self.settings.theme_changed.connect(self._theme)
        self.client.link.connect(self._link)
        self.client.notify.connect(self._notify)
        self.client.entities_ready.connect(self._entities)
        self.client.failed.connect(self._failed)
        self.client.registered.connect(self._registered)
        self.client.service_done.connect(self._service_done)
        self.client.entity_state.connect(self._entity_state)
        self._watcher = QFileSystemWatcher()
        kde = str(config_dir().parent / "kdeglobals")
        if os.path.isfile(kde):
            self._watcher.addPath(kde)
        self._watcher.fileChanged.connect(self._desktop_theme)
        hints = QGuiApplication.styleHints()
        if hasattr(hints, "colorSchemeChanged"):
            hints.colorSchemeChanged.connect(self._desktop_theme)
        self._apply_theme()

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
            self.settings.set_status(tr(19))
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
        self.cfg["language"] = values["language"]
        self.cfg["theme"] = values["theme"]
        if not self.cfg["device_id"]:
            self.cfg["device_id"] = uuid.uuid4().hex
        if changed:
            self.cfg["webhook_id"] = ""
        save_config(self.cfg)
        try:
            apply_autostart(self.cfg["autostart"])
        except OSError as exc:
            self.settings.set_status(trf(20, exc))
            return
        use(self.cfg["language"])
        self._retranslate()
        self._apply_theme()
        self.settings.set_busy(True)
        self.settings.set_status(tr(21))
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
        handle = self.popup.windowHandle()
        if handle is not None:
            handle.setTransientParent(None)
        if QGuiApplication.platformName() == "wayland" and not self._placer_ok and not self._place_warned:
            self._place_warned = True
            self._announce("yoin", tr(23))

    def _close_popup(self) -> None:
        self.popup.dismiss()
        QTimer.singleShot(0, self.popup.drop_window)

    def _pressed(self, index: int) -> None:
        buttons = self.cfg["buttons"]
        entity = buttons[index] if index < len(buttons) else ""
        QTimer.singleShot(0, lambda: self._finish_press(entity))

    def _finish_press(self, entity: str) -> None:
        self._refresh_popup()
        if not entity:
            self.open_settings()
            return
        if entity in self._busy:
            return
        self._busy.add(entity)
        known = self.known.get(entity) or {}
        self.client.run(entity, str(known.get("state") or ""))

    def _service_done(self, entity: str, _ok: bool) -> None:
        self._busy.discard(entity)

    def _link(self, up: bool) -> None:
        if not up:
            if self.tray.toolTip() == "yoin":
                self.tray.setToolTip("yoin — " + tr(24))
            return
        self._last_error = ""
        self.tray.setToolTip("yoin")
        self.settings.set_busy(False)
        self.settings.set_status(tr(22))

    def _notify(self, title: str, message: str) -> None:
        self._announce(title, message)

    def _announce(self, title: str, message: str) -> None:
        icon = os.path.abspath(asset_path("icon.png"))
        if announce(title, message, icon):
            return
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
        self._announce("yoin", text)

    def _refresh_popup(self) -> None:
        buttons = self.cfg["buttons"]
        self.popup.set_faces(
            labels_for(buttons, self.known),
            ons_for(buttons, self.known),
            [bool(item) for item in buttons],
        )

    def _language(self, name: str) -> None:
        from yoin.i18n import languages

        if name not in languages():
            name = "English"
        self.cfg["language"] = name
        save_config(self.cfg)
        use(name)
        self._retranslate()

    def _theme(self, choice: str) -> None:
        if choice not in ("system", "light", "dark"):
            choice = "system"
        self.cfg["theme"] = choice
        save_config(self.cfg)
        self._apply_theme()

    def _desktop_theme(self, *_args) -> None:
        kde = config_dir().parent / "kdeglobals"
        if kde.is_file() and str(kde) not in self._watcher.files():
            self._watcher.addPath(str(kde))
        if self.cfg.get("theme") in ("light", "dark"):
            return
        self._apply_theme()

    def _system_dark(self) -> bool:
        scheme = QGuiApplication.styleHints().colorScheme()
        name = "unknown"
        if scheme == Qt.ColorScheme.Dark:
            name = "dark"
        elif scheme == Qt.ColorScheme.Light:
            name = "light"
        text = None
        kde = config_dir().parent / "kdeglobals"
        if name == "unknown" and kde.is_file():
            try:
                text = kde.read_text(encoding="utf-8", errors="replace")
            except OSError:
                text = None
        luma = None
        if name == "unknown" and text is None:
            color = QApplication.palette().color(QPalette.ColorRole.Window)
            luma = 0.2126 * color.red() + 0.7152 * color.green() + 0.0722 * color.blue()
        return prefers_dark(name, text, luma)

    def _apply_theme(self) -> None:
        kind = resolved_theme(self.cfg.get("theme") or "system", self._system_dark())
        self.settings.apply_theme(kind)
        self.popup.apply_theme(kind)
        self.menu.setStyleSheet(dialog_stylesheet(kind))
        icon_name = "tray-dark.png" if kind == "light" else "tray-light.png"
        path = asset_path(icon_name)
        if not os.path.isfile(path):
            path = asset_path("icon.png")
        self.tray.setIcon(QIcon(path))

    def _retranslate(self) -> None:
        self._settings_action.setText(tr(1))
        self._quit_action.setText(tr(2))
        self.settings.retranslate()
        if self.tray.toolTip().startswith("yoin —"):
            self.tray.setToolTip("yoin — " + tr(24))
        self._refresh_popup()


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationName("yoin")
    app.setOrganizationName("yoin")
    app.setDesktopFileName("io.github.yrmuq.yoin")
    app.setQuitOnLastWindowClosed(False)
    config_dir().mkdir(parents=True, exist_ok=True)
    lock = QLockFile(str(config_dir() / "yoin.lock"))
    if not lock.tryLock(200):
        return 0
    cfg = load_config()
    if not cfg.get("language"):
        cfg["language"] = match_language(list(QLocale.system().uiLanguages()))
        try:
            save_config(cfg)
        except OSError:
            pass
    use(cfg["language"])
    icon = QIcon(asset_path("icon.png"))
    app.setWindowIcon(icon)
    controller = Controller(cfg, icon)
    controller.lock = lock
    controller.start()
    return app.exec()
