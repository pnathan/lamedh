;;; Lisp-to-Lisp optimizer passes.
;;;
;;; Runs its own analysis (use counting, liveness, dead binding removal,
;;; constant propagation), then hands the result to the builtin (OPTIMIZE ...)
;;; for constant folding and algebraic simplification.
;;;
;;; Entry points:
;;;   (optimize-form form)   -- pure transform, returns optimized S-expr
;;;   $opt                   -- vau: evaluates its argument with optimization applied
;;;   deffun-typed-opt       -- vau: optimize source, then hand DEFFUN-TYPED to compiler

;;; ─── Helpers ──────────────────────────────────────────────────────────────

(defun opt-sum-list (lst)
  "Sum a list of integers."
  (cond ((null lst) 0)
        (t (+ (car lst) (opt-sum-list (cdr lst))))))

;;; Is FORM side-effect free? Conservative: only known pure primitives.
(defun opt-pure-p (form)
  "Return T if FORM has no observable side effects."
  (cond
    ((null form) t)
    ((atom form) t)
    ((eq (car form) 'quote) t)
    ((eq (car form) 'lambda) t)
    ((eq (car form) 'function) t)
    ((member (car form)
             '(+ - * / car cdr cons list not null atom
               numberp floatp symbolp consp listp = < > <= >=
               eq equal zerop onep minusp plusp fixp))
     (opt-all-pure-p (cdr form)))
    (t nil)))

(defun opt-all-pure-p (forms)
  (cond ((null forms) t)
        ((not (opt-pure-p (car forms))) nil)
        (t (opt-all-pure-p (cdr forms)))))

;;; Count free occurrences of SYM in FORM.
;;; Scope-aware: does not count into lambda/let bodies where SYM is re-bound.
(defun count-refs (sym form)
  "Count free occurrences of symbol SYM in FORM."
  (cond
    ((null form) 0)
    ((atom form) (if (eq form sym) 1 0))
    ;; (quote ...) - never a reference
    ((eq (car form) 'quote) 0)
    ;; (lambda (params...) body...) - sym is shadowed if in params
    ((eq (car form) 'lambda)
     (if (member sym (cadr form))
         0
         (opt-sum-list (mapcar (lambda (b) (count-refs sym b)) (cddr form)))))
    ;; (let ((v e) ...) body) - count inits freely, body only if sym not bound
    ((eq (car form) 'let)
     (let* ((bindings (cadr form))
            (vars     (mapcar (lambda (b) (car b)) bindings))
            (inits    (mapcar (lambda (b) (cadr b)) bindings))
            (init-refs (opt-sum-list (mapcar (lambda (e) (count-refs sym e)) inits)))
            (body-refs (if (member sym vars)
                           0
                           (count-refs sym (caddr form)))))
       (+ init-refs body-refs)))
    ;; (prog (vars...) stmts...) - sym shadowed if in prog var list
    ((eq (car form) 'prog)
     (if (member sym (cadr form))
         0
         (opt-sum-list (mapcar (lambda (s) (count-refs sym s)) (cddr form)))))
    ;; General: walk car and cdr
    (t (+ (count-refs sym (car form))
          (count-refs sym (cdr form))))))

;;; Is SYM ever mutated (setq/set) in FORM?
(defun opt-mutated-p (sym form)
  "Return T if SYM is the target of SETQ/SET anywhere in FORM."
  (cond
    ((atom form) nil)
    ((eq (car form) 'quote) nil)
    ((or (eq (car form) 'setq) (eq (car form) 'set))
     (eq (cadr form) sym))
    (t (or (opt-mutated-p sym (car form))
           (opt-mutated-p sym (cdr form))))))

;;; ─── Main pass ────────────────────────────────────────────────────────────

(defun opt-pass (form)
  "Recursively apply Lisp-level optimization passes to FORM."
  (cond
    ((atom form) form)
    ((eq (car form) 'quote) form)
    ((eq (car form) 'lambda)
     ;; Optimize lambda body
     (cons 'lambda
           (cons (cadr form)
                 (mapcar #'opt-pass (cddr form)))))
    ((eq (car form) 'let)
     (opt-pass-let form))
    ((eq (car form) 'progn)
     (opt-pass-progn (cdr form)))
    ((eq (car form) 'if)
     (opt-pass-if form))
    ;; General: optimize all sub-expressions
    (t (mapcar #'opt-pass form))))

;;; ── LET pass ──────────────────────────────────────────────────────────────

(defun opt-pass-let (form)
  "Optimize (let ((v e) ...) body) by removing dead bindings and inlining constants."
  (let* ((bindings (cadr form))
         (body     (caddr form))
         ;; First, optimize all init expressions
         (opt-bindings (mapcar (lambda (b) (list (car b) (opt-pass (cadr b))))
                               bindings))
         ;; Filter: remove dead pure bindings, inline atom-constant bindings
         (reduced  (opt-reduce-bindings opt-bindings body)))
    ;; reduced = (new-bindings . new-body)
    (let ((new-bindings (car reduced))
          (new-body     (cdr reduced)))
      (if (null new-bindings)
          (opt-pass new-body)
          (list 'let new-bindings (opt-pass new-body))))))

(defun opt-reduce-bindings (bindings body)
  "Return (new-bindings . body) after dead-binding removal and atom inlining."
  (cond
    ((null bindings) (cons nil body))
    (t
     (let* ((b    (car bindings))
            (var  (car b))
            (init (cadr b))
            (rest (cdr bindings))
            (uses (count-refs var body))
            (mutated (opt-mutated-p var body)))
       (cond
         ;; Dead binding: pure init, 0 uses — drop it
         ((and (= uses 0) (opt-pure-p init))
          (opt-reduce-bindings rest body))
         ;; Inline: atom/number init, used exactly once, never mutated
         ;; Safe because atomic inits have no side-effects and are duplicable
         ((and (= uses 1)
               (not mutated)
               (atom init)
               (not (null init)))
          ;; Substitute init for var in remaining bindings and body
          (let* ((new-body  (subst init var body))
                 (new-rest  (mapcar (lambda (rb)
                                      (list (car rb) (subst init var (cadr rb))))
                                    rest)))
            (opt-reduce-bindings new-rest new-body)))
         ;; Keep binding, recurse on rest
         (t
          (let ((tail (opt-reduce-bindings rest body)))
            (cons (cons b (car tail)) (cdr tail)))))))))

;;; ── PROGN pass ────────────────────────────────────────────────────────────

(defun opt-pass-progn (forms)
  "Flatten nested PROGNs and drop dead non-tail pure forms."
  (let ((flat (opt-flatten-progn forms)))
    (cond
      ((null flat) nil)
      ((null (cdr flat)) (opt-pass (car flat)))
      (t (cons 'progn (opt-progn-prune flat))))))

(defun opt-flatten-progn (forms)
  "Flatten (progn (progn a b) c) -> (a b c)."
  (cond
    ((null forms) nil)
    ((and (consp (car forms)) (eq (caar forms) 'progn))
     (append (cdar forms) (opt-flatten-progn (cdr forms))))
    (t (cons (car forms) (opt-flatten-progn (cdr forms))))))

(defun opt-progn-prune (forms)
  "Remove pure non-tail forms from a PROGN sequence."
  (cond
    ((null forms) nil)
    ((null (cdr forms)) (list (opt-pass (car forms))))   ; tail: always keep
    ((opt-pure-p (car forms))                             ; non-tail pure: drop
     (opt-progn-prune (cdr forms)))
    (t (cons (opt-pass (car forms)) (opt-progn-prune (cdr forms))))))

;;; ── IF pass ───────────────────────────────────────────────────────────────

(defun opt-pass-if (form)
  "Optimize (if cond then else) — constant-condition cases handled by builtin later."
  (list 'if
        (opt-pass (cadr form))
        (opt-pass (caddr form))
        (if (cdddr form) (opt-pass (cadddr form)) nil)))

;;; ─── Top-level entry point ────────────────────────────────────────────────

(defun optimize-form (form)
  "Run Lisp-level passes then the builtin constant-folder on FORM."
  (optimize (opt-pass form)))

;;; ─── $opt vau ────────────────────────────────────────────────────────────
;;;
;;; Usage: ($opt expr)
;;; Evaluates EXPR with optimization applied before execution.
(def $opt
  ($vau (x e)
    (eval (optimize-form (car x)) e)))

;;; ─── Optimizer → typed compiler bridge ───────────────────────────────────
;;;
;;; Usage:
;;;   (deffun-typed-opt (name ret) ((arg ty) ...) body...)
;;;
;;; Reconstructs the ordinary DEFFUN-TYPED form, runs the Lisp optimizer passes
;;; over that source, and evaluates the optimized definition in the caller's
;;; environment. The evaluator's existing DEFFUN-TYPED path then performs HM
;;; checking and native compilation as usual.
(defvau deffun-typed-opt (x e)
  "Optimize a DEFFUN-TYPED definition before HM checking and native compilation."
  (eval (optimize-form (cons 'deffun-typed x)) e))
