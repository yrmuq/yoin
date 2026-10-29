#!/bin/sh
# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
# Refresh the appimagetool pin. Exit 2 when it is already current.
set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
BUILD="$ROOT/packaging/appimage/build.sh"
json="$(curl -fsSL --retry 3 -H "Accept: application/vnd.github+json" -H "User-Agent: yoin-pins" \
    https://api.github.com/repos/AppImage/appimagetool/releases/latest)"
url="$(printf '%s\n' "$json" | jq -r '.assets[] | select(.name=="appimagetool-x86_64.AppImage") | .browser_download_url')"
case "$url" in
    https://github.com/AppImage/appimagetool/releases/download/*/appimagetool-x86_64.AppImage) ;;
    *)
        echo "could not find appimagetool" >&2
        exit 1
        ;;
esac
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
curl -fL --retry 3 -o "$tmp" "$url"
sha="$(sha256sum "$tmp" | awk '{ print $1 }')"
old_url="$(sed -n 's/^TOOL_URL="\(.*\)"/\1/p' "$BUILD")"
old_sha="$(sed -n 's/^TOOL_SHA="\(.*\)"/\1/p' "$BUILD")"
if [ "$old_url" = "$url" ] && [ "$old_sha" = "$sha" ]; then
    echo "pin already current"
    exit 2
fi
sed -i "s|^TOOL_URL=.*|TOOL_URL=\"${url}\"|" "$BUILD"
sed -i "s|^TOOL_SHA=.*|TOOL_SHA=\"${sha}\"|" "$BUILD"
echo "appimagetool ${url}"
