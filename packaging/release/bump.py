# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
"""Write one version into the program and into chit/lore.md."""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def write_version(version):
    if re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) is None:
        raise SystemExit("version is not X.Y.Z")
    cargo = ROOT / "Cargo.toml"
    lock = ROOT / "Cargo.lock"
    lore = ROOT / "chit" / "lore.md"
    cargo_text = cargo.read_text(encoding="utf-8")
    cargo_new, cargo_count = re.subn(
        r'(?m)^(version = ")[0-9]+\.[0-9]+\.[0-9]+(")',
        r"\g<1>" + version + r"\2",
        cargo_text,
        count=1,
    )
    lore_text = lore.read_text(encoding="utf-8")
    lore_new, lore_count = re.subn(
        r"(Current: `)[0-9]+\.[0-9]+\.[0-9]+(`)",
        r"\g<1>" + version + r"\2",
        lore_text,
        count=1,
    )
    lock_new, lock_count = _bump_lock(lock.read_text(encoding="utf-8"), version)
    if cargo_count != 1 or lore_count != 1 or lock_count != 1:
        raise SystemExit("version line was not found")
    cargo.write_text(cargo_new, encoding="utf-8")
    lore.write_text(lore_new, encoding="utf-8")
    lock.write_text(lock_new, encoding="utf-8")


def _bump_lock(text, version):
    lines = text.splitlines(keepends=True)
    hit = False
    count = 0
    out = []
    for line in lines:
        if line.strip() == 'name = "yoin"':
            hit = True
        elif hit and line.startswith("version = "):
            line = re.sub(r'"[^"]*"', '"%s"' % version, line, count=1)
            hit = False
            count += 1
        out.append(line)
    return "".join(out), count


def main():
    if len(sys.argv) != 2:
        raise SystemExit(2)
    write_version(sys.argv[1])


if __name__ == "__main__":
    main()
