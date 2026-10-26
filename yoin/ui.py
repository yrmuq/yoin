# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

from __future__ import annotations

import os

from PySide6.QtCore import QEvent, Qt, Signal
from PySide6.QtGui import QCursor, QPainter, QPalette, QPen
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from yoin import __version__
from yoin.ha import button_label, button_on


def asset_path(name: str) -> str:
    env = os.environ.get("YOIN_ASSETS")
    if env:
        candidate = os.path.join(env, name)
        if os.path.isfile(candidate):
            return candidate
    here = os.path.join(os.path.dirname(__file__), "..", "assets", name)
    if os.path.isfile(here):
        return here
    packaged = "/app/share/icons/hicolor/256x256/apps/io.github.yrmuq.yoin.png"
    if name.endswith(".png") and os.path.isfile(packaged):
        return packaged
    return here


class Settings(QDialog):
    connect_requested = Signal()
    save_requested = Signal()

    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("yoin")
        self.setMinimumWidth(440)
        self.url = QLineEdit()
        self.url.setPlaceholderText("http://homeassistant.local:8123")
        self.token = QLineEdit()
        self.token.setEchoMode(QLineEdit.EchoMode.Password)
        self.token.setPlaceholderText("Long-lived access token")
        self.device = QLineEdit()
        self.hint = QLabel(
            "The device name becomes notify.mobile_app_<name> in Home Assistant. "
            "Spaces become underscores."
        )
        self.hint.setWordWrap(True)
        self.box = [QComboBox(), QComboBox()]
        for box in self.box:
            box.addItem("Not set", "")
        self.autostart = QCheckBox("Start with KDE")
        self.status = QLabel("")
        self.status.setWordWrap(True)
        connect = QPushButton("Connect")
        save = QPushButton("Save")
        connect.clicked.connect(self.connect_requested.emit)
        save.clicked.connect(self.save_requested.emit)
        self._connect = connect

        form = QFormLayout()
        form.addRow("Address", self.url)
        form.addRow("Token", self.token)
        form.addRow("Device name", self.device)
        form.addRow(self.hint)
        form.addRow("Button 1", self.box[0])
        form.addRow("Button 2", self.box[1])
        form.addRow(self.autostart)
        buttons = QHBoxLayout()
        buttons.addStretch(1)
        buttons.addWidget(connect)
        buttons.addWidget(save)
        lay = QVBoxLayout(self)
        lay.addLayout(form)
        lay.addWidget(self.status)
        lay.addLayout(buttons)
        version = QLabel(__version__)
        version.setAlignment(Qt.AlignmentFlag.AlignRight)
        lay.addWidget(version)

    def set_status(self, text: str) -> None:
        self.status.setText(text)

    def set_busy(self, busy: bool) -> None:
        self._connect.setEnabled(not busy)

    def fill(self, cfg: dict) -> None:
        self.url.setText(cfg.get("url") or "")
        self.token.setText(cfg.get("token") or "")
        self.device.setText(cfg.get("device_name") or "")
        self.autostart.setChecked(cfg.get("autostart") is True)
        self._pick(cfg.get("buttons") or ["", ""])

    def values(self) -> dict:
        return {
            "url": self.url.text().strip(),
            "token": self.token.text().strip(),
            "device_name": self.device.text().strip(),
            "buttons": [
                self.box[0].currentData() or "",
                self.box[1].currentData() or "",
            ],
            "autostart": self.autostart.isChecked(),
        }

    def set_entities(self, entities: list[dict], selected: list[str]) -> None:
        chosen = [self.box[i].currentData() or "" for i in range(2)]
        for i in range(2):
            if not chosen[i]:
                chosen[i] = selected[i] if i < len(selected) else ""
        for i, box in enumerate(self.box):
            box.blockSignals(True)
            box.clear()
            box.addItem("Not set", "")
            for ent in entities:
                label = ent["name"]
                if ent["name"] != ent["entity_id"]:
                    label = f"{ent['name']}  ({ent['entity_id']})"
                box.addItem(label, ent["entity_id"])
            index = box.findData(chosen[i])
            box.setCurrentIndex(index if index >= 0 else 0)
            box.blockSignals(False)

    def _pick(self, selected: list[str]) -> None:
        for i, box in enumerate(self.box):
            want = selected[i] if i < len(selected) else ""
            index = box.findData(want)
            if index >= 0:
                box.setCurrentIndex(index)


