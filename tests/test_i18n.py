# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

import unittest

from yoin.config import normalize
from yoin.i18n import _read, _file, current, match_language, tr, trf, use
from yoin.theme import colors, kde_is_dark, prefers_dark, resolved_theme


class LanguageTests(unittest.TestCase):
    def tearDown(self) -> None:
        use("English")

    def test_english_and_russian_share_ids_and_placeholders(self):
        english = _read(_file("English"))
        russian = _read(_file("Russian"))
        self.assertEqual(set(english), set(russian))
        for number, row in english.items():
            self.assertEqual(row["text"].count("{}"), russian[number]["text"].count("{}"))
            self.assertEqual(row["en"], russian[number]["en"])

    def test_system_list_picks_russian_once(self):
        self.assertEqual(match_language(["ru_FI", "en_US"]), "Russian")
        self.assertEqual(match_language(["fi", "sv"]), "English")
        self.assertEqual(match_language([]), "English")
        use("Russian")
        self.assertEqual(current(), "Russian")
        self.assertEqual(tr(1), "Настройки")
        self.assertEqual(trf(20, "disk"), "Не удалось записать автозапуск: disk")
        use("Missing")
        self.assertEqual(current(), "English")
        self.assertEqual(tr(1), "Settings")

    def test_old_config_leaves_language_unset_and_follows_the_system_theme(self):
        cfg = normalize({"url": "http://ha.local"})
        self.assertEqual(cfg["language"], "")
        self.assertEqual(cfg["theme"], "system")
        kept = normalize({"language": "Russian", "theme": "light"})
        self.assertEqual(kept["language"], "Russian")
        self.assertEqual(kept["theme"], "light")
        self.assertEqual(normalize({"theme": "blue"})["theme"], "system")


class ThemeTests(unittest.TestCase):
    def test_contrast_and_resolution(self):
        light = colors("light")
        dark = colors("dark")
        self.assertLess(self._luma(light["fg"]), 80)
        self.assertGreater(self._luma(light["bg"]), 200)
        self.assertGreater(self._luma(dark["fg"]), 180)
        self.assertLess(self._luma(dark["bg"]), 50)
        self.assertGreater(abs(self._luma(light["fg"]) - self._luma(light["bg"])), 120)
        self.assertGreater(abs(self._luma(dark["fg"]) - self._luma(dark["bg"])), 120)
        self.assertEqual(resolved_theme("system", True), "dark")
        self.assertEqual(resolved_theme("system", False), "light")
        self.assertEqual(resolved_theme("light", True), "light")
        self.assertEqual(resolved_theme("dark", False), "dark")

    def test_kde_background_wins_when_qt_has_no_preference(self):
        text = "[General]\nColorScheme=BreezeLight\n\n[Colors:Window]\nBackgroundNormal=30,32,38\n"
        self.assertTrue(kde_is_dark(text))
        self.assertFalse(prefers_dark("unknown", "[General]\nColorScheme=BreezeLight\n", None))
        self.assertTrue(prefers_dark("dark", "[General]\nColorScheme=BreezeLight\n", None))
        self.assertFalse(prefers_dark("unknown", "", 200))
        self.assertTrue(prefers_dark("unknown", "", 20))

    def _luma(self, value: str) -> float:
        red = int(value[1:3], 16)
        green = int(value[3:5], 16)
        blue = int(value[5:7], 16)
        return 0.2126 * red + 0.7152 * green + 0.0722 * blue


if __name__ == "__main__":
    unittest.main()
