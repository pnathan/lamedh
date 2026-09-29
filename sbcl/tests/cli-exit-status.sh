#!/bin/sh
# cli-exit-status.sh -- regression test for #535: the documented
# `sbcl --eval ... (lamedh-rt:toplevel)` script invocation must exit 0 on a
# clean script and 1 on an erroring one, without an unhandled SBCL condition
# on the way out (an erroring exit path would also yield status 1, so the
# absence of "Unhandled" is what distinguishes the fix on the error run).
set -u
cd "$(dirname "$0")/.." || exit 1
tmp=$(mktemp -d) || exit 1
trap 'rm -rf "$tmp"' EXIT
echo '(+ 1 2)' > "$tmp/ok.lisp"
echo '(undefined-fn)' > "$tmp/bad.lisp"

fail=0
check() { # check NAME EXPECTED-STATUS
  sbcl --non-interactive \
    --eval '(require :asdf)' \
    --eval '(asdf:load-asd (truename "lamedh.asd"))' \
    --eval '(asdf:load-system :lamedh)' \
    --eval '(lamedh-rt:toplevel)' \
    "$tmp/$1.lisp" > "$tmp/$1.out" 2>&1
  status=$?
  if [ "$status" -ne "$2" ]; then
    echo "FAIL: $1.lisp exited $status, expected $2"; fail=1
  elif grep -q 'Unhandled' "$tmp/$1.out"; then
    echo "FAIL: $1.lisp exited through an unhandled condition:"
    grep -A4 'Unhandled' "$tmp/$1.out"; fail=1
  else
    echo "ok: $1.lisp exited $status"
  fi
}
check ok 0
check bad 1
exit $fail
