#!/usr/bin/env bash
# Print the open `claude` work queue (the tree we iterate over).
#
# NOTE: `gh issue list --label claude` silently returns empty on this repo
# (Projects-classic GraphQL path). So we list everything and filter client-side.
#
# Usage: ai/scripts/queue.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$HERE/lib.sh"

echo "== Open '$CLAUDE_LABEL' issues (work queue, ascending) =="
gh issue list --state open --limit 300 --json number,title,labels --jq \
  "[ .[] | select(any(.labels[]; .name == \"$CLAUDE_LABEL\")) ]
   | sort_by(.number)
   | .[]
   | \"#\(.number)\t[\([.labels[].name] | map(select(. != \"$CLAUDE_LABEL\")) | join(\", \"))]\t\(.title)\""
