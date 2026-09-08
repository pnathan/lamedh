;;;; extra.lisp -- native primitives for the condensation/records, module,
;;;; guard-fence, and instrumentation layers (lib/19,20,22,23,26,27,28,29).
;;;;
;;;; These back files that are otherwise pure Lamedh source (see
;;;; sbcl/README.md's "reused unmodified as data" list); every function
;;;; here is a small kernel hook a Lamedh library file calls into, mirroring
;;;; a `*` -suffixed Rust builtin (RECORD-NEW*, DECLARE-TYPE!, ...).

(in-package #:lamedh-rt)

;;; ---- condition system extras (lib/16-conditions.lisp) --------------------

(defbuiltin "ERRORSET" (form &optional print-p)
  (declare (ignore print-p))
  (handler-case (list (leval form (or *current-env* *global-env*)))
    (error () nil)))

(defun offset-line-col (src offset)
  (let ((line 1) (col 1))
    (dotimes (i (min offset (length src)))
      (if (char= (char src i) #\Newline) (progn (incf line) (setf col 1)) (incf col)))
    (values line col)))

(defbuiltin "READ-ALL-POSITIONED" (src)
  (let ((c (make-cursor src)) out)
    (loop
      (skip-ws c)
      (when (cur-eof-p c) (return (nreverse out)))
      (let ((start (cursor-pos c)) (form (read-form c)))
        (multiple-value-bind (line col) (offset-line-col src start)
          (push (list form line col) out))))))

(defbuiltin "SEE-SOURCE" (name)
  (let ((v (env-resolve (or *current-env* *global-env*) name)))
    (if (lambda-obj-p v)
        (let* ((params (lambda-obj-params v))
               (rest (lambda-obj-rest v))
               (plist (if rest (append params (list (lsym "&REST") rest)) params)))
          (list (lsym "LAMBDA") plist (lambda-obj-body v)))
        (lamedh-error (format nil "SEE-SOURCE: ~A is not an inspectable user function" name)))))

;;; ---- records (lib/20-condensation.lisp, lib/25-variants.lisp) ------------

(defbuiltin "RECORD-DECLARE" (name field-specs) (record-declare* name field-specs))
(defbuiltin "RECORD-NEW" (name &rest values) (apply #'record-new* name values))
(defbuiltin "RECORD-REF" (self field) (record-ref* self field))
(defbuiltin "RECORD-WITH" (self &rest kvs) (apply #'record-with* self kvs))
(defbuiltin "RECORD-SET!" (self field val) (record-set!* self field val))
(defbuiltin "RECORD-BRAND" (v) (record-brand* v))
(defbuiltin "RECORD-FIELDS" (v) (record-fields* v))
(defbuiltin "RECORD-COMPILED-P" (name)
  "This port has no separate compiled tier (every record uses the same
LAMEDH-STRUCT representation -- see sbcl/README.md), so this always
reports NIL: honest, not a lie, since there is no performance distinction
to report as true."
  (declare (ignore name))
  nil)
(defbuiltin "VARIANT-DECLARE" (name ctor-names) (putp name "variant-ctors" ctor-names) name)

;;; ---- the approximated type-checker surface --------------------------------

(defbuiltin "DECLARE-TYPE!" (name scheme) (declare-type!* name scheme))
(defbuiltin "SEE-TYPE" (name) (see-type* name))
(defbuiltin "DECLARE-INSTANCE!" (name scheme)
  (putp name "declared-instances" (cons scheme (getp name "declared-instances")))
  name)
(defbuiltin "DECLARE-PROTOCOL-DISPATCH!" (name idx) (putp name "protocol-dispatch" idx) name)

(defun lc-backtick-instance-msg (msg)
  "The reference implementation's static CHECK-TYPE reports a missing
protocol instance as \"no `NAME` instance for ...\" (backtick-quoted,
lib/29-protocols.lisp's header). This port's CHECK-TYPE below discovers
the same fact dynamically instead (by actually running the expression),
so its runtime dispatcher's plain-text \"no NAME instance for ...\"
(same file, DEFPROTOCOL's dispatcher) is reformatted to match that
convention -- same information, consistent presentation, no fabricated
static verdict."
  (let ((p2 (search " instance for" msg)))
    (if (and p2 (>= p2 3) (string= "no " msg :end2 3))
        (concatenate 'string "no `" (subseq msg 3 p2) "`" (subseq msg p2))
        msg)))

(defspecial "CHECK-TYPE" (args env whole)
  (declare (ignore whole))
  "CHECK-TYPE is part of the reference implementation's HM-checker/JIT
elaboration surface (SpecialForm::CheckType calls into `jit_check_expr`)
-- not ported here (see README's \"The type checker\"). Rather than leave
the name unbound, this gives an honest, dynamic approximation: actually
run the expression and report what happened, instead of statically
inferring a type for it. A successful run cannot distinguish \"well-typed\"
from \"happened not to error this time\", so it is reported as such, not
as a verified type."
  (unless args (lamedh-error "CHECK-TYPE: expected an argument"))
  (done
   (handler-case
       (progn (leval (car args) env)
              "not statically checked in this port -- ran without error at runtime")
     (lamedh-condition (c) (lc-backtick-instance-msg (lamedh-condition-value-string c)))
     (error (c) (format nil "~A" c)))))

;;; ---- module system (lib/06-require.lisp, lib/27-modules.lisp) -----------

(defun sexpr-rename-tail (tail rmap)
  (cond ((null tail) nil)
        ((consp tail) (cons (sexpr-rename* (car tail) rmap) (sexpr-rename-tail (cdr tail) rmap)))
        (t (sexpr-rename* tail rmap))))

(defun sexpr-rename* (form rmap)
  (cond
    ((symbolp form) (multiple-value-bind (v p) (gethash form rmap) (if p v form)))
    ((not (consp form)) form)
    ((and (symbolp (car form)) (member (car form) (list (lsym "QUOTE") (lsym "QUASIQUOTE")))) form)
    (t (cons (sexpr-rename* (car form) rmap) (sexpr-rename-tail (cdr form) rmap)))))

(defbuiltin "SEXPR-RENAME" (form rmap) (sexpr-rename* form rmap))

(defparameter *module-name->file*
  '(("SHELL" . "07-shell") ("LISP15" . "09-lisp15") ("TESTING" . "10-testing")
    ("OPTIMIZER-VAU" . "11-optimizer-vau") ("CALL-GRAPH" . "19-call-graph")
    ("CONDENSATION" . "20-condensation") ("GUARD" . "22-guard") ("MATCH" . "23-match")
    ("RULES" . "24-rules") ("VARIANTS" . "25-variants") ("INSTRUMENT" . "26-instrument")
    ("MODULES" . "27-modules") ("TYPES" . "28-types") ("PROTOCOLS" . "29-protocols")
    ("TEXT" . "30-text") ("PORTS" . "31-ports") ("BASE64" . "32-base64")
    ("HEX" . "33-hex") ("URL" . "34-url") ("JSON" . "35-json") ("MIME" . "36-mime")
    ("NET" . "37-net") ("TCP" . "38-tcp") ("UDP" . "39-udp") ("HTTP" . "40-http")
    ("OS" . "41-os") ("OS-LINUX" . "42-os-linux") ("TLS" . "43-tls") ("REGEX" . "44-regex")
    ("DOC-RENDERER" . "97-doc-renderer") ("HELP-SYSTEM" . "98-help-system")
    ("HELP-DATA" . "99-help-data")))

(defbuiltin "$MODULE-SOURCE-LOOKUP" (name-string)
  (let* ((entry (assoc (string-upcase name-string) *module-name->file* :test #'string=))
         (path (and entry (asdf:system-relative-pathname :lamedh (format nil "lib/~A.lisp" (cdr entry))))))
    (if (and path (probe-file path))
        (cons (uiop:read-file-string path) "embedded")
        nil)))
(defbuiltin "$MODULE-SEARCH-PATHS" () nil)
(defbuiltin "$EVAL-MODULE-SOURCE" (name source)
  (declare (ignore name))
  (dolist (form (lread-all source)) (leval form *global-env*)))

;;; ---- instrumentation / guard fences (lib/22,26) ---------------------------

(defbuiltin "KERNEL-FUEL-REMAINING" () *kernel-fuel*)
(defbuiltin "KERNEL-FUEL-SET!" (n) (setf *kernel-fuel* n))
(defbuiltin "MONOTONIC-MICROS" () (round (* (get-internal-real-time) (/ 1000000 internal-time-units-per-second))))
(defbuiltin "FEATURE-ENABLED-P" (name) (bool (feature-active-p name)))
(defbuiltin "CAPABILITY-MASK-ALLOWS-P" (name) (bool (capability-mask-allows-p name)))
(defbuiltin "SET" (sym val)
  (if (dynamic-sym-p sym) (setf (symbol-value sym) val) (env-set-local *global-env* sym val)))

;;; ---- SPAWN/AWAIT (lib/22-guard.lisp): real SBCL threads -------------------
;;;
;;; "Share-nothing" per the reference implementation's design: the child
;;; runs in a FRESH root environment (a snapshot of the parent's global
;;; bindings at fork time, via MAKE-FRESH-ROOT-ENV -- see runtime.lisp),
;;; not the parent's live *GLOBAL-ENV*, so neither thread's later global
;;; definitions/redefinitions are visible to the other. DEVIATION
;;; (documented in sbcl/README.md): process-wide caches that sit outside
;;; the Lamedh environment model proper -- property lists, record/type
;;; schemas, the dynamic-symbol registry -- are still process-wide CL
;;; hash tables shared by every thread; this is a real, if narrow, gap in
;;; the isolation guarantee, not a full share-nothing implementation.

(defstruct lchannel thread (joined nil) cached)

(defbuiltin "SPAWN-THREAD" (body-source effective-caps fuel)
  (let* ((forms (lread-all body-source))
         (child-env (make-fresh-root-env)))
    (make-lchannel
     :thread (sb-thread:make-thread
              (lambda ()
                (handler-case
                    (let ((*global-env* child-env) (*capability-mask* effective-caps) (*kernel-fuel* fuel)
                          (*current-env* nil))
                      (list (lsym ":OK") (let (result) (dolist (f forms result) (setf result (leval f child-env))))))
                  (error (c) (list (lsym ":ERROR") (princ-to-string c)))))
              :name "lamedh-spawn"))))

(defbuiltin "CHANNEL-RECV" (handle)
  (if (lchannel-joined handle)
      (lchannel-cached handle)
      (let ((result (sb-thread:join-thread (lchannel-thread handle) :default (list (lsym ":ERROR") "thread died"))))
        (setf (lchannel-cached handle) result (lchannel-joined handle) t)
        result)))
