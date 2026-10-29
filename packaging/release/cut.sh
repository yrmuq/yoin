#!/bin/sh
# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
# Cut one tag. patch is the automatic dependency release. minor is manual.
set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
KIND="${1:-patch}"

file_ver="$(sed -n 's/^version = "\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)"/\1/p' Cargo.toml | head -n 1)"
latest="$(git tag -l 'v*.*.*' | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sort -V | tail -n 1 || true)"
if [ "$KIND" = "patch" ] && [ "v${file_ver}" != "${latest}" ]; then
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
            Cargo.toml|Cargo.lock|.github/dependabot.yml|.github/workflows/*|packaging/appimage/build.sh) ;;
            *) IFS=$old_ifs; return 1 ;;
        esac
    done
    IFS=$old_ifs
}

bump() {
    printf '%s\n' "$2" | awk -F. -v kind="$1" '{
        if (kind == "patch") printf "%d.%d.%d\n", $1, $2, $3 + 1
        else printf "%d.%d.0\n", $1, $2 + 1
    }'
}

write_version() {
    ver="$1"
    sed -i "s/^version = \"[0-9][0-9.]*\"/version = \"${ver}\"/" Cargo.toml
    awk -v ver="$ver" '
        $0 == "name = \"yoin\"" { hit = 1 }
        hit && /^version = / { sub(/"[^"]*"/, "\"" ver "\""); hit = 0 }
        { print }
    ' Cargo.lock > Cargo.lock.tmp
    mv Cargo.lock.tmp Cargo.lock
}

if [ "$KIND" = "minor" ]; then
    new="$(bump minor "$file_ver")"
elif [ "$KIND" = "patch" ]; then
    if [ -z "$(git log "${latest}..HEAD" --format='%an')" ]; then
        echo "no commits since ${latest}"
        exit 0
    fi
    if ! bots_only || ! known_files; then
        echo "changes are not dependency updates; waiting for a manual release"
        exit 0
    fi
    new="$(bump patch "$file_ver")"
else
    echo "kind must be patch or minor" >&2
    exit 1
fi

write_version "$new"
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
git add Cargo.toml Cargo.lock
git commit -m "Version ${new}."
git tag -a "v${new}" -m "$notes"
git push origin HEAD
git push origin "v${new}"
echo "tagged v${new}"
