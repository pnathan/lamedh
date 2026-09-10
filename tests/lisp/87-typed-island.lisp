;; Lisp-level coverage for the typed-island front end
;; (lib/47-typed-island.lisp) and the codegen-mode gate it stands on
;; (lib/46-hm-check.lisp, sections 7b and 10), alongside
;; tests/test_typed_island.rs's string-pinned host-visible assertions.
;;
;; The standard is the same as the checker's: FIDELITY with the kernel. A
;; verdict here is right when the kernel would say the same, so every test
;; that can read the kernel's verdict back does.

;;; ---- the gate -------------------------------------------------------------

(deftest island-gate-blocks-ambiguous-arithmetic
  (assert-equal (hm-compile-lambda 'f '(x) '((* x x)))
                '(blocked "`*`: cannot infer operand type")))

(deftest island-gate-admits-pinned-arithmetic
  (assert-equal (hm-compile-lambda 'f '(x) '((* x 2)))
                '(compileable (-> (int64) int64)))
  (assert-equal (hm-compile-lambda 'f '(x) '((* x 1.5)))
                '(compileable (-> (float64) float64))))

(deftest island-gate-needs-bool-where-the-checker-does-not
  ;; A known int64 condition is fine for the checker (truthiness) and a
  ;; blocker for codegen; a free parameter is pinned to bool by codegen.
  (assert-equal (car (hm-check-lambda '(x) '((if (+ x 1) 1 2)))) 'checked)
  (assert-equal (car (hm-compile-lambda 'f '(x) '((if (+ x 1) 1 2)))) 'blocked)
  (assert-equal (hm-compile-lambda 'f '(x) '((if x 1 2)))
                '(compileable (-> (bool) int64)))
  (assert-equal (hm-compile-lambda 'f '(x) '((if (> x 0) 1 2)))
                '(compileable (-> (int64) int64))))

(deftest island-gate-knows-loops-and-local-setq
  (assert-equal (hm-compile-lambda 'f '(n)
                  '((let ((i 0) (acc 0))
                      (progn (while (< i n) (setq acc (+ acc i)) (setq i (+ i 1)))
                             acc))))
                '(compileable (-> (int64) int64))))

(deftest island-gate-rejects-checking-only-heads-as-unknown-calls
  (assert-equal (hm-compile-lambda 'f '(xs) '((car xs)))
                '(blocked "call to unknown function `CAR`")))

(deftest island-gate-resolves-only-the-compileable-lattice
  (assert-true (hm-compileable-ty-p 'int64))
  (assert-true (hm-compileable-ty-p '(array float64)))
  (assert-false (hm-compileable-ty-p '(list int64)))
  (assert-false (hm-compileable-ty-p 'string))
  (assert-false (hm-compileable-ty-p 'any)))

;;; ---- freezing -------------------------------------------------------------

(deftest island-freeze-expands-global-macros
  (assert-equal (island-freeze '(when p 1)) '(if p (progn 1) nil))
  (assert-equal (island-freeze '(unless p 1)) '(if p nil (progn 1))))

(deftest island-freeze-leaves-quoted-data-alone
  (assert-equal (island-freeze '(quote (when p 1))) '(quote (when p 1))))

;;; ---- islands --------------------------------------------------------------

(defun isl87-ev (n) (if (= n 0) (= 1 1) (isl87-od (- n 1))))
(defun isl87-od (n) (if (= n 0) (= 1 0) (isl87-ev (- n 1))))
(defun isl87-addp (a b) (+ a b))
(defun isl87-usea (x) (isl87-addp x 1.5))
(defun isl87-bad (n) (car (list n)))
(defun isl87-mid (n) (if (> n 0) (isl87-bad n) 1))

(deftest island-admits-mutual-recursion
  (let ((isl (typed-island '(isl87-ev isl87-od))))
    (assert-equal (island-member-names isl) '(isl87-ev isl87-od))
    (assert-equal (island-signature isl 'isl87-ev) '(-> (int64) bool))
    (assert-equal (island-rejected isl) nil)))

(deftest island-caller-pins-a-helper
  (let ((isl (typed-island '(isl87-addp isl87-usea))))
    (assert-equal (island-signature isl 'isl87-addp) '(-> (float64 float64) float64))
    (assert-equal (island-signature isl 'isl87-usea) '(-> (float64) float64))))

(deftest island-is-closed-under-calls
  (let ((isl (typed-island '(isl87-mid isl87-bad))))
    (assert-equal (island-members isl) nil)
    (assert-equal (island-rejection isl 'isl87-bad) "call to unknown function `CAR`")
    (assert-equal (island-rejection isl 'isl87-mid) "call to unknown function `ISL87-BAD`")))

(deftest island-forms-declare-then-define
  (let ((forms (island-forms (typed-island '(isl87-ev isl87-od)))))
    (assert-equal (mapcar #'car forms)
                  '(declare-typed declare-typed defun-typed defun-typed))
    (assert-equal (car forms) '(declare-typed (isl87-ev bool) ((n int64))))))

(deftest island-optimize-preserves-signatures
  (let* ((isl (typed-island '(isl87-ev isl87-od)))
         (opt (island-optimize isl)))
    (assert-equal (island-regressions opt) nil)
    (assert-equal (island-member-names opt) (island-member-names isl))
    (assert-equal (island-signature opt 'isl87-od) (island-signature isl 'isl87-od))))

;;; ---- the kernel hand-off (only where a kernel exists) ---------------------

;; ISLAND-INSTALL! binds in the CALLER's environment, like every definition
;; form (DEF, DEFUN, DEFUN-TYPED, EDIT!), so the hand-off happens at top level
;; and the test reads the result.
(defun isl87-cnt (n)
  (let ((i 0) (acc 0))
    (progn (while (< i n) (setq acc (+ acc i)) (setq i (+ i 1))) acc)))

(def $isl87-report
  (if (island-kernel-p)
      (island-install! (typed-island '(isl87-ev isl87-od isl87-addp isl87-usea isl87-cnt)))
      'no-kernel))

(deftest island-install-agrees-with-the-kernel
  (if (island-kernel-p)
      (progn
        (assert-equal (cdr (island-agreement $isl87-report)) nil)
        (assert-equal (car (island-agreement $isl87-report))
                      '(isl87-ev isl87-od isl87-addp isl87-usea isl87-cnt))
        (assert-equal (isl87-ev 10) t)
        (assert-equal (isl87-usea 2.0) 3.5)
        ;; GUARDED (the default): an argument outside the signature still gets
        ;; the dynamic definition's answer, never a membrane error.
        (assert-equal (isl87-cnt 10) 45)
        (assert-equal (isl87-cnt 3.0) 3))
      (assert-true t)))

(deftest island-annotations-are-pins
  (assert-equal (island-annotation-p 'boxed) t)
  (assert-equal (island-annotation-p '(array int64)) t)
  (assert-equal (island-annotation-p 'string) nil)
  (assert-equal (island-pin-of-defun-typed
                 '(defun-typed (f int64) ((h boxed) (n int64)) (+ n 1)))
                '(annotated (boxed int64) int64))
  (assert-equal (island-pin-of-defun-star '(defun* f (x float64) y int64 (+ x y)))
                '(annotated (float64 ?) int64))
  (assert-equal (island-pin-of-defun-star '(defun* f (h cap) (logand h cap))) nil))
