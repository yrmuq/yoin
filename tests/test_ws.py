# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

import unittest

from yoin.wsproto import (
    Assembler,
    ProtocolError,
    accept_for,
    decode_frames,
    encode_frame,
    handshake_request,
    split_handshake,
)

MASK = b"\x01\x02\x03\x04"


class WsTests(unittest.TestCase):
    def test_rfc_accept_key(self):
        self.assertEqual(accept_for("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")

    def test_handshake_roundtrip(self):
        raw = handshake_request("ha.local:8123", "/api/websocket", "dGhlIHNhbXBsZSBub25jZQ==")
        self.assertTrue(raw.startswith(b"GET /api/websocket HTTP/1.1\r\n"))
        reply = (
            b"HTTP/1.1 101 Switching Protocols\r\n"
            b"Upgrade: websocket\r\n"
            b"Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"
            b"\r\n"
            b"leftover"
        )
        accept, rest = split_handshake(reply)
        self.assertEqual(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")
        self.assertEqual(rest, b"leftover")
        with self.assertRaises(ProtocolError):
            split_handshake(b"HTTP/1.1 404 No\r\n\r\n")

    def test_text_frame_and_fragments(self):
        frame = encode_frame(0x1, b"hello", MASK)
        frames, rest = decode_frames(frame)
        self.assertEqual(rest, b"")
        self.assertEqual(frames, [(True, 0x1, b"hello")])
        # split across reads
        frames, rest = decode_frames(frame[:3])
        self.assertEqual(frames, [])
        frames, rest = decode_frames(rest + frame[3:])
        self.assertEqual(frames[0][2], b"hello")
        box = Assembler()
        self.assertEqual(box.push(False, 0x1, b"hel"), [])
        self.assertEqual(box.push(True, 0x0, b"lo"), [("text", b"hello")])

    def test_server_must_not_use_our_mask_bit(self):
        # A server ping is unmasked. The client answers with opcode 0xA.
        ping = bytes([0x89, 0x00])
        frames, _rest = decode_frames(ping)
        events = Assembler().push(*frames[0])
        self.assertEqual(events, [("ping", b"")])
        pong = encode_frame(0xA, b"", MASK)
        self.assertEqual(pong[0] & 0x0F, 0x0A)


if __name__ == "__main__":
    unittest.main()
