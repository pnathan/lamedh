;;; Condensation metadata substrate.
;;;
;;; Condensed forms attach their source, expansion, generated symbols, and
;;; verification status to the defining symbol's property list. This file is
;;; intentionally boring: higher-level forms should build on these helpers
;;; instead of inventing new metadata conventions.

(def *condense-kind-key* "condense.kind")
(def *condense-source-key* "condense.source")
(def *condense-expansion-key* "condense.expansion")
(def *condense-generated-key* "condense.generated")
(def *condense-contracts-key* "condense.contracts")
(def *condense-laws-key* "condense.laws")
(def *condense-check-status-key* "condense.check-status")
(def *condense-dynamic-frontier-key* "condense.dynamic-frontier")
(def *condense-fields-key* "condense.fields")
(def *condense-invariant-key* "condense.invariant")
(def *condense-derivations-key* "condense.derivations")
(def *condense-examples-key* "condense.examples")
(def *condense-concept-key* "condense.concept")
(def *condense-assert-key* "condense.assert")
(def *condense-given-key* "condense.given")
(def *condense-expect-key* "condense.expect")

(defun condense-put (sym key value)
  "Store condensation metadata VALUE for SYM under KEY."
  (putp sym key value))

(defun condense-get (sym key)
  "Retrieve condensation metadata for SYM under KEY."
  (getp sym key))

(defun condense-kind (sym)
  "Return the condensation kind recorded for SYM."
  (condense-get sym *condense-kind-key*))

(defun condense-source (sym)
  "Return the source form recorded for SYM."
  (condense-get sym *condense-source-key*))

(defun condense-expansion (sym)
  "Return the expansion recorded for SYM."
  (condense-get sym *condense-expansion-key*))

(defun condense-generated (sym)
  "Return the generated symbols recorded for SYM."
  (condense-get sym *condense-generated-key*))

(defun condense-record! (sym kind source expansion generated)
  "Record the core condensation trace for SYM."
  (condense-put sym *condense-kind-key* kind)
  (condense-put sym *condense-source-key* source)
  (condense-put sym *condense-expansion-key* expansion)
  (condense-put sym *condense-generated-key* generated)
  (condense-put sym *condense-contracts-key* nil)
  (condense-put sym *condense-laws-key* nil)
  (condense-put sym *condense-check-status-key* nil)
  (condense-put sym *condense-dynamic-frontier-key* nil)
  sym)

(defun condense-check-type-result (sym)
  (eval (list 'check-type (list 'quote sym))))

