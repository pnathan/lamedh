;; SBCL-port-only regressions for KERNEL.md conformance deviations.
;; Unlike the other fixtures here, this file is NOT a copy of the reference
;; implementation's tests/lisp/; it pins port bugs fixed against the spec.

;; #539: (zerop x) accepts only a fixnum (KERNEL: "(zerop 0.0) is an error").
(deftest kernel-zerop-fixnum-only
  (assert-equal (errorset '(zerop 0)) '(t))
  (assert-equal (errorset '(zerop 5)) '(nil))
  (assert-equal (errorset '(zerop -3)) '(nil))
  (assert-nil (errorset '(zerop 0.0)))
  (assert-nil (errorset '(zerop -0.0)))
  (assert-nil (errorset '(zerop 1.5)))
  (assert-nil (errorset '(zerop 'a))))

;; #539: IF takes exactly three operands; no implicit NIL else.
(deftest kernel-if-exactly-three-operands
  (assert-equal (if nil 1 2) 2)
  (assert-equal (if t 1 2) 1)
  (assert-nil (errorset '(if nil 1)))
  (assert-nil (errorset '(if t 1)))
  (assert-nil (errorset '(if t)))
  (assert-nil (errorset '(if)))
  (assert-nil (errorset '(if nil 1 2 3))))

;; #539: a compiled (DEFUN) body obeys the same IF arity as the evaluator.
;; The compiler used to accept (if test then) as an implicit-NIL else.
(deftest kernel-if-arity-in-compiled-bodies
  (defun kc-if2 (x) (if x 1))
  (defun kc-if4 (x) (if x 1 2 3))
  (defun kc-if3 (x) (if x 1 2))
  (assert-equal (kc-if3 nil) 2)
  (assert-equal (kc-if3 t) 1)
  (assert-nil (errorset '(kc-if2 nil)))
  (assert-nil (errorset '(kc-if2 t)))
  (assert-nil (errorset '(kc-if4 nil))))

;; #539: KERNEL Part V -- any comparison pair involving a float is compared
;; as f64, so a fixnum above 2^53 compares equal to the float it rounds to.
;; Pairs of integers stay exact. The chain is over ADJACENT pairs and stops
;; at the first false pair, exactly like the Rust reference.
;; (#516 asks the Rust core to make this exact instead; until KERNEL.md is
;; changed, f64 is the spec.)
(deftest kernel-mixed-compare-via-f64
  (assert-equal (= 9007199254740993 9007199254740992.0) t)
  (assert-equal (= 9007199254740992.0 9007199254740993) t)
  (assert-nil (< 9007199254740992.0 9007199254740993))
  (assert-nil (> 9007199254740993 9007199254740992.0))
  (assert-nil (lessp 9007199254740992.0 9007199254740993))
  (assert-nil (greaterp 9007199254740993 9007199254740992.0))
  (assert-equal (<= 9007199254740992.0 9007199254740993) t)
  ;; pairwise: the integer pair is exact (unequal), as in Rust
  (assert-nil (= 9007199254740993 9007199254740992 9007199254740992.0))
  (assert-equal (= 9007199254740992 9007199254740992.0 9007199254740993) t)
  ;; two integers: exact
  (assert-nil (= 9007199254740993 9007199254740992))
  (assert-equal (< 9007199254740992 9007199254740993) t)
  ;; characters
  (assert-equal (= #\a 97.0) t)
  (assert-equal (= #\a 97) t)
  (assert-equal (= 5 5.0) t)
  (assert-equal (< 1 2.5 3) t)
  (assert-nil (= 1.0 1.5))
  ;; the result is the Lamedh T, not the host's
  (assert-equal (eq (< 1 2) t) t)
  (assert-equal (eq (= 1 1.0) t) t))

;; NaN / infinity in mixed pairs
(deftest kernel-mixed-compare-nan-inf
  (assert-nil (= (/ 0.0 0.0) (/ 0.0 0.0)))
  (assert-nil (< 1 (/ 0.0 0.0)))
  (assert-nil (> 1 (/ 0.0 0.0)))
  (assert-equal (< 9223372036854775807 (/ 1.0 0.0)) t)
  (assert-equal (> 9223372036854775807 (/ -1.0 0.0)) t))

;; Arity and operand errors (KERNEL: two or more operands, non-numbers error)
(deftest kernel-compare-arity-and-operands
  (assert-nil (errorset '(=)))
  (assert-nil (errorset '(= 1)))
  (assert-nil (errorset '(<)))
  (assert-nil (errorset '(< 1)))
  (assert-nil (errorset '(> 1)))
  (assert-nil (errorset '(lessp 1)))
  (assert-nil (errorset '(greaterp 1)))
  (assert-nil (errorset '(= 1 1.0 'a)))
  (assert-nil (errorset '(= 1 'a)))
  (assert-nil (errorset '(< 1.0 "a")))
  ;; short-circuit: the chain stops at the first false pair (as in Rust)
  (assert-equal (errorset '(< 2 1 'a)) '(nil)))

;; compiled bodies take the same path
(deftest kernel-mixed-compare-via-f64-compiled
  (defun kc-eq (a b) (= a b))
  (defun kc-lt (a b) (< a b))
  (assert-equal (kc-eq 9007199254740993 9007199254740992.0) t)
  (assert-nil (kc-lt 9007199254740992.0 9007199254740993)))

;; IF arity in nested positions of compiled bodies
(deftest kernel-if-arity-nested-compiled
  (defun kc-nest2 (x) (let ((y 1)) (if x y)))
  (defun kc-nest4 (x) (cond (x (if x 1 2 3)) (t 0)))
  (defun kc-lam (x) ((lambda (y) (if y 1)) x))
  (assert-nil (errorset '(kc-nest2 t)))
  (assert-nil (errorset '(kc-nest4 t)))
  (assert-nil (errorset '(kc-lam t))))
