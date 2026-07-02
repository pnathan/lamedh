;;; Lisp-to-Lisp optimizer passes.
;;;
;;; Runs its own analysis (use counting, liveness, dead binding removal,
;;; constant propagation), then hands the result to the builtin (OPTIMIZE ...)
;;; for constant folding and algebraic simplification.
;;;
;;; Entry points:
;;;   (optimize-form form)         -- pure transform, returns optimized S-expr
;;;   $opt                         -- vau: evaluates its argument with optimization applied
;;;   defun-typed-opt              -- vau: optimize source, then hand DEFUN-TYPED to compiler
;;;   (pure-p name)                -- lazy, memoized purity query for a named function
;;;   (defun-check-purity! n body) -- force-annotate N's plist; also sets cache flag

;;; ─── Purity checker pass ──────────────────────────────────────────────────
;;;
;;; Conservative leaf-level purity certificate for DEFUN bodies.
;;; A body is considered pure when it contains:
;;;   - no SETQ or SET forms (no mutation of shared state)
;;;   - no calls to known IO builtins (listed in *io-builtin-set*)
;;;
;;; Purity is now computed LAZILY on first query via (pure-p name).  The DEFUN
;;; macro clears the "pure-checked" cache flag on every redefinition so stale
;;; results cannot accumulate.  Calling (defun-check-purity! name body) still
;;; works for eager / forced annotation.
;;;
;;; This is a conservative leaf-level certificate.  Interprocedural
;;; propagation (e.g. inferring that a caller of only pure functions is itself
;;; pure) is deferred to a follow-up issue.

;;; Editable list of IO builtins that disqualify a function from purity.
;;; Add or remove names here to tune the conservativeness of the check.
(def *io-builtin-set*
  '(read print prin1 princ terpri
    shell write-file read-file
    load load-file
    open-file close-file
    format))

