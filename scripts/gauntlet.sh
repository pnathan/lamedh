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
# The exit status agrees with the verdict: 0 iff all four markers are present.
# The verdict file stays the record; the exit status exists so a failure can
# no longer scroll past as a missing line in an otherwise quiet run.
#
# Extra feature suites (e.g. `--features net-tls`) are the caller's job to
# add on top when a branch touches feature-gated code.
set -u

VERDICT="${1:-${TMPDIR:-/tmp}/lamedh-verdict}"
LOGDIR="${TMPDIR:-/tmp}"
rm -f "$VERDICT"

# Test RUN time dominates the gauntlet (compile+link is about a minute per
# phase; the runs were 5+ minutes each). `cargo test` runs the test binaries
# one after another, and nextest schedules every test across all binaries at
# once, which roughly halves the wall time. nextest does not run doctests, so
# they run separately. Without nextest installed, fall back to `cargo test`:
# same verdicts, just slower.
run_tests() {
  if cargo nextest --version > /dev/null 2>&1; then
    cargo nextest run --release "$@" && cargo test --release --doc "$@"
  else
    cargo test --release "$@"
  fi
}

echo "gauntlet: default suite + fuzz battery (release)..."
# The randomized JIT differential/metamorphic battery lives behind the `fuzz`
# feature, which gates only that test target (tests/brutal_correctness.rs) and
# changes nothing else, so one `--features fuzz` build and run serves both the
# DEFAULT and FUZZ verdicts. FUZZ-GREEN additionally requires the battery to
# appear in the log, so a build where it was compiled out cannot pass. Set
# BRUTAL=1 in the environment for the deep sweep (CI / release checks).
if run_tests --features fuzz > "$LOGDIR/gauntlet-default.log" 2>&1; then
  echo DEFAULT-GREEN >> "$VERDICT"
  grep -q brutal_correctness "$LOGDIR/gauntlet-default.log" && echo FUZZ-GREEN >> "$VERDICT"
fi

echo "gauntlet: --no-default-features (release)..."
run_tests --no-default-features > "$LOGDIR/gauntlet-ndf.log" 2>&1 && echo NDF-GREEN >> "$VERDICT"

echo "gauntlet: clippy (warnings are errors)..."
# `--features fuzz` so the fuzz battery is linted too: `fuzz` gates nothing
# but that test target, and without it clippy (like plain `cargo test`) never
# compiles the file at all.
cargo clippy --workspace --all-targets --features fuzz -- -D warnings > "$LOGDIR/gauntlet-clippy.log" 2>&1 && echo CLIPPY-GREEN >> "$VERDICT"

echo "gauntlet: fmt..."
cargo fmt --all

echo "=== verdict ($VERDICT) ==="
cat "$VERDICT" 2>/dev/null
echo "==="
echo "logs: $LOGDIR/gauntlet-{default,ndf,clippy}.log"

for marker in DEFAULT-GREEN NDF-GREEN FUZZ-GREEN CLIPPY-GREEN; do
  if ! grep -qx "$marker" "$VERDICT" 2>/dev/null; then
    echo "gauntlet: $marker missing -- NOT authorized to ship"
    exit 1
  fi
done
echo "gauntlet: all four markers present"
