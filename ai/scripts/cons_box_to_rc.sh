#!/usr/bin/env bash
# Mechanically rewrite cons-cell construction from Box to Rc for issue #111
# (tier-1 change A: structural sharing makes list clones O(1)).
#
# Only `car:`/`cdr:` fields are touched — these are unambiguously cons cells.
# Other Box<LispVal> uses (Lambda/Fexpr/Macro body, LispError::Return) are
# intentionally left as Box.
#
# Idempotent: re-running is a no-op once converted.
set -euo pipefail

cd "$(dirname "$0")/../.."

files=(src/lib.rs src/reader.rs src/evaluator.rs src/optimizer.rs src/printer.rs)

for f in "${files[@]}"; do
    sed -i \
        -e 's/\bcar: Box::new(/car: Rc::new(/g' \
        -e 's/\bcdr: Box::new(/cdr: Rc::new(/g' \
        "$f"
done

echo "Rewrote car:/cdr: Box::new -> Rc::new in: ${files[*]}"
