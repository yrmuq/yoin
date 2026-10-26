# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""WebSocket client on QtNetwork. QtWebSockets ships in PySide6_Addons."""

from __future__ import annotations

import base64
import os

from PySide6.QtCore import QObject, QUrl, Signal
from PySide6.QtNetwork import QAbstractSocket, QSslSocket, QTcpSocket

from yoin.wsproto import (
    Assembler,
    ProtocolError,
    accept_for,
    decode_frames,
    encode_frame,
    handshake_request,
    split_handshake,
)


class WsClient(QObject):
    textMessageReceived = Signal(str)
    disconnected = Signal()
    errorOccurred = Signal(str)

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._sock: QTcpSocket | None = None
        self._state = QAbstractSocket.SocketState.UnconnectedState
        self._closed = True
        self._error = ""
        self._buf = b""
        self._upgraded = False
        self._accept = ""
        self._assembler = Assembler()

    def state(self):
        return self._state

    def errorString(self) -> str:
        return self._error

    def open(self, url: QUrl) -> None:
        self.abort()
        scheme = url.scheme()
        if scheme not in ("ws", "wss"):
            self._fail("Address must start with ws:// or wss://")
            return
        host = url.host()
        if not host:
            self._fail("Address has no host")
            return
        port = url.port(443 if scheme == "wss" else 80)
        path = url.path() or "/"
        query = url.query()
        if query:
            path = f"{path}?{query}"
        header_host = f"[{host}]" if ":" in host else host
        if not (scheme == "ws" and port == 80) and not (scheme == "wss" and port == 443):
            header_host = f"{header_host}:{port}"
        key = base64.b64encode(os.urandom(16)).decode("ascii")
        self._request = handshake_request(header_host, path, key)
        self._accept = accept_for(key)
        self._buf = b""
        self._upgraded = False
        self._assembler = Assembler()
        self._closed = False
        self._error = ""
        self._state = QAbstractSocket.SocketState.ConnectingState
        sock: QTcpSocket = QSslSocket(self) if scheme == "wss" else QTcpSocket(self)
        self._sock = sock
        sock.errorOccurred.connect(self._on_error)
        sock.disconnected.connect(self._on_disconnected)
        sock.readyRead.connect(self._on_ready)
        if scheme == "wss":
            sock.encrypted.connect(self._send_handshake)
            sock.connectToHostEncrypted(host, port)
        else:
            sock.connected.connect(self._send_handshake)
            sock.connectToHost(host, port)

    def sendTextMessage(self, text: str) -> None:
        if self._state != QAbstractSocket.SocketState.ConnectedState or self._sock is None:
            self._error = "Not connected"
            self.errorOccurred.emit(self._error)
            return
        self._write(encode_frame(0x1, text.encode("utf-8"), os.urandom(4)))

    def abort(self) -> None:
        sock = self._sock
        self._sock = None
        if sock is not None:
            sock.blockSignals(True)
            sock.abort()
            sock.deleteLater()
        was = not self._closed
        self._closed = True
        self._state = QAbstractSocket.SocketState.UnconnectedState
        if was:
            self.disconnected.emit()

    def _send_handshake(self) -> None:
        self._write(self._request)

    def _write(self, data: bytes) -> None:
        sock = self._sock
        if sock is None:
            return
        if sock.write(data) < 0:
            self._fail(sock.errorString() or "write failed")

    def _on_ready(self) -> None:
        sock = self._sock
        if sock is None:
            return
        self._buf += bytes(sock.readAll())
        try:
            if not self._upgraded:
                parsed = split_handshake(self._buf)
                if parsed is None:
                    return
                accept, rest = parsed
                if accept != self._accept:
                    raise ProtocolError("handshake accept key does not match")
                self._buf = rest
                self._upgraded = True
                self._state = QAbstractSocket.SocketState.ConnectedState
            self._drain()
        except (ProtocolError, UnicodeError) as exc:
            self._fail(str(exc))

    def _drain(self) -> None:
        frames, rest = decode_frames(self._buf)
        self._buf = rest
        for fin, opcode, payload in frames:
            if self._closed:
                return
            for kind, data in self._assembler.push(fin, opcode, payload):
                if kind == "ping":
                    self._write(encode_frame(0xA, data, os.urandom(4)))
                elif kind == "close":
                    self.abort()
                    return
                elif kind == "text":
                    self.textMessageReceived.emit(data.decode("utf-8"))

    def _on_error(self, _err) -> None:
        sock = self._sock
        text = sock.errorString() if sock is not None else ""
        if not text or "closed" in text.lower():
            return
        self._error = text
        self.errorOccurred.emit(text)

    def _on_disconnected(self) -> None:
        if self._closed:
            return
        sock = self._sock
        self._sock = None
        if sock is not None:
            sock.blockSignals(True)
            sock.deleteLater()
        self._closed = True
        self._state = QAbstractSocket.SocketState.UnconnectedState
        self.disconnected.emit()

    def _fail(self, text: str) -> None:
        self._error = text
        self.errorOccurred.emit(text)
        self.abort()
