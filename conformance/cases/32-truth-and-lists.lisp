;; Conformance cases: truth, NIL, car/cdr on NIL, symbols, property lists.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "truth/zero-is-true" "KERNEL.md Part IV: Truth is exactly 'not Nil'" "(if 0 'yes 'no)" "YES")
(case "truth/nil-is-false" "KERNEL.md Part IV: Truth is exactly 'not Nil'" "(if nil 'yes 'no)" "NO")
(case "nil/not-a-symbol" "KERNEL.md Part IV: NIL is not a symbol" "(symbolp nil)" "()")
(case "nil/car-of-nil" "KERNEL.md Part IV: (car nil) is NIL" "(car nil)" "()")
(case "nil/car-of-atom-error" "KERNEL.md Part IV: CAR of a non-cons is an error" "(car 5)" "ERROR")
(case "plist/putp-getp" "KERNEL.md Part XI: Symbol property lists (GETP/PUTP)" "(progn (putp 'k 'color 'red) (getp 'k 'color))" "RED")
