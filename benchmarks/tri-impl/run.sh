#!/bin/bash
# Tri-implementation benchmark: Rust reference (interpreter / default / typed
# JIT), SBCL port, x86-64 assembly port. See README.md for the method.
#
#   ./run.sh               build, verify outputs, time everything
#   RUNS=10 ./run.sh       more timed runs per cell (default 5, plus 1 warmup)
#   ONLY="fib ack" ./run.sh   restrict to some kernels
#   SKIP_BUILD=1 ./run.sh  reuse existing binaries
#
# Writes results/ (hyperfine JSON, outputs, tiers, machine.txt, summary.md).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
RES="$HERE/results"
RUNS=${RUNS:-5}
WARMUP=${WARMUP:-1}
RUST="$ROOT/target/release/lamedh"
ASM="$ROOT/lamedh-asm/build/lamedhc"
IMPLS=(rust-interp rust-default rust-jit sbcl asm asm-scaled rust-jit-scaled)

need() { command -v "$1" >/dev/null || { echo "missing: $1 (apt-get install -y $2)" >&2; exit 1; }; }
need hyperfine hyperfine; need sbcl sbcl; need nasm nasm; need python3 python3

if [ -z "${SKIP_BUILD:-}" ]; then
  (cd "$ROOT" && cargo build --release -q)
  (cd "$ROOT/lamedh-asm" && make -s lamedhc >/dev/null 2>&1)
  # Warm SBCL's ASDF fasl cache so the first timed run is not a compile.
  "$HERE/sbcl-run.sh" /dev/null >/dev/null 2>&1 || true
fi
"$HERE/gen.sh"
rm -rf "$RES"; mkdir -p "$RES/json"
: > "$RES/empty.lisp"

cmd_for() {  # impl file -> command line
  case "$1" in
    rust-*) echo "$RUST $2" ;;
    asm*)   echo "$ASM $2" ;;
    sbcl)   echo "$HERE/sbcl-run.sh $2" ;;
  esac
}
normalize() { grep -v '^;' | tr -d ' \n\r'; }

# ---- machine / versions ---------------------------------------------------
{
  echo "date:      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "commit:    $(git -C "$ROOT" rev-parse HEAD)"
  echo "cpu:       $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //')"
  echo "nproc:     $(nproc)"
  echo "kernel:    $(uname -r)"
  echo "rustc:     $(rustc --version)"
  echo "lamedh:    $("$RUST" --version 2>&1 | head -1)"
  echo "sbcl:      $(sbcl --version)"
  echo "nasm:      $(nasm -v)"
  echo "hyperfine: $(hyperfine --version)"
  echo "runs:      $RUNS timed + $WARMUP warmup per cell"
} > "$RES/machine.txt"
cat "$RES/machine.txt"

kernels=()
for k in "$HERE"/kernels/*.lisp; do
  n=$(basename "$k" .lisp)
  if [ -n "${ONLY:-}" ] && [[ " $ONLY " != *" $n "* ]]; then continue; fi
  kernels+=("$n")
done

# ---- correctness: every implementation must print the same value ----------
echo; echo "== outputs"
printf "kernel\timpl\toutput\n" > "$RES/outputs.tsv"
for n in "${kernels[@]}"; do
  for impl in "${IMPLS[@]}"; do
    f="$HERE/build/$impl/$n.lisp"
    [ -e "$f" ] || { [[ $impl == *-scaled ]] || printf "%s\t%s\t%s\n" "$n" "$impl" "N/A" >> "$RES/outputs.tsv"; continue; }
    out=$($(cmd_for "$impl" "$f") 2>"$RES/stderr-$impl-$n.txt" | normalize || true)
    printf "%s\t%s\t%s\n" "$n" "$impl" "${out:-<none>}" >> "$RES/outputs.tsv"
  done
done
sed 's/\t/  /g' "$RES/outputs.tsv"

# ---- tier report: what the Rust host actually ran --------------------------
echo; echo "== rust tiers (compiled-p per DEFUN / DEFUN-TYPED)"
: > "$RES/tiers.txt"
for impl in rust-interp rust-default rust-jit; do
  for n in "${kernels[@]}"; do
    f="$HERE/build/$impl/$n.lisp"; [ -e "$f" ] || continue
    probe="$RES/probe.lisp"
    grep -v '^(print' "$f" > "$probe"
    for x in $(grep -oiE '^\(defun(-typed)?[[:space:]]+\(?[^[:space:]()]+' "$f" | awk '{print $2}' | tr -d '('); do
      echo "(print (list '$x (compiled-p '$x)))" >> "$probe"
    done
    echo "$impl $n: $("$RUST" "$probe" 2>&1 | tr '\n' ' ')" >> "$RES/tiers.txt"
  done
done
rm -f "$RES/probe.lisp"
cat "$RES/tiers.txt"

# ---- startup ---------------------------------------------------------------
echo; echo "== startup (empty program)"
for impl in rust asm sbcl; do
  case $impl in rust) c="$RUST $RES/empty.lisp";; asm) c="$ASM $RES/empty.lisp";; sbcl) c="$HERE/sbcl-run.sh $RES/empty.lisp";; esac
  hyperfine -N -i -w 2 -r $((RUNS * 2)) --export-json "$RES/json/startup-$impl.json" "$c" 2>&1 | grep -E 'Time|Range' || true
done

# ---- timing ------------------------------------------------------------------
echo; echo "== timing"
for n in "${kernels[@]}"; do
  for impl in "${IMPLS[@]}"; do
    f="$HERE/build/$impl/$n.lisp"; [ -e "$f" ] || continue
    echo "-- $n / $impl"
    # -i: the SBCL CLI always exits 1 (known :UNIX-STATUS bug); correctness is
    # established by the output check above, not by exit status.
    hyperfine -N -i -w "$WARMUP" -r "$RUNS" --export-json "$RES/json/$n--$impl.json" \
      "$(cmd_for "$impl" "$f")" 2>&1 | grep -E 'Time|Range' || true
  done
done

python3 "$HERE/summarize.py" "$RES" | tee "$RES/summary.md"