(defun condense-check-type-status (result)
  (cond
    ((string-index-of result "type error") 'type-error)
    ((string-index-of result "not a checkable") 'dynamic)
    ((string-index-of result "any") 'dynamic)
    (t 'checked)))

(defun condense-check-type-one (sym)
  (let* ((result (condense-check-type-result sym))
         (status (condense-check-type-status result)))
    (list sym status result)))

(defun condense-check-type-list (syms)
  (if (null syms)
      nil
      (cons (condense-check-type-one (car syms))
            (condense-check-type-list (cdr syms)))))

(defun condense-dynamic-checks (results)
  (cond
    ((null results) nil)
    ((or (eq (cadr (car results)) 'dynamic)
         (eq (cadr (car results)) 'type-error))
     (cons (car results) (condense-dynamic-checks (cdr results))))
    (t (condense-dynamic-checks (cdr results)))))

(defun condense-check-type (sym)
  "Run CHECK-TYPE over generated symbols for SYM and store checker metadata."
  (let* ((targets (if (condense-generated sym)
                      (condense-generated sym)
                      (list sym)))
         (results (condense-check-type-list targets))
         (dynamic (condense-dynamic-checks results)))
    (condense-put sym *condense-check-status-key* results)
    (condense-put sym *condense-dynamic-frontier-key* dynamic)
    results))

(defun condense-trace (sym)
  "Return an alist describing the condensation trace recorded for SYM."
  (list
    (cons 'kind (condense-kind sym))
    (cons 'source (condense-source sym))
    (cons 'expansion (condense-expansion sym))
    (cons 'generated (condense-generated sym))
    (cons 'contracts (condense-get sym *condense-contracts-key*))
    (cons 'laws (condense-get sym *condense-laws-key*))
    (cons 'check-status (condense-get sym *condense-check-status-key*))
    (cons 'dynamic-frontier (condense-get sym *condense-dynamic-frontier-key*))
    (cons 'fields (condense-get sym *condense-fields-key*))
    (cons 'invariant (condense-get sym *condense-invariant-key*))
    (cons 'derivations (condense-get sym *condense-derivations-key*))
    (cons 'examples (condense-get sym *condense-examples-key*))
    (cons 'concept (condense-get sym *condense-concept-key*))
    (cons 'assert (condense-get sym *condense-assert-key*))
    (cons 'given (condense-get sym *condense-given-key*))
    (cons 'expect (condense-get sym *condense-expect-key*))))

;;; ---- defconcept v0 -------------------------------------------------------

(defun condense-symbol-append3 (a b c)
  (intern (concat a (princ-to-string b) c)))

(defun condense-constructor-symbol (concept)
  (condense-symbol-append3 "MAKE-" concept ""))

(defun condense-predicate-symbol (concept)
  (condense-symbol-append3 "" concept "-P"))

(defun condense-validator-symbol (concept)
  (condense-symbol-append3 "VALIDATE-" concept ""))

(defun condense-printer-symbol (concept)
  (condense-symbol-append3 "" concept "->PLIST"))

(defun condense-equality-symbol (concept)
  (condense-symbol-append3 "" concept "-EQUAL"))

(defun condense-accessor-symbol (concept field)
  (intern (concat (princ-to-string concept) "-" (princ-to-string field))))

(defun condense-field-name (field-spec)
  (car field-spec))

(defun condense-field-names (field-specs)
  (mapcar #'condense-field-name field-specs))

(defun condense-accessor-symbols (concept fields)
  (mapcar (lambda (field) (condense-accessor-symbol concept field)) fields))

(defun condense-accessor-forms (concept fields index)
  (if (null fields)
      nil
      (let* ((field (car fields))
             (accessor (condense-accessor-symbol concept field)))
        (cons `(defun ,accessor (self)
                 (nth ,index self))
              (condense-accessor-forms concept (cdr fields) (+ index 1))))))

(defun condense-validator-bindings (concept fields)
  (if (null fields)
      nil
      (let* ((field (car fields))
             (accessor (condense-accessor-symbol concept field)))
        (cons (list field (list accessor 'self))
              (condense-validator-bindings concept (cdr fields))))))

(defun condense-invariant-expression (invariant-section)
  (cond
    ((null invariant-section) t)
    ((null (cdr invariant-section)) t)
    ((null (cddr invariant-section)) (cadr invariant-section))
    (t (cons 'and (cdr invariant-section)))))

(defun condense-concept-generated (concept fields)
  (append
    (list (condense-constructor-symbol concept)
          (condense-predicate-symbol concept)
          (condense-validator-symbol concept))
    (condense-accessor-symbols concept fields)))

(defun condense-append-new (xs ys)
  (cond
    ((null ys) xs)
    ((member (car ys) xs) (condense-append-new xs (cdr ys)))
    (t (condense-append-new (append xs (list (car ys))) (cdr ys)))))

(defun condense-concept-expansion (concept fields invariant-section)
  (let* ((constructor (condense-constructor-symbol concept))
         (predicate (condense-predicate-symbol concept))
         (validator (condense-validator-symbol concept))
         (invariant (condense-invariant-expression invariant-section)))
    (cons 'progn
          (append
            (append
              (list
                `(defun ,constructor ,fields
                   (list ',concept ,@fields))
                `(defun ,predicate (self)
                   (and (consp self) (eq (car self) ',concept))))
              (condense-accessor-forms concept fields 1))
            (list
              `(defun ,validator (self)
                 (and (,predicate self)
                      (let ,(condense-validator-bindings concept fields)
                        ,invariant)))
              `',concept)))))

(defvau defconcept (x e)
  "Define a compact concept with fields, generated operations, and trace metadata."
  (let* ((concept (car x))
         (sections (cdr x))
         (fields-section (assoc :fields sections))
         (invariant-section (assoc :invariant sections)))
    (if (null fields-section)
        (error "defconcept requires a :fields section")
        (let* ((field-specs (cadr fields-section))
               (fields (condense-field-names field-specs))
               (expansion (condense-concept-expansion concept fields invariant-section))
              (generated (condense-concept-generated concept fields))
               (source (cons 'defconcept x)))
          (eval expansion e)
          (condense-record! concept 'concept source expansion generated)
          (condense-put concept *condense-fields-key* field-specs)
          (condense-put concept *condense-invariant-key* invariant-section)
          concept))))

;;; ---- derive v0 -----------------------------------------------------------

(defun condense-printer-pairs (concept fields)
  (if (null fields)
      nil
      (let* ((field (car fields))
             (accessor (condense-accessor-symbol concept field)))
        (cons `(cons ',field (,accessor self))
              (condense-printer-pairs concept (cdr fields))))))

(defun condense-derive-symbol (concept derivation)
  (cond
    ((eq derivation 'printer) (condense-printer-symbol concept))
    ((eq derivation 'equality) (condense-equality-symbol concept))
    (t (error "unknown derive target"))))

(defun condense-derive-form (concept derivation fields)
  (cond
    ((eq derivation 'printer)
     (let ((printer (condense-printer-symbol concept)))
       `(defun ,printer (self)
          (list ,@(condense-printer-pairs concept fields)))))
    ((eq derivation 'equality)
     (let ((equal-fn (condense-equality-symbol concept))
           (predicate (condense-predicate-symbol concept)))
       `(defun ,equal-fn (a b)
          (and (,predicate a)
               (,predicate b)
               (equal a b)))))
    (t (error "unknown derive target"))))

(defun condense-derive-forms (concept derivations fields)
  (if (null derivations)
      nil
      (cons (condense-derive-form concept (car derivations) fields)
            (condense-derive-forms concept (cdr derivations) fields))))

(defun condense-derive-symbols (concept derivations)
  (if (null derivations)
      nil
      (cons (condense-derive-symbol concept (car derivations))
            (condense-derive-symbols concept (cdr derivations)))))

(defvau derive (x e)
  "Generate deterministic support code from concept metadata."
  (let* ((concept (car x))
         (derivations (cdr x))
         (field-specs (condense-get concept *condense-fields-key*)))
    (if (not (eq (condense-kind concept) 'concept))
        (error "derive requires a condensed concept")
        (let* ((fields (condense-field-names field-specs))
               (old-derivations (condense-get concept *condense-derivations-key*))
               (all-derivations (condense-append-new old-derivations derivations))
               (forms (condense-derive-forms concept derivations fields))
               (generated (condense-derive-symbols concept all-derivations))
               (expansion (cons 'progn (append forms (list `',concept))))
               (base-generated (condense-concept-generated concept fields)))
          (eval expansion e)
          (condense-put concept *condense-derivations-key* all-derivations)
          (condense-put concept *condense-generated-key*
                        (append base-generated generated))
          concept))))

;;; ---- laws, examples, and checks -----------------------------------------

(defun condense-section-value (section)
  (if (null section) nil (cadr section)))

(defun condense-require-concept (concept action)
  (if (eq (condense-kind concept) 'concept)
      concept
      (error (concat action " requires a condensed concept"))))

(defun condense-law-expansion (name concept assertion)
  (let* ((field-specs (condense-get concept *condense-fields-key*))
         (fields (condense-field-names field-specs))
         (predicate (condense-predicate-symbol concept)))
    `(defun ,name (self)
       (and (,predicate self)
            (let ,(condense-validator-bindings concept fields)
              ,assertion)))))

(defvau deflaw (x e)
  "Attach a named predicate law to a condensed concept."
  (let* ((name (car x))
         (sections (cdr x))
         (concept (condense-section-value (assoc :for sections)))
         (assertion (condense-section-value (assoc :assert sections))))
    (condense-require-concept concept "deflaw")
    (if (null assertion)
        (error "deflaw requires an :assert section")
        (let* ((expansion (condense-law-expansion name concept assertion))
               (source (cons 'deflaw x))
               (old-laws (condense-get concept *condense-laws-key*)))
          (eval expansion e)
          (condense-record! name 'law source expansion (list name))
          (condense-put name *condense-concept-key* concept)
          (condense-put name *condense-assert-key* assertion)
          (condense-put concept *condense-laws-key*
                        (condense-append-new old-laws (list name)))
          name))))

(defun condense-example-expansion (name given expect)
  `(defun ,name ()
     (let ((*it* ,given))
       ,expect)))

(defvau example (x e)
  "Attach an executable example check to a condensed concept."
  (let* ((name (car x))
         (sections (cdr x))
         (concept (condense-section-value (assoc :for sections)))
         (given (condense-section-value (assoc :given sections)))
         (expect (condense-section-value (assoc :expect sections))))
    (condense-require-concept concept "example")
    (if (null expect)
        (error "example requires an :expect section")
        (let* ((expansion (condense-example-expansion name given expect))
               (source (cons 'example x))
               (old-examples (condense-get concept *condense-examples-key*)))
          (eval expansion e)
          (condense-record! name 'example source expansion (list name))
          (condense-put name *condense-concept-key* concept)
          (condense-put name *condense-given-key* given)
          (condense-put name *condense-expect-key* expect)
          (condense-put concept *condense-examples-key*
                        (condense-append-new old-examples (list name)))
          name))))

(defun condense-run-check-list (checks)
  (if (null checks)
      nil
      (let ((name (car checks)))
        (cons (cons name (funcall name))
              (condense-run-check-list (cdr checks))))))

(defun condense-check-results-pass-p (results)
  (cond
    ((null results) t)
    ((cdr (car results)) (condense-check-results-pass-p (cdr results)))
    (t nil)))

(defun condense-check (sym)
  "Run executable condensation checks for SYM and return (PASS . RESULTS)."
  (cond
    ((eq (condense-kind sym) 'concept)
     (let ((results (condense-run-check-list
                     (condense-get sym *condense-examples-key*))))
       (cons (condense-check-results-pass-p results) results)))
    ((eq (condense-kind sym) 'example)
     (let ((result (funcall sym)))
       (cons result (list (cons sym result)))))
    (t (error "condense-check requires a concept or example"))))
