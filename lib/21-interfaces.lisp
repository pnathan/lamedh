;;; Interfaces: named method sets with checker-verified conformance.
;;;
;;; The shape is Go's, not CLOS's: an interface is a set of operation
;;; signatures, a method is an ORDINARY function following the TYPE-OP naming
;;; convention (INVOICE-EQUAL, INT64-BUMP), and satisfaction is structural --
;;; a type implements an interface because the functions exist and their
;;; checker verdicts subsume the declared signatures, not because anyone
;;; registered a dictionary. IMPLEMENTS! is the Rust-flavored explicit
;;; assertion on top: check now, record the claim, error loudly on failure.
;;;
;;; Conformance is graded with the same honesty vocabulary as condensation:
;;;   CONFORMS  the verdict (TYPED, informative CHECKED, or DECLARED row scheme)
;;;             subsumes the declared signature at self := the type's structural
;;;             identity -- a real guarantee
;;;   UNPROVEN  the function exists but its verdict is VACUOUS or DYNAMIC, or the
;;;             type has no structural type the checker can read -- nothing
;;;             confirmed, nothing denied
;;;   MISMATCH  the verdict conflicts with the declared signature
;;;   MISSING   no such function
;;; The subsumption test is the kernel's own row unifier, reached through the
;;; SCHEME-SUBSUMES? builtin -- not a Lisp reimplementation. There is no
;;; dispatch table anywhere in this file. METHOD is one deterministic name
;;; computation.

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

;;; ---- signature subsumption over checker verdicts ---------------------------
;;;
;;; The declared signature, with SELF substituted by the type's *structural*
;;; identity, is checked against the scheme SEE-TYPE reports -- not by a Lisp
;;; reimplementation of unification, but by the kernel's own row unifier via
;;; SCHEME-SUBSUMES?. This is the fix for the seam where a method carrying an
;;; informative row scheme (the strongest evidence the checker can produce)
;;; graded MISMATCH because SELF was substituted with the concept *symbol*
;;; while the verdict lived in the *record* type language.
;;;
;;; SELF becomes:
;;;   - a closed record of the concept's fields, for a row concept -- so a
;;;     row-polymorphic accessor scheme like
;;;     (forall (a) (-> ((record ((hp int64)) a)) int64)) subsumes it;
;;;   - the type symbol itself, for a ground builtin type (int64, ...);
;;;   - nothing (NIL), for a concept with unmappable fields -- the checker has
;;;     no structural type to read, so conformance is honestly UNPROVEN.

(defun iface-substitute-self (form type)
  "FORM with every occurrence of the symbol SELF replaced by TYPE (a type form)."
  (cond
    ((eq form 'self) type)
    ((consp form) (cons (iface-substitute-self (car form) type)
                        (iface-substitute-self (cdr form) type)))
    (t form)))

(defun iface-concept-record-type (type)
  "The closed record type of row concept TYPE, or NIL when TYPE is not a
concept whose every field maps into the checker's type language."
  (let ((fields (condense-row-fields (condense-get type "condense.fields"))))
    (if (null fields) nil (list 'record fields))))

(defun iface-want-type (type sig)
  "SIG with SELF replaced by TYPE's structural identity for the checker: a
closed record for a row concept, the symbol for a ground type, or NIL when
TYPE is a concept the checker cannot read structurally."
  (let ((record-type (iface-concept-record-type type)))
    (cond
      (record-type (iface-substitute-self sig record-type))
      ((eq (condense-kind type) 'concept) nil)
      (t (iface-substitute-self sig type)))))

(defun iface-verdict-scheme (verdict)
  "The type scheme a SEE-TYPE verdict carries (TYPED sig / CHECKED / DECLARED),
or NIL for TYPE-ERROR and DYNAMIC, which carry a message rather than a scheme."
  (if (member (car verdict) '(typed checked declared)) (cadr verdict) nil))

;;; ---- conformance -----------------------------------------------------------

(defun iface-op-status (type op sig)
  "Grade TYPE's method for OP against declared SIG:
CONFORMS  the method's scheme (TYPED / informative CHECKED / DECLARED) subsumes
          SIG at self := TYPE's structural type -- a real guarantee
UNPROVEN  the method exists but its scheme is VACUOUS/DYNAMIC, or TYPE has no
          structural type the checker can read -- nothing confirmed or denied
MISMATCH  the scheme contradicts SIG (or the method is a TYPE-ERROR)
MISSING   no such method"
  (let* ((fn (method-symbol type op))
         (exists (car (errorset (list 'functionp fn)))))
    (if (not exists)
        (list op 'missing fn)
        (let* ((verdict (see-type fn))
               (scheme (iface-verdict-scheme verdict))
               (want (iface-want-type type sig)))
          (cond
            ((eq (car verdict) 'type-error) (list op 'mismatch fn (cadr verdict)))
            ((and (eq (car verdict) 'checked) (condense-vacuous-p scheme))
             (list op 'unproven fn scheme))
            ((null scheme) (list op 'unproven fn (cadr verdict)))
            ((null want) (list op 'unproven fn scheme))
            ((scheme-subsumes? scheme want) (list op 'conforms fn scheme))
            (t (list op 'mismatch fn scheme)))))))

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
