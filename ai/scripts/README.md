# ai/scripts

Reusable automation for the AI-driven development loop on Lamedh.

The workflow: GitHub issues labeled **`claude`** are the work tree ("the pump").
We file findings there, then pull from the queue and work each one, iterating
until the tree is exhausted.

## Scripts

- **`lib.sh`** — shared helpers. `source` it. Provides `mk_issue`, `gh_safe`.
- **`queue.sh`** — print the open `claude` work queue (the tree), newest-relevant first.
- **`mk-issue.sh`** — create a `claude`-labeled issue from a title + body file/stdin.
- **`triage-existing-issues.sh`** — one-time triage record (2026-06-23): close
  completed issues, relabel/reframe the rest. Idempotent-ish; safe to re-read.

## Conventions

- Branches: `claude/<slug>` (matches existing repo history).
- Every new issue gets the `claude` label plus topical labels (bug, enhancement,
  lisp-1.5, difficulty: *, etc.).
- Long issue bodies go through `--body-file` (or here-docs) to dodge shell quoting.
