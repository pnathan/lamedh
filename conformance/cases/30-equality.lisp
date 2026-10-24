;; Conformance cases: EQ and EQUAL.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "eq/cons-identity-self" "KERNEL.md Part IV: EQ on cons cells (pointer identity)" "(let ((x (cons 1 2))) (eq x x))" "T")
(case "eq/distinct-cons-cells" "KERNEL.md Part IV: EQ on cons cells (pointer identity)" "(eq (cons 1 2) (cons 1 2))" "()")
(case "eq/aliased-cons-via-cdr" "KERNEL.md Part IV: EQ on cons cells (same allocation via another path)" "(let ((x (cons 1 2))) (eq (cdr (cons 0 x)) x))" "T")
(case "eq/char-not-number" "KERNEL.md Part IV: Char and Number are distinct types" "(eq 'a' 97)" "()")
(case "eq/nan-is-eq" "KERNEL.md Part IV: EQ on floats (NaN is EQ to NaN)" "(eq (/ 0.0 0.0) (/ 0.0 0.0))" "T")
(case "eq/closures-different-frames" "KERNEL.md Part IV: EQ on closures (different captured environments)" "(defun mk () (lambda (x) x)) (eq (mk) (mk))" "()")
(case "eq/let-init-lambda-outer-env" "KERNEL.md Part VI: LET inits evaluate in the outer environment, so closures differ in identity" "(let ((f (lambda (x) x))) (eq f (lambda (x) x)))" "()")
(case "equal/no-numeric-contagion" "KERNEL.md Part IV: EQUAL performs no numeric contagion" "(equal 5 5.0)" "()")
(case "equal/char-not-number" "KERNEL.md Part IV: EQUAL performs no numeric contagion" "(equal 'a' 97)" "()")
(case "equal/structural-lists" "KERNEL.md Part IV: EQUAL is deep structural equality" "(equal '(1 (2 3)) (list 1 (list 2 3)))" "T")
(case "equal/record-brand-differs" "KERNEL.md Part IV: Records compare brand and fields" "(equal #S(P 1) #S(Q 1))" "()")
