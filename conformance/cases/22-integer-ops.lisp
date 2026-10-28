;; Conformance cases: remainder, mod, and rounding primitives.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "num/remainder-sign-of-dividend" "KERNEL.md Part V: Remainder and modulus (remainder)" "(remainder -7 2)" "-1")
(case "num/remainder-float-error" "KERNEL.md Part V: Remainder and modulus (fixnums only)" "(remainder 7.0 2)" "ERROR")
(case "num/mod-euclidean" "KERNEL.md Part V: Remainder and modulus (mod)" "(mod -7 2)" "1")
(case "num/round-half-away-positive" "KERNEL.md Part V: Other numeric primitives (round half away from zero)" "(round 2.5)" "3")
(case "num/round-half-away-negative" "KERNEL.md Part V: Other numeric primitives (round half away from zero)" "(round -2.5)" "-3")
(case "num/round-nan-is-zero" "KERNEL.md Part V: Other numeric primitives (NaN argument yields 0)" "(round (/ 0.0 0.0))" "0")
