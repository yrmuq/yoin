# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Home Assistant message shapes. No Qt in this file."""

from __future__ import annotations

APP_ID = "io.github.yrmuq.yoin"
APP_NAME = "yoin"


def ws_url(url: str) -> str:
    base = url.strip().rstrip("/")
    if base.startswith("https://"):
        return "wss://" + base[len("https://") :] + "/api/websocket"
    if base.startswith("http://"):
        return "ws://" + base[len("http://") :] + "/api/websocket"
    raise ValueError("Address must start with http:// or https://")


def http_url(url: str, path: str) -> str:
    return url.strip().rstrip("/") + path


def registration_body(
    device_id: str, device_name: str, version: str, os_version: str
) -> dict:
    return {
        "device_id": device_id,
        "app_id": APP_ID,
        "app_name": APP_NAME,
        "app_version": version,
        "device_name": device_name,
        "manufacturer": "PC",
        "model": "KDE",
        "os_name": "Linux",
        "os_version": os_version,
        "supports_encryption": False,
        "app_data": {"push_websocket_channel": True},
    }


def action_for(entity_id: str) -> tuple[str, str] | None:
    if not entity_id or "." not in entity_id:
        return None
    domain = entity_id.split(".", 1)[0]
    if domain == "switch":
        return ("switch", "toggle")
    if domain == "script":
        return ("script", "turn_on")
    return None


def notification_text(event: dict) -> tuple[str, str] | None:
    if not isinstance(event, dict):
        return None
    title = event.get("title")
    message = event.get("message")
    if title is None and message is None:
        return None
    title_s = str(title).strip() if title is not None else ""
    message_s = str(message).strip() if message is not None else ""
    if not title_s and not message_s:
        return None
    if not title_s:
        title_s = APP_NAME
    return (title_s, message_s)


def confirm_id(event: dict) -> str | None:
    if not isinstance(event, dict):
        return None
    value = event.get("hass_confirm_id")
    if isinstance(value, str) and value:
        return value
    return None


def entities_from_states(states) -> list[dict]:
    out = []
    if not isinstance(states, list):
        return out
    for st in states:
        if not isinstance(st, dict):
            continue
        eid = st.get("entity_id")
        if not isinstance(eid, str) or "." not in eid:
            continue
        domain = eid.split(".", 1)[0]
        if domain not in ("script", "switch"):
            continue
        attrs = st.get("attributes") if isinstance(st.get("attributes"), dict) else {}
        name = attrs.get("friendly_name")
        if not isinstance(name, str) or not name.strip():
            name = eid
        state = st.get("state")
        out.append(
            {
                "entity_id": eid,
                "name": name,
                "domain": domain,
                "state": state if isinstance(state, str) else "",
            }
        )
    out.sort(key=lambda e: (e["name"].casefold(), e["entity_id"]))
    return out


def state_event(event: dict) -> dict | None:
    if not isinstance(event, dict) or event.get("event_type") != "state_changed":
        return None
    body = event.get("data")
    if not isinstance(body, dict) or not isinstance(body.get("new_state"), dict):
        return None
    found = entities_from_states([body["new_state"]])
    return found[0] if found else None


def button_label(entity_id: str, known: dict | None) -> str:
    if not entity_id:
        return "Not set"
    if not known:
        return entity_id
    name = str(known.get("name") or entity_id)
    state = known.get("state")
    if isinstance(state, str) and state:
        return f"{name}: {state}"
    return name


def button_on(known: dict | None) -> bool:
    return isinstance(known, dict) and known.get("state") == "on"
