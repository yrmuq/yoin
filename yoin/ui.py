# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

from __future__ import annotations

import os

from PySide6.QtCore import QEvent, Qt, Signal
from PySide6.QtGui import QCursor, QPainter, QPen
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
from yoin.i18n import languages, tr
from yoin.place import popup_origin
from yoin.theme import colors, dialog_stylesheet, popup_stylesheet


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
        self.box.addItem(tr(12), "")
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
    language_changed = Signal(str)
    theme_changed = Signal(str)

    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("yoin")
        self.setMinimumWidth(440)
        self.url = QLineEdit()
        self.url.setPlaceholderText("http://homeassistant.local:8123")
        self.token = QLineEdit()
        self.token.setEchoMode(QLineEdit.EchoMode.Password)
        self.device = QLineEdit()
        self.hint = QLabel()
        self.hint.setObjectName("hint")
        self.hint.setWordWrap(True)
        self.autostart = QCheckBox()
        self.status = QLabel("")
        self.status.setWordWrap(True)
        self._entities: list[dict] = []
        self._rows: list[_ButtonRow] = []
        connect = QPushButton()
        save = QPushButton()
        connect.clicked.connect(self.connect_requested.emit)
        save.clicked.connect(self.save_requested.emit)
        self._connect = connect
        self._save = save
        self._add = QPushButton("+")
        self._add.clicked.connect(self._add_row)

        self._address = QLabel()
        self._token = QLabel()
        self._device = QLabel()
        form = QFormLayout()
        form.addRow(self._address, self.url)
        form.addRow(self._token, self.token)
        form.addRow(self._device, self.device)
        form.addRow(self.hint)
        form.addRow(self.autostart)
        connection = QWidget()
        connection.setObjectName("page")
        connection.setLayout(form)

        self._list_host = QWidget()
        self._list_host.setObjectName("page")
        self._list = QVBoxLayout(self._list_host)
        self._list.setContentsMargins(0, 0, 0, 0)
        self._list.addStretch(1)
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setWidget(self._list_host)
        scroll.setMinimumHeight(220)
        buttons_page = QWidget()
        buttons_page.setObjectName("page")
        buttons_lay = QVBoxLayout(buttons_page)
        buttons_lay.addWidget(scroll)
        buttons_lay.addWidget(self._add)

        self.language = QComboBox()
        for name in languages():
            self.language.addItem(name, name)
        self.language.currentIndexChanged.connect(self._language_picked)
        self.theme = QComboBox()
        self.theme.currentIndexChanged.connect(self._theme_picked)
        self._language_label = QLabel()
        self._theme_label = QLabel()
        appearance_form = QFormLayout()
        appearance_form.addRow(self._language_label, self.language)
        appearance_form.addRow(self._theme_label, self.theme)
        appearance = QWidget()
        appearance.setObjectName("page")
        appearance.setLayout(appearance_form)

        tabs = QTabWidget()
        tabs.addTab(connection, "")
        tabs.addTab(buttons_page, "")
        tabs.addTab(appearance, "")
        self._tabs = tabs

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
        self.retranslate()
        self.apply_theme("light")

    def retranslate(self) -> None:
        self._address.setText(tr(3))
        self._token.setText(tr(4))
        self._device.setText(tr(5))
        self.hint.setText(tr(6))
        self.autostart.setText(tr(7))
        self._connect.setText(tr(8))
        self._save.setText(tr(9))
        self.token.setPlaceholderText(tr(25))
        self._tabs.setTabText(0, tr(10))
        self._tabs.setTabText(1, tr(11))
        self._tabs.setTabText(2, tr(18))
        self._language_label.setText(tr(13))
        self._theme_label.setText(tr(14))
        self._fill_theme()
        selected = [row.entity() for row in self._rows] or [""]
        self._render(selected)

    def apply_theme(self, kind: str) -> None:
        self.setStyleSheet(dialog_stylesheet(kind))

    def set_status(self, text: str) -> None:
        self.status.setText(text)

    def set_busy(self, busy: bool) -> None:
        self._connect.setEnabled(not busy)

    def fill(self, cfg: dict) -> None:
        self.url.setText(cfg.get("url") or "")
        self.token.setText(cfg.get("token") or "")
        self.device.setText(cfg.get("device_name") or "")
        self.autostart.setChecked(cfg.get("autostart") is True)
        self._select(self.language, cfg.get("language") or "English")
        self._select(self.theme, cfg.get("theme") or "system")
        self._render(cfg.get("buttons") or [""])

    def values(self) -> dict:
        return {
            "url": self.url.text().strip(),
            "token": self.token.text().strip(),
            "device_name": self.device.text().strip(),
            "buttons": [row.entity() for row in self._rows][:MAX_BUTTONS] or [""],
            "autostart": self.autostart.isChecked(),
            "language": self.language.currentData() or "English",
            "theme": self.theme.currentData() or "system",
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

    def _fill_theme(self) -> None:
        current = self.theme.currentData() or "system"
        self.theme.blockSignals(True)
        self.theme.clear()
        self.theme.addItem(tr(15), "system")
        self.theme.addItem(tr(16), "light")
        self.theme.addItem(tr(17), "dark")
        self._select(self.theme, current)
        self.theme.blockSignals(False)

    def _select(self, box: QComboBox, value: str) -> None:
        box.blockSignals(True)
        index = box.findData(value)
        box.setCurrentIndex(index if index >= 0 else 0)
        box.blockSignals(False)

    def _language_picked(self) -> None:
        name = self.language.currentData() or "English"
        self.language_changed.emit(name)

    def _theme_picked(self) -> None:
        self.theme_changed.emit(self.theme.currentData() or "system")

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
            Qt.WindowType.Window
            | Qt.WindowType.FramelessWindowHint
            | Qt.WindowType.WindowStaysOnTopHint
            | Qt.WindowType.NoDropShadowWindowHint
        )
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
        self.setFixedWidth(280)
        self.buttons: list[QPushButton] = []
        self._bound: list[bool] = []
        self._colors = colors("light")
        self._lay = QVBoxLayout(self)
        self._lay.setContentsMargins(14, 14, 14, 14)
        self._lay.setSpacing(8)

    def apply_theme(self, kind: str) -> None:
        self._colors = colors(kind)
        self.setStyleSheet(popup_stylesheet(kind))
        self.update()

    def set_faces(self, labels: list[str], ons: list[bool], bound: list[bool]) -> None:
        count = max(1, len(labels))
        while len(self.buttons) < count:
            index = len(self.buttons)
            button = QPushButton(tr(12))
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
        padded_label = list(labels) + [tr(12)] * count
        padded_bound = list(bound) + [False] * count
        self._bound = padded_bound
        for button, label, on, is_bound in zip(self.buttons, padded_label, padded_on, padded_bound):
            button.setText(label)
            button.setCheckable(is_bound)
            button.setChecked(bool(on) and is_bound)
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
        width, height = self.width(), self.height()
        pos = QCursor.pos()
        tray = (
            int(tray_geo.x()),
            int(tray_geo.y()),
            int(tray_geo.width()),
            int(tray_geo.height()),
        )
        x, y = popup_origin(
            tray,
            (int(pos.x()), int(pos.y())),
            (int(area.x()), int(area.y()), int(area.width()), int(area.height())),
            (width, height),
        )
        self.move(x, y)
        return x, y

    def paintEvent(self, _event) -> None:
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        pen = QPen(self._qcolor(self._colors["border"]))
        painter.setPen(pen)
        painter.setBrush(self._qcolor(self._colors["bg"]))
        painter.drawRoundedRect(self.rect().adjusted(1, 1, -1, -1), 16, 16)
        painter.end()

    def _qcolor(self, value: str):
        from PySide6.QtGui import QColor

        return QColor(value)


def labels_for(buttons: list[str], known: dict[str, dict]) -> list[str]:
    labels = []
    for entity_id in buttons:
        label = button_label(entity_id, known.get(entity_id))
        labels.append(tr(12) if label == "Not set" else label)
    return labels


def ons_for(buttons: list[str], known: dict[str, dict]) -> list[bool]:
    return [button_on(known.get(entity_id)) for entity_id in buttons]
