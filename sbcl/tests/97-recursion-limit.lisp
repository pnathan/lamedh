;; Recursion depth guard (#532): runaway non-tail recursion is a catchable
;; Lamedh error ("recursion limit exceeded (10000 eval frames); ..."), as in the
;; reference implementation (tests/test_recursion_limit.rs), never a crash of
;; the host process. Port-specific: not a copy of a reference tests/lisp file.

(defun rl-rec (n) (+ 1 (rl-rec n)))
(defun rl-count (n) (if (= n 0) 0 (+ 1 (rl-count (- n 1)))))
(defun rl-loop (n) (if (= n 0) 'done (rl-loop (- n 1))))
(defdynamic *rl-d* 0)
(defun rl-dyn (*rl-d*) (+ 1 (rl-dyn *rl-d*)))

(deftest rl-issue-532-repro
  (assert-equal (handler-case (rl-rec 1) (error (e) 'caught)) 'caught))

(deftest rl-reference-message
  (assert-true
    (string-index-of (handler-case (rl-rec 1) (error (e) (error-message e)))
                     "recursion limit exceeded")))

(deftest rl-survives-repeated-overflow
  ;; The depth counter unwinds with the error: a second overflow is caught
  ;; the same way, and ordinary recursion afterwards is unaffected.
  (progn
    (handler-case (rl-rec 1) (error (e) nil))
    (assert-equal (handler-case (rl-rec 1) (error (e) 'again)) 'again)
    (assert-equal (rl-count 3000) 3000)))

(deftest rl-dynamic-parameter-recursion
  (assert-equal (handler-case (rl-dyn 1) (error (e) 'caught)) 'caught))

(deftest rl-tail-calls-do-not-count
  (assert-equal (rl-loop 200000) 'done))
