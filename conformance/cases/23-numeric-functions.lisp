;; Conformance cases: gcd, expt, float conversion, float printing.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "num/gcd-variadic" "KERNEL.md Part V: Other numeric primitives (gcd is variadic)" "(gcd 12 18 8)" "2")
(case "num/expt-overflow-error" "KERNEL.md Part V: Other numeric primitives (expt errors on overflow)" "(expt 2 70)" "ERROR")
(case "num/expt-negative-exponent-float" "KERNEL.md Part V: Other numeric primitives (negative exponent yields float)" "(expt 2 -1)" "0.5")
(case "num/float-shortest-round-trip" "KERNEL.md Part III: Printer (Float shortest round-trip)" "(* 0.1 3)" "0.30000000000000004")
