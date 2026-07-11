;;; 25-variants.lisp -- sum types: DEFVARIANT, VARIANT-CASE, Option, Result.
;;;
;;; A variant is a CLOSED set of branded constructors plus a checker-level
;;; union type:
;;;
;;;   (defvariant shape
;;;     (circle (r int64))
;;;     (rect   (w int64) (h int64)))
;;;
;;; Each constructor is an ordinary record (branded, one StructObj
;;; representation, #S-printable) whose constructor function is the BARE
;;; constructor name -- (circle 3), not (make-circle 3). The variant name is
;;; denotable in type surfaces: a CIRCLE value flows wherever a SHAPE is
;;; demanded (constructor-into-variant absorption in the checker), while two
;;; variants stay nominally distinct. VARIANT-CASE dispatches on the brand
;;; and ENFORCES EXHAUSTIVENESS: a case over a variant that misses a
;;; constructor (and has no ELSE) is an error naming the missing brands.
;;; MATCH destructures constructors with #S patterns: (match v (#S(CIRCLE ?r) ...)).
;;;
;;; OPTION and RESULT are defined here as ordinary variants -- nothing about
;;; them is special-cased -- together with the usual helpers.

;;; ---- defvariant --------------------------------------------------------

(defun $variant-ctor-forms (ctor field-specs variant params)
  "Definitions for one constructor: branded record, bare-name constructor,
getters, and predicate -- every scheme declared in lockstep. PARAMS non-nil
= a parametric variant (0.3 HM generics): the constructor is declared over
the variant's parameters and returns the applied constructor type, which
absorbs into the applied variant."
  (let ((argnames (condense-field-names field-specs))
        (argtys (mapcar #'cadr field-specs))
        (pred (condense-predicate-symbol ctor)))
    (append
     (if params
         (list `(record-declare '(,ctor ,@params) ',field-specs)
               `(defun ,ctor ,argnames (record-new ',ctor ,@argnames))
               `(declare-type! ',ctor
                               '(forall ,params (-> ,argtys (,ctor ,@params)))))
         (list `(record-declare ',ctor ',field-specs)
               `(defun ,ctor ,argnames (record-new ',ctor ,@argnames))
               `(declare-type! ',ctor '(-> ,argtys ,ctor))))
     (append
      (list `(defun ,pred (v) (eq (record-brand v) ',ctor))
            `(declare-type! ',pred '(forall (a) (-> (a) bool))))
      (if params
          ($record-generic-getter-forms ctor params field-specs)
          ($record-getter-forms ctor field-specs))))))

(defun $variant-normalize-ctor (spec)
  "Normalize (ctor (field ty)...) -- field specs normalize like defrecord's."
  (cons (car spec) (mapcar #'record-normalize-field-spec (cdr spec))))

(defun $variant-expansion (name params ctor-specs)
  (let ((ctor-names (mapcar #'car ctor-specs))
        (pred (condense-predicate-symbol name)))
    `(progn
       ,(if params
            `(variant-declare '(,name ,@params) ',ctor-names)
            `(variant-declare ',name ',ctor-names))
       ,@(reduce #'append
                 (mapcar (lambda (spec)
                           ($variant-ctor-forms (car spec) (cdr spec) name params))
                         ctor-specs)
                 nil)
       (defun ,pred (v) (if (member (record-brand v) ',ctor-names) t nil))
       (declare-type! ',pred '(forall (a) (-> (a) bool)))
       ',name)))

(defun $variant-generated (name ctor-specs)
  (append
   (list (condense-predicate-symbol name))
   (reduce #'append
           (mapcar (lambda (spec)
                     (append (list (car spec)
                                   (condense-predicate-symbol (car spec)))
                             (mapcar (lambda (f)
                                       (condense-accessor-symbol (car spec) (car f)))
                                     (cdr spec))))
                   ctor-specs)
           nil)))

(defvau defvariant (x e)
  "(DEFVARIANT Name (ctor (field type)...) ...) -- define a sum type: a
closed set of branded record constructors plus the checker-level union
Name. Constructors are called by their BARE names -- (circle 3) -- and a
nullary constructor is written (none) and called (none). Generates
ctor/ctor-p/ctor-field per constructor and Name-p for the union.
Destructure with VARIANT-CASE (exhaustive) or MATCH #S patterns."
  (let* ((head (car x))
         ;; Parametric head (0.3 HM generics): (defvariant (option a) ...).
         (name (if (consp head) (car head) head))
         (params (if (consp head) (cdr head) ()))
         (raw-specs (cdr x)))
    ;; Register arities BEFORE normalizing ctor fields, so generic
    ;; applications in payload types pass through.
    (if params
        (progn
          (condense-put name "condense.arity" (length params))
          (mapc (lambda (spec)
                  (condense-put (if (consp spec) (car spec) spec)
                                "condense.arity" (length params)))
                raw-specs))
        ())
    (if (null raw-specs)
        (error "defvariant requires at least one constructor")
        (let* ((ctor-specs (mapcar #'$variant-normalize-ctor raw-specs))
               (ctor-names (mapcar #'car ctor-specs))
               (expansion ($variant-expansion name params ctor-specs))
               (source (cons 'defvariant x))
               (previous (condense-expansion name)))
          (eval expansion e)
          (condense-record! name 'variant source expansion
                            ($variant-generated name ctor-specs))
          (condense-put name "condense.ctors" ctor-names)
          (condense-put name "condense.last-diff"
                        (if previous (condense-diff previous expansion) nil))
          (mapc (lambda (spec)
                  (condense-put (car spec) "condense.variant" name)
                  (condense-put (car spec) "condense.fields" (cdr spec)))
                ctor-specs)
          (condense-fingerprint! name)
          name))))

(defun variant-ctors (name)
  "The constructor brands of variant NAME."
  (condense-get name "condense.ctors"))

(defun variant-of (value)
  "The variant a VALUE's brand belongs to, or NIL."
  (let ((brand (record-brand value)))
    (if (null brand) nil (condense-get brand "condense.variant"))))

;;; ---- variant-case ------------------------------------------------------

(defun $variant-clause-ctors (clauses)
  (mapcar #'car
          (filter (lambda (c) (not (eq (car c) 'else))) clauses)))

(defun $variant-check-exhaustive (clauses)
  "Error unless the ctor clauses cover their variant or an ELSE is present.
The variant is identified from the first constructor clause's brand."
  (let* ((covered ($variant-clause-ctors clauses))
         (has-else (not (null (assoc 'else clauses))))
         (variant (if covered (condense-get (car covered) "condense.variant") nil)))
    (if (or has-else (null variant))
        t
        (let ((missing (filter (lambda (c) (not (member c covered)))
                               (condense-get variant "condense.ctors"))))
          (if (null missing)
              t
              (error (concat "variant-case over "
                             (princ-to-string variant)
                             " is not exhaustive; missing: "
                             (princ-to-string missing))))))))

(defun $variant-bindings (ctor vars fields)
  (cond
    ((and (null vars) (null fields)) nil)
    ((or (null vars) (null fields))
     (error (concat "variant-case: clause for " (princ-to-string ctor)
                    " binds the wrong number of fields")))
    (t (cons (list (car vars) (list 'quote (car fields)))
             ($variant-bindings ctor (cdr vars) (cdr fields))))))

(defun $variant-dispatch (val brand clauses e)
  (cond
    ((null clauses)
     (error (concat "variant-case: no clause for brand "
                    (princ-to-string brand))))
    ((eq (car (car clauses)) 'else)
     (eval (cons 'progn (cdr (car clauses))) e))
    ((eq (car (car clauses)) brand)
     (let ((clause (car clauses)))
       (eval `(let ,($variant-bindings brand (cadr clause) (record-fields val))
                ,@(cddr clause))
             e)))
    (t ($variant-dispatch val brand (cdr clauses) e))))

(defvau variant-case (x e)
  "(VARIANT-CASE expr (ctor (vars...) body...) ... [(else body...)])
-- dispatch on EXPR's constructor brand, binding the constructor's fields
to VARS positionally. EXHAUSTIVE: unless an ELSE clause is present, every
constructor of the variant must be covered or the case errors, naming the
missing brands."
  (let ((val (eval (car x) e))
        (clauses (cdr x)))
    ($variant-check-exhaustive clauses)
    ($variant-dispatch val (record-brand val) clauses e)))

;;; ---- Option ------------------------------------------------------------

(defvariant (option a)
  (some (value a))
  (none))

(defun option-of (x)
  "Bridge from nil-punning: () becomes (none), anything else (some x)."
  (if (null x) (none) (some x)))

(defun unwrap (o)
  "The value inside (some v); error on (none)."
  (variant-case o
    (some (v) v)
    (none () (error "unwrap: (none)"))))
(declare-type! 'unwrap '(forall (a) (-> ((option a)) a)))

(defun unwrap-or (o default)
  (variant-case o
    (some (v) v)
    (none () default)))
(declare-type! 'unwrap-or '(forall (a) (-> ((option a) a) a)))

(defun option-map (f o)
  (variant-case o
    (some (v) (some (funcall f v)))
    (none () o)))
(declare-type! 'option-map
               '(forall (a b) (-> ((-> (a) b) (option a)) (option b))))

(defun option-then (f o)
  "Monadic bind, function first like OPTION-MAP and the CL HOF
convention: (some v) -> (funcall f v) [itself an option]; (none) stays."
  (variant-case o
    (some (v) (funcall f v))
    (none () o)))
(declare-type! 'option-then
               '(forall (a b) (-> ((-> (a) (option b)) (option a)) (option b))))

;;; ---- Result ------------------------------------------------------------

(defvariant (result a e)
  (ok (value a))
  (err (message e)))

(defun unwrap-result (r)
  "The value inside (ok v); error with the message on (err m)."
  (variant-case r
    (ok (v) v)
    (err (m) (error (princ-to-string m)))))
(declare-type! 'unwrap-result '(forall (a e) (-> ((result a e)) a)))

(defun result-or (r default)
  (variant-case r
    (ok (v) v)
    (err (m) default)))
(declare-type! 'result-or '(forall (a e) (-> ((result a e) a) a)))

(defun result-map (f r)
  (variant-case r
    (ok (v) (ok (funcall f v)))
    (err (m) r)))
(declare-type! 'result-map
               '(forall (a b e) (-> ((-> (a) b) (result a e)) (result b e))))

(defun result-then (f r)
  "Monadic bind: (ok v) -> (funcall f v) [itself a result]; (err m) stays."
  (variant-case r
    (ok (v) (funcall f v))
    (err (m) r)))
(declare-type! 'result-then
               '(forall (a b e) (-> ((-> (a) (result b e)) (result a e)) (result b e))))

(defun try-call (f &rest args)
  "Call F, capturing a signaled error as (err message): the bridge from the
condition system into Result."
  (handler-case (ok (apply f args))
    (error (e) (err (error-message e)))))

;;; REQUIRE-ABLE (issue #256): `(require 'variants)` on a with_prelude()
;;; environment loads exactly this file. with_stdlib() still loads it
;;; unconditionally, unchanged.
(provide 'variants)
