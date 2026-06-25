#!/usr/bin/env bash
#
# Compare lamedh evaluator performance between two git revisions on the
# portable "realistic" workload (benchmarks/realistic/realistic.lisp).
#
# It builds each revision in its own throwaway git worktree, runs the SAME
# workload file on both (so only the engine differs), checks that both produce
# the same checksum (i.e. did identical work), and reports wall-clock time and
# the speedup.
#
# Usage:
#   benchmarks/compare-revisions.sh <rev_a> <rev_b> [reps]
#
# Example (how far we've come since the original benchmark suite):
#   benchmarks/compare-revisions.sh 83c2891 main 15
#
# Notes:
#   - Each revision is built from scratch (~15s), so the first run is slow.
#   - The workload uses only primitives common to old and new builds, so it
#     runs across a wide span of history.
set -euo pipefail

REV_A="${1:?usage: compare-revisions.sh <rev_a> <rev_b> [reps]}"
REV_B="${2:?usage: compare-revisions.sh <rev_a> <rev_b> [reps]}"
REPS="${3:-15}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKLOAD="$REPO_ROOT/benchmarks/realistic/realistic.lisp"
TMP="$(mktemp -d)"

cleanup() {
  for d in "$TMP"/wt-*; do
    [ -d "$d" ] && git -C "$REPO_ROOT" worktree remove --force "$d" >/dev/null 2>&1 || true
  done
  rm -rf "$TMP"
}
trap cleanup EXIT

# Build <rev> in a worktree, run the workload twice, echo "min_seconds checksum".
build_and_time() {
  local rev="$1" tag="$2" wt bin checksum t best
  wt="$TMP/wt-$tag"
  git -C "$REPO_ROOT" worktree add --detach "$wt" "$rev" >/dev/null 2>&1
  ( cd "$wt" && cargo build --release >/dev/null 2>&1 )
  bin="$wt/target/release/lamedh"
  checksum="$("$bin" -i "$WORKLOAD" -s "(bench $REPS)" 2>/dev/null | tail -1)"
  best=""
  for _ in 1 2; do
    local s e
    s=$(date +%s.%N)
    "$bin" -i "$WORKLOAD" -s "(bench $REPS)" >/dev/null 2>&1
    e=$(date +%s.%N)
    t=$(awk -v s="$s" -v e="$e" 'BEGIN{printf "%.3f", e-s}')
    best=$(awk -v t="$t" -v b="$best" 'BEGIN{if(b==""||t+0<b+0)print t; else print b}')
  done
  echo "$best $checksum"
}

echo "Workload:  $WORKLOAD   (reps=$REPS)"
echo "Building and timing $REV_A ..."
read -r TA CA < <(build_and_time "$REV_A" a)
echo "Building and timing $REV_B ..."
read -r TB CB < <(build_and_time "$REV_B" b)

echo
echo "rev A  $REV_A : ${TA}s   checksum=$CA"
echo "rev B  $REV_B : ${TB}s   checksum=$CB"
echo
if [ "$CA" != "$CB" ]; then
  echo "WARNING: checksums differ ($CA vs $CB) — the two builds did NOT do the"
  echo "same work; the timing comparison is not meaningful."
  exit 1
fi
awk -v a="$TA" -v b="$TB" -v ra="$REV_A" -v rb="$REV_B" 'BEGIN{
  printf "same checksum (identical work)\n";
  if (b+0>0) printf "%s is %.2fx the speed of %s\n", rb, a/b, ra;
}'
