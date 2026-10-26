# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging" / "release"))

from pins import (  # noqa: E402
    apply_build,
    apply_cache_key,
    apply_flatpak,
    local_python_tar,
    python_choice,
    python_url,
    tool_choice,
)
from version import bump_minor, bump_patch, is_prerelease, parse_version, replace_version  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
ASSET = "cpython-3.12.15+20261013-x86_64-unknown-linux-gnu-install_only_stripped.tar.gz"


class VersionTests(unittest.TestCase):
    def test_patch_and_minor(self):
        self.assertEqual(parse_version("0.1.0"), (0, 1, 0))
        self.assertEqual(bump_patch("0.1.0"), "0.1.1")
        self.assertEqual(bump_patch("0.2.9"), "0.2.10")
        self.assertEqual(bump_minor("0.1.4"), "0.2.0")
        self.assertFalse(is_prerelease("0.2.0"))
        self.assertTrue(is_prerelease("0.2.1"))

    def test_replace_assignment(self):
        source = '__version__ = "0.1.0"\n'
        self.assertEqual(replace_version(source, "0.1.1"), '__version__ = "0.1.1"\n')


class PinTests(unittest.TestCase):
    def test_python_choice_keeps_3_12_and_the_newer_date(self):
        older = {
            "tag_name": "20261001",
            "assets": [{"name": ASSET.replace("20261013", "20261001")}],
        }
        newer = {"tag_name": "20261013", "assets": [{"name": ASSET}]}
        other = {
            "tag_name": "20261020",
            "assets": [
                {
                    "name": "cpython-3.13.1+20261020-x86_64-unknown-linux-gnu-install_only_stripped.tar.gz"
                }
            ],
        }
        chosen = python_choice([older, other, newer])
        self.assertEqual(chosen["tag"], "20261013")
        self.assertIn("%2B20261013", chosen["url"])
        self.assertEqual(local_python_tar(ASSET), "cpython-3.12.15-install_only_stripped.tar.gz")

    def test_tool_choice_skips_prereleases(self):
        self.assertIsNone(tool_choice({"tag_name": "2.0.0", "prerelease": True, "assets": []}))
        chosen = tool_choice(
            {
                "tag_name": "v1.9.1",
                "assets": [{"name": "appimagetool-x86_64.AppImage"}],
            }
        )
        self.assertEqual(
            chosen["url"],
            "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage",
        )

    def test_build_script_roundtrip(self):
        text = (ROOT / "packaging/appimage/build.sh").read_text(encoding="utf-8")
        python = {"tag": "20261013", "name": ASSET, "url": python_url("20261013", ASSET)}
        tool = tool_choice({"tag_name": "1.9.1", "assets": [{"name": "appimagetool-x86_64.AppImage"}]})
        sha = "178d3d83b6e8e99285a65d3fce9c16b87952cc5efdb4d1a0e3ff3be18ff0fc9c"
        tool_sha = "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"
        self.assertEqual(apply_build(text, python, sha, tool, tool_sha), text)

    def test_flatpak_and_cache_key(self):
        flatpak = (ROOT / "packaging/flatpak/io.github.yrmuq.yoin.yml").read_text(encoding="utf-8")
        url = python_url("20261013", ASSET)
        updated = apply_flatpak(
            flatpak,
            url,
            "178d3d83b6e8e99285a65d3fce9c16b87952cc5efdb4d1a0e3ff3be18ff0fc9c",
        )
        self.assertIn(url, updated)
        workflow = (ROOT / ".github/workflows/appimage.yml").read_text(encoding="utf-8")
        self.assertIn("yoin-appimage-deps-20261020", apply_cache_key(workflow, "20261020"))


if __name__ == "__main__":
    unittest.main()
