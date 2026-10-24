;; Conformance cases: arithmetic operators of the numeric tower.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "num/add-empty" "KERNEL.md Part V: The four arithmetic operators ((+) is 0)" "(+)" "0")
(case "num/float-contagion" "KERNEL.md Part V: Contagion" "(+ 1 2.0)" "3.0")
(case "num/char-coerced-in-arithmetic" "KERNEL.md Part V: Contagion (Char coerces to its code)" "(* 2 'a')" "194")
(case "num/fixnum-division-truncates" "KERNEL.md Part V: Fixnum / truncates toward zero" "(/ -7 2)" "-3")
(case "num/fixnum-division-by-zero-error" "KERNEL.md Part V: Fixnum / never returns a float" "(/ 1 0)" "ERROR")
(case "num/float-division-by-zero" "KERNEL.md Part V: Float / never checks for zero" "(/ 1.0 0)" "inf")
(case "num/add-overflow-wraps" "KERNEL.md Part V: Overflow wraps modulo 2^64" "(+ 9223372036854775807 1)" "-9223372036854775808")
(case "num/overflow-sets-flag" "KERNEL.md Part V: Overflow sets the OVERFLOW flag" "(clear-all-flags) (+ 9223372036854775807 1) (flag-set-p 'OVERFLOW)" "T")
