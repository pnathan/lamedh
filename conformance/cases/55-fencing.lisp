;; Conformance cases: capability gates and step-budget fuel.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.
;; Every case runs in a fresh environment with no capability granted.

(case "cap/feature-denied-in-fresh-env" "KERNEL.md Part IX: Standing grant is empty in a fresh environment" "(feature-enabled-p 'READ-FS)" "()")
(case "cap/with-capabilities-cannot-widen" "KERNEL.md Part IX: The mask only narrows; no standing grant means no permission" "(with-capabilities '(READ-FS) (feature-enabled-p 'READ-FS))" "()")
(case "cap/gated-read-denied" "KERNEL.md Part IX: A gated primitive without permission signals a catchable error" "(read-file \"/\")" "ERROR")
(case "fuel/set-then-disarm" "KERNEL.md Part X: kernel-fuel-set! outside a fence is unrestricted" "(kernel-fuel-set! 1000) (kernel-fuel-set! nil) (kernel-fuel-remaining)" "()")
(case "fuel/zero-budget-exhausts" "KERNEL.md Part X: (with-fuel 0 form) exhausts on the first step" "(with-fuel 0 1)" "ERROR")
(case "fuel/nested-fence-attenuates" "KERNEL.md Part X: A nested fence cannot grant more than the enclosing fence has left" "(with-fuel 100 (with-fuel 100000 (< (kernel-fuel-remaining) 101)))" "T")
(case "fuel/nested-spend-debits-outer" "KERNEL.md Part X: Fuel spent in a nested fence is debited from the outer fence" "(with-fuel 1000 (with-fuel 500 1) (< (kernel-fuel-remaining) 1000))" "T")
(case "fuel/exhaustion-not-caught-inside-fence" "KERNEL.md Part X: Guest HANDLER-CASE cannot intercept fuel exhaustion" "(with-fuel 1000 (handler-case (while t nil) (error (e) 'caught)))" "ERROR")
(case "fuel/widen-inside-fence-error" "KERNEL.md Part X: Inside an armed fence, raising the budget is an error" "(with-fuel 10 (kernel-fuel-set! 1000))" "ERROR")
(case "fuel/lower-inside-fence-allowed" "KERNEL.md Part X: Inside an armed fence, lowering the budget is permitted" "(with-fuel 1000 (kernel-fuel-set! 5) 'ok)" "OK")
