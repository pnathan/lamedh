#!/usr/bin/env bash
# Fix value-extraction from cons children after the Box->Rc change (issue #111).
#
# Cons children are now Rc<LispVal>. Pattern bindings produce either Rc<LispVal>
# (matched by value) or &Rc<LispVal> (matched by reference), so neither
# `*x.clone()` (moves out of Rc) nor `(*x).clone()` (clones the Rc, not the
# value) is correct uniformly. `x.as_ref().clone()` yields the inner LispVal in
# both cases and is what we want everywhere a cons child's *value* is taken.
#
# Scoped to the known cons-child binding names so unrelated `*foo.clone()` is
# left alone. Idempotent.
set -euo pipefail

cd "$(dirname "$0")/../.."

files=(src/lib.rs src/evaluator.rs src/optimizer.rs src/printer.rs src/reader.rs)
vars=(car cdr rest last_expr pair_val val)

for f in "${files[@]}"; do
    for v in "${vars[@]}"; do
        # (*v).clone()  ->  v.as_ref().clone()
        perl -i -pe "s/\\(\\*${v}\\)\\.clone\\(\\)/${v}.as_ref().clone()/g" "$f"
        # *v.clone()    ->  v.as_ref().clone()   (bare, not already parenthesized)
        perl -i -pe "s/(?<![\\w.)])\\*${v}\\.clone\\(\\)/${v}.as_ref().clone()/g" "$f"
    done
done

echo "Rewrote cons-child value extraction to .as_ref().clone() in: ${files[*]}"
