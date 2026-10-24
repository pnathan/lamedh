;; Conformance cases: hash table contract.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "hash/absent-key-is-nil" "KERNEL.md Part IV: Hash table (gethash of absent key is NIL)" "(gethash (make-hash-table) 'k)" "()")
(case "hash/get-after-set" "KERNEL.md Part IV: Hash table (insert then lookup)" "(let ((h (make-hash-table))) (sethash h 'k 'v) (gethash h 'k))" "V")
(case "hash/fixnum-and-float-keys-distinct" "KERNEL.md Part IV: Hash table (1 and 1.0 are distinct keys)" "(let ((h (make-hash-table))) (sethash h 1 'a) (gethash h 1.0))" "()")
(case "hash/zero-and-negative-zero-same-key" "KERNEL.md Part IV: Hash table (0.0 and -0.0 are the same key)" "(let ((h (make-hash-table))) (sethash h 0.0 'z) (gethash h -0.0))" "Z")
(case "hash/cons-key-by-structure" "KERNEL.md Part IV: Hash table (cons keys by recursive equality)" "(let ((h (make-hash-table))) (sethash h (cons 1 2) 'v) (gethash h (cons 1 2)))" "V")
(case "hash/make-hash-table-takes-no-arguments" "KERNEL.md Part IV: Hash table (make-hash-table takes no arguments)" "(make-hash-table 1)" "ERROR")
