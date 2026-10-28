;; Conformance cases: reader character, string, array, and record literals.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "reader/char-literal-quoted" "KERNEL.md Part II: Character literals ('x')" "'a'" "'a'")
(case "reader/char-literal-hash-name" "KERNEL.md Part II: #\\ character literals (named)" "#\\space" "' '")
(case "reader/char-literal-escape" "KERNEL.md Part II: Character literals (escapes)" "'\\n'" "'\\n'")
(case "reader/string-unknown-escape-kept" "KERNEL.md Part II: String literals (unknown escape kept)" "\"a\\qb\"" "\"a\\\\qb\"")
(case "reader/array-literal-unevaluated" "KERNEL.md Part II: Array literals (elements not evaluated)" "#(1 (2) 3)" "#(1 (2) 3)")
(case "reader/record-literal" "KERNEL.md Part II: Record literals" "#S(POINT 1 2)" "#S(POINT 1 2)")
