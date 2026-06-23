#!/usr/bin/env bash
# Create a `claude`-labeled issue.
# Usage:
#   ai/scripts/mk-issue.sh "Title" "bug,difficulty: low" body.md
#   echo "body" | ai/scripts/mk-issue.sh "Title" "enhancement"   # body from stdin
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$HERE/lib.sh"

title="${1:?title required}"
labels="${2:-}"
if [[ -n "${3:-}" ]]; then
  mk_issue "$title" "$labels" "@$3"
else
  tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT
  cat > "$tmp"
  mk_issue "$title" "$labels" "@$tmp"
fi