;;; Return T if FORM structurally contains no SETQ/SET and no calls to any
;;; symbol in *io-builtin-set*. Stops at QUOTE to avoid walking data literals.
;;; Walks the full cons structure so nested calls are caught.
(defun body-check-pure-p (form)
  "Return T if FORM contains no SETQ/SET or IO builtin calls."
  (cond
    ((null form) t)
    ((atom form) t)
    ((eq (car form) 'quote) t)
    ((or (eq (car form) 'setq) (eq (car form) 'set)) nil)
    ((member (car form) *io-builtin-set*) nil)
    (t (and (body-check-pure-p (car form))
            (body-check-pure-p (cdr form))))))

(defun body-all-pure-p (forms)
  "Return T if every form in the list is pure per body-check-pure-p."
  (cond
    ((null forms) t)
    ((not (body-check-pure-p (car forms))) nil)
    (t (body-all-pure-p (cdr forms)))))

;;; Annotate NAME's plist with property \"pure\" = :PURE when all BODY-FORMS
;;; pass the purity check, or remove the property when they do not.
;;; Returns :PURE when pure, NIL when impure.
;;; No longer called automatically from DEFUN (purity is now lazy, see pure-p).
;;; Can still be called directly to force an immediate purity annotation.
(defun defun-check-purity! (name body-forms)
  "Purity-annotate NAME: set plist \"pure\"=:PURE if BODY-FORMS are pure."
  (if (body-all-pure-p body-forms)
      (progn (putp name "pure" :pure)
             (putp name "pure-checked" t)
             :pure)
      (progn (remprop name "pure")
             (putp name "pure-checked" t)
             nil)))

;;; Lazy, memoized purity query.
;;;
;;; Returns :PURE if NAME's current body is pure, NIL if impure.  The result
;;; is cached on the symbol's plist under "pure-checked" / "pure" so repeated
;;; calls are O(1).  The DEFUN macro clears "pure-checked" on each redefinition
;;; so the cache is always fresh for the current body.
;;;
;;; If NAME is not an inspectable user-defined function (e.g. a builtin), the
;;; call returns NIL without caching anything useful.
(defun pure-p (name)
  "Return :PURE if NAME is a pure function, NIL if impure or not inspectable.
Lazily computes and caches the purity verdict using the current function body."
  (if (getp name "pure-checked")
      ;; Cache hit: return the stored verdict (:PURE or NIL).
      (getp name "pure")
      ;; Cache miss: retrieve the body via see-source and analyse it.
      (let ((src (errorset (list 'see-source (list 'quote name)))))
        (if (null src)
            ;; Not an inspectable user function — leave cache unpopulated.
            nil
            (let* ((lam  (car src))
                   (body (cddr lam)))
              (defun-check-purity! name body))))))

;;; ─── Helpers ──────────────────────────────────────────────────────────────

(defun opt-sum-list (lst)
  "Sum a list of integers."
  (cond ((null lst) 0)
        (t (+ (car lst) (opt-sum-list (cdr lst))))))

;;; Is FORM side-effect free? Conservative: only known pure primitives.
;;; BOUND is a list of locally-bound names that shadow global definitions;
;;; a call through a shadowed name is not certifiably pure (#225).
(defun opt-pure-p (form &rest bound-arg)
  "Return T if FORM has no observable side effects."
  (let ((bound (car bound-arg)))
    (cond
      ((null form) t)
      ((atom form) t)
      ((eq (car form) 'quote) t)
      ((eq (car form) 'lambda) t)
      ((eq (car form) 'function) t)
      ((and (not (member (car form) bound))
            (member (car form)
                    '(+ - * / car cdr cons list not null atom
                      numberp floatp symbolp consp listp = < > <= >=
                      eq equal zerop onep minusp plusp fixp)))
       (opt-all-pure-p (cdr form) bound))
      (t nil))))

(defun opt-all-pure-p (forms &rest bound-arg)
  (let ((bound (car bound-arg)))
    (cond ((null forms) t)
          ((not (opt-pure-p (car forms) bound)) nil)
          (t (opt-all-pure-p (cdr forms) bound)))))

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

(defun opt-pass (form &rest bound-arg)
  "Recursively apply Lisp-level optimization passes to FORM.
BOUND is an optional list of locally-bound names that shadow globals."
  (let ((bound (car bound-arg)))
    (cond
      ((atom form) form)
      ((eq (car form) 'quote) form)
      ((eq (car form) 'lambda)
       ;; Lambda params shadow outer bound names inside the body
       (let ((inner-bound (append (cadr form) bound)))
         (cons 'lambda
               (cons (cadr form)
                     (mapcar (lambda (b) (opt-pass b inner-bound)) (cddr form))))))
      ((eq (car form) 'let)
       (opt-pass-let form bound))
      ((eq (car form) 'progn)
       (opt-pass-progn (cdr form) bound))
      ((eq (car form) 'if)
       (opt-pass-if form bound))
      ;; General: optimize all sub-expressions
      (t (mapcar (lambda (f) (opt-pass f bound)) form)))))

;;; ── LET pass ──────────────────────────────────────────────────────────────

(defun opt-pass-let (form &rest bound-arg)
  "Optimize (let ((v e) ...) body) by removing dead bindings and inlining constants."
  (let* ((bound    (car bound-arg))
         (bindings (cadr form))
         (body     (caddr form))
         ;; First, optimize all init expressions (in outer scope)
         (opt-bindings (mapcar (lambda (b) (list (car b) (opt-pass (cadr b) bound)))
                               bindings))
         ;; Filter: remove dead pure bindings, inline atom-constant bindings
         (reduced  (opt-reduce-bindings opt-bindings body)))
    ;; reduced = (new-bindings . new-body)
    (let* ((new-bindings (car reduced))
           (new-body     (cdr reduced))
           (inner-bound  (append (mapcar #'car new-bindings) bound)))
      (if (null new-bindings)
          (opt-pass new-body bound)
          (list 'let new-bindings (opt-pass new-body inner-bound))))))

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
         ;; Dead binding: pure init, 0 uses — drop it.
         ;; Pass the full binding-name list so shadowed pure names
         ;; are not falsely certified pure (#225).
         ((and (= uses 0)
               (opt-pure-p init (mapcar #'car bindings)))
          (opt-reduce-bindings rest body))
         ;; Inline: atom/number init, used exactly once, never mutated
         ;; Safe because atomic inits have no side-effects and are duplicable
         ((and (= uses 1)
               (not mutated)
               (atom init)
               (not (null init)))
          ;; Substitute init for var in body only; sibling inits are NOT
          ;; touched because LET evaluates all inits in the enclosing
          ;; environment — a sibling init's free reference to var always
          ;; resolves to the outer binding, not the one being inlined.
          (opt-reduce-bindings rest (subst init var body)))
         ;; Keep binding, recurse on rest
         (t
          (let ((tail (opt-reduce-bindings rest body)))
            (cons (cons b (car tail)) (cdr tail)))))))))

;;; ── PROGN pass ────────────────────────────────────────────────────────────

(defun opt-pass-progn (forms &rest bound-arg)
  "Flatten nested PROGNs and drop dead non-tail pure forms."
  (let ((bound (car bound-arg))
        (flat (opt-flatten-progn forms)))
    (cond
      ((null flat) nil)
      ((null (cdr flat)) (opt-pass (car flat) bound))
      (t (cons 'progn (opt-progn-prune flat bound))))))

(defun opt-flatten-progn (forms)
  "Flatten (progn (progn a b) c) -> (a b c)."
  (cond
    ((null forms) nil)
    ((and (consp (car forms)) (eq (caar forms) 'progn))
     (append (cdar forms) (opt-flatten-progn (cdr forms))))
    (t (cons (car forms) (opt-flatten-progn (cdr forms))))))

(defun opt-progn-prune (forms &rest bound-arg)
  "Remove pure non-tail forms from a PROGN sequence."
  (let ((bound (car bound-arg)))
    (cond
      ((null forms) nil)
      ((null (cdr forms)) (list (opt-pass (car forms) bound)))   ; tail: always keep
      ((opt-pure-p (car forms) bound)                             ; non-tail pure: drop
       (opt-progn-prune (cdr forms) bound))
      (t (cons (opt-pass (car forms) bound) (opt-progn-prune (cdr forms) bound))))))

;;; ── IF pass ───────────────────────────────────────────────────────────────

(defun opt-pass-if (form &rest bound-arg)
  "Optimize (if cond then else) — constant-condition cases handled by builtin later."
  (let ((bound (car bound-arg)))
    (list 'if
          (opt-pass (cadr form) bound)
          (opt-pass (caddr form) bound)
          (if (cdddr form) (opt-pass (cadddr form) bound) nil))))

;;; ── COLLAPSE-FRAMES pass ──────────────────────────────────────────────────
;;;
;;; Three rewrites:
;;;   1. (let ((x e)) x)       → e          — identity let elimination
;;;   2. (let ((x e)) body)    → body[x←e]  — single-use open-position inline
;;;      when x has exactly one free occurrence that is *not* inside a closure
;;;      body and is not mutated.  Safe for single-binding lets regardless of
;;;      whether e is pure, because e is evaluated exactly once in both cases.
;;;   3. (let outer (let inner body)) → (let merged body)  — frame merge
;;;      when outer and inner binding names are disjoint AND no inner-binding
;;;      init references any outer-bound variable (merging would otherwise
;;;      evaluate those inner inits in the pre-outer scope, changing semantics).

;;; Count free references to SYM that are in "open" (non-closure) position.
;;; References inside lambda / macro / fexpr / vau bodies are excluded because
;;; inlining there would change when the init expression is evaluated.
(defun count-refs-open (sym form)
  "Count occurrences of SYM in FORM that are outside any closure boundary."
  (cond
    ((null form) 0)
    ((atom form) (if (eq form sym) 1 0))
    ((eq (car form) 'quote) 0)
    ;; Closure forms: do not descend — a captured reference is not open.
    ((member (car form) '(lambda function macro fexpr vau)) 0)
    ;; Let: inits are in outer (open) scope; body is open only if not shadowed.
    ((eq (car form) 'let)
     (let* ((bindings (cadr form))
            (vars     (mapcar (lambda (b) (car b)) bindings))
            (inits    (mapcar (lambda (b) (cadr b)) bindings))
            (init-refs (opt-sum-list
                         (mapcar (lambda (e) (count-refs-open sym e)) inits)))
            (body-refs (if (member sym vars)
                           0
                           (count-refs-open sym (caddr form)))))
       (+ init-refs body-refs)))
    ;; Prog: body is open if sym not in the prog var list.
    ((eq (car form) 'prog)
     (if (member sym (cadr form))
         0
         (opt-sum-list (mapcar (lambda (s) (count-refs-open sym s)) (cddr form)))))
    ;; General: walk car and cdr.
    (t (+ (count-refs-open sym (car form))
          (count-refs-open sym (cdr form))))))

;;; Return T if any symbol in SYMS appears in SET.
(defun opt-any-member-p (syms set)
  "Return T if any element of SYMS is a member of SET."
  (cond ((null syms) nil)
        ((member (car syms) set) t)
        (t (opt-any-member-p (cdr syms) set))))

;;; Return T if INIT references any variable in OUTER-VARS.
(defun opt-init-uses-outer-p (init outer-vars)
  "Return T if INIT contains a free reference to any variable in OUTER-VARS."
  (cond ((null outer-vars) nil)
        ((> (count-refs (car outer-vars) init) 0) t)
        (t (opt-init-uses-outer-p init (cdr outer-vars)))))

;;; Return T if any init in INNER-INITS references any variable in OUTER-VARS.
(defun opt-inner-inits-use-outer-p (outer-vars inner-inits)
  "Return T if any init in INNER-INITS references a variable in OUTER-VARS."
  (cond ((null inner-inits) nil)
        ((opt-init-uses-outer-p (car inner-inits) outer-vars) t)
        (t (opt-inner-inits-use-outer-p outer-vars (cdr inner-inits)))))

;;; Try to merge (let outer (let inner body)) → (let merged body).
;;; Bails (returns original nested form) on any of:
;;;   • name conflict — an inner binding name also appears in outer bindings
;;;   • init dependency — an inner binding's init references an outer-bound var
(defun opt-try-merge-lets (outer-bindings inner-form)
  "Merge two non-shadowing let frames into one, or return the original nested form."
  (let* ((inner-bindings (cadr inner-form))
         (inner-body     (caddr inner-form))
         (outer-vars     (mapcar (lambda (b) (car b)) outer-bindings))
         (inner-vars     (mapcar (lambda (b) (car b)) inner-bindings))
         (inner-inits    (mapcar (lambda (b) (cadr b)) inner-bindings))
         (name-conflict  (opt-any-member-p inner-vars outer-vars))
         (init-dep       (opt-inner-inits-use-outer-p outer-vars inner-inits)))
    (if (or name-conflict init-dep)
        (list 'let outer-bindings inner-form)
        (list 'let (append outer-bindings inner-bindings) inner-body))))

;;; Apply the single-binding collapse rules to (let ((var init)) body).
(defun opt-collapse-single-binding (var init body)
  "Collapse (let ((var init)) body) using identity or single-use open inline."
  (let* ((total-refs (count-refs var body))
         (open-refs  (count-refs-open var body)))
    (cond
      ;; Rule 1: identity — body is exactly the bound variable.
      ((eq body var)
       init)
      ;; Rule 2: single open-position use, not mutated.
      ;; count-refs = open-refs = 1 guarantees the occurrence is not inside a
      ;; closure, so substituting init for var preserves evaluation order.
      ((and (= total-refs 1)
            (= open-refs 1)
            (not (opt-mutated-p var body)))
       (opt-collapse-frames (subst init var body)))
      ;; Default: keep the binding.
      (t (list 'let (list (list var init)) body)))))

;;; Main collapse-frames recursive walk.
(defun opt-collapse-frames (form)
  "Recursively apply collapse-frames passes to FORM."
  (cond
    ((atom form) form)
    ((eq (car form) 'quote) form)
    ((eq (car form) 'lambda)
     (cons 'lambda
           (cons (cadr form)
                 (mapcar #'opt-collapse-frames (cddr form)))))
    ((eq (car form) 'let)
     (let* ((bindings (cadr form))
            (body     (opt-collapse-frames (caddr form))))
       (cond
         ;; Single-binding let: try identity / single-use inline.
         ((and (consp bindings) (null (cdr bindings)))
          (opt-collapse-single-binding (car (car bindings))
                                       (cadr (car bindings))
                                       body))
         ;; Multi-binding let whose body is itself a let: try frame merge.
         ((and (consp body) (eq (car body) 'let))
          (opt-try-merge-lets bindings body))
         ;; Default: reassemble.
         (t (list 'let bindings body)))))
    ;; General: recurse into all sub-expressions.
    (t (mapcar #'opt-collapse-frames form))))

;;; ─── Top-level entry point ────────────────────────────────────────────────

(defun optimize-form (form)
  "Run Lisp-level passes, frame-collapse, then the builtin constant-folder on FORM."
  (optimize (opt-collapse-frames (opt-pass form))))

;;; ─── $opt vau ────────────────────────────────────────────────────────────
;;;
;;; Usage: ($opt expr)
;;; Evaluates EXPR with optimization applied before execution.
;;; Memoized: the same unevaluated form (by eq identity) is optimized once (#234).
(def $opt-cache (make-hash-table))
(def $opt
  ($vau (x e)
    (let ((form (car x)))
      (if (has-key-p $opt-cache form)
          (eval (gethash form $opt-cache) e)
          (let ((opt (optimize-form form)))
            (set-bang $opt-cache form opt)
            (eval opt e))))))

;;; ─── Optimizer → typed compiler bridge ───────────────────────────────────
;;;
;;; Usage:
;;;   (defun-typed-opt (name ret) ((arg ty) ...) body...)
;;;
;;; Reconstructs the ordinary DEFUN-TYPED form, runs the Lisp optimizer passes
;;; over that source, and evaluates the optimized definition in the caller's
;;; environment. The evaluator's existing DEFUN-TYPED path then performs HM
;;; checking and native compilation as usual.
(defvau defun-typed-opt (x e)
  "Optimize a DEFUN-TYPED definition before HM checking and native compilation."
  (eval (optimize-form (cons 'defun-typed x)) e))

