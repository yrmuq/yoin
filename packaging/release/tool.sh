#!/bin/sh
# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later
set -eu
ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
export CARGO_TARGET_DIR="$ROOT/target"
exec cargo run --quiet --manifest-path "$ROOT/packaging/tools/Cargo.toml" -- "$@"
