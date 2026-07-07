;;; Guard fences: composable dynamic-extent attenuation of execution budget
;;; (WITH-FUEL) and capability authority (WITH-CAPABILITIES). Issue #284.
;;;
;;; Both constructs are monotone attenuators: WITH-FUEL clamps its budget to
;;; the remaining enclosing budget, WITH-CAPABILITIES intersects with the
;;; currently effective capability set. Nesting order never matters for
;;; soundness, so the two compose freely.
;;;
;;; Mechanism (pure Lisp, "Phase 1" of #284): each fence is a vau operative
;;; that (1) for fuel, code-walks its body inserting ($GUARD-TICK) at lambda
;;; entries and loop back-edges — the Erlang-reductions/Wasm-fuel discipline;
;;; (2) seals the body in an immediately-applied lambda whose parameters
;;; lexically shadow the tick, the introspection functions, and the escape
;;; hatches (EVAL, APPLY, FUNCALL, the map family, SORT, and the
;;; capability-gated builtins). Operator dispatch resolves heads through the
;;; lexical chain (the FLET mechanism), so the shadows govern the fenced
;;; body. The fuel counter lives in a one-element array reachable only
;;; through the tick/reader closures — no nameable path from guarded code
;;; can refill it.
;;;
;;; Threat model: safety against ACCIDENT (runaway generated loops,
;;; unintended writes), not against an adversary studying the fence.
;;; Documented Phase-1 leaks:
;;;   - Closures defined OUTSIDE a fence and called inside are charged only
;;;     at the call (via the shadowed higher-order functions), not per
;;;     internal step, and see their own definition environment's builtins
;;;     rather than the capability shadows. The kernel's host-level
;;;     capability grants still apply to them.
;;;   - Builtin-internal work is atomic (one tick per shadowed call, plus
;;;     one per element for the map family).
;;;   - (EVAL form EXPLICIT-ENV) with a foreign environment escapes the
;;;     shadows (kernel grants still apply).
;;;   - User macros that expand to loops are metered only at the function
;;;     calls inside them, not at their back-edges.
;;;   - Bodies inside QUASIQUOTE unquotes are not instrumented.
;;; "Phase 2" (#284) closes the interpreter-side leaks with two small kernel
;;; hooks that inherit these semantics and this test suite.
;;;
;;; No-compile (#284 + #168): inside a fence, JIT-OPTIMIZE is rewritten to a
;;; no-op returning COMPILE-DISABLED-BY-GUARD, DEFUN-TYPED signals an error,
;;; and DEFUN* is downgraded to plain DEFUN (type annotations stripped) —
;;; compiled editions would bypass the tick instrumentation.

;;; ------------------------------------------------------------------
;;; Small helpers (kept dependency-light on purpose).

(defun $guard-probe (sym env)
  "Value of SYM in ENV, or NIL if unbound. Used to find an enclosing fence."
  (ignore-errors (eval sym env)))

(defun $guard-member (item lst)
  "MEMBER by EQUAL, returning T/NIL."
  (cond ((null lst) nil)
        ((equal item (car lst)) t)
        (t ($guard-member item (cdr lst)))))

(defun $guard-intersect (a b)
  "Elements of A that also appear in B (EQUAL), preserving A's order."
  (cond ((null a) nil)
        (($guard-member (car a) b)
         (cons (car a) ($guard-intersect (cdr a) b)))
        (t ($guard-intersect (cdr a) b))))

(defun $guard-subset-p (a b)
  "T when every element of A appears in B."
  (cond ((null a) t)
        (($guard-member (car a) b) ($guard-subset-p (cdr a) b))
        (t nil)))

(defun $guard-seal (forms bindings)
  "Build ((LAMBDA (names...) forms...) 'v1 'v2 ...) from BINDINGS, an alist
of (NAME . VALUE). Evaluating the result applies the fence: each NAME
lexically shadows its global meaning for FORMS, bound to the fence VALUE.
Values ride in under QUOTE so closures survive the trip through EVAL."
  (cons (cons 'lambda
              (cons (mapcar (lambda (b) (car b)) bindings) forms))
        (mapcar (lambda (b) (list 'quote (cdr b))) bindings)))

;;; ------------------------------------------------------------------
;;; Global (outside-any-fence) introspection defaults. Inside a fence these
;;; names are lexically shadowed by the fence's own closures.

(defun fuel-remaining ()
  "Remaining fuel in the innermost enclosing WITH-FUEL fence, or NIL when no
fence is active."
  nil)

(defun $guard-host-capabilities ()
  "The capability set granted by the host process (the outermost authority)."
  (let ((all '(READ-FS CREATE-FS TEMP-FS SHELL IO))
        (held nil))
    (dolist (c all (reverse held))
      (when (feature-enabled-p (princ-to-string c))
        (setq held (cons c held))))))

(defun capabilities-effective ()
  "The currently effective capability set: host grants intersected with
every enclosing WITH-CAPABILITIES fence."
  ($guard-host-capabilities))

;;; ------------------------------------------------------------------
;;; The instrumentation walker (fuel). Inserts ($GUARD-TICK) at lambda/defun
;;; entries and at loop back-edges (WHILE/FOR bodies, PROG labels), and
;;; applies the no-compile rewrites. Descends generically elsewhere; QUOTE
;;; and QUASIQUOTE subtrees are left untouched.

(defun $guard-strip-params (params)
  "DEFUN* param list with type annotations removed: (X INT64) -> X."
  (mapcar (lambda (p) (if (consp p) (car p) p)) params))

;; The symbol the walker emits tick calls under. Each WITH-FUEL fence mints
;; a fresh GENSYM before walking, so guarded code has no nameable path to
;; the tick and cannot rebind its way out of being charged (gensyms as
;; binders work since the issue #285 fix). $GUARD-TICK stays bound in each
;; fence too, as the chaining channel nested fences probe for.
(setq $guard-walk-tick-name '$guard-tick)

(defun $guard-fresh-tick-name ()
  (gensym))

;; Kernel-fuel backstop (issue #284 Phase 2). The Lisp-level budget is
;; charged at function entries and loop back-edges; the kernel counter is
;; charged once per evaluator trampoline step, closing the Phase-1 leaks
;; (closures defined outside the fence, user macros expanding to loops,
;; quasiquote bodies). One back-edge tick spans many trampoline steps, so
;; the kernel budget is the Lisp budget times this multiplier — generous
;; enough that ordinary fenced work exhausts the Lisp budget first, small
;; enough that leaked unmetered loops still terminate.
(setq $guard-kernel-step-multiplier 256)

(defun $guard-tick-call ()
  (list $guard-walk-tick-name))

(defun $guard-walk-list (forms)
  (if (consp forms)
      (cons ($guard-walk (car forms)) ($guard-walk-list (cdr forms)))
      forms))

(defun $guard-walk-prog-body (body)
  "PROG bodies: a bare symbol is a GO label — meter the back-edge by
ticking immediately after each label."
  (cond ((null body) nil)
        ((not (consp body)) body)
        ((symbolp (car body))
         (cons (car body)
               (cons ($guard-tick-call) ($guard-walk-prog-body (cdr body)))))
        (t (cons ($guard-walk (car body))
                 ($guard-walk-prog-body (cdr body))))))

(defun $guard-walk (form)
  (cond
    ((not (consp form)) form)
    ((not (symbolp (car form))) ($guard-walk-list form))
    ((eq (car form) 'quote) form)
    ((eq (car form) 'quasiquote) form)
    ;; No-compile rewrites (#284, #168): compiled editions would bypass the
    ;; tick instrumentation entirely.
    ((eq (car form) 'jit-optimize) (list 'quote 'compile-disabled-by-guard))
    ((eq (car form) 'defun-typed)
     (list 'error "defun-typed is disabled inside a guard fence (no-compile, issue #284)"))
    ((eq (car form) 'defun*)
     ;; Downgrade to an ordinary defun: same behavior, no inference/compile.
     ($guard-walk (cons 'defun
                        (cons (car (cdr form))
                              (cons ($guard-strip-params (car (cdr (cdr form))))
                                    (cdr (cdr (cdr form))))))))
    ((eq (car form) 'lambda)
     (cons 'lambda
           (cons (car (cdr form))
                 (cons ($guard-tick-call) ($guard-walk-list (cdr (cdr form)))))))
    ((eq (car form) 'defun)
     (cons 'defun
           (cons (car (cdr form))
                 (cons (car (cdr (cdr form)))
                       (cons ($guard-tick-call)
                             ($guard-walk-list (cdr (cdr (cdr form)))))))))
    ((eq (car form) 'while)
     (cons 'while
           (cons ($guard-walk (car (cdr form)))
                 (cons ($guard-tick-call) ($guard-walk-list (cdr (cdr form)))))))
    ((eq (car form) 'for)
     (cons 'for
           (cons (car (cdr form))
                 (cons ($guard-tick-call) ($guard-walk-list (cdr (cdr form)))))))
    ((eq (car form) 'prog)
     (cons 'prog
           (cons (car (cdr form)) ($guard-walk-prog-body (cdr (cdr form))))))
    ;; DOLIST/DOTIMES are stdlib macros whose expansion (MAPC / FOR) happens
    ;; after the walk, so meter their bodies here — the tick lands inside the
    ;; per-iteration lambda/body the expansion builds.
    ((or (eq (car form) 'dolist) (eq (car form) 'dotimes))
     (cons (car form)
           (cons (car (cdr form))
                 (cons ($guard-tick-call) ($guard-walk-list (cdr (cdr form)))))))
    (t ($guard-walk-list form))))

;;; ------------------------------------------------------------------
;;; WITH-FUEL.

(defvau with-fuel (x e)
  "(WITH-FUEL N FORM...) — evaluate FORMs under an execution budget of N
steps (charged at function entries and loop back-edges). Exhaustion signals
a catchable 'fuel exhausted' error. Nested budgets clamp to the enclosing
remainder and every step charges all enclosing fences. Inside the fence,
(FUEL-REMAINING) reports the live budget; JIT compilation is disabled
(no-compile, issue #284)."
  (let* ((budget (eval (car x) e))
         (outer-tick ($guard-probe '$guard-tick e))
         (outer-reader ($guard-probe '$guard-fuel-reader e))
         (effective (if outer-reader (min budget (funcall outer-reader)) budget))
         (tick-name ($guard-fresh-tick-name))
         (cell (array 1))
         (bcell (array 1)))
    (store cell 0 effective)
    (let* ((tick (lambda ()
                   (when outer-tick (funcall outer-tick))
                   (store cell 0 (- (fetch cell 0) 1))
                   (when (< (fetch cell 0) 0)
                     (error (concat "fuel exhausted (budget "
                                    (princ-to-string effective) ")")))))
           (reader (lambda () (fetch cell 0)))
           (bindings
            (list
             ;; Instrumented code charges through the unguessable TICK-NAME;
             ;; $GUARD-TICK is the chaining channel nested fences probe for.
             (cons tick-name tick)
             (cons '$guard-tick tick)
             (cons '$guard-fuel-reader reader)
             (cons 'fuel-remaining reader)
             ;; The kernel backstop's setter is fence-internal state: deny
             ;; it to guarded code (reading via KERNEL-FUEL-REMAINING stays
             ;; allowed, per the introspection-visible policy).
             (cons 'kernel-fuel-set!
                   (lambda (n)
                     (error "kernel-fuel-set! is disabled inside a guard fence (#284)")))
             ;; Escape hatches: EVAL re-instruments and re-seals with this
             ;; fence's own bindings (shared cell — eval'd work charges us);
             ;; the higher-order functions charge per callback so external,
             ;; uninstrumented functions still cost per element.
             (cons 'eval
                   (lambda (form &rest renv)
                     (setq $guard-walk-tick-name tick-name)
                     (eval ($guard-seal (list ($guard-walk form)) (fetch bcell 0))
                           (if renv (car renv) e))))
             (cons 'apply
                   (lambda (f args) (funcall tick) (apply f args)))
             (cons 'funcall
                   (lambda (f &rest args) (funcall tick) (apply f args)))
             (cons 'mapcar
                   (lambda (f lst)
                     (mapcar (lambda (i) (funcall tick) (funcall f i)) lst)))
             (cons 'mapc
                   (lambda (f lst)
                     (mapc (lambda (i) (funcall tick) (funcall f i)) lst)))
             (cons 'maplist
                   (lambda (f lst)
                     (maplist (lambda (i) (funcall tick) (funcall f i)) lst)))
             (cons 'sort
                   (lambda (lst pred)
                     (funcall tick)
                     (sort lst (lambda (a b) (funcall tick) (funcall pred a b))))))))
      (store bcell 0 bindings)
      (setq $guard-walk-tick-name tick-name)
      ;; Arm the kernel step backstop (issue #284 Phase 2), clamped to any
      ;; enclosing fence's remaining kernel budget.
      (let* ((kprev (kernel-fuel-remaining))
             (kbudget (* effective $guard-kernel-step-multiplier))
             (karmed (if kprev (min kprev kbudget) kbudget)))
        (kernel-fuel-set! karmed)
        (unwind-protect
            (eval ($guard-seal ($guard-walk-list (cdr x)) bindings) e)
          (progn
            ;; Retire the fence: closures that escaped this extent keep
            ;; their captured tick, but against an effectively infinite
            ;; budget (they still chain to any still-active enclosing
            ;; fences).
            (store cell 0 1000000000000)
            ;; Restore the enclosing kernel budget without refunding what
            ;; this fence spent: the smaller of the snapshot and what is
            ;; left now. NIL snapshot disarms.
            (let ((know (kernel-fuel-remaining)))
              (kernel-fuel-set! (if (and kprev know) (min kprev know) kprev)))))))))

;;; ------------------------------------------------------------------
;;; WITH-CAPABILITIES.

;; Capability requirements of the gated builtins, mirroring the kernel's
;; require_* checks in src/evaluator/apply.rs (and SHELL/READ in
;; builtins_extra.rs). RENAME-FILE needs both sets per issue #273.
(setq $guard-gated-builtins
      '((load-file READ-FS) (read-file READ-FS) (read-file-byte READ-FS)
        (read-file-section READ-FS) (file-exists-p READ-FS)
        (directory-p READ-FS) (file-p READ-FS) (file-readable-p READ-FS)
        (file-writable-p READ-FS) (file-executable-p READ-FS)
        (file-size READ-FS) (directory-files READ-FS) (file-newer-p READ-FS)
        (write-file CREATE-FS) (chmod CREATE-FS) (create-directory CREATE-FS)
        (delete-file CREATE-FS)
        (rename-file READ-FS CREATE-FS)
        (make-temp-file TEMP-FS) (make-temp-directory TEMP-FS)
        (shell SHELL)
        (read IO)))

(defun $guard-cap-shadow (name required original effective)
  "A wrapper for gated builtin NAME requiring the REQUIRED capabilities:
denies with a catchable error unless REQUIRED is a subset of EFFECTIVE,
else delegates to ORIGINAL (which may itself be an outer fence's wrapper)."
  (lambda (&rest args)
    (if ($guard-subset-p required effective)
        (apply original args)
        (error (concat "capability denied: " (princ-to-string name)
                       " requires " (princ-to-string required)
                       "; effective " (princ-to-string effective))))))

(defun $guard-cap-shadows (table effective env)
  "Fence bindings for every gated builtin in TABLE that is bound in ENV."
  (cond ((null table) nil)
        (t (let* ((entry (car table))
                  (name (car entry))
                  (required (cdr entry))
                  (original ($guard-probe name env))
                  (rest ($guard-cap-shadows (cdr table) effective env)))
             (if original
                 (cons (cons name ($guard-cap-shadow name required original effective))
                       rest)
                 rest)))))

(defvau with-capabilities (x e)
  "(WITH-CAPABILITIES '(CAP...) FORM...) — evaluate FORMs with the effective
capability set narrowed to the intersection of the listed capabilities and
the currently effective set (attenuation only: a fence can never add a
capability). Gated operations outside the set signal a catchable
'capability denied' error naming the operation, the requirement, and the
effective set. (CAPABILITIES-EFFECTIVE) reports the live set."
  (let* ((requested (eval (car x) e))
         (outer (funcall ($guard-probe 'capabilities-effective e)))
         (effective ($guard-intersect requested outer))
         (bcell (array 1))
         (bindings
          (append
           (list
            (cons 'capabilities-effective (lambda () effective))
            ;; EVAL re-seals so evaluated data inherits the fence.
            (cons 'eval
                  (lambda (form &rest renv)
                    (eval ($guard-seal (list form) (fetch bcell 0))
                          (if renv (car renv) e)))))
           ($guard-cap-shadows $guard-gated-builtins effective e))))
    (store bcell 0 bindings)
    (eval ($guard-seal (cdr x) bindings) e)))

;;; ------------------------------------------------------------------
;;; Convenience combinator.

(defvau sandboxed (x e)
  "(SANDBOXED (:FUEL N :CAPABILITIES (CAP...)) FORM...) — WITH-FUEL and
WITH-CAPABILITIES in one fence. Either key may be omitted."
  (let* ((spec (car x))
         (body (cdr x))
         (fuel ($guard-plist-get spec ':fuel))
         (caps ($guard-plist-get spec ':capabilities))
         (form (cond ((and fuel caps)
                      (list 'with-capabilities (list 'quote caps)
                            (cons 'with-fuel (cons fuel body))))
                     (fuel (cons 'with-fuel (cons fuel body)))
                     (caps (list 'with-capabilities (list 'quote caps)
                                 (cons 'progn body)))
                     (t (cons 'progn body)))))
    (eval form e)))

(defun $guard-plist-get (plist key)
  "Value after KEY in PLIST, or NIL."
  (cond ((null plist) nil)
        ((null (cdr plist)) nil)
        ((eq (car plist) key) (car (cdr plist)))
        (t ($guard-plist-get (cdr (cdr plist)) key))))

;;; ------------------------------------------------------------------
;;; Static capability manifests (issue #284 follow-on): join the call
;;; graph (lib/19-call-graph.lisp) with the gated-builtin table to answer
;;; "which capabilities does running this function need?" before running
;;; it — so a host can grant minimally, or wrap the call in the matching
;;; WITH-CAPABILITIES fence.

(defun $guard-union (a b)
  "Set union of A into B (EQUAL), preserving B then A's new elements."
  (cond ((null a) b)
        (($guard-member (car a) b) ($guard-union (cdr a) b))
        (t ($guard-union (cdr a) (append b (list (car a)))))))

(defun $guard-caps-walk (worklist seen caps)
  (cond ((null worklist) caps)
        (($guard-member (car worklist) seen)
         ($guard-caps-walk (cdr worklist) seen caps))
        (t (let* ((name (car worklist))
                  (entry (assoc name $guard-gated-builtins))
                  (caps2 (if entry ($guard-union (cdr entry) caps) caps))
                  (callees (if (call-graph-has-p name)
                               (call-graph-callees name)
                               nil)))
             ($guard-caps-walk (append callees (cdr worklist))
                               (cons name seen)
                               caps2)))))

(defun capabilities-needed (name)
  "Static capability manifest for the function bound to symbol NAME: the
union of capability requirements of every gated builtin reachable through
the call graph (transitive, cycle-safe). Conservative in both directions:
dynamic calls (FUNCALL/APPLY of computed values, EVAL of data) are
invisible to the call graph, and reachability does not mean the gated
call executes on every path. NIL means no gated builtin is reachable."
  ($guard-caps-walk (list name) nil nil))

(defun capabilities-needed-form (form)
  "CAPABILITIES-NEEDED for a raw FORM: analyze the form's own calls (via
the call-graph collector) and close over the call graph transitively."
  ($guard-caps-walk (cg-collect-callees form nil nil) nil nil))
