#!/usr/bin/env bash
# One-time triage of pre-existing issues (record kept; 2026-06-23).
# - #2/#3/#4/#6 closed separately (apply/append/member/assoc verified done).
# - Here we relabel/reframe the richer survivors and bring them into the `claude` tree.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$HERE/lib.sh"

# #25 EQUAL/NULL/listp/not — implemented in lib/ (equal, null, listp, consp) and
# NOT is a builtin. Core of the issue is done; close as completed.
gh issue comment 25 --body "EQUAL, NULL, LISTP, CONSP are implemented in \`lib/01-list.lisp\` / \`lib/04-predicates.lisp\` and NOT is a builtin; all verified working on \`main\`. Closing as completed. The optional \`properlist-p\` and any further predicate work is folded into the Lisp 1.5 audit (#66)." >/dev/null
gh issue close 25 --reason completed >/dev/null
echo "closed #25 (predicates - done)"

# #26 RPLACA/RPLACD — copy-based (functional) version is implemented, which is
# exactly the issue's recommended first step. Keep open, scope to TRUE destructive
# semantics, and bring into the claude tree.
gh issue edit 26 --add-label "claude" >/dev/null
gh issue comment 26 --body "Status: the **copy-based** RPLACA/RPLACD recommended as step 1 in this issue are implemented and working. Re-scoping this issue to the remaining goal: **true destructive in-place mutation** of cons cells (requires interior mutability, e.g. \`Rc<RefCell<..>>\` cons cells), which is needed for circular/shared-structure semantics. Tracked in the epic (#68). Added to the \`claude\` work tree." >/dev/null
echo "relabeled #26 (rplaca/rplacd -> true destructive)"

# #30 Lisp testing framework — not done; bring into the tree.
gh issue edit 30 --add-label "claude" >/dev/null
gh issue comment 30 --body "Brought into the \`claude\` work tree (epic #68). Good medium task once the embedding/typed-eval pieces land so the runner can report structured results." >/dev/null
echo "labeled #30 (testing framework)"

# #31 JIT — reframe per project direction: a *pluggable* high-performance backend,
# keeping the AST interpreter and classic Lisp 1.5 structures as the reference model.
gh issue edit 31 --add-label "claude" >/dev/null
gh issue comment 31 --body "Reframing per project direction: rather than committing to a hand-rolled x86-64 JIT, the goal is a **pluggable high-performance backend behind a trait** (e.g. a bytecode VM first, optionally a Cranelift-based compiler later), with the tree-walking interpreter remaining the reference implementation. Classic Lisp 1.5 data structures stay the source of truth and \"plug into\" the backend at eval/compile time. The universal-call / mixed compiled-interpreted design in this issue is the right north star for interop. Kept as the stretch goal in epic #68 and added to the \`claude\` tree." >/dev/null
echo "relabeled+reframed #31 (perf backend)"

echo "--- existing-issue triage complete ---"
