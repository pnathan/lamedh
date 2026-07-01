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
(def *condense-fingerprints-key* "condense.fingerprints")
(def *condense-last-diff-key* "condense.last-diff")
(def *condense-instances-key* "condense.instances")

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
    (cons 'expect (condense-get sym *condense-expect-key*))
    (cons 'instances (condense-get sym *condense-instances-key*))
    (cons 'last-diff (condense-get sym *condense-last-diff-key*))
    (cons 'stale (condense-stale sym))))

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
               (source (cons 'defconcept x))
               (previous (condense-expansion concept)))
          (eval expansion e)
          (condense-record! concept 'concept source expansion generated)
          (condense-put concept *condense-fields-key* field-specs)
          (condense-put concept *condense-invariant-key* invariant-section)
          (condense-put concept *condense-last-diff-key*
                        (if previous (condense-diff previous expansion) nil))
          (condense-fingerprint! concept)
          concept))))

;;; ---- derive v0 -----------------------------------------------------------

(defun condense-printer-pairs (concept fields)
  (if (null fields)
      nil
      (let* ((field (car fields))
             (accessor (condense-accessor-symbol concept field)))
        (cons `(cons ',field (,accessor self))
              (condense-printer-pairs concept (cdr fields))))))

(defun condense-builder-symbol (concept)
  (condense-symbol-append3 "PLIST->" concept ""))

(defun condense-lens-law-symbol (concept)
  (condense-symbol-append3 "" concept "-LENS-ROUNDTRIP"))

(defun condense-builder-args (fields)
  (if (null fields)
      nil
      (cons `(alist-get view ',(car fields))
            (condense-builder-args (cdr fields)))))

(defun condense-derive-symbol-list (concept derivation)
  (cond
    ((eq derivation 'printer) (list (condense-printer-symbol concept)))
    ((eq derivation 'equality) (list (condense-equality-symbol concept)))
    ((eq derivation 'lens) (list (condense-printer-symbol concept)
                                 (condense-builder-symbol concept)
                                 (condense-lens-law-symbol concept)))
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
    ((eq derivation 'lens)
     (let ((printer (condense-printer-symbol concept))
           (builder (condense-builder-symbol concept))
           (constructor (condense-constructor-symbol concept)))
       `(progn
          (defun ,printer (self)
            (list ,@(condense-printer-pairs concept fields)))
          (defun ,builder (view)
            (,constructor ,@(condense-builder-args fields))))))
    (t (error "unknown derive target"))))

(defun condense-derive-forms (concept derivations fields)
  (if (null derivations)
      nil
      (cons (condense-derive-form concept (car derivations) fields)
            (condense-derive-forms concept (cdr derivations) fields))))

(defun condense-derive-symbols (concept derivations)
  (if (null derivations)
      nil
      (condense-append-new
        (condense-derive-symbol-list concept (car derivations))
        (condense-derive-symbols concept (cdr derivations)))))

;; Derivations map onto standard typeclasses (declared in lib/21-typeclasses.lisp).
;; An instance is installed only when the class is actually declared, so the
;; substrate keeps working in a stripped environment without the typeclass
;; layer.
(defun condense-instance-form (concept derivation)
  (cond
    ((eq derivation 'equality)
     `(definstance eqv ,concept
        (:eqv ,(condense-equality-symbol concept))))
    ((eq derivation 'printer)
     `(definstance show ,concept
        (:show ,(condense-printer-symbol concept))))
    ((eq derivation 'lens)
     `(definstance lens ,concept
        (:view ,(condense-printer-symbol concept))
        (:build ,(condense-builder-symbol concept))))
    (t nil)))

(defun condense-install-instance (concept derivation e)
  (let ((form (condense-instance-form concept derivation)))
    (if (and form (eq (getp (cadr form) "typeclass.kind") 'typeclass))
        (progn
          (eval form e)
          (condense-put concept *condense-instances-key*
                        (condense-append-new
                          (condense-get concept *condense-instances-key*)
                          (list (cadr form))))
          (cadr form))
        nil)))

(defun condense-install-instances (concept derivations e)
  (if (null derivations)
      nil
      (progn
        (condense-install-instance concept (car derivations) e)
        (condense-install-instances concept (cdr derivations) e))))

(defun condense-derive-post-form (concept derivation)
  (if (eq derivation 'lens)
      (let ((law (condense-lens-law-symbol concept))
            (printer (condense-printer-symbol concept))
            (builder (condense-builder-symbol concept)))
        (list `(deflaw ,law
                 (:for ,concept)
                 (:assert (equal (,builder (,printer self)) self)))))
      nil))

(defun condense-derive-post-forms (concept derivations)
  (if (null derivations)
      nil
      (append (condense-derive-post-form concept (car derivations))
              (condense-derive-post-forms concept (cdr derivations)))))

(defun condense-eval-forms (forms e)
  (if (null forms)
      nil
      (progn
        (eval (car forms) e)
        (condense-eval-forms (cdr forms) e))))

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
          (condense-eval-forms (condense-derive-post-forms concept derivations) e)
          (condense-install-instances concept derivations e)
          (condense-put concept *condense-derivations-key* all-derivations)
          (condense-put concept *condense-generated-key*
                        (condense-append-new base-generated generated))
          (condense-fingerprint! concept)
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

;;; ---- change tracking: diffs, fingerprints, staleness ----------------------
;;;
;;; Change is first-class as data on the source/expansion plane: a structural
;;; diff plus a re-verification trigger. Fingerprints snapshot generated
;;; definitions (via SEE-SOURCE) at derivation time; a definition that drifts
;;; from its fingerprint marks the condensed seed as stale, so the trace never
;;; silently vouches for hand-edited expansions.

(defun condense-proper-list-p (x)
  (cond
    ((null x) t)
    ((consp x) (condense-proper-list-p (cdr x)))
    (t nil)))

(defun condense-diff-children (old new index path)
  (if (null old)
      nil
      (append (condense-diff-node (car old) (car new) (cons index path))
              (condense-diff-children (cdr old) (cdr new) (+ index 1) path))))

(defun condense-diff-node (old new path)
  (cond
    ((equal old new) nil)
    ((and (consp old)
          (consp new)
          (condense-proper-list-p old)
          (condense-proper-list-p new)
          (equal (length old) (length new)))
     (condense-diff-children old new 0 path))
    (t (list (list (reverse path) old new)))))

(defun condense-diff (old new)
  "Return a structural diff between OLD and NEW as (path old new) entries.
Each path is the list of positions from the root to the changed node."
  (condense-diff-node old new nil))

(defun condense-definition-of (sym)
  "Return SYM's current definition form, or NIL when it has none."
  (car (errorset (list 'see-source (list 'quote sym)))))

(defun condense-fingerprint-list (syms)
  (if (null syms)
      nil
      (cons (cons (car syms) (condense-definition-of (car syms)))
            (condense-fingerprint-list (cdr syms)))))

(defun condense-fingerprint! (sym)
  "Snapshot the current definitions of SYM's generated symbols."
  (condense-put sym *condense-fingerprints-key*
                (condense-fingerprint-list (condense-generated sym)))
  sym)

(defun condense-stale-entries (fingerprints)
  (cond
    ((null fingerprints) nil)
    ((equal (cdr (car fingerprints))
            (condense-definition-of (car (car fingerprints))))
     (condense-stale-entries (cdr fingerprints)))
    (t (cons (car (car fingerprints))
             (condense-stale-entries (cdr fingerprints))))))

(defun condense-stale (sym)
  "Return generated symbols of SYM whose definitions drifted since fingerprinting."
  (condense-stale-entries (condense-get sym *condense-fingerprints-key*)))

(defun condense-drift-entries (fingerprints)
  (cond
    ((null fingerprints) nil)
    ((equal (cdr (car fingerprints))
            (condense-definition-of (car (car fingerprints))))
     (condense-drift-entries (cdr fingerprints)))
    (t (cons (cons (car (car fingerprints))
                   (condense-diff (cdr (car fingerprints))
                                  (condense-definition-of (car (car fingerprints)))))
             (condense-drift-entries (cdr fingerprints))))))

(defun condense-drift (sym)
  "Return (generated-symbol . diff) pairs for drifted definitions of SYM."
  (condense-drift-entries (condense-get sym *condense-fingerprints-key*)))

(defun condense-recheck! (sym)
  "Re-verify SYM: staleness, examples, and checker status. Updates metadata."
  (let* ((stale (condense-stale sym))
         (drift (condense-drift sym))
         (types (condense-check-type sym))
         (checks (if (and (eq (condense-kind sym) 'concept)
                          (condense-get sym *condense-examples-key*))
                     (condense-check sym)
                     (cons t nil))))
    (list
      (cons 'stale stale)
      (cons 'drift drift)
      (cons 'checks checks)
      (cons 'check-status types))))
