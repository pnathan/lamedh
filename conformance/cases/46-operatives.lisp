;; Conformance cases: VAU, fexprs, and macros.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "vau/operand-list-and-caller-env" "KERNEL.md Part VI: VAU binds the unevaluated operands and the caller environment" "((vau (o e) (list o (eval (car o) e))) (+ 1 2))" "(((+ 1 2)) 3)")
(case "fexpr/single-parameter-gets-operand-list" "KERNEL.md Part VI: A one-parameter fexpr receives the whole operand list" "(defexpr fl (args) args) (fl 1 2)" "(1 2)")
(case "macro/expansion-evaluated-in-caller" "KERNEL.md Part VI: A macro expansion is evaluated in the caller's environment" "(defmacro addone (x) (list '+ x 1)) (let ((y 10)) (addone y))" "11")
(case "macro/operands-not-evaluated" "KERNEL.md Part VI: Macro operands are not evaluated (the expansion is (QUOTE IGNORED))" "(defmacro ign (x) ''ignored) (ign (car 5))" "IGNORED")
(case "macro/macroexpand-one-step" "KERNEL.md Part VI: MACROEXPAND performs one fresh expansion step" "(defmacro mq (x) (list 'quote x)) (macroexpand '(mq a))" "(QUOTE A)")
