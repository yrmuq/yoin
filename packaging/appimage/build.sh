#!/bin/sh
# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
# Build a pre-release AppImage. Not a 1.0 release. x86_64 only.
# Flatpak and arm64 wait until 1.0. See packaging/appimage/BUNDLED.txt.
set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
PACK="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
WORK="$PACK/build"
CACHE="$WORK/cache"
APPDIR="$WORK/AppDir"
DIST="$ROOT/dist"
mkdir -p "$CACHE" "$DIST"

PY_URL="https://github.com/astral-sh/python-build-standalone/releases/download/20261013/cpython-3.12.15%2B20261013-x86_64-unknown-linux-gnu-install_only_stripped.tar.gz"
PY_SHA="178d3d83b6e8e99285a65d3fce9c16b87952cc5efdb4d1a0e3ff3be18ff0fc9c"
PY_TAR="$CACHE/cpython-3.12.15-install_only_stripped.tar.gz"
TOOL="$CACHE/appimagetool-x86_64.AppImage"
TOOL_URL="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
TOOL_SHA="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"

fetch() {
    url="$1"
    dest="$2"
    want="$3"
    if [ -s "$dest" ] && echo "$want  $dest" | sha256sum -c - >/dev/null 2>&1; then
        return 0
    fi
    echo "fetch $url"
    curl -fL --retry 3 -o "$dest.part" "$url"
    mv "$dest.part" "$dest"
    if ! echo "$want  $dest" | sha256sum -c -; then
        echo "sha256 mismatch for $dest" >&2
        rm -f "$dest"
        exit 1
    fi
}

fetch "$PY_URL" "$PY_TAR" "$PY_SHA"
fetch "$TOOL_URL" "$TOOL" "$TOOL_SHA"
chmod +x "$TOOL"

if ! command -v file >/dev/null 2>&1; then
    echo "appimagetool needs the file(1) command. Install the file package and rerun." >&2
    exit 1
fi

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" \
    "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps" \
    "$APPDIR/assets"

tar -xzf "$PY_TAR" -C "$APPDIR"
test -x "$APPDIR/python/bin/python3"

"$APPDIR/python/bin/python3" -m pip install \
    --disable-pip-version-check \
    --no-warn-script-location \
    -r "$ROOT/requirements.txt"
"$APPDIR/python/bin/python3" -m pip freeze | grep -i '^PySide6' > "$APPDIR/payload-pyside.txt" || true
site=$("$APPDIR/python/bin/python3" -c 'import sysconfig; print(sysconfig.get_path("purelib"))')
rm -rf "$site/pip" "$site"/pip-*.dist-info
rm -f "$APPDIR/python/bin/pip" "$APPDIR/python/bin/pip3" "$APPDIR/python/bin/pip3.12"

mkdir -p "$APPDIR/payload"
cp -a "$ROOT/yoin" "$ROOT/LICENSE" "$ROOT/NOTICE" "$ROOT/README.md" "$APPDIR/payload/"
cp -a "$ROOT/languages" "$APPDIR/payload/languages"
cp -a "$PACK/BUNDLED.txt" "$APPDIR/payload/BUNDLED.txt"
mv "$APPDIR/payload-pyside.txt" "$APPDIR/payload/PYSIDE.txt"
cp -a "$ROOT/assets/icon.png" "$ROOT/assets/icon.svg" "$ROOT/assets/tray-dark.png" "$ROOT/assets/tray-light.png" "$APPDIR/assets/"
find "$APPDIR/payload" -type d -name __pycache__ -print0 | xargs -0r rm -rf
find "$APPDIR/payload" -name '*.pyc' -delete

cp "$PACK/AppRun" "$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun"
ln -s ../../AppRun "$APPDIR/usr/bin/yoin"
cp "$PACK/yoin.desktop" "$APPDIR/yoin.desktop"
cp "$PACK/yoin.desktop" "$APPDIR/usr/share/applications/yoin.desktop"
cp "$ROOT/assets/icon.png" "$APPDIR/yoin.png"
cp "$ROOT/assets/icon.png" "$APPDIR/.DirIcon"
cp "$ROOT/assets/icon.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/yoin.png"

OUT="$DIST/yoin-x86_64.AppImage"
rm -f "$OUT"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" "$APPDIR" "$OUT"
chmod +x "$OUT"
echo "wrote $OUT"
ls -lh "$OUT"
