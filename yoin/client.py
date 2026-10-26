# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Websocket and registration. Local push only.

shortcut: no cloud push, so a quit or offline yoin misses notifications.
Add a push URL only if the desktop must receive them while it is not running.
"""

from __future__ import annotations

import json
import os

from PySide6.QtCore import QObject, QTimer, QUrl, Signal
from PySide6.QtNetwork import (
    QAbstractSocket,
    QNetworkAccessManager,
    QNetworkReply,
    QNetworkRequest,
)

from yoin import __version__
from yoin.ha import (
    action_for,
    confirm_id,
    entities_from_states,
    http_url,
    notification_text,
    registration_body,
    state_event,
    ws_url,
)
from yoin.i18n import tr, trf
from yoin.ws import WsClient


class HaClient(QObject):
    link = Signal(bool)
    notify = Signal(str, str)
    entities_ready = Signal(list)
    entity_state = Signal(str, str, str)
    failed = Signal(str)
    registered = Signal(str)
    service_done = Signal(str, bool)

    def __init__(self) -> None:
        super().__init__()
        self._want = False
        self._reopen = False
        self._registering = False
        self._base = ""
        self._token = ""
        self._webhook_id = ""
        self._device_id = ""
        self._device_name = ""
        self._next_id = 0
        self._push_id: int | None = None
        self._state_id: int | None = None
        self._pending: dict[int, str] = {}
        self._services: dict[int, str] = {}
        self._ws = WsClient(self)
        self._nam = QNetworkAccessManager(self)
        self._reply: QNetworkReply | None = None
        self._retry = QTimer(self)
        self._retry.setSingleShot(True)
        self._retry.setInterval(5000)
        self._retry.timeout.connect(self._begin)
        self._ws.textMessageReceived.connect(self._on_text)
        self._ws.disconnected.connect(self._on_disconnected)
        self._ws.errorOccurred.connect(self._on_socket_error)

    def start(
        self,
        base: str,
        token: str,
        webhook_id: str,
        device_id: str,
        device_name: str,
    ) -> None:
        self._retry.stop()
        self._close_reply()
        self._registering = False
        self._base = base.strip()
        self._token = token.strip()
        self._webhook_id = webhook_id or ""
        self._device_id = device_id
        self._device_name = device_name
        self._want = True
        if self._ws.state() != QAbstractSocket.SocketState.UnconnectedState:
            self._reopen = True
            self._ws.abort()
            return
        self._begin()

    def stop(self) -> None:
        self._want = False
        self._reopen = False
        self._retry.stop()
        self._close_reply()
        self._ws.abort()

    def run(self, entity_id: str, state: str = "") -> None:
        action = action_for(entity_id, state)
        if action is None or self._push_id is None:
            self.failed.emit(tr(26))
            self.service_done.emit(entity_id, False)
            return
        domain, service = action
        ident = self._send(
            {
                "type": "call_service",
                "domain": domain,
                "service": service,
                "target": {"entity_id": entity_id},
            }
        )
        self._services[ident] = entity_id

    def _begin(self) -> None:
        if not self._want:
            return
        self._push_id = None
        self._state_id = None
        if not self._webhook_id:
            self._register()
        else:
            self._open()

    def _schedule(self) -> None:
        if self._want and not self._retry.isActive() and not self._registering:
            self._retry.start()

    def _open(self) -> None:
        try:
            url = ws_url(self._base)
        except ValueError as exc:
            self._want = False
            self.failed.emit(str(exc))
            return
        self._ws.open(QUrl(url))

    def _send(self, payload: dict) -> int:
        self._next_id += 1
        body = dict(payload)
        body["id"] = self._next_id
        self._ws.sendTextMessage(json.dumps(body))
        return self._next_id

    def _register(self) -> None:
        if self._registering:
            return
        try:
            url = http_url(self._base, "/api/mobile_app/registrations")
            ws_url(self._base)
        except ValueError as exc:
            self._want = False
            self.failed.emit(str(exc))
            return
        self._registering = True
        req = QNetworkRequest(QUrl(url))
        req.setHeader(
            QNetworkRequest.KnownHeaders.ContentTypeHeader, "application/json"
        )
        req.setRawHeader(b"Authorization", b"Bearer " + self._token.encode())
        body = json.dumps(
            registration_body(
                self._device_id,
                self._device_name,
                __version__,
                os.uname().release,
            )
        ).encode()
        self._close_reply()
        self._reply = self._nam.post(req, body)
        self._reply.finished.connect(self._register_done)

    def _close_reply(self) -> None:
        reply = self._reply
        self._reply = None
        if reply is not None:
            reply.blockSignals(True)
            reply.abort()
            reply.deleteLater()

    def _register_done(self) -> None:
        reply = self._reply
        self._reply = None
        self._registering = False
        if reply is None:
            return
        status = reply.attribute(QNetworkRequest.Attribute.HttpStatusCodeAttribute)
        raw = bytes(reply.readAll()).decode("utf-8", "replace")
        err = reply.error()
        err_s = reply.errorString()
        reply.deleteLater()
        if not self._want:
            return
        try:
            code = int(status) if status is not None else 0
        except (TypeError, ValueError):
            code = 0
        if code in (401, 403):
            self._want = False
            self.failed.emit(tr(27))
            return
        if code not in (200, 201):
            if err != QNetworkReply.NetworkError.NoError and not code:
                self.failed.emit(err_s or tr(28))
            else:
                self.failed.emit(trf(29, code))
            self._schedule()
            return
        try:
            data = json.loads(raw)
        except json.JSONDecodeError:
            self.failed.emit(tr(30))
            self._schedule()
            return
        webhook = data.get("webhook_id") if isinstance(data, dict) else None
        if not isinstance(webhook, str) or not webhook:
            self.failed.emit(tr(31))
            self._schedule()
            return
        self._webhook_id = webhook
        self.registered.emit(webhook)
        self._open()

    def _on_socket_error(self, _err) -> None:
        if not self._want or self._reopen or self._registering:
            return
        self.failed.emit(self._ws.errorString() or tr(32))

    def _on_disconnected(self) -> None:
        self._push_id = None
        self.link.emit(False)
        if self._reopen:
            self._reopen = False
            self._begin()
            return
        if self._want and not self._registering:
            self._schedule()

    def _on_text(self, message: str) -> None:
        try:
            data = json.loads(message)
        except json.JSONDecodeError:
            return
        if not isinstance(data, dict):
            return
        kind = data.get("type")
        if kind == "auth_required":
            self._ws.sendTextMessage(
                json.dumps({"type": "auth", "access_token": self._token})
            )
            return
        if kind == "auth_invalid":
            self._want = False
            self.failed.emit(tr(27))
            self._ws.abort()
            return
        if kind == "auth_ok":
            self._push_id = self._send(
                {
                    "type": "mobile_app/push_notification_channel",
                    "webhook_id": self._webhook_id,
                    "support_confirm": True,
                }
            )
            self._pending[self._push_id] = "push"
            self._pending[self._send({"type": "get_states"})] = "states"
            self._state_id = self._send(
                {"type": "subscribe_events", "event_type": "state_changed"}
            )
            self.link.emit(True)
            return
        if kind == "ping":
            self._ws.sendTextMessage(
                json.dumps({"id": data.get("id"), "type": "pong"})
            )
            return
        if kind == "event":
            self._on_event(data)
            return
        if kind == "result":
            self._on_result(data)

    def _on_event(self, data: dict) -> None:
        ident = data.get("id")
        event = data.get("event")
        if not isinstance(event, dict):
            return
        if ident == self._push_id:
            cid = confirm_id(event)
            if cid:
                self._send(
                    {
                        "type": "mobile_app/push_notification_confirm",
                        "webhook_id": self._webhook_id,
                        "confirm_id": cid,
                    }
                )
            text = notification_text(event)
            if text:
                self.notify.emit(text[0], text[1])
            return
        if ident != self._state_id:
            return
        found = state_event(event)
        if found:
            self.entity_state.emit(found["entity_id"], found["state"], found["name"])

    def _on_result(self, data: dict) -> None:
        ident = data.get("id")
        if not isinstance(ident, int):
            return
        kind = self._pending.pop(ident, None)
        entity = self._services.pop(ident, None)
        ok = data.get("success") is True
        if kind == "push" and not ok:
            self._webhook_id = ""
            self._push_id = None
            self.registered.emit("")
            self._reopen = True
            self._ws.abort()
            return
        if kind == "states" and ok:
            self.entities_ready.emit(entities_from_states(data.get("result")))
            return
        if entity is not None:
            self.service_done.emit(entity, ok)
            if not ok:
                err = data.get("error")
                message = tr(33)
                if isinstance(err, dict) and err.get("message"):
                    message = str(err["message"])
                self.failed.emit(message)
