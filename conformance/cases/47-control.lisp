;; Conformance cases: BLOCK, CATCH/THROW, PROG, FOR, UNWIND-PROTECT.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "block/return-from-value" "KERNEL.md Part VI: RETURN-FROM unwinds to the enclosing BLOCK" "(block b (return-from b 5) 6)" "5")
(case "block/dynamic-extent" "KERNEL.md Part VI: BLOCK exits are dynamic, not lexical" "(defun ret-it () (return-from blk 7)) (block blk (ret-it) 8)" "7")
(case "catch/equal-tag-match" "KERNEL.md Part VI: CATCH tags match by EQUAL" "(catch '(a b) (throw (list 'a 'b) 3))" "3")
(case "catch/nonmatching-passes-outward" "KERNEL.md Part VI: Non-matching CATCH frames re-raise the throw" "(catch 'x (catch 'y (throw 'x 4)) 5)" "4")
(case "prog/go-loop" "KERNEL.md Part VI: PROG labels and GO" "(prog (x) (setq x 0) top (setq x (+ x 1)) (if (< x 3) (go top) nil) (return x))" "3")
(case "for/inclusive-range" "KERNEL.md Part VI: FOR iterates from start to end inclusive" "(let ((s 0)) (for (i 1 4) (setq s (+ s i))) s)" "10")
(case "unwind/cleanup-runs-on-throw" "KERNEL.md Part VI: UNWIND-PROTECT cleanup runs on a non-local exit" "(let ((log 0)) (catch 'c (unwind-protect (throw 'c 1) (setq log 1))) log)" "1")
(case "unwind/cleanup-error-discarded" "KERNEL.md Part VI: An error in a cleanup form is discarded" "(unwind-protect 1 (car 5))" "1")
(case "unwind/cleanup-runs-before-handler" "KERNEL.md Part VI: UNWIND-PROTECT cleanup runs when an error passes through" "(let ((log 0)) (handler-case (unwind-protect (car 5) (setq log 1)) (error (e) log)))" "1")
