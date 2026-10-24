;; Conformance cases: printed representation (PRIN1 style). The runner prints
;; each value with the printer, so EXPECT is the printed text of that value.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "print/nil-is-empty-list" "KERNEL.md Part III: Nil prints as ()" "'()" "()")
(case "print/float-integral-gets-point-zero" "KERNEL.md Part III: Float prints with .0 when integral" "3.0" "3.0")
(case "print/float-negative-zero" "KERNEL.md Part III: -0.0 prints as -0.0" "-0.0" "-0.0")
(case "print/float-large-plain-digits" "KERNEL.md Part III: Extreme magnitudes print as plain digits" "1e30" "1000000000000000000000000000000.0")
(case "print/positive-infinity" "KERNEL.md Part III: Positive infinity prints as inf" "(/ 1.0 0.0)" "inf")
(case "print/nan" "KERNEL.md Part III: NaN prints as NaN" "(/ 0.0 0.0)" "NaN")
(case "print/char-escape-newline" "KERNEL.md Part III: A Char prints escapes for \\n" "'\\n'" "'\\n'")
(case "print/string-quote-escape" "KERNEL.md Part III: A String escapes double quotes" "\"a\\\"b\"" "\"a\\\"b\"")
(case "print/improper-list" "KERNEL.md Part III: An improper list prints its final cdr after ' . '" "(cons 'a 'b)" "(A . B)")
(case "print/array" "KERNEL.md Part III: An array prints as #(...)" "'#(1 2)" "#(1 2)")
(case "print/record" "KERNEL.md Part III: A record prints as #S(BRAND fields...)" "#S(P 1 \"x\")" "#S(P 1 \"x\")")
(case "print/prin1-to-string-quotes-string" "KERNEL.md Part III: PRIN1-TO-STRING quotes a string" "(prin1-to-string \"hi\")" "\"\\\"hi\\\"\"")
(case "print/read-print-roundtrip" "KERNEL.md Part III: (read (print x)) is EQUAL to x" "(equal (read-from-string (prin1-to-string '(a (b . c) \"s\" 2.5 -3))) '(a (b . c) \"s\" 2.5 -3))" "T")
