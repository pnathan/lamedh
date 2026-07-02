;;; Interfaces: named method sets with checker-verified conformance.
;;;
;;; The shape is Go's, not CLOS's: an interface is a set of operation
;;; signatures, a method is an ORDINARY function following the TYPE-OP naming
;;; convention (INVOICE-EQUAL, INT64-BUMP), and satisfaction is structural --
;;; a type implements an interface because the functions exist and their
;;; checker verdicts unify with the declared signatures, not because anyone
;;; registered a dictionary. IMPLEMENTS! is the Rust-flavored explicit
;;; assertion on top: check now, record the claim, error loudly on failure.
;;;
;;; Conformance is graded with the same honesty vocabulary as condensation:
;;;   CONFORMS  the verdict (TYPED, informative CHECKED, or DECLARED) unifies with the
;;;             declared signature at self := type -- a real guarantee
;;;   UNPROVEN  the function exists but its verdict is VACUOUS or DYNAMIC --
;;;             nothing confirmed, nothing denied
;;;   MISMATCH  the verdict conflicts with the declared signature
;;;   MISSING   no such function
;;; There is no dispatch table anywhere in this file. METHOD is one
;;; deterministic name computation.

;;; ---- declaration -----------------------------------------------------------

(defvau definterface (x e)
  "Declare a named method set.

  (definterface counter
    (:ops ((bump (-> (self) self))
           (reset (-> (self) self)))))

SELF in a signature stands for the implementing type. A method of TYPE for
operation OP is the ordinary function TYPE-OP."
  (let* ((name (car x))
         (ops-section (assoc :ops (cdr x))))
    (if (null ops-section)
        (error "definterface requires an :ops section")
        (progn
          (putp name "interface.kind" 'interface)
          (putp name "interface.source" (cons 'definterface x))
          (putp name "interface.ops" (cadr ops-section))
          name))))

(defun interface-p (sym)
  "Return T when SYM names a declared interface."
  (eq (getp sym "interface.kind") 'interface))

(defun interface-ops (iface)
  "Return the (op signature) list declared for IFACE."
  (getp iface "interface.ops"))

(defun iface-require (iface)
  (if (interface-p iface)
      iface
      (error (concat "not a declared interface: " (princ-to-string iface)))))

;;; ---- methods by naming convention ------------------------------------------

(defun method-symbol (type op)
  "Return the method name for TYPE and OP: the symbol TYPE-OP."
  (intern (concat (princ-to-string type) "-" (princ-to-string op))))

(defun condense-type-of (value)
  "Dispatch type of VALUE: its concept tag, or a ground builtin type."
  (cond
    ((and (consp value)
          (symbolp (car value))
          (eq (condense-kind (car value)) 'concept))
     (car value))
    ((consp value) 'list)
    ((null value) 'list)
    ((charp value) 'char)
    ((floatp value) 'float64)
    ((numberp value) 'int64)
    ((stringp value) 'string)
    ((symbolp value) 'symbol)
    (t (error "condense-type-of: unsupported value"))))

(defun method (op subject &rest args)
  "Call SUBJECT's method OP: the ordinary function <type>-<op>.
Not a dispatch table -- one deterministic name computation, Go-style. The
method is a plain function, so it realizes, type-checks, edits, and traces
like any other definition."
  (apply (method-symbol (condense-type-of subject) op) (cons subject args)))

;;; ---- signature unification over checker verdicts ---------------------------
;;;
;;; The declared signature, with SELF substituted by the concrete type, is
;;; unified against the scheme SEE-TYPE reports. The unifier is one-sided:
;;; the declared side is ground, the verdict side may contain its FORALL
;;; variables, which bind consistently or the unification fails.

(defun iface-self-type (type)
  "The type SELF stands for: a concept's closed record when its fields map
into the checker's row language, otherwise TYPE itself."
  (let ((fields (if (eq (condense-kind type) 'concept)
                    (condense-row-fields (condense-get type "condense.fields"))
                    nil)))
    (if fields (list 'record fields) type)))

(defun iface-substitute-self (form type)
  (let ((self-type (iface-self-type type)))
    (iface-substitute-self-walk form self-type)))

(defun iface-substitute-self-walk (form self-type)
  (cond
    ((eq form 'self) self-type)
    ((consp form) (cons (iface-substitute-self-walk (car form) self-type)
                        (iface-substitute-self-walk (cdr form) self-type)))
    (t form)))

(defun iface-scheme-vars (scheme)
  (if (and (consp scheme) (eq (car scheme) 'forall)) (cadr scheme) nil))

(defun iface-scheme-body (scheme)
  (if (and (consp scheme) (eq (car scheme) 'forall)) (caddr scheme) scheme))

(defun iface-record-p (form)
  (and (consp form) (eq (car form) 'record)))

(defun iface-unify (want got vars bindings)
  "Unify ground WANT against GOT whose variables are VARS.
Returns the updated bindings alist, or the symbol FAIL. ANY is absorbing
(mirroring the checker); records unify by label with row-tail awareness."
  (cond
    ((eq bindings 'fail) 'fail)
    ((member got vars)
     (let ((bound (assoc got bindings)))
       (cond
         ((null bound) (cons (cons got want) bindings))
         ((equal (cdr bound) want) bindings)
         (t 'fail))))
    ((or (eq want 'any) (eq got 'any)) bindings)
    ((equal want got) bindings)
    ((and (iface-record-p want) (iface-record-p got))
     (iface-unify-records want got vars bindings))
    ((and (consp want)
          (consp got)
          (condense-proper-list-p want)
          (condense-proper-list-p got)
          (equal (length want) (length got)))
     (iface-unify-list want got vars bindings))
    (t 'fail)))

(defun iface-fields-minus (fields labels)
  (filter (lambda (f) (not (member (car f) labels))) fields))

(defun iface-unify-fields (want-fields got-fields vars bindings)
  (cond
    ((eq bindings 'fail) 'fail)
    ((null got-fields) bindings)
    (t (let ((want-field (assoc (car (car got-fields)) want-fields)))
         (if (null want-field)
             'fail
             (iface-unify-fields want-fields (cdr got-fields) vars
                                 (iface-unify (cadr want-field)
                                              (cadr (car got-fields))
                                              vars bindings)))))))

(defun iface-unify-records (want got vars bindings)
  "Unify a ground (closed) WANT record against a GOT record with an optional
row tail: every GOT field must appear in WANT with a unifiable type; WANT's
remaining fields are absorbed by GOT's row tail (or must be none)."
  (let* ((want-fields (cadr want))
         (got-fields (cadr got))
         (got-tail (caddr got))
         (bindings (iface-unify-fields want-fields got-fields vars bindings)))
    (cond
      ((eq bindings 'fail) 'fail)
      ((null got-tail)
       (if (equal (length want-fields) (length got-fields)) bindings 'fail))
      (t (iface-unify (list 'record
                            (iface-fields-minus want-fields
                                                (mapcar #'car got-fields)))
                      got-tail
                      vars
                      bindings)))))

(defun iface-unify-list (want got vars bindings)
  (if (null want)
      bindings
      (iface-unify-list (cdr want) (cdr got) vars
                        (iface-unify (car want) (car got) vars bindings))))

(defun iface-unifies-p (want scheme)
  (not (eq (iface-unify want
                        (iface-scheme-body scheme)
                        (iface-scheme-vars scheme)
                        nil)
           'fail)))

;;; ---- conformance -----------------------------------------------------------

(defun iface-op-status (type op sig)
  (let* ((fn (method-symbol type op))
         (exists (car (errorset (list 'functionp fn)))))
    (if (not exists)
        (list op 'missing fn)
        (let ((verdict (see-type fn))
              (want (iface-substitute-self sig type)))
          (cond
            ((eq (car verdict) 'type-error)
             (list op 'mismatch fn (cadr verdict)))
            ((eq (car verdict) 'typed)
             (if (iface-unifies-p want (cadr verdict))
                 (list op 'conforms fn (cadr verdict))
                 (list op 'mismatch fn (cadr verdict))))
            ((eq (car verdict) 'checked)
             (cond
               ((condense-vacuous-p (cadr verdict))
                (list op 'unproven fn (cadr verdict)))
               ((iface-unifies-p want (cadr verdict))
                (list op 'conforms fn (cadr verdict)))
               (t (list op 'mismatch fn (cadr verdict)))))
            ;; A DECLARED (row) scheme is generator-backed evidence: unify the
            ;; wanted signature against it like any other scheme.
            ((eq (car verdict) 'declared)
             (if (iface-unifies-p want (cadr verdict))
                 (list op 'conforms fn (cadr verdict))
                 (list op 'mismatch fn (cadr verdict))))
            (t (list op 'unproven fn (cadr verdict))))))))

(defun implements? (type iface)
  "Structural conformance report of TYPE against IFACE: (PASS . PER-OP).
PASS is T when no operation is MISSING or MISMATCH -- Go-style implicit
satisfaction. UNPROVEN operations exist but carry no confirmable type yet;
they do not fail the check, and they do not count as verified."
  (iface-require iface)
  (let* ((results (mapcar (lambda (spec)
                            (iface-op-status type (car spec) (cadr spec)))
                          (interface-ops iface)))
         (bad (filter (lambda (r) (member (cadr r) '(missing mismatch)))
                      results)))
    (cons (null bad) results)))

(defun implements! (type iface)
  "Assert TYPE implements IFACE: check now, record the claim, error on failure.
The Rust-flavored explicit form of the Go-style structural check."
  (let ((report (implements? type iface)))
    (if (car report)
        (progn
          (putp type "interface.implements"
                (condense-append-new (getp type "interface.implements")
                                     (list iface)))
          (putp iface "interface.types"
                (condense-append-new (getp iface "interface.types")
                                     (list type)))
          report)
        (error (concat "implements!: " (princ-to-string type)
                       " does not implement " (princ-to-string iface)
                       ": " (princ-to-string (cdr report)))))))

(defun interface-trace (iface)
  "Return inspectable metadata for IFACE."
  (iface-require iface)
  (list
    (cons 'kind 'interface)
    (cons 'source (getp iface "interface.source"))
    (cons 'ops (interface-ops iface))
    (cons 'types (getp iface "interface.types"))))
