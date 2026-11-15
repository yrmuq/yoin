#!/bin/sh
# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
# Merge REF into HEAD. A clean merge is kept.
# If the only conflicts are the version files, take those files from REF.
# Any other conflict stops the merge.
set -eu

ref="${1:?ref}"
name="${ref#origin/}"
git merge "$ref" -m "Merge branch '${name}'" && exit 0
conflicts="$(git diff --name-only --diff-filter=U)"
other="$(printf '%s\n' "$conflicts" | grep -v -E '^(Cargo.toml|Cargo.lock)$' || true)"
if [ -n "$other" ] || [ -z "$conflicts" ]; then
    echo "stopped on a conflict with ${ref}" >&2
    printf '%s\n' "$conflicts" >&2
    exit 1
fi
git checkout --theirs -- Cargo.toml Cargo.lock
git add Cargo.toml Cargo.lock
git commit -m "Merge branch '${name}'"
