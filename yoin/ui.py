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
    QScrollArea,
    QSizePolicy,
    QTabWidget,
    QVBoxLayout,
    QWidget,
)

from yoin import __version__
from yoin.config import MAX_BUTTONS
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


class _ButtonRow(QWidget):
    def __init__(self) -> None:
        super().__init__()
        self.box = QComboBox()
        self.box.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        self.up = QPushButton("↑")
        self.down = QPushButton("↓")
        self.remove = QPushButton("−")
        for button in (self.up, self.down, self.remove):
            button.setFixedWidth(32)
        row = QHBoxLayout(self)
        row.setContentsMargins(0, 0, 0, 0)
        row.addWidget(self.box, 1)
        row.addWidget(self.up)
        row.addWidget(self.down)
        row.addWidget(self.remove)

    def fill(self, entities: list[dict], selected: str) -> None:
        self.box.blockSignals(True)
        self.box.clear()
        self.box.addItem("Not set", "")
        for ent in entities:
            label = ent["name"]
            if ent["name"] != ent["entity_id"]:
                label = f"{ent['name']}  ({ent['entity_id']})"
            self.box.addItem(label, ent["entity_id"])
        if selected and self.box.findData(selected) < 0:
            self.box.addItem(selected, selected)
        index = self.box.findData(selected)
        self.box.setCurrentIndex(index if index >= 0 else 0)
        self.box.blockSignals(False)

    def entity(self) -> str:
        return self.box.currentData() or ""


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
        self.autostart = QCheckBox("Start with KDE")
        self.status = QLabel("")
        self.status.setWordWrap(True)
        self._entities: list[dict] = []
        self._rows: list[_ButtonRow] = []
        connect = QPushButton("Connect")
        save = QPushButton("Save")
        connect.clicked.connect(self.connect_requested.emit)
        save.clicked.connect(self.save_requested.emit)
        self._connect = connect
        self._add = QPushButton("+")
        self._add.clicked.connect(self._add_row)

        form = QFormLayout()
        form.addRow("Address", self.url)
        form.addRow("Token", self.token)
        form.addRow("Device name", self.device)
        form.addRow(self.hint)
        form.addRow(self.autostart)
        connection = QWidget()
        connection.setLayout(form)

        self._list_host = QWidget()
        self._list = QVBoxLayout(self._list_host)
        self._list.setContentsMargins(0, 0, 0, 0)
        self._list.addStretch(1)
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setWidget(self._list_host)
        scroll.setMinimumHeight(220)
        buttons_page = QWidget()
        buttons_lay = QVBoxLayout(buttons_page)
        buttons_lay.addWidget(scroll)
        buttons_lay.addWidget(self._add)

        tabs = QTabWidget()
        tabs.addTab(connection, "Connection")
        tabs.addTab(buttons_page, "Buttons")

        bar = QHBoxLayout()
        bar.addStretch(1)
        bar.addWidget(connect)
        bar.addWidget(save)
        lay = QVBoxLayout(self)
        lay.addWidget(tabs)
        lay.addWidget(self.status)
        lay.addLayout(bar)
        version = QLabel(__version__)
        version.setAlignment(Qt.AlignmentFlag.AlignRight)
        lay.addWidget(version)
        self._render([""])

    def set_status(self, text: str) -> None:
        self.status.setText(text)

    def set_busy(self, busy: bool) -> None:
        self._connect.setEnabled(not busy)

    def fill(self, cfg: dict) -> None:
        self.url.setText(cfg.get("url") or "")
        self.token.setText(cfg.get("token") or "")
        self.device.setText(cfg.get("device_name") or "")
        self.autostart.setChecked(cfg.get("autostart") is True)
        self._render(cfg.get("buttons") or [""])

    def values(self) -> dict:
        return {
            "url": self.url.text().strip(),
            "token": self.token.text().strip(),
            "device_name": self.device.text().strip(),
            "buttons": [row.entity() for row in self._rows][:MAX_BUTTONS] or [""],
            "autostart": self.autostart.isChecked(),
        }

    def set_entities(self, entities: list[dict], selected: list[str]) -> None:
        self._entities = list(entities)
        if not self._rows:
            self._render(selected or [""])
            return
        for index, row in enumerate(self._rows):
            current = row.entity()
            if not current and index < len(selected):
                current = selected[index]
            row.fill(self._entities, current)

    def _render(self, selected: list[str]) -> None:
        while self._rows:
            row = self._rows.pop()
            self._list.removeWidget(row)
            row.setParent(None)
            row.deleteLater()
        chosen = [item for item in selected[:MAX_BUTTONS]] or [""]
        for entity in chosen:
            self._append(entity if isinstance(entity, str) else "")
        self._sync_rows()

    def _append(self, entity: str) -> None:
        row = _ButtonRow()
        row.up.clicked.connect(lambda _checked=False, target=row: self._move(target, -1))
        row.down.clicked.connect(lambda _checked=False, target=row: self._move(target, 1))
        row.remove.clicked.connect(lambda _checked=False, target=row: self._remove(target))
        row.fill(self._entities, entity)
        self._rows.append(row)
        self._list.insertWidget(len(self._rows) - 1, row)

    def _add_row(self) -> None:
        if len(self._rows) >= MAX_BUTTONS:
            return
        self._append("")
        self._sync_rows()

    def _remove(self, row: _ButtonRow) -> None:
        if len(self._rows) <= 1 or row not in self._rows:
            return
        self._rows.remove(row)
        self._list.removeWidget(row)
        row.setParent(None)
        row.deleteLater()
        self._sync_rows()

    def _move(self, row: _ButtonRow, delta: int) -> None:
        index = self._rows.index(row)
        target = index + delta
        if target < 0 or target >= len(self._rows):
            return
        self._rows.pop(index)
        self._rows.insert(target, row)
        self._list.removeWidget(row)
        self._list.insertWidget(target, row)
        self._sync_rows()

    def _sync_rows(self) -> None:
        last = len(self._rows) - 1
        self._add.setEnabled(len(self._rows) < MAX_BUTTONS)
        for index, row in enumerate(self._rows):
            row.up.setEnabled(index > 0)
            row.down.setEnabled(index < last)
            row.remove.setEnabled(len(self._rows) > 1)


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
        self.buttons: list[QPushButton] = []
        self._lay = QVBoxLayout(self)
        self._lay.setContentsMargins(14, 14, 14, 14)
        self._lay.setSpacing(8)

    def set_faces(self, labels: list[str], ons: list[bool]) -> None:
        count = max(1, len(labels))
        while len(self.buttons) < count:
            index = len(self.buttons)
            button = QPushButton("Not set")
            button.setMinimumHeight(40)
            button.clicked.connect(lambda _checked=False, i=index: self.triggered.emit(i))
            self.buttons.append(button)
            self._lay.addWidget(button)
        while len(self.buttons) > count:
            button = self.buttons.pop()
            self._lay.removeWidget(button)
            button.setParent(None)
            button.deleteLater()
        padded_on = list(ons) + [False] * count
        padded_label = list(labels) + ["Not set"] * count
        for button, label, on in zip(self.buttons, padded_label, padded_on):
            button.setText(label)
            button.setCheckable(label != "Not set")
            button.setChecked(bool(on))
        self.adjustSize()

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
