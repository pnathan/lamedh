#!/usr/bin/env bash
# Shared helpers for the Lamedh AI dev loop. `source` this file.
# Requires: gh (authenticated), run from inside the repo.

set -euo pipefail

# The label that defines the work tree / queue.
CLAUDE_LABEL="${CLAUDE_LABEL:-claude}"

# mk_issue <title> <comma-labels> <body-or-@file>
# Always adds the claude label. Prints "NUMBER<TAB>TITLE".
# If body starts with '@', the rest is treated as a body file path.
mk_issue() {
  local title="$1" labels="$2" body="$3"
  case "$labels" in
    *"$CLAUDE_LABEL"*) : ;;            # already present
    "" ) labels="$CLAUDE_LABEL" ;;
    *  ) labels="$CLAUDE_LABEL,$labels" ;;
  esac
  local url
  if [[ "$body" == @* ]]; then
    url=$(gh issue create --title "$title" --label "$labels" --body-file "${body:1}")
  else
    url=$(gh issue create --title "$title" --label "$labels" --body "$body")
  fi
  printf '%s\t%s\n' "${url##*/}" "$title"
}

# gh_safe: run a gh command, swallowing the noisy "Projects (classic) is being
# deprecated" GraphQL warning that pollutes issue/PR views on this repo.
gh_safe() {
  gh "$@" 2>&1 | grep -vE 'Projects \(classic\) is being deprecated' || true
}
