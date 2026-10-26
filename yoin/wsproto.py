# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""WebSocket framing. No Qt in this file."""

from __future__ import annotations

import base64
import hashlib

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
MAX_MESSAGE = 64 * 1024 * 1024


class ProtocolError(Exception):
    pass


def accept_for(key: str) -> str:
    digest = hashlib.sha1((key + GUID).encode("ascii")).digest()
    return base64.b64encode(digest).decode("ascii")


def handshake_request(host: str, path: str, key: str) -> bytes:
    return (
        f"GET {path} HTTP/1.1\r\n"
        f"Host: {host}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        "Sec-WebSocket-Version: 13\r\n"
        "\r\n"
    ).encode("ascii")


def split_handshake(buf: bytes) -> tuple[str, bytes] | None:
    cut = buf.find(b"\r\n\r\n")
    if cut < 0:
        if len(buf) > 16384:
            raise ProtocolError("handshake is too long")
        return None
    head = buf[:cut].decode("latin1")
    line = head.split("\r\n", 1)[0]
    parts = line.split()
    if len(parts) < 2 or parts[1] != "101":
        raise ProtocolError(line or "handshake was refused")
    accept = ""
    for header in head.split("\r\n")[1:]:
        name, sep, value = header.partition(":")
        if sep and name.strip().lower() == "sec-websocket-accept":
            accept = value.strip()
    if not accept:
        raise ProtocolError("handshake has no accept key")
    return accept, buf[cut + 4 :]


def encode_frame(opcode: int, payload: bytes, mask: bytes) -> bytes:
    if len(mask) != 4:
        raise ProtocolError("mask must be 4 bytes")
    if len(payload) > MAX_MESSAGE:
        raise ProtocolError("message is too large")
    first = 0x80 | (opcode & 0x0F)
    size = len(payload)
    out = bytearray([first])
    if size < 126:
        out.append(0x80 | size)
    elif size < 65536:
        out.append(0x80 | 126)
        out += size.to_bytes(2, "big")
    else:
        out.append(0x80 | 127)
        out += size.to_bytes(8, "big")
    out += mask
    masked = bytearray(payload)
    for i, byte in enumerate(masked):
        masked[i] = byte ^ mask[i & 3]
    out += masked
    return bytes(out)


def decode_frames(buf: bytes) -> tuple[list[tuple[bool, int, bytes]], bytes]:
    frames = []
    i = 0
    n = len(buf)
    while n - i >= 2:
        first = buf[i]
        second = buf[i + 1]
        if first & 0x70:
            raise ProtocolError("reserved bits are set")
        opcode = first & 0x0F
        masked = second & 0x80
        size = second & 0x7F
        head = 2
        if size == 126:
            head = 4
            if n - i < head:
                break
            size = int.from_bytes(buf[i + 2 : i + 4], "big")
        elif size == 127:
            head = 10
            if n - i < head:
                break
            size = int.from_bytes(buf[i + 2 : i + 10], "big")
        if size > MAX_MESSAGE:
            raise ProtocolError("message is too large")
        if opcode & 0x08 and size > 125:
            raise ProtocolError("control frame is too large")
        mask_len = 4 if masked else 0
        total = head + mask_len + size
        if n - i < total:
            break
        payload = bytearray(buf[i + head + mask_len : i + total])
        if masked:
            key = buf[i + head : i + head + 4]
            for j, byte in enumerate(payload):
                payload[j] = byte ^ key[j & 3]
        frames.append((bool(first & 0x80), opcode, bytes(payload)))
        i += total
    return frames, buf[i:]


class Assembler:
    def __init__(self) -> None:
        self._opcode: int | None = None
        self._parts: list[bytes] = []

    def push(self, fin: bool, opcode: int, payload: bytes) -> list[tuple[str, bytes]]:
        if opcode & 0x08:
            if not fin:
                raise ProtocolError("fragmented control frame")
            if opcode == 0x9:
                return [("ping", payload)]
            if opcode == 0x8:
                return [("close", payload)]
            if opcode == 0xA:
                return []
            raise ProtocolError("unknown opcode")
        if opcode == 0x0:
            if self._opcode is None:
                raise ProtocolError("continuation without a start")
            self._parts.append(payload)
        elif opcode in (0x1, 0x2):
            if self._opcode is not None:
                raise ProtocolError("new frame before the last one finished")
            self._opcode = opcode
            self._parts = [payload]
        else:
            raise ProtocolError("unknown opcode")
        if not fin:
            return []
        data = b"".join(self._parts)
        kind = self._opcode
        self._opcode = None
        self._parts = []
        if kind == 0x2:
            return []
        return [("text", data)]