class Popup(QWidget):
    triggered = Signal(int)

    def __init__(self) -> None:
        super().__init__(None)
        self.setWindowTitle("yoin-buttons")
        self.setWindowFlags(
            Qt.WindowType.Tool
            | Qt.WindowType.FramelessWindowHint
            | Qt.WindowType.WindowStaysOnTopHint
            | Qt.WindowType.NoDropShadowWindowHint
        )
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setFixedWidth(280)
        self.buttons = [QPushButton("Not set"), QPushButton("Not set")]
        lay = QVBoxLayout(self)
        lay.setContentsMargins(14, 14, 14, 14)
        lay.setSpacing(8)
        for index, button in enumerate(self.buttons):
            button.setMinimumHeight(40)
            button.clicked.connect(lambda _checked=False, i=index: self.triggered.emit(i))
            lay.addWidget(button)

    def set_faces(self, labels: list[str], ons: list[bool]) -> None:
        for button, label, on in zip(self.buttons, labels, ons):
            button.setText(label)
            button.setCheckable(label != "Not set")
            button.setChecked(bool(on))

    def eventFilter(self, watched, event) -> bool:
        if (
            self.isVisible()
            and event.type() == QEvent.Type.MouseButtonPress
            and hasattr(event, "globalPosition")
        ):
            pos = event.globalPosition().toPoint()
            if not self.frameGeometry().contains(pos):
                self.dismiss()
        return super().eventFilter(watched, event)

    def is_mapped(self) -> bool:
        window = self.windowHandle()
        return self.isVisible() and window is not None and window.isVisible()

    def dismiss(self) -> None:
        self.hide()
        window = self.windowHandle()
        if window is not None:
            window.setMouseGrabEnabled(False)
            window.setKeyboardGrabEnabled(False)

    def drop_window(self) -> None:
        if self.isVisible():
            return
        self.dismiss()
        self.destroy()

    def place(self, tray_geo) -> tuple[int, int]:
        self.adjustSize()
        screen = self.screen()
        if screen is None:
            from PySide6.QtWidgets import QApplication

            screen = QApplication.primaryScreen()
        area = screen.availableGeometry()
        full = screen.geometry()
        width, height = self.width(), self.height()
        pos = QCursor.pos()
        tray_on_panel = (
            tray_geo.isValid()
            and tray_geo.width() > 1
            and tray_geo.height() > 1
            and not area.contains(tray_geo.center())
        )
        cursor_on_panel = (
            pos.y() >= area.bottom() - 2
            or pos.y() <= area.top() + 2
            or pos.x() <= area.left() + 2
            or pos.x() >= area.right() - 2
        )
        if tray_on_panel:
            x = tray_geo.center().x() - width // 2
            y = tray_geo.top() - height - 8
            if y < area.top():
                y = tray_geo.bottom() + 8
        elif cursor_on_panel:
            x = pos.x() - width // 2
            y = area.top() + 8 if pos.y() <= area.top() else area.bottom() - height - 8
        elif full.top() < area.top() - 4:
            x = area.right() - width - 8
            y = area.top() + 8
        elif full.left() < area.left() - 4:
            x = area.left() + 8
            y = area.bottom() - height - 8
        else:
            x = area.right() - width - 8
            y = area.bottom() - height - 8
        x = max(area.left(), min(x, area.right() - width))
        y = max(area.top(), min(y, area.bottom() - height))
        self.move(x, y)
        return x, y

    def paintEvent(self, _event) -> None:
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        pen = QPen(self.palette().color(QPalette.ColorRole.Mid))
        painter.setPen(pen)
        painter.setBrush(self.palette().color(QPalette.ColorRole.Window))
        painter.drawRoundedRect(self.rect().adjusted(1, 1, -1, -1), 16, 16)
        painter.end()


def labels_for(buttons: list[str], known: dict[str, dict]) -> list[str]:
    return [button_label(entity_id, known.get(entity_id)) for entity_id in buttons]


def ons_for(buttons: list[str], known: dict[str, dict]) -> list[bool]:
    return [button_on(known.get(entity_id)) for entity_id in buttons]
