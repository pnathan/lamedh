#!/bin/bash
# Generate per-implementation sources from kernels/*.lisp into build/<impl>/.
#   asm          verbatim (the kernels are written in the asm port's dialect)
#   rust-interp  generic operators + (declaim (no-compile ...)) for every DEFUN
#   rust-default generic operators, plain DEFUN (auto-compile left to the host)
#   sbcl         generic operators
#   rust-jit     kernels-typed/*.lisp verbatim (hand-typed DEFUN-TYPED editions)
#   asm-scaled / rust-jit-scaled
#                the two native tiers with the final (print ...) replaced by the
#                kernel's ";; scaled:" line (larger inputs; the interpreter-sized
#                runs are too short on these tiers to resolve against startup)
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$HERE/build"
rm -rf "$OUT"; mkdir -p "$OUT"/{asm,rust-interp,rust-default,sbcl,rust-jit,asm-scaled,rust-jit-scaled}
generic() {
  # Symbol-boundary rewrite of the asm-only operator names.
  perl -pe 'next if /^\s*;/;
    my $b = q{(?<![^\s()\x27`,])};  my $e = q{(?![^\s()])};
    s/${b}F\+${e}/+/gi; s/${b}F-${e}/-/gi; s/${b}F\*${e}/*/gi;
    s/${b}F\/${e}/\//gi; s/${b}F<${e}/</gi;
    s/${b}STRING-APPEND${e}/concat/gi; s/${b}STRING-LENGTH${e}/length/gi;
  ' "$1"
}
for k in "$HERE"/kernels/*.lisp; do
  n=$(basename "$k")
  cp "$k" "$OUT/asm/$n"
  generic "$k" > "$OUT/sbcl/$n"
  generic "$k" > "$OUT/rust-default/$n"
  names=$(grep -oiE '^\(defun[[:space:]]+[^[:space:]()]+' "$k" | awk '{print $2}' | tr '\n' ' ')
  { echo "(declaim (no-compile $names))"; generic "$k"; } > "$OUT/rust-interp/$n"
done
for k in "$HERE"/kernels-typed/*.lisp; do
  [ -e "$k" ] && cp "$k" "$OUT/rust-jit/$(basename "$k")"
done
scale() {  # file scaled-form
  grep -v '^(print' "$1"; echo "$2"
}
for k in "$HERE"/kernels/*.lisp; do
  n=$(basename "$k")
  line=$(sed -n 's/^;; scaled: //p' "$k")
  [ -n "$line" ] || continue
  scale "$k" "$line" > "$OUT/asm-scaled/$n"
  [ -e "$HERE/kernels-typed/$n" ] && scale "$HERE/kernels-typed/$n" "$line" > "$OUT/rust-jit-scaled/$n"
done
true
