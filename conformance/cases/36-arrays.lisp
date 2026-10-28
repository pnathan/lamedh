;; Conformance cases: array and typed-array contracts.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "array/store-returns-value" "KERNEL.md Part IV: Array (store returns the stored value)" "(let ((a (array 2))) (store a 0 'x))" "X")
(case "array/fresh-slots-are-nil" "KERNEL.md Part IV: Array (each slot starts as NIL)" "(fetch (array 2) 1)" "()")
(case "array/out-of-range-error" "KERNEL.md Part IV: Array (out-of-range index is an error)" "(fetch (array 2) 2)" "ERROR")
(case "array/self-containment" "KERNEL.md Part IV: Array (an array may contain itself)" "(let ((a (array 1))) (store a 0 a) (eq (fetch a 0) a))" "T")
(case "typed/int64-zero-initialized" "KERNEL.md Part IV: Typed array (slots zero-initialized)" "(fetch (typed-array 1 'int64) 0)" "0")
(case "typed/int64-rejects-float" "KERNEL.md Part IV: Typed array (INT64 accepts only a Number)" "(store (typed-array 1 'int64) 0 1.5)" "ERROR")
(case "typed/float64-accepts-number" "KERNEL.md Part IV: Typed array (FLOAT64 converts a Number)" "(let ((a (typed-array 1 'float64))) (store a 0 2) (fetch a 0))" "2.0")
(case "typed/unknown-element-type-error" "KERNEL.md Part IV: Typed array (elem-type must be INT64 or FLOAT64)" "(typed-array 1 'int32)" "ERROR")
