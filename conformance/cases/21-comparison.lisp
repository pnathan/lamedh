;; Conformance cases: numeric comparison and predicates.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "num/compare-chain-false" "KERNEL.md Part V: Comparison (monotone chain)" "(< 1 3 2)" "()")
(case "num/compare-nonnumeric-error" "KERNEL.md Part V: Comparison (non-numeric operand is an error)" "(< 1 \"a\")" "ERROR")
(case "num/compare-stops-at-first-false" "KERNEL.md Part V: Comparison (chain stops at first false pair)" "(< 2 1 'a)" "()")
(case "num/compare-nan-not-equal" "KERNEL.md Part V: Comparison (= is IEEE ==)" "(= (/ 0.0 0.0) (/ 0.0 0.0))" "()")
(case "num/compare-adjacent-pair-f64" "KERNEL.md Part V: Comparison (mixed pair compared as f64, #553)" "(= 9007199254740993 9007199254740992.0)" "T")
(case "num/compare-two-fixnums-exact" "KERNEL.md Part V: Comparison (two fixnums exact, #553)" "(= 9007199254740993 9007199254740992 9007199254740992.0)" "()")
(case "num/zerop-float-error" "KERNEL.md Part V: zerop rejects every Float (#553)" "(zerop 0.0)" "ERROR")
