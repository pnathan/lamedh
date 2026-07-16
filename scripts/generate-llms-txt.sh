#!/bin/bash
# Generate llms.txt: the single-file, dense LLM reference for Lamedh.
#
# Two pieces are spliced into docs/llms-txt-template.md (which holds the
# hand-written framing: intro, gotchas, worked examples):
#
#   {{MODULE_TABLE}}    - the stdlib module/tier/key-functions table,
#                          extracted verbatim from src/lib.rs's doc
#                          comments (the authoritative, human-maintained
#                          module summary -- see src/lib.rs around
#                          "| File | Tier | Requirable as | Key functions |").
#   {{FUNCTION_INDEX}}   - a dense one-line-per-symbol reference generated
#                          live from the built interpreter's HELP-DB via
#                          lib/97-doc-renderer.lisp's render-llms-index.
#
# Usage: ./scripts/generate-llms-txt.sh
# Called from scripts/generate-docs.sh; also runnable standalone.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TEMPLATE="$PROJECT_DIR/docs/llms-txt-template.md"
OUT="$PROJECT_DIR/llms.txt"
LIB_RS="$PROJECT_DIR/src/lib.rs"

echo "Generating llms.txt..."

MODULE_TABLE="$(awk '
  /^\/\/! \| File \| Tier \| Requirable as \| Key functions \|/ { capture=1 }
  capture && started && /^\/\/![[:space:]]*$/ { exit }
  capture { line=$0; sub(/^\/\/! ?/, "", line); print line; started=1 }
' "$LIB_RS")"

if [ -z "$MODULE_TABLE" ]; then
  echo "error: could not extract the stdlib module table from $LIB_RS" >&2
  echo "       (looked for a '| File | Tier | Requirable as | Key functions |' header)" >&2
  exit 1
fi

FUNCTION_INDEX="$(cd "$PROJECT_DIR" && cargo run --release -- -s "(render-llms-index)" 2>/dev/null)"

if [ -z "$FUNCTION_INDEX" ]; then
  echo "error: (render-llms-index) produced no output -- is lib/97-doc-renderer.lisp intact?" >&2
  exit 1
fi

# `lamedh -s` always auto-prints the top-level expression's return value
# after its side effects; RENDER-LLMS-INDEX's last form is a TERPRI (which
# returns NIL), so the captured output ends in a stray "()" line that is
# not part of the index -- drop it.
FUNCTION_INDEX="$(printf '%s' "$FUNCTION_INDEX" | sed -e '$ { /^()$/d }')"

awk -v modtable="$MODULE_TABLE" -v funcindex="$FUNCTION_INDEX" '
  $0 == "{{MODULE_TABLE}}"   { print modtable;  next }
  $0 == "{{FUNCTION_INDEX}}" { print funcindex; next }
  { print }
' "$TEMPLATE" > "$OUT"

cp "$OUT" "$PROJECT_DIR/docs/llms.txt"

SIZE=$(wc -c < "$OUT")
echo "Generated:"
echo "  - llms.txt          ($SIZE bytes)"
echo "  - docs/llms.txt     (copy, served by GitHub Pages / mdBook)"
if [ "$SIZE" -gt 100000 ]; then
  echo "error: llms.txt is $SIZE bytes, over the 100 KB budget" >&2
  exit 1
fi
