#!/bin/sh
# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
# Cut one tag. patch is the automatic dependency release. minor is manual.
set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
KIND="${1:-patch}"

file_ver="$(python3 -c 'import pathlib,re; t=pathlib.Path("yoin/__init__.py").read_text(encoding="utf-8"); print(re.search(r"__version__ = \"([0-9]+\.[0-9]+\.[0-9]+)\"", t).group(1))')"
latest="$(git tag -l 'v*.*.*' | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sort -V | tail -n 1 || true)"
if [ "v${file_ver}" != "${latest}" ]; then
    echo "tag ${latest:-<none>} does not match ${file_ver}" >&2
    exit 1
fi

bots_only() {
    old_ifs=$IFS
    IFS='
'
    for name in $(git log "${latest}..HEAD" --format='%an'); do
        case "$name" in
            "dependabot[bot]"|"github-actions[bot]") ;;
            *) IFS=$old_ifs; return 1 ;;
        esac
    done
    IFS=$old_ifs
}

known_files() {
    old_ifs=$IFS
    IFS='
'
    for path in $(git diff --name-only "${latest}..HEAD"); do
        case "$path" in
            requirements.txt|packaging/appimage/build.sh|packaging/flatpak/io.github.yrmuq.yoin.yml|.github/dependabot.yml|.github/workflows/*) ;;
            *) IFS=$old_ifs; return 1 ;;
        esac
    done
    IFS=$old_ifs
}

if [ "$KIND" = "minor" ]; then
    new="$(python3 packaging/release/version.py minor "$file_ver")"
elif [ "$KIND" = "patch" ]; then
    if [ -z "$(git log "${latest}..HEAD" --format='%an')" ]; then
        echo "no commits since ${latest}"
        exit 0
    fi
    if ! bots_only || ! known_files; then
        echo "changes are not dependency updates; waiting for a manual release"
        exit 0
    fi
    new="$(python3 packaging/release/version.py patch "$file_ver")"
else
    echo "kind must be patch or minor" >&2
    exit 1
fi

python3 packaging/release/version.py write "$new" yoin/__init__.py
if [ -n "${NOTES:-}" ]; then
    notes="$NOTES"
elif [ "$KIND" = "minor" ]; then
    notes='**Changes:**
Manual release.'
else
    notes='**Changes:**
Dependency updates.'
fi
export GIT_AUTHOR_NAME="${GIT_AUTHOR_NAME:-github-actions[bot]}"
export GIT_AUTHOR_EMAIL="${GIT_AUTHOR_EMAIL:-41898282+github-actions[bot]@users.noreply.github.com}"
export GIT_COMMITTER_NAME="${GIT_COMMITTER_NAME:-$GIT_AUTHOR_NAME}"
export GIT_COMMITTER_EMAIL="${GIT_COMMITTER_EMAIL:-$GIT_AUTHOR_EMAIL}"
git add yoin/__init__.py
git commit -m "Version ${new}."
git tag -a "v${new}" -m "$notes"
git push origin HEAD
git push origin "v${new}"
echo "tagged v${new}"
