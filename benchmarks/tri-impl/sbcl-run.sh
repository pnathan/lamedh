#!/bin/bash
# Run one Lamedh file on the SBCL port (loads the port + embedded stdlib first).
F="$(realpath "$1")"
cd "$(dirname "$0")/../../sbcl" && exec sbcl --noinform --non-interactive \
  --eval '(require :asdf)' --eval '(asdf:load-asd (truename "lamedh.asd"))' \
  --eval '(asdf:load-system :lamedh)' --eval '(lamedh-rt:toplevel)' "$F"
