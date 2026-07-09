;; `defun` defines a function and then — the one-door policy (2026-07) —
;; quietly attempts typed compilation via `jit-optimize`: HM inference fires
;; under the hood, and on success the name is rebound to a native membrane
;; that fast-paths typed calls and falls back to the original closure for
;; anything else. Functions that are not a fully-inferable typed island are
;; left exactly as-is, silently. Types are weather, not architecture.
;;
;; This reverses the old deliberate default ("auto-optimizing every defun is
;; not the default") because both recorded objections have since been
;; retired: numeric edge semantics now match across every tier (the #210
;; parity audit, Euclidean MOD #280, and the flag-differential suites), and
;; the membrane keeps the original closure so introspection and fallback
;; behavior are preserved. Opt out per function with a leading
;; `(declare (no-compile))` in the body, or globally with
;; `(declaim (no-compile name...))` — both also disable a later explicit
;; `(jit-optimize name)` (issue #168).
;;
;; Purity and call-graph analyses are computed LAZILY on first query (via
;; `pure-p` and `call-graph-callees`/`call-graph-callers`) rather than eagerly
;; at definition time.  This avoids multi-second startup costs during stdlib
;; loading.  The purity cache is invalidated on every redefinition so queries
;; always reflect the current body.  The call-graph pending list ($cg-pending)
;; accumulates names for `call-graph-callers` to flush on demand.

;; The quiet compile attempt behind one-door `defun`. Defined before `defun`
;; itself because every subsequent stdlib definition routes through it.
(def $defun-auto-compile
  (lambda (name)
    (if (getp name "no-compile")
        name
        ;; JIT-OPTIMIZE is a special form taking its symbol UNevaluated, so
        ;; build the call with the target name spliced in and eval it.
        (progn (eval (list 'jit-optimize name)) name))))


;; --- &OPTIONAL / &KEY parameter lists (0.3 regularity) ----------------------
;;
;; (defun f (a &optional b (c 10) &key (d 2) e &rest r) ...) expands to a
;; variadic lambda plus a LET* prologue that peels optionals positionally
;; (later defaults may reference earlier parameters), binds &REST to what
;; remains, and reads keys from that remainder as a :keyword plist. Extended
;; lists are DEFUN-level sugar (a bare LAMBDA keeps only &REST) and are
;; variadic, so they stay on the dynamic tier. Kernel-only helpers ($-названия
;; via DEF, not DEFUN) because DEFUN itself expands through them before the
;; list library exists.

(def $params-extended-p
  (lambda (ps)
    (if (atom ps)
        ()
        (if (eq (car ps) '&optional)
            t
            (if (eq (car ps) '&key) t ($params-extended-p (cdr ps)))))))

(def $key-lookup
  (lambda (plist key default)
    (if (atom plist)
        default
        (if (eq (car plist) key)
            (car (cdr plist))
            ($key-lookup (cdr (cdr plist)) key default)))))

(def $param-keyword
  (lambda (sym) (intern (concat ":" (princ-to-string sym)))))

(def $split-params
  ;; -> (fixed optionals rest-sym keys), each spec normalized to (sym default)
  (lambda (ps mode fixed opts rest keys)
    (if (atom ps)
        (cons fixed (cons opts (cons rest (cons keys ()))))
        (if (eq (car ps) '&optional)
            ($split-params (cdr ps) 'opt fixed opts rest keys)
            (if (eq (car ps) '&key)
                ($split-params (cdr ps) 'key fixed opts rest keys)
                (if (eq (car ps) '&rest)
                    ($split-params (cdr (cdr ps)) mode fixed opts
                                   (car (cdr ps)) keys)
                    (if (eq mode 'fix)
                        ($split-params (cdr ps) mode
                                       (append fixed (cons (car ps) ())) opts rest keys)
                        (if (eq mode 'opt)
                            ($split-params (cdr ps) mode fixed
                                           (append opts (cons (if (atom (car ps))
                                                                  (cons (car ps) (cons () ()))
                                                                  (car ps))
                                                              ()))
                                           rest keys)
                            ($split-params (cdr ps) mode fixed opts rest
                                           (append keys (cons (if (atom (car ps))
                                                                  (cons (car ps) (cons () ()))
                                                                  (car ps))
                                                              ()))))))))))) 

(def $opt-bindings
  (lambda (opts g)
    (if (atom opts)
        ()
        (cons (cons (car (car opts))
                    (cons (cons 'if (cons (cons 'consp (cons g ()))
                                          (cons (cons 'car (cons g ()))
                                              (cons (car (cdr (car opts))) ()))))
                          ()))
              (cons (cons g (cons (cons 'if (cons (cons 'consp (cons g ()))
                                                  (cons (cons 'cdr (cons g ()))
                                                        (cons () ()))))
                            ()))
                    ($opt-bindings (cdr opts) g))))))

(def $key-bindings
  (lambda (keys g)
    (if (atom keys)
        ()
        (cons (cons (car (car keys))
                    (cons (cons '$key-lookup
                                (cons g
                                      (cons (cons 'quote (cons ($param-keyword (car (car keys))) ()))
                                            (cons (car (cdr (car keys))) ()))))
                          ()))
              ($key-bindings (cdr keys) g)))))

(def $extended-lambda
  (lambda (params body)
    (let* ((g (gensym))
           (split ($split-params params 'fix () () () ()))
           (fixed (car split))
           (opts (car (cdr split)))
           (rest-sym (car (cdr (cdr split))))
           (keys (car (cdr (cdr (cdr split)))))
           (bindings (append ($opt-bindings opts g)
                             (append (if rest-sym
                                         (cons (cons rest-sym (cons g ())) ())
                                         ())
                                     ($key-bindings keys g)))))
      (cons 'lambda
            (cons (append fixed (cons '&rest (cons g ())))
                  (cons (cons 'let* (cons bindings body)) ()))))))

(defmacro defun (name params &rest body)
  ;; Split off an optional leading docstring, and an optional leading
  ;; `(declare (no-compile))` (issue #168) which pins the function to the
  ;; tree-walker: it is stripped from the body and recorded on the plist.
  (let* ((has-doc     (stringp (car body)))
         (doc         (if has-doc (car body) nil))
         (body-1      (if has-doc (cdr body) body))
         ;; Kernel-only structural test: EQUAL is not yet defined when
         ;; 00-core's own first defun expands.
         (first-form  (car body-1))
         (has-nc      (if (atom first-form)
                          nil
                          (if (eq (car first-form) 'declare)
                              (if (atom (car (cdr first-form)))
                                  nil
                                  (eq (car (car (cdr first-form))) 'no-compile))
                              nil)))
         (body-forms  (if has-nc (cdr body-1) body-1))
         (auto        (if has-nc
                          `(putp ',name "no-compile" t)
                          `($defun-auto-compile ',name)))
         (lambda-expr (if ($params-extended-p params)
                          ($extended-lambda params body-forms)
                          (cons 'lambda (cons params body-forms)))))
    (if has-doc
      `(progn
         (def ,name ,lambda-expr ,doc)
         (remprop ',name "pure-checked")
         (remprop ',name "source-form")
         (if (boundp '$cg-pending)
             (setq $cg-pending (cons ',name $cg-pending))
             nil)
         (if (boundp '$call-graph)
             (delete-key $call-graph ',name)
             nil)
         ,auto
         ',name)
      `(progn
         (def ,name ,lambda-expr)
         (remprop ',name "pure-checked")
         (remprop ',name "source-form")
         (if (boundp '$cg-pending)
             (setq $cg-pending (cons ',name $cg-pending))
             nil)
         (if (boundp '$call-graph)
             (delete-key $call-graph ',name)
             nil)
         ,auto
         ',name))))

;; Global compile policy (issue #168): pin named functions to the
;; tree-walker. Takes effect for definitions made AFTER the declaim, and
;; disables any later explicit (jit-optimize name).
(defmacro declaim (spec)
  (if (eq (car spec) 'no-compile)
      (cons 'progn
            (mapcar (lambda (n) (list 'putp (list 'quote n) "no-compile" t))
                    (cdr spec)))
      (list 'error "declaim: only (no-compile name...) is supported")))

;; Named vau operative with optional docstring as first body form.
;; (defvau name (operands-param env-param) ["docstring"] body...)
;; Expands to (def name ($vau (operands-param env-param) body...) "docstring").
(defmacro defvau (name params &rest body)
  (if (stringp (car body))
    (let ((vau-expr (cons '$vau (cons params (cdr body)))))
      `(def ,name ,vau-expr ,(car body)))
    (let ((vau-expr (cons '$vau (cons params body))))
      `(def ,name ,vau-expr))))

(defun prog2 (first second &rest rest)
  "Evaluate all forms; return the value of the second."
  second)

(defmacro cset (sym val)
  "Set the global value of symbol SYM (unevaluated) to VAL."
  `(setq ,sym ,val))

(defmacro csetq (sym val)
  "Alias for CSET; set the global value of SYM to VAL."
  `(setq ,sym ,val))
