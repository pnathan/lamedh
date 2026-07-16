#!/usr/bin/env bash
#
# The authoritative gauntlet — the single command that decides whether a
# branch may ship. Runs the full verification and writes an EXPLICIT verdict
# file the caller must gate its ship decision on (never chain a commit/PR/
# merge after a bare `cat` — the PR #290 / #351 lesson: a red suite that
# scrolled past is still red).
#
# RELEASE by default. The two slowest suites (test_examples, which builds a
# fresh stdlib environment per example, and brutal_correctness, the tiered
# JIT differential fuzzer) are EXECUTION-bound, not compile-bound; release
# runs them several times faster, more than repaying release's one-time
# compile. Iterate on a single failing test in debug (`cargo test --test X`);
# run THIS for the final verdict.
#
# Usage:
#   scripts/gauntlet.sh [VERDICT_FILE]
#   VERDICT_FILE defaults to $TMPDIR/lamedh-verdict.
#
# A ship is authorized iff the verdict file contains exactly:
#   DEFAULT-GREEN
#   NDF-GREEN
#   FUZZ-GREEN
#   CLIPPY-GREEN
#
# Extra feature suites (e.g. `--features net-tls`) are the caller's job to
# add on top when a branch touches feature-gated code.
set -u

VERDICT="${1:-${TMPDIR:-/tmp}/lamedh-verdict}"
LOGDIR="${TMPDIR:-/tmp}"
rm -f "$VERDICT"

echo "gauntlet: default suite (release)..."
cargo test --release > "$LOGDIR/gauntlet-default.log" 2>&1 && echo DEFAULT-GREEN >> "$VERDICT"

echo "gauntlet: --no-default-features (release)..."
cargo test --release --no-default-features > "$LOGDIR/gauntlet-ndf.log" 2>&1 && echo NDF-GREEN >> "$VERDICT"

echo "gauntlet: fuzz battery (release)..."
# The randomized JIT differential/metamorphic battery lives behind the `fuzz`
# feature so it never taxes plain `cargo test`. The ship gate runs it — in
# release, where minutes of debug fuzz become seconds. Set BRUTAL=1 in the
# environment for the deep sweep (CI / release checks).
cargo test --release --features fuzz --test brutal_correctness > "$LOGDIR/gauntlet-fuzz.log" 2>&1 && echo FUZZ-GREEN >> "$VERDICT"

echo "gauntlet: clippy (warnings are errors)..."
cargo clippy --workspace --all-targets -- -D warnings > "$LOGDIR/gauntlet-clippy.log" 2>&1 && echo CLIPPY-GREEN >> "$VERDICT"

echo "gauntlet: fmt..."
cargo fmt --all

echo "=== verdict ($VERDICT) ==="
cat "$VERDICT" 2>/dev/null
echo "==="
echo "logs: $LOGDIR/gauntlet-{default,ndf,clippy}.log"
