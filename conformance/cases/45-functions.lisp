;; Conformance cases: LAMBDA parameter lists, arity, tail calls, depth bound, dynamic variables.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "lambda/rest-parameter" "KERNEL.md Part VI: LAMBDA rest parameter collects surplus" "((lambda (a &rest r) r) 1 2 3)" "(2 3)")
(case "lambda/too-few-arguments-error" "KERNEL.md Part VI: Fixed arity is exact" "((lambda (a b) a) 1)" "ERROR")
(case "lambda/optional-parameter-rejected" "KERNEL.md Part VI: &OPTIONAL is an error at definition time" "(lambda (&optional x) x)" "ERROR")
(case "tail/iterations-from-if-branch" "KERNEL.md Part VI: Tail calls from IF branches are eliminated" "(defun tl (n) (if (= n 0) 'done (tl (- n 1)))) (tl 300)" "DONE")
(case "dynamic/declaration-is-retroactive" "KERNEL.md Part VI: DEFDYNAMIC is global and retroactive" "(defun mkx (x) (lambda () x)) (setq g (mkx 10)) (defdynamic x 99) (funcall g)" "99")
(case "dynamic/binding-restored-on-throw" "KERNEL.md Part VI: Dynamic bindings are restored on non-local exit" "(defdynamic *x* 1) (catch 'c (let ((*x* 2)) (throw 'c 0))) *x*" "1")
