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
(defbuiltin "FEATURE-ENABLED-P" (name)
  "This port does not gate host-facing builtins behind capabilities (see
sbcl/README.md) -- every feature is always enabled, subject only to the
introspection-level WITH-CAPABILITIES mask."
  (bool (capability-mask-allows-p name)))
(defbuiltin "CAPABILITY-MASK-ALLOWS-P" (name) (bool (capability-mask-allows-p name)))
(defbuiltin "SET" (sym val)
  (if (dynamic-sym-p sym) (setf (symbol-value sym) val) (env-set-local *global-env* sym val)))

;;; ---- SPAWN/AWAIT (lib/22-guard.lisp) --------------------------------------
;;;
;;; DEVIATION (documented in sbcl/README.md): the reference implementation
;;; runs SPAWN's body on a genuine share-nothing interpreter thread of its
;;; own. This port evaluates it SYNCHRONOUSLY on the calling thread instead
;;; -- there is no real concurrency here, only the same functional result
;;; AWAIT would eventually see. A capability/fuel-armed child still gets
;;; its own attenuated mask/budget for the extent of its (synchronous) run.

(defbuiltin "SPAWN-THREAD" (body-source effective-caps fuel)
  (let ((forms (lread-all body-source)))
    (handler-case
        (let ((*capability-mask* effective-caps) (*kernel-fuel* fuel))
          (list (lsym ":OK") (let (result) (dolist (f forms result) (setf result (leval f *global-env*))))))
      (error (c) (list (lsym ":ERROR") (princ-to-string c))))))
(defbuiltin "CHANNEL-RECV" (handle) handle)
