# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Light and dark colors. system follows the desktop until the user picks one."""

from __future__ import annotations


def resolved_theme(choice: str, system_dark: bool) -> str:
    if choice == "light":
        return "light"
    if choice == "dark":
        return "dark"
    return "dark" if system_dark else "light"


def prefers_dark(scheme: str, kde_text: str | None, window_luma: float | None) -> bool:
    if scheme == "dark":
        return True
    if scheme == "light":
        return False
    kde = kde_is_dark(kde_text)
    if kde is not None:
        return kde
    if window_luma is not None:
        return window_luma < 128
    return False


def kde_is_dark(text: str | None) -> bool | None:
    if not text:
        return None
    section = ""
    scheme = ""
    background = ""
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1]
            continue
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip()
        if section == "General" and key == "ColorScheme":
            scheme = value
        elif section == "Colors:Window" and key == "BackgroundNormal":
            background = value
    if background:
        parts = []
        for piece in background.split(",")[:3]:
            try:
                parts.append(int(piece.strip()))
            except ValueError:
                return None
        if len(parts) == 3:
            luma = 0.2126 * parts[0] + 0.7152 * parts[1] + 0.0722 * parts[2]
            return luma < 128
    if scheme:
        return "dark" in scheme.casefold()
    return None


def colors(kind: str) -> dict[str, str]:
    if kind == "dark":
        return {
            "bg": "#1A1C22",
            "fg": "#F2F4F7",
            "muted": "#A7ADB8",
            "field": "#242830",
            "border": "#3A3F4A",
            "button": "#2E333D",
            "button_fg": "#F2F4F7",
            "accent": "#D7DEE8",
            "on_fg": "#1A1C22",
        }
    return {
        "bg": "#F4F5F7",
        "fg": "#1C1E22",
        "muted": "#5C6370",
        "field": "#FFFFFF",
        "border": "#D5D8DE",
        "button": "#E6E8ED",
        "button_fg": "#1C1E22",
        "accent": "#2F3A4A",
        "on_fg": "#F4F6F8",
    }


def dialog_stylesheet(kind: str) -> str:
    c = colors(kind)
    return f"""
    QDialog, QTabWidget::pane, QScrollArea, QWidget#page {{
        background: {c["bg"]};
        color: {c["fg"]};
    }}
    QTabBar::tab {{
        background: {c["button"]};
        color: {c["button_fg"]};
        padding: 6px 12px;
        border: 1px solid {c["border"]};
    }}
    QTabBar::tab:selected {{
        background: {c["field"]};
        color: {c["fg"]};
    }}
    QLabel {{
        background: transparent;
        color: {c["fg"]};
    }}
    QLabel#hint {{
        color: {c["muted"]};
    }}
    QCheckBox {{
        background: transparent;
        color: {c["fg"]};
    }}
    QLineEdit, QComboBox {{
        background: {c["field"]};
        color: {c["fg"]};
        border: 1px solid {c["border"]};
        padding: 6px;
        selection-background-color: {c["accent"]};
        selection-color: {c["on_fg"]};
    }}
    QComboBox QAbstractItemView {{
        background: {c["field"]};
        color: {c["fg"]};
        border: 1px solid {c["border"]};
        selection-background-color: {c["accent"]};
        selection-color: {c["on_fg"]};
        outline: 0;
    }}
    QPushButton {{
        background: {c["button"]};
        color: {c["button_fg"]};
        border: 1px solid {c["border"]};
        padding: 8px 14px;
    }}
    QPushButton:disabled {{
        color: {c["muted"]};
    }}
    QMenu {{
        background: {c["field"]};
        color: {c["fg"]};
        border: 1px solid {c["border"]};
    }}
    QMenu::item:selected {{
        background: {c["accent"]};
        color: {c["on_fg"]};
    }}
    """


def popup_stylesheet(kind: str) -> str:
    c = colors(kind)
    return f"""
    QPushButton {{
        background: {c["button"]};
        color: {c["button_fg"]};
        border: 1px solid {c["border"]};
        border-radius: 10px;
        padding: 8px 14px;
    }}
    QPushButton:checked {{
        background: {c["accent"]};
        color: {c["on_fg"]};
        border: 1px solid {c["accent"]};
    }}
    """
