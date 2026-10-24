;; Conformance cases: LET, LET*, SETQ, variable resolution.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "let/parallel-init-scope" "KERNEL.md Part VI: LET binds in parallel" "(let ((x 1)) (let ((x 2) (y x)) y))" "1")
(case "let-star/sequential-init" "KERNEL.md Part VI: LET* binds sequentially" "(let* ((x 1) (y (+ x 1))) y)" "2")
(case "let/empty-body-error" "KERNEL.md Part VI: LET requires at least one body form" "(let ())" "ERROR")
(case "let/nil-target-error" "KERNEL.md Part VI: NIL is not a symbol and cannot be bound" "(let ((nil 1)) 1)" "ERROR")
(case "setq/creates-binding-when-unbound" "KERNEL.md Part VI: SETQ of an unknown name creates a binding" "(let ((x 1)) (setq y 2) y)" "2")
(case "variable/unbound-error" "KERNEL.md Part VI: An unbound symbol is an error" "unbound-conformance-var" "ERROR")
