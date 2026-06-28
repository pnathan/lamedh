#!/bin/bash
# Generate Markdown documentation from Lisp help database
# Usage: ./scripts/generate-docs.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "Generating documentation from Lisp source..."

# Generate the main function reference from help database
cargo run --release -- -s "(dump-docs)" 2>/dev/null > "$PROJECT_DIR/docs/generated-reference.md"

# Generate function index
cargo run --release -- -s "(render-function-index-md)" 2>/dev/null > "$PROJECT_DIR/docs/generated-function-index.md"

echo "Generated:"
echo "  - docs/generated-reference.md   (Complete function reference)"
echo "  - docs/generated-function-index.md (Alphabetical function index)"
echo ""
echo "Documentation structure:"
echo "  - Conceptual docs (introduction, data types, syntax) are hand-written in docs/"
echo "  - Function reference is auto-generated from lib/99-help-data.lisp"
echo "  - Use (help) in the REPL for interactive documentation"
