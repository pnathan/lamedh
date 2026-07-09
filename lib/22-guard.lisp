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
  "Remaining fuel (kernel steps) in the innermost enclosing WITH-FUEL
fence, or NIL when no fence is active."
  (kernel-fuel-remaining))

;; Custom capabilities (0.3 modules): names REGISTERED by a module's
;; (:provides CAP) clause. They extend the capability VOCABULARY, never the
;; kernel's grants: a custom capability gates only Lisp-level checks
;; (REQUIRE-CAPABILITY), is held by registration at the outermost level,
;; and attenuates through WITH-CAPABILITIES exactly like a built-in.
(def $custom-capabilities ())

(defun $guard-host-capabilities ()
  "The capability set granted by the host process (the outermost
authority), plus registered custom capabilities (module-provided names,
held by registration, attenuable like any other)."
  (let ((all '(READ-FS CREATE-FS TEMP-FS SHELL IO))
        (held nil))
    (dolist (c all (append (reverse held) $custom-capabilities))
      (when (feature-enabled-p (princ-to-string c))
        (setq held (cons c held))))))

(defun require-capability (c)
  "Signal a catchable 'capability denied' error unless C is in the
effective set. The gate module code places in front of operations guarded
by a custom capability."
  (if (member c (capabilities-effective))
      t
      (error (concat "capability denied: " (princ-to-string c)
                     " (effective: "
                     (princ-to-string (capabilities-effective)) ")"))))

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

(defun $guard-walk-list (forms)
  (if (consp forms)
      (cons ($guard-walk (car forms)) ($guard-walk-list (cdr forms)))
      forms))

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
    ;; In-fence DEFUN pins to the interpreted tier: the kernel step counter
    ;; meters trampoline iterations, and a NATIVE edition's internal loops
    ;; never return to the trampoline — compiling would open a fuel escape.
    ((eq (car form) 'defun)
     (cons 'defun
           (cons (car (cdr form))
                 (cons (car (cdr (cdr form)))
                       (cons '(declare (no-compile))
                             ($guard-walk-list (cdr (cdr (cdr form)))))))))
    (t ($guard-walk-list form))))

;;; ------------------------------------------------------------------
;;; WITH-FUEL.

