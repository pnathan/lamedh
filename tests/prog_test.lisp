;;; PROG feature tests

(DEFUN test-prog-basic ()
  (PROG (X Y)
    (SETQ X 10)
    (SETQ Y 20)
    (RETURN (PLUS X Y))))

(DEFUN test-prog-return ()
  (PROG (X)
    (SETQ X 100)
    (RETURN X)
    (SETQ X 200)))

(DEFUN test-prog-go-forward ()
  (PROG (X)
    (SETQ X 1)
    (GO B)
    A
    (SETQ X (PLUS X 10))
    (RETURN X)
    B
    (SETQ X (PLUS X 100))
    (GO A)))

(DEFUN test-prog-go-backward-loop ()
  (PROG (I SUM)
    (SETQ I 1)
    (SETQ SUM 0)
    LOOP
    (IF (EQUAL-NUMBER I 6) (RETURN SUM) NIL)
    (SETQ SUM (PLUS SUM I))
    (SETQ I (PLUS I 1))
    (GO LOOP)))

(DEFUN test-prog-fall-through ()
  (PROG (X)
    (SETQ X 10)))

(DEFUN test-nested-prog ()
  (PROG (X)
    (SETQ X (PROG (Y) (RETURN 10)))
    (RETURN X)))

;; It's hard to test for the error case in the integration test framework,
;; as it panics on error. This test is more for manual verification.
;; (DEFUN test-prog-bad-go ()
;;   (PROG () (GO NON-EXISTENT-LABEL)))
