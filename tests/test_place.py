# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

import unittest

from yoin.place import place_script


class PlaceTests(unittest.TestCase):
    def test_script_carries_the_target(self):
        text = place_script("yoin-buttons", 12, 34, 280, 120)
        self.assertIn('indexOf("yoin-buttons")', text)
        self.assertIn("Qt.rect(12, 34, 280, 120)", text)
        self.assertIn("workspace.windowList", text)

    def test_title_quotes_do_not_break_the_script(self):
        text = place_script('say "hi"', 0, 0, 1, 1)
        self.assertIn(r'say \"hi\"', text)
        self.assertIn("Qt.rect(0, 0, 1, 1)", text)


if __name__ == "__main__":
    unittest.main()
