;; Conformance cases: first-class environments.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "env/make-environment-has-no-capabilities" "KERNEL.md Part IV: Environment (fresh root has no capability grant)" "(eval '(feature-enabled-p 'READ-FS) (make-environment))" "()")
(case "env/make-environment-has-no-stdlib" "KERNEL.md Part IV: Environment (fresh root holds only native builtins; DEFUN is library code)" "(eval '(defun zz () 1) (make-environment))" "ERROR")
(case "env/child-sees-parent-binding" "KERNEL.md Part IV: Environment ((make-environment parent) sees parent bindings)" "(let ((x 3)) (eval 'x (make-environment (the-environment))))" "3")