(defvau with-fuel (x e)
  "(WITH-FUEL N FORM...) — evaluate FORMs under an execution budget of N
KERNEL STEPS (one trampoline iteration each — the exact unit STEP-COUNT
measures, so (car (step-count form)) sizes the budget). Exhaustion signals
a catchable 'fuel exhausted' error. Nested budgets clamp to the enclosing
remainder and spend from it. Inside the fence, (FUEL-REMAINING) reports
the live budget; native JIT compilation is disabled for in-fence
definitions (no-compile, issue #284 — a native edition's internal loops
would never return to the metered trampoline)."
  (let* ((budget (eval (car x) e))
         (bcell (array 1))
         (bindings
          (list
           ;; One ruler: introspection reads the kernel step counter.
           (cons 'fuel-remaining (lambda () (kernel-fuel-remaining)))
           ;; The budget cell is fence-internal state: deny the setter to
           ;; guarded code (reading via KERNEL-FUEL-REMAINING stays allowed,
           ;; per the introspection-visible policy).
           (cons 'kernel-fuel-set!
                 (lambda (n)
                   (error "kernel-fuel-set! is disabled inside a guard fence (#284)")))
           ;; EVAL re-applies the no-compile rewrites and re-seals, so
           ;; eval'd definitions cannot mint native editions either.
           (cons 'eval
                 (lambda (form &rest renv)
                   (eval ($guard-seal (list ($guard-walk form)) (fetch bcell 0))
                         (if renv (car renv) e))))))
         ;; Fence setup (walk + seal) happens BEFORE arming: the budget pays
         ;; for the guarded work, not for the fence's own bookkeeping.
         (sealed ($guard-seal ($guard-walk-list (cdr x)) bindings))
         (kprev (kernel-fuel-remaining))
         (karmed (if kprev (min kprev budget) budget)))
    (store bcell 0 bindings)
    (kernel-fuel-set! karmed)
    (unwind-protect
        (eval sealed e)
      ;; Restore the enclosing budget MINUS what this fence spent (a nil
      ;; KNOW means exhaustion disarmed the counter: everything spent).
      (let* ((know (kernel-fuel-remaining))
             (spent (- karmed (if know know 0))))
        (kernel-fuel-set!
         (if kprev
             (let ((left (- kprev spent))) (if (< left 0) 0 left))
             ()))))))

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
            ;; The Lisp-level gate attenuates with the fence too (custom
            ;; capabilities, 0.3 modules).
            (cons 'require-capability
                  (lambda (c)
                    (if (member c effective)
                        t
                        (error (concat "capability denied: "
                                       (princ-to-string c)
                                       " (effective: "
                                       (princ-to-string effective) ")")))))
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

;;; ------------------------------------------------------------------
;;; Capability processes (issue #140): SPAWN is the fences worn as a
;;; thread. A spawned child is a share-nothing interpreter with its own
;;; 512 MiB stack, its authority the intersection of the requested
;;; capabilities with what the parent effectively holds (attenuation only,
;;; the same monotone law as WITH-CAPABILITIES), and an optional fuel
;;; budget wired to the kernel step backstop. The body crosses the thread
;;; boundary as serialized data — code is data — and the result comes back
;;; the same way, so nothing shared, nothing to race.
;;;
;;;   (spawn (:capabilities (READ-FS) :fuel 1000000) form...)  ; => handle
;;;   (await handle)          ; block for the child's value (or re-signal
;;;                           ; its error in the parent)
;;;   (spawn-value handle)    ; the raw (:ok v) / (:error msg) datum
;;;
;;; For an agent this is orchestration with capability arithmetic: fan work
;;; out to workers that provably cannot exceed the authority you grant or
;;; run longer than the fuel you set.

(defvau spawn (x e)
  "(SPAWN (:capabilities (CAP...) :fuel N) form...) — evaluate FORMs on a
fresh share-nothing interpreter thread whose capabilities are the
requested set intersected with the parent's effective set, under an
optional fuel budget. Returns a handle to AWAIT. The body is serialized
across the boundary, so it must be self-contained data (no captured
closures); refer to spawned workers by the values they return."
  (let* ((spec (car x))
         (body (cdr x))
         (requested (or ($guard-plist-get spec ':capabilities) nil))
         (fuel (or ($guard-plist-get spec ':fuel) nil))
         ;; Resolve CAPABILITIES-EFFECTIVE in the CALLER's environment so an
         ;; enclosing WITH-CAPABILITIES fence's attenuation is honored (the
         ;; operative's own lexical scope would see only the global).
         (outer-caps (funcall ($guard-probe 'capabilities-effective e)))
         (effective ($guard-intersect requested outer-caps))
         (body-form (if (null (cdr body)) (car body) (cons 'progn body)))
         (body-src (prin1-to-string body-form)))
    (spawn-thread body-src effective fuel)))

(defun spawn* (spec form)
  "Functional SPAWN: like SPAWN but FORM is an already-built, evaluated
data form (so the caller can splice runtime values into a self-contained
body — the share-nothing boundary carries no closures). SPEC is a plist
with optional :capabilities and :fuel. Example, a parameterized fan-out:
  (mapcar (lambda (n) (spawn* () (list 'squared n))) items)"
  (let* ((requested (or ($guard-plist-get spec ':capabilities) nil))
         (fuel (or ($guard-plist-get spec ':fuel) nil))
         (effective ($guard-intersect requested (capabilities-effective))))
    (spawn-thread (prin1-to-string form) effective fuel)))

(defun spawn-value (handle)
  "Block until the spawned child completes; return its raw outcome datum,
either (:OK value) or (:ERROR message)."
  (channel-recv handle))

(defun spawn-error-p (outcome)
  "T when a SPAWN-VALUE outcome is an error."
  (and (consp outcome) (eq (car outcome) ':error)))

(defun await (handle)
  "Block for the spawned child's value. Returns it on success; re-signals
the child's error in the parent on failure."
  (let ((outcome (spawn-value handle)))
    (if (spawn-error-p outcome)
        (error (concat "spawned child failed: " (car (cdr outcome))))
        (car (cdr outcome)))))
