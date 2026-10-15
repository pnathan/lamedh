;; WITH-FUEL fence accounting and KERNEL-FUEL-SET! fence guard (#455).
;; Mirrors the reference implementation's tests/test_guard_fences.rs: spend
;; inside a fence is debited from the enclosing budget on every exit path, and
;; KERNEL-FUEL-SET! is narrow-only while inside a fence.
;; Port-specific: not a copy of a reference tests/lisp file.

(defun ff-n (n) (if (< n 1) 'd (ff-n (- n 1))))
(defun ff-spin (n) (ff-spin (+ n 1)))

(deftest ff-normal-exit-debits-enclosing-budget
  ;; 100 recursive calls cost >= 100 trampoline steps; before the fix the
  ;; outer budget was restored to its pre-fence value, so r0 - r1 was ~0.
  (assert-true
    (with-fuel 100000
      (let ((r0 (fuel-remaining)))
        (with-fuel 50000 (ff-n 100))
        (< (fuel-remaining) (- r0 100))))))

(deftest ff-error-exit-debits-enclosing-budget
  ;; An exhausted inner fence spends everything armed to it (50).
  (assert-true
    (with-fuel 100000
      (let ((r0 (fuel-remaining)))
        (handler-case (with-fuel 50 (ff-spin 0)) (error (e) nil))
        (<= (fuel-remaining) (- r0 50))))))

(deftest ff-nonlocal-exit-debits-enclosing-budget
  (assert-true
    (with-fuel 100000
      (let ((r0 (fuel-remaining)))
        (catch 'ff-tag (with-fuel 50000 (ff-n 100) (throw 'ff-tag 1)))
        (< (fuel-remaining) (- r0 100))))))

(deftest ff-unarmed-outer-stays-unarmed
  (assert-equal (progn (with-fuel 1000 (ff-n 10)) (fuel-remaining)) nil))

(deftest ff-fuel-set-cannot-widen-inside-fence
  (assert-true
    (string-index-of
      (handler-case (with-fuel 1000 (kernel-fuel-set! 1000000) 'widened)
        (error (e) (error-message e)))
      "cannot widen or disarm inside a fuel fence")))

(deftest ff-fuel-set-cannot-disarm-inside-fence
  (assert-true
    (string-index-of
      (handler-case (with-fuel 1000 (kernel-fuel-set! nil) 'disarmed)
        (error (e) (error-message e)))
      "cannot widen or disarm inside a fuel fence")))

(deftest ff-fuel-set-may-narrow-inside-fence
  (assert-equal (with-fuel 1000 (kernel-fuel-set! 10) 'ok) 'ok))

(deftest ff-fuel-set-unrestricted-outside-fence
  (assert-equal (progn (kernel-fuel-set! nil) 'ok) 'ok))

(deftest ff-fuel-set-guard-ends-with-fence
  (assert-equal (progn (handler-case (with-fuel 1000 (kernel-fuel-set! nil)) (error (e) nil))
                       (kernel-fuel-set! nil)
                       'ok)
                'ok))

(deftest ff-fuel-set-cannot-widen-after-exhaustion
  ;; charge-kernel-fuel disarms (NIL) on exhaustion; a guest HANDLER-CASE
  ;; inside the fence must not be able to re-arm with a larger budget.
  (assert-true
    (string-index-of
      (with-fuel 100000
        (with-fuel 50
          (handler-case (ff-spin 0)
            (error (e)
              (handler-case (progn (kernel-fuel-set! 1000000) 'widened)
                (error (e2) (error-message e2)))))))
      "cannot widen or disarm inside a fuel fence")))

(deftest ff-inner-larger-than-outer-is-clamped-and-debited
  (assert-true
    (with-fuel 1000
      (handler-case (with-fuel 1000000 (ff-spin 0)) (error (e) nil))
      (<= (or (fuel-remaining) 0) 0))))
