;; Conformance cases: reader numeric literals.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "reader/octal-suffix" "KERNEL.md Part II: Integer literals (octal, Q suffix)" "17Q" "15")
(case "reader/hex-suffix" "KERNEL.md Part II: Integer literals (hex, H suffix)" "0FFh" "255")
(case "reader/hex-needs-leading-decimal-digit" "KERNEL.md Part II: Integer literals (hex, H suffix)" "'FFh" "FFH")
(case "reader/token-boundary" "KERNEL.md Part II: Token boundary (issue #503)" "12abc" "ERROR")
(case "reader/float-literal" "KERNEL.md Part II: Float literals" "-2.5e1" "-25.0")
(case "reader/oversized-decimal-is-float" "KERNEL.md Part II: Integer literals (decimal overflow reads as float); printed by the Part III Float rule" "9223372036854775808" "9223372036854776000.0")
