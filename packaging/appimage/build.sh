#!/bin/sh
# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
# Build the Rust AppImage. Not a 1.0 release. x86_64 only.
set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
PACK="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
WORK="$PACK/build"
CACHE="$WORK/cache"
APPDIR="$WORK/AppDir"
DIST="$ROOT/dist"
mkdir -p "$CACHE" "$DIST"

TOOL="$CACHE/appimagetool-x86_64.AppImage"
TOOL_URL="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
TOOL_SHA="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"

if [ ! -s "$TOOL" ] || ! echo "$TOOL_SHA  $TOOL" | sha256sum -c - >/dev/null 2>&1; then
    echo "fetch $TOOL_URL"
    curl -fL --retry 3 -o "$TOOL.part" "$TOOL_URL"
    mv "$TOOL.part" "$TOOL"
    echo "$TOOL_SHA  $TOOL" | sha256sum -c -
fi
chmod +x "$TOOL"

if ! command -v file >/dev/null 2>&1; then
    echo "appimagetool needs the file(1) command. Install the file package and rerun." >&2
    exit 1
fi

# The workflow installs fontconfig, xkbcommon, and wayland. FemtoVG also
# needs the OpenGL and EGL headers. The driver itself stays on the host.
if ! pkg-config --exists gl || ! pkg-config --exists egl; then
    echo "installing OpenGL and EGL headers"
    if command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
        SUDO=sudo
    elif [ "$(id -u)" -eq 0 ]; then
        SUDO=
    else
        echo "OpenGL and EGL development files are required (libgl, libegl)." >&2
        exit 1
    fi
    $SUDO apt-get update
    $SUDO apt-get install -y libgl1-mesa-dev libegl1-mesa-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev
fi

cd "$ROOT"
cargo build --release
BIN="$ROOT/target/release/yoin"
test -x "$BIN"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib" \
    "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps" \
    "$APPDIR/usr/share/doc/yoin" \
    "$APPDIR/assets" "$APPDIR/languages"

cp "$BIN" "$APPDIR/usr/bin/yoin"
cp -a "$ROOT/languages/." "$APPDIR/languages/"
cp -a "$ROOT/assets/icon.png" "$ROOT/assets/icon.svg" "$ROOT/assets/tray-dark.png" "$ROOT/assets/tray-light.png" "$APPDIR/assets/"
cp "$ROOT/LICENSE" "$ROOT/NOTICE" "$ROOT/README.md" "$APPDIR/usr/share/doc/yoin/"
cp "$PACK/BUNDLED.txt" "$APPDIR/usr/share/doc/yoin/BUNDLED.txt"

: > "$APPDIR/usr/share/doc/yoin/LIBS.txt"
ldd "$BIN" | awk '/=>/ { print $3 }' | while read -r lib; do
    [ -n "$lib" ] || continue
    [ -f "$lib" ] || continue
    base=$(basename "$lib")
    case "$base" in
        libc.so*|libm.so*|libdl.so*|libpthread.so*|librt.so*|libresolv.so*|libutil.so*|ld-linux*)
            continue
            ;;
        libGL.so*|libEGL.so*|libGLdispatch.so*|libGLX.so*|libOpenGL.so*|libGLESv2.so*|libGLX_mesa.so*|libEGL_mesa.so*|libglapi.so*|libdrm.so*|libgbm.so*|libwayland-egl.so*|libgallium*.so*)
            continue
            ;;
    esac
    cp -a "$lib" "$APPDIR/usr/lib/$base"
    echo "$base" >> "$APPDIR/usr/share/doc/yoin/LIBS.txt"
done
sort -u "$APPDIR/usr/share/doc/yoin/LIBS.txt" -o "$APPDIR/usr/share/doc/yoin/LIBS.txt"

cp "$PACK/AppRun" "$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun" "$APPDIR/usr/bin/yoin"
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
ls -lh "$OUT" "$BIN"
