#!/usr/bin/env bash
# Fail if an LGPL crate reaches the distributable dependency tree.
#
# malachite-q is the differential-test oracle and is LGPL-3.0-only. That licence
# is precisely why it cannot be a real dependency of a crate destined to be
# statically linked into proprietary binaries, so this check enforces
# mechanically what the Cargo.toml comment promises: dev-dependencies only.
set -euo pipefail
cd "$(dirname "$0")/.."

# `cargo tree --edges normal` excludes dev- and build-dependencies.
tree="$(cargo tree --edges normal --all-features --prefix none 2>/dev/null)"

violations="$(printf '%s\n' "$tree" | grep -iE '^(malachite|malachite-q|malachite-base)\b' || true)"
if [ -n "$violations" ]; then
    echo "FAIL: LGPL crate(s) present in the non-dev dependency tree:" >&2
    printf '%s\n' "$violations" >&2
    exit 1
fi
echo "OK: no LGPL crates in the distributable dependency tree."
