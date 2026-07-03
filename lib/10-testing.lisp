;; A minimal xUnit-style unit-testing framework for Lamedh, built with defmacro.
;;
;;   (deftest my-test
;;     (assert-equal (+ 1 2) 3)
;;     (assert-true  (member 'b '(a b c))))
;;
;;   (run-tests)   ; runs every registered test, prints a summary,
;;                 ; and returns T iff all assertions passed.
;;
;; Assertions record pass/fail into globals; the runner reports them. The
;; design avoids unquote-splicing and LABELS (not yet in the language) and
;; builds the test thunk by explicit cons, like the bootstrap `defun`.

(def *tests* nil)            ; list of (name . thunk), newest first
(def *test-pass* 0)          ; passing assertion count for the current run
(def *test-fail* 0)          ; failing assertion count for the current run
(def *test-failures* nil)    ; list of (test-name . message)
(def *current-test* nil)     ; name of the test currently executing

;; --- assertions -------------------------------------------------------------

(defun test-pass ()
  (setq *test-pass* (+ *test-pass* 1)))

(defun test-fail (msg)
  (setq *test-fail* (+ *test-fail* 1))
  (setq *test-failures* (cons (cons *current-test* msg) *test-failures*)))

(defun check (ok msg)
  "Core assertion: pass when OK is non-nil, otherwise fail recording MSG."
  (if ok (test-pass) (test-fail msg)))

(defun assert-true (x)
  "Pass when X is non-nil."
  (check x (list 'assert-true 'got x)))

(defun assert-false (x)
  "Pass when X is nil."
  (check (null x) (list 'assert-false 'got x)))

(defun assert-nil (x)
  "Pass when X is nil."
  (check (null x) (list 'assert-nil 'got x)))

(defun assert-equal (actual expected)
  "Pass when ACTUAL is structurally equal to EXPECTED."
  (check (equal actual expected) (list 'expected expected 'got actual)))

(defun assert-eq (actual expected)
  "Alias for assert-equal."
  (assert-equal actual expected))

;; --- registration -----------------------------------------------------------

(defun tests-remove (name lst)
  "Drop any registered test named NAME from LST (supports re-registration)."
  (cond ((null lst) nil)
        ((eq (car (car lst)) name) (tests-remove name (cdr lst)))
        (t (cons (car lst) (tests-remove name (cdr lst))))))

(defmacro deftest (name &rest body)
  "Register a test NAME whose BODY is a sequence of assertions.
Re-registering an existing NAME replaces the old test (issue #241),
so reloading a test file does not double-run everything."
  `(def *tests*
        (cons (cons ',name ,(cons 'lambda (cons nil body)))
              (tests-remove ',name *tests*))))

;; --- runner -----------------------------------------------------------------

(defun run-one-test (entry)
  "Run one test, trapping any error escaping its body as a failure.
A buggy test must not take down the whole run (issue #241)."
  (setq *current-test* (car entry))
  (handler-case (funcall (cdr entry))
    (error (e) (test-fail (list 'test-body-error (error-message e))))))

(defun run-test-list (lst)
  (if (null lst)
      nil
      (progn (run-one-test (car lst))
             (run-test-list (cdr lst)))))

(defun reset-tests ()
  "Clear counters before a run (does not unregister tests)."
  (setq *test-pass* 0)
  (setq *test-fail* 0)
  (setq *test-failures* nil))

(defun clear-tests ()
  "Unregister all tests."
  (setq *tests* nil))

(defun run-tests ()
  "Run all registered tests, print a summary, and return T iff all passed."
  (reset-tests)
  (run-test-list (reverse *tests*))
  (print (list 'assertions-passed *test-pass* 'failed *test-fail*))
  (if *test-failures*
      (print (cons 'failures (reverse *test-failures*)))
      (print 'all-tests-passed))
  (zerop *test-fail*))
