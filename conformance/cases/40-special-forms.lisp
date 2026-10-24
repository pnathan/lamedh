;; Conformance cases: IF, COND, OR, quasiquote.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "if/two-operands-error" "KERNEL.md Part VI: IF takes exactly three operands" "(if t 1)" "ERROR")
(case "if/only-selected-branch-evaluated" "KERNEL.md Part VI: IF evaluates exactly the selected branch" "(if nil (car 5) 2)" "2")
(case "cond/atom-clause-error" "KERNEL.md Part VI: COND clause must be a non-empty list" "(cond 5)" "ERROR")
(case "or/returns-first-truthy" "KERNEL.md Part VI: OR returns the first truthy value (0 is truthy)" "(or nil 0 2)" "0")
(case "quasiquote/unquote-splicing" "KERNEL.md Part VI: QUASIQUOTE splices proper lists" "`(1 ,@'(2 3) 4)" "(1 2 3 4)")
(case "quasiquote/no-nesting-level" "KERNEL.md Part VI: QUASIQUOTE has no nesting-level tracking" "`(a `(b ,(+ 1 2)))" "(A (QUASIQUOTE (B 3)))")
