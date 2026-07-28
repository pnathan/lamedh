#!/usr/bin/env bash
# Run the Verus verifier over the crate.
#
# Requires a `verus` binary (a release from
# https://github.com/verus-lang/verus/releases, or a `vargo build --release`
# tree). Point $VERUS at it if it is not on PATH.
#
#   VERUS=/path/to/verus ./scripts/verify.sh
#
# Extra flags are forwarded, e.g. `./scripts/verify.sh --profile`.
set -euo pipefail
VERUS="${VERUS:-verus}"
cd "$(dirname "$0")/.."
exec "$VERUS" --crate-type=lib --rlimit "${VERUS_RLIMIT:-100}" "$@" src/lib.rs
