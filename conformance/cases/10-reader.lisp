;; Conformance cases: reader tokens, lists, comments.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "reader/nil-token-is-empty-list" "KERNEL.md Part II: Symbols (NIL is not a symbol)" "'nil" "()")
(case "reader/dotted-pair" "KERNEL.md Part II: Lists (dotted tail)" "'(a . b)" "(A . B)")
(case "reader/dot-needs-preceding-element" "KERNEL.md Part II: Lists (dotted tail requires an element)" "(. 5)" "ERROR")
(case "reader/unexpected-close-paren" "KERNEL.md Part II: Dispatch order (')' outside a list)" ")" "ERROR")
(case "reader/line-comment" "KERNEL.md Part II: Source text and whitespace (line comments)" "; note\n7" "7")
(case "reader/nested-block-comment" "KERNEL.md Part II: Source text and whitespace (block comments nest)" "#| a #| b |# c |# 5" "5")
(case "reader/one-minus-token-boundary" "KERNEL.md Part II: Token boundary (1- followed by a digit)" "'(1-5)" "ERROR")
(case "reader/dot-splits-symbol-in-list" "KERNEL.md Part II: Symbols (general, '.' is not a constituent)" "'(a.b)" "(A . B)")
