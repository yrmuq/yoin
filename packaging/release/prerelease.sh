#!/bin/sh
# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
# Tag the next dependency prerelease. The caller has already tested this tree.
# Old patch tags stay, so the number does not go back to 1.
set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

file_ver="$(sed -n 's/^version = "\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)"/\1/p' Cargo.toml | head -n 1)"
major="$(printf '%s\n' "$file_ver" | awk -F. '{ print $1 }')"
minor="$(printf '%s\n' "$file_ver" | awk -F. '{ print $2 }')"
max="$(printf '%s\n' "$file_ver" | awk -F. '{ print $3 }')"
for tag in $(git tag -l "v${major}.${minor}.*"); do
    case "$tag" in
        v[0-9]*.[0-9]*.[0-9]*) ;;
        *) continue ;;
    esac
    patch="$(printf '%s\n' "$tag" | awk -F. '{ print $3 }' | tr -cd '0-9')"
    if [ -n "$patch" ] && [ "$patch" -gt "$max" ]; then
        max="$patch"
    fi
done
new="${major}.${minor}.$((max + 1))"
if git rev-parse -q --verify "refs/tags/v${new}" >/dev/null; then
    echo "tag v${new} already exists" >&2
    exit 1
fi

sed -i "s/^version = \"[0-9][0-9.]*\"/version = \"${new}\"/" Cargo.toml
awk -v ver="$new" '
    $0 == "name = \"yoin\"" { hit = 1 }
    hit && /^version = / { sub(/"[^"]*"/, "\"" ver "\""); hit = 0 }
    { print }
' Cargo.lock > Cargo.lock.tmp
mv Cargo.lock.tmp Cargo.lock

notes='**Changes:**
Dependency updates.'
export GIT_AUTHOR_NAME="${GIT_AUTHOR_NAME:-github-actions[bot]}"
export GIT_AUTHOR_EMAIL="${GIT_AUTHOR_EMAIL:-41898282+github-actions[bot]@users.noreply.github.com}"
export GIT_COMMITTER_NAME="${GIT_COMMITTER_NAME:-$GIT_AUTHOR_NAME}"
export GIT_COMMITTER_EMAIL="${GIT_COMMITTER_EMAIL:-$GIT_AUTHOR_EMAIL}"
git add Cargo.toml Cargo.lock
git commit -m "Version ${new}."
git tag -a "v${new}" -m "$notes"
git push --force origin HEAD:refs/heads/deps
git push origin "refs/tags/v${new}"
printf '%s\n' "$new"
