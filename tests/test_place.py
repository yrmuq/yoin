# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

import os
import unittest

from yoin.notify import notification_arguments
from yoin.place import placer_script, popup_origin


class PlaceTests(unittest.TestCase):
    def test_script_pins_the_popup_to_the_panel(self):
        text = placer_script()
        self.assertIn('caption === "yoin-buttons"', text)
        self.assertIn('resourceClass) === "plasmashell"', text)
        self.assertIn("workspace.cursorPos", text)
        self.assertIn("workspace.windowAdded.connect(track)", text)
        self.assertIn("window.frameGeometry = target", text)
        self.assertIn("skipTaskbar", text)

    def test_origin_sits_on_the_tray(self):
        # Panel along the bottom. Available area ends at y=1000. Tray is on the panel.
        origin = popup_origin((1800, 1040, 32, 32), (1816, 1056), (0, 0, 1920, 1000), (280, 120))
        self.assertEqual(origin, (1640, 880))

    def test_origin_uses_a_cursor_on_the_bottom_edge(self):
        origin = popup_origin((0, 0, 0, 0), (400, 990), (0, 0, 1920, 1000), (280, 100))
        self.assertEqual(origin, (260, 882))

    def test_origin_uses_a_cursor_on_the_top_edge(self):
        origin = popup_origin((0, 0, 0, 0), (400, 10), (0, 40, 1920, 1000), (280, 100))
        self.assertEqual(origin, (260, 40))


class NotifyTests(unittest.TestCase):
    def test_arguments_carry_the_icon_and_are_not_transient(self):
        args = notification_arguments("Door", "open", "/tmp/yoin/assets/icon.png", 8000)
        self.assertEqual(args[0], "yoin")
        self.assertEqual(args[2], os.path.expanduser("~/.cache/yoin/notify.png"))
        self.assertEqual(args[3], "Door")
        self.assertEqual(args[4], "open")
        self.assertEqual(args[6], {})
        self.assertNotIn("transient", args[6])
        self.assertEqual(args[7], 8000)
        self.assertTrue(os.path.isfile(args[2]))

    def test_missing_icon_is_empty(self):
        args = notification_arguments("yoin", "hi", "/tmp/yoin/assets/missing.png")
        self.assertEqual(args[2], "")

    def test_announce_does_not_raise_without_a_notification_daemon(self):
        from yoin.notify import announce

        self.assertIs(announce("yoin", "hi", "/tmp/yoin/assets/icon.png"), False)


if __name__ == "__main__":
    unittest.main()
