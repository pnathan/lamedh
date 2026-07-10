;;; Guard fences: composable DYNAMIC-EXTENT attenuation of execution budget
;;; (WITH-FUEL) and capability authority (WITH-CAPABILITIES). Issue #284,
;;; hardened to dynamic extent in 0.3 (#320).
;;;
;;; Both fences are KERNEL SPECIAL FORMS with RAII save/restore: WITH-FUEL
;;; arms the kernel step counter (clamped to the enclosing remainder) and
;;; WITH-CAPABILITIES arms a thread-local capability mask (intersected with
;;; the enclosing mask — attenuation only). Every kernel capability check
;;; and the step counter consult that state on every operation, so
;;; attenuation follows the CALL, not the fence's lexical body: helpers
;;; called from inside a fence are fenced too, eval'd code is fenced, and
;;; there is no Lisp-callable way to widen either state (KERNEL-FUEL-SET!
;;; is narrow-only while armed; no capability-mask setter exists at all).
;;;
;;; Escaped closures follow the same rule in reverse: a closure created
;;; under a fence but CALLED outside runs with the caller's authority —
;;; ambient authority belongs to the execution, not the definition site
;;; (the same law SPAWN applies at thread boundaries).
;;;
;;; No-compile (#284 + #168): while a fuel budget is armed, JIT-OPTIMIZE
;;; returns COMPILE-DISABLED-BY-GUARD, DEFUN-TYPED errors, and one-door
;;; membranes take their interpreted fallback — a native edition's internal
;;; loops never return to the metered trampoline. (Pre-existing DEFUN-TYPED
;;; natives lack an interpreted fallback and remain a documented hole.)

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
  "The capability set effective RIGHT NOW: host-granted builtins (already
mask-aware — FEATURE-ENABLED-P consults the dynamic fence) plus registered
custom capabilities filtered through the same mask."
  (let ((all '(READ-FS CREATE-FS TEMP-FS SHELL IO))
        (held nil))
    (dolist (c all (append (reverse held)
                           (filter #'capability-mask-allows-p
                                   $custom-capabilities)))
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

;; Capability requirements of the gated builtins, mirroring the kernel's
;; require_* checks. Enforcement is IN the kernel (dynamic mask, #320);
;; this table remains as the STATIC-analysis half — capabilities-needed
;; manifests join it with the call graph.
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

;;; ------------------------------------------------------------------
;;; WITH-FUEL and WITH-CAPABILITIES are KERNEL SPECIAL FORMS (0.3, #320):
;;; dynamic-extent state armed and restored in Rust, with narrow-only Lisp
;;; access. Nothing to define here — this file provides the introspection
;;; layer (fuel-remaining, capabilities-effective, require-capability),
;;; custom capabilities, manifests, SANDBOXED, and SPAWN.

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
         ;; Dynamic extent (#320): the effective set is thread state now,
         ;; no lexical probe needed.
         (outer-caps (capabilities-effective))
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
